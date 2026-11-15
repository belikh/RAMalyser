//! Software model of the timing engine plus a DRAM, at register level.
//!
//! Mirrors the behaviour of gateware/rtl/tester_core.v closely enough to
//! exercise the firmware algorithms on a PC: march elements, pattern modes,
//! compare masks, the read strobe versus access time, EDO/FPM output
//! behaviour, weak pulls, the sense read-back path and refresh/retention.

use crate::ramspec;
use crate::regs::*;
use crate::tests::CTL_SENSE_ORDER;
use crate::Platform;

#[derive(Clone, Debug, PartialEq)]
pub enum Fault {
    /// One cell bit reads a fixed value.
    StuckBit { addr: u32, bit: u8, value: bool },
    /// A multiplexed address input of the device is stuck low.
    AddrLineLow(u8),
    /// Two data outputs shorted (wired-AND).
    DqShort(u8, u8),
    /// Control line (index in sense order) stuck at a level on the adapter.
    CtlStuck(u8, bool),
    /// Two control lines shorted together (wired-AND).
    CtlShort(u8, u8),
}

#[derive(Clone, Debug)]
pub struct SimDut {
    pub row_bits: u8,
    pub col_bits: u8,
    /// DQ lines the device drives on a read.
    pub width_mask: u64,
    pub split0: bool,
    pub split1: bool,
    pub t_rac_ns: f64,
    pub edo: bool,
    /// Cell retention without refresh, microseconds.
    pub retention_us: Option<u64>,
    /// Works only inside this supply window (millivolts).
    pub v_min: u16,
    pub v_max: u16,
    pub current_ma: u16,
    pub faults: Vec<Fault>,
    mem: Vec<u64>,
    row_refreshed: Vec<u64>,
}

impl SimDut {
    pub fn new(row_bits: u8, col_bits: u8, width_mask: u64, t_rac_ns: f64) -> Self {
        SimDut {
            row_bits, col_bits, width_mask, split0: false, split1: false, t_rac_ns,
            edo: false, retention_us: None, v_min: 4400, v_max: 5600, current_ma: 120,
            faults: Vec::new(),
            mem: vec![0xAAAA_AAAA_AAAA; 1usize << (row_bits + col_bits)],
            row_refreshed: vec![0; 1usize << row_bits],
        }
    }
}

pub struct SimPlatform {
    pub reg: [u32; 64],
    pub dut: Option<SimDut>,
    pub tick_ps: u32,
    /// Fixed delay of the adapter buffers in the strobe path, picoseconds.
    pub path_ps: u32,
    pub time_us: u64,
    last_all_refresh: u64,
    pub vdut_mv: u16,
    pub vdut_on: bool,
    /// ID / presence-detect lines as the adapter presents them.
    pub id_lines: u8,
    /// Operations executed (reads + writes), for speed estimates.
    pub ops: u64,
    frac_ps: u64,
}

impl SimPlatform {
    pub fn new(dut: Option<SimDut>) -> Self {
        let mut reg = [0u32; 64];
        reg[ID as usize] = ID_VALUE;
        SimPlatform {
            reg, dut, tick_ps: 3077, path_ps: 9000, time_us: 0, last_all_refresh: 0,
            vdut_mv: 0, vdut_on: false, id_lines: 0xFF, ops: 0, frac_ps: 0,
        }
    }

    fn r(&self, a: u8) -> u32 {
        self.reg[a as usize]
    }

    fn r40(&self, lo: u8) -> u64 {
        self.r(lo) as u64 | ((self.r(lo + 1) as u64 & 0xFF) << 32)
    }

    fn w40(&mut self, lo: u8, v: u64) {
        self.reg[lo as usize] = v as u32;
        self.reg[lo as usize + 1] = (v >> 32) as u32 & 0xFF;
    }

    fn gpio_out(&self, io: u8) -> Option<bool> {
        let (w, b) = ((io / 32) as usize, io % 32);
        if self.reg[GPIO_OE as usize + w] >> b & 1 == 1 {
            Some(self.reg[GPIO_OUT as usize + w] >> b & 1 == 1)
        } else {
            None
        }
    }

    /// Level of undriven DQ lines: even bits follow PULL_A, odd bits PULL_B.
    fn pulled(&self) -> u64 {
        let a = self.gpio_out(ramspec::IO_PULL_A).unwrap_or(true);
        let b = self.gpio_out(ramspec::IO_PULL_B).unwrap_or(true);
        let even = 0x55_5555_5555u64;
        (if a { even } else { 0 }) | (if b { !even & 0xFF_FFFF_FFFF } else { 0 })
    }

    fn advance_ps(&mut self, ps: u64) {
        self.frac_ps += ps;
        self.time_us += self.frac_ps / 1_000_000;
        self.frac_ps %= 1_000_000;
        if self.r(CTRL) & CTRL_RFSH_EN != 0 && self.r(CTRL) & CTRL_ENGINE != 0 {
            self.last_all_refresh = self.time_us;
        }
    }

    fn dut_ok(&self) -> bool {
        match &self.dut {
            Some(d) => self.vdut_on && self.vdut_mv >= d.v_min && self.vdut_mv <= d.v_max,
            None => false,
        }
    }

    /// Row and column as the device sees them on its address pins.
    fn dut_addr(d: &SimDut, row: u32, col: u32) -> (usize, usize) {
        let mut stuck = 0u32;
        for f in &d.faults {
            if let Fault::AddrLineLow(l) = f {
                stuck |= 1 << l;
            }
        }
        let r = (row & !stuck) & ((1 << d.row_bits) - 1);
        let c = (col & !stuck) & ((1 << d.col_bits) - 1);
        (r as usize, c as usize)
    }

    fn run_element(&mut self) {
        let march = self.r(MARCH);
        let nops = march & 7;
        let down = march >> 3 & 1 == 1;
        let patmode = march >> 4 & 3;
        let ops = march >> 8 & 0xFFF;
        let (rb, cb) = (self.r(GEOM) & 0xF, self.r(GEOM) >> 4 & 0xF);
        let row_fast = self.r(CTRL) & CTRL_ROW_FAST != 0;
        let pat = self.r40(PAT_LO);
        let mask = self.r40(MASK_LO);
        let drv = self.r40(DRV_LO);
        let sel = self.r(SEL);
        let (split0, split1) = (sel >> 12 & 1 == 1, sel >> 13 & 1 == 1);
        let lines_ok = sel & 0xF != 0 && sel >> 4 & 0xF != 0;
        let (start, end) = (self.r(ADDR_START), self.r(ADDR_END));
        let t0 = self.r(TIM0);
        let t1 = self.r(TIM1);
        let (t_ras_r, t_cas_r) = (t0 >> 24, t0 >> 16 & 0xFF);
        let (t_smp2, t_smp, t_end, t_ras_f) = (t1 >> 24, t1 >> 16 & 0xFF, t1 >> 8 & 0xFF, t1 & 0xFF);
        let fine = self.r(TIM2) & 0x7F;
        let strobe_ps = (t_smp as i64 - t_ras_f as i64) * self.tick_ps as i64
            - (fine * FINE_STEP_PS) as i64 - self.path_ps as i64;
        let cycle_ps = (t_end as u64 + 20) * self.tick_ps as u64;
        let all40 = 0xFF_FFFF_FFFFu64;

        let (mut errors, mut fail_addr, mut fail_bits, mut fail_data) = (0u32, 0u32, 0u64, 0u64);
        let (mut last, mut last2) = (self.r40(LAST_LO), self.r40(SMP2_LO));
        let mut addr = if down { end } else { start };
        loop {
            let (lo_bits, hi_shift) = if row_fast { (rb, rb) } else { (cb, cb) };
            let lo = addr & ((1 << lo_bits) - 1);
            let hi = addr >> hi_shift;
            let (row, col) = if row_fast { (lo & ((1 << rb) - 1), hi & ((1 << cb) - 1)) }
                             else { (hi & ((1 << rb) - 1), lo) };
            let ah = (addr ^ addr >> 8 ^ addr >> 16) as u64 & 0xFF;
            let base = match patmode {
                0 => pat,
                1 => pat ^ if (row ^ col) & 1 == 1 { all40 } else { 0 },
                _ => pat ^ (ah | ah << 8 | ah << 16 | ah << 24 | ah << 32),
            };
            for i in 0..nops {
                let op = ops >> (2 * i) & 3;
                let wdata = if op & 2 != 0 { !base & all40 } else { base };
                let mut expect = wdata;
                if split0 { expect = expect & !2 | (wdata & 1) << 1 }
                if split1 { expect = expect & !(1 << 33) | (wdata >> 32 & 1) << 33 }
                self.ops += 1;
                self.advance_ps(cycle_ps);
                let pulled = self.pulled();
                let ok = self.dut_ok() && lines_ok;
                let now = self.time_us;
                let all_ref = self.last_all_refresh;
                if op & 1 == 1 {
                    if ok {
                        let d = self.dut.as_mut().unwrap();
                        let (r, c) = Self::dut_addr(d, row, col);
                        // the device stores what arrives on its inputs; split
                        // parts return it on a different pin
                        let mut v = wdata & drv;
                        if d.split0 { v = (v & 1) << 1 }
                        if d.split1 { v = v & !(1u64 << 33) | (wdata >> 32 & 1) << 33 }
                        d.mem[r << d.col_bits | c] = v & d.width_mask;
                        d.row_refreshed[r] = now;
                    }
                } else {
                    let mut smp = pulled;
                    let mut smp2 = pulled;
                    if ok {
                        let d = self.dut.as_mut().unwrap();
                        let (r, c) = Self::dut_addr(d, row, col);
                        if let Some(ret) = d.retention_us {
                            if now - d.row_refreshed[r].max(all_ref) > ret {
                                for cc in 0..(1usize << d.col_bits) {
                                    d.mem[r << d.col_bits | cc] ^= d.width_mask; // charge lost
                                }
                            }
                        }
                        d.row_refreshed[r] = now;
                        let a = (r << d.col_bits | c) as u32;
                        let mut v = d.mem[a as usize];
                        for f in &d.faults {
                            match *f {
                                Fault::StuckBit { addr, bit, value } if addr == a => {
                                    v = v & !(1 << bit) | (value as u64) << bit;
                                }
                                Fault::DqShort(x, y) => {
                                    let w = v >> x & v >> y & 1;
                                    v = v & !(1 << x) & !(1 << y) | w << x | w << y;
                                }
                                _ => {}
                            }
                        }
                        let valid = strobe_ps >= (d.t_rac_ns * 1000.0) as i64;
                        let out = if valid { v } else { !v };
                        smp = pulled & !d.width_mask | out & d.width_mask;
                        // after CAS rises: EDO keeps driving while RAS is low
                        if d.edo && t_ras_r > t_cas_r && t_smp2 < t_ras_r {
                            smp2 = pulled & !d.width_mask | v & d.width_mask;
                        }
                    }
                    last = smp;
                    last2 = smp2;
                    let diff = (smp ^ expect) & mask;
                    if diff != 0 {
                        if errors == 0 {
                            fail_addr = addr;
                            fail_data = smp;
                        }
                        errors = errors.saturating_add(1);
                        fail_bits |= diff;
                    }
                }
            }
            if addr == if down { start } else { end } {
                break;
            }
            addr = if down { addr - 1 } else { addr + 1 };
        }
        self.reg[ERR_COUNT as usize] = errors;
        self.reg[FAIL_ADDR as usize] = fail_addr;
        self.w40(FAIL_BITS_LO, fail_bits);
        self.w40(FAIL_DATA_LO, fail_data);
        self.w40(LAST_LO, last);
        self.w40(SMP2_LO, last2);
        self.reg[STATUS as usize] = STATUS_DONE | if errors != 0 { STATUS_ERR } else { 0 };
    }

    /// Sense read-back: the selected group of control lines as they appear
    /// on the socket side of the adapter's drivers.
    fn sense_byte(&self) -> u8 {
        let sel: Vec<u8> = (0..5u8)
            .filter(|g| self.gpio_out(ramspec::IO_SENSE_SEL0_N + g) == Some(false))
            .collect();
        if sel.len() != 1 {
            return 0xFF;
        }
        if sel[0] >= 3 {
            return if sel[0] == 3 { self.id_lines } else { 0 };
        }
        let driving = self.gpio_out(ramspec::IO_CTL_DRV_EN_N) == Some(false) && self.vdut_on;
        let mut level = [true; 24];
        for (k, io) in CTL_SENSE_ORDER.iter().enumerate() {
            level[k] = driving && self.gpio_out(*io).unwrap_or(true);
        }
        if let Some(d) = &self.dut {
            for f in &d.faults {
                match *f {
                    Fault::CtlStuck(k, v) => level[k as usize] = v,
                    Fault::CtlShort(a, b) => {
                        let w = level[a as usize] && level[b as usize];
                        level[a as usize] = w;
                        level[b as usize] = w;
                    }
                    _ => {}
                }
            }
        }
        let g = sel[0] as usize;
        (0..8).fold(0u8, |acc, i| acc | (level[g * 8 + i] as u8) << i)
    }
}

impl Platform for SimPlatform {
    fn reg_read(&mut self, addr: u8) -> u32 {
        match addr {
            TICK_PS => self.tick_ps,
            STATUS => self.r(STATUS) | if self.vdut_on { STATUS_DUT_PG } else { 0 },
            a if (GPIO_IN..GPIO_IN + 4).contains(&a) => {
                let w = (a - GPIO_IN) as u32;
                let s0 = ramspec::IO_SENSE0 as u32;
                let word = (self.sense_byte() as u128) << s0;
                (word >> (32 * w)) as u32
            }
            _ => self.r(addr),
        }
    }

    fn reg_write(&mut self, addr: u8, value: u32) {
        match addr {
            CTRL => {
                self.reg[CTRL as usize] = value & !(CTRL_START | CTRL_ABORT);
                if value & CTRL_START != 0 && value & CTRL_ENGINE != 0 && self.r(MARCH) & 7 != 0 {
                    self.run_element();
                }
            }
            ID | STATUS | ERR_COUNT..=TICK_PS => {}
            _ => self.reg[addr as usize] = value,
        }
    }

    fn delay_us(&mut self, us: u32) {
        self.advance_ps(us as u64 * 1_000_000);
    }

    fn set_vdut_mv(&mut self, mv: u16) {
        self.vdut_mv = mv;
    }

    fn vdut_enable(&mut self, on: bool) -> bool {
        self.vdut_on = on;
        if on {
            if let Some(d) = &self.dut {
                if d.current_ma > 2000 {
                    self.vdut_on = false; // current limit trips
                    return false;
                }
            }
        }
        true
    }

    fn vdut_current_ma(&mut self) -> u16 {
        self.dut.as_ref().map(|d| d.current_ma).unwrap_or(0)
    }
}
