//! Whole-chain simulation of the RAMalyser.
//!
//! The firmware side is the real code: the test logic of `rt-core` runs on
//! the `rt-hal` board driver, which talks SPI to the FPGA, I2C to the DAC
//! and the current monitor, and switches the supply through GPIO pins.
//!
//! The FPGA side is the real gateware: `spi_regs.v` and `tester_core.v`,
//! compiled with Verilator and clocked at the real tick.
//!
//! Between and around them this crate models, in software:
//!
//! * the DDR pad cells (first and second half of each clock cycle on the
//!   way out, rising and falling edge samples with the fine input delay on
//!   the way back),
//! * a cartridge: drive buffers, receive buffers, byte-lane enables, weak
//!   pulls on the data lines, and the sense read-back of the control lines,
//! * a DRAM at pin level, with access time, EDO or FPM outputs, retention
//!   and injectable faults,
//! * the DUT supply: DAC set point, buck enable, switch, over-voltage veto
//!   and current monitor, as ideal parts.
//!
//! What it does not model: analogue signal quality, the PLL, the ECP5
//! primitives, the cartridge's level shifting, and supply ramps. Buffer
//! delays are fixed numbers. The enables of the cartridge take the same
//! delay as its data path.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::c_void;
use std::rc::Rc;

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{self, InputPin, OutputPin};
use embedded_hal::i2c::{self, I2c};
use embedded_hal::spi::{self, Operation, SpiDevice};
use rt_core::device::Profile;
use rt_core::ramspec;
pub use rt_core::sim::Fault;
use rt_core::tests::CTL_SENSE_ORDER;

extern "C" {
    fn cosim_new() -> *mut c_void;
    fn cosim_free(h: *mut c_void);
    fn cosim_cycle(h: *mut c_void, q0: *const u32, q1: *const u32, pins: u32, d0: *mut u32,
                   d1: *mut u32, oe: *mut u32) -> u32;
}

/// Half a clock period of the gateware, picoseconds.
pub const TICK_PS: u64 = 3077;
/// Cartridge drive buffer delay, FPGA pad to socket.
pub const T_DRV_PS: u64 = 4000;
/// Cartridge receive buffer delay, socket to FPGA pad.
pub const T_RX_PS: u64 = 5000;
/// Round trip the firmware has to subtract from a strobe position.
pub const PATH_PS: u32 = (T_DRV_PS + T_RX_PS) as u32;
/// DRAM output turn-off time.
const T_OFF_PS: u64 = 3000;
const FINE_PS: u64 = 25;
/// SPI clock of the register bus, as rt-fw sets it.
const SPI_HALF_PS: u64 = 125_000;
const ALL40: u64 = 0xFF_FFFF_FFFF;
const HISTORY_PS: u64 = 60_000;

/// The memory device in the socket.
#[derive(Clone, Debug)]
pub struct Dut {
    pub row_bits: u8,
    pub col_bits: u8,
    /// DQ lines the device drives on a read.
    pub width_mask: u64,
    /// Data in on DQ0, data out on DQ1.
    pub split0: bool,
    /// Parity in on DQ32, parity out on DQ33.
    pub split1: bool,
    pub t_rac_ps: u64,
    pub t_cac_ps: u64,
    pub edo: bool,
    /// Cell retention without refresh, picoseconds.
    pub retention_ps: Option<u64>,
    /// Works only inside this supply window, millivolts.
    pub v_min: u32,
    pub v_max: u32,
    pub current_ma: u16,
    pub faults: Vec<Fault>,
}

impl Dut {
    pub fn new(row_bits: u8, col_bits: u8, width_mask: u64, t_rac_ns: u32) -> Self {
        Dut {
            row_bits, col_bits, width_mask, split0: false, split1: false,
            t_rac_ps: t_rac_ns as u64 * 1000, t_cac_ps: t_rac_ns as u64 * 250, edo: false,
            retention_ps: None, v_min: 4400, v_max: 5600, current_ma: 120, faults: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Snap {
    t: u64,
    out: u128,
    oe: u128,
}

#[derive(Default)]
struct Dram {
    mem: Vec<u64>,
    last_ref: Vec<u64>,
    row: usize,
    cbr_row: usize,
    ras_n: bool,
    cas_n: bool,
    oe_n: bool,
    t_ras: u64,
    drive: bool,
    off_at: Option<u64>,
    value: u64,
    t_valid: u64,
}

/// The DUT supply as ideal parts.
pub struct Power {
    pub dac_code: u16,
    pub buck_en: bool,
    pub dut_sw: bool,
    pub discharge: bool,
    pub dut_pg: bool,
    /// Over-voltage comparator threshold, millivolts.
    pub ovp_mv: u32,
}

impl Power {
    /// Regulator output for the DAC code, from the feedback network:
    /// 0.8 V reference, 100k to the rail, 20k to ground, 100k to the DAC.
    pub fn rail_mv(&self) -> u32 {
        if !self.buck_en {
            return 0;
        }
        let vdac = self.dac_code as f64 * 3300.0 / 4096.0;
        (800.0 * (1.0 + 100.0 / 20.0) + (800.0 - vdac) * 100.0 / 100.0).max(0.0) as u32
    }

    pub fn tripped(&self) -> bool {
        self.rail_mv() > self.ovp_mv
    }

    /// Voltage at the socket.
    pub fn socket_mv(&self) -> u32 {
        if self.dut_sw && !self.tripped() { self.rail_mv() } else { 0 }
    }
}

pub struct World {
    gw: *mut c_void,
    /// Time of the next rising clock edge, picoseconds.
    pub now: u64,
    d_old: (u128, u128, u128),
    s0: u128,
    s1: u128,
    s1n: u128,
    fine: u64,
    sck: bool,
    cs_n: bool,
    mosi: bool,
    miso: bool,
    rst: bool,
    pub busy: bool,
    hist: VecDeque<Snap>,
    pending: VecDeque<Snap>,
    stable_from: u64,
    cache: Option<u128>,
    pub profile: &'static Profile,
    pub dut: Option<Dut>,
    dram: Dram,
    pub power: Power,
    /// ID and presence-detect lines of the cartridge (sense group 3).
    pub id_lines: u8,
    /// Clock cycles simulated.
    pub cycles: u64,
}

impl Drop for World {
    fn drop(&mut self) {
        unsafe { cosim_free(self.gw) }
    }
}

fn words(v: u128) -> [u32; 4] {
    [v as u32, (v >> 32) as u32, (v >> 64) as u32, (v >> 96) as u32]
}

fn wide(w: [u32; 4]) -> u128 {
    w[0] as u128 | (w[1] as u128) << 32 | (w[2] as u128) << 64 | (w[3] as u128) << 96
}

impl World {
    pub fn new(profile: &'static Profile, dut: Option<Dut>) -> Self {
        let mut dram = Dram { ras_n: true, cas_n: true, oe_n: true, ..Dram::default() };
        if let Some(d) = &dut {
            // power-up contents: neither all zeros nor all ones
            dram.mem = (0..1usize << (d.row_bits + d.col_bits))
                .map(|a| (a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 20 & d.width_mask)
                .collect();
            dram.last_ref = vec![0; 1 << d.row_bits];
        }
        let mut w = World {
            gw: unsafe { cosim_new() }, now: 0, d_old: (0, 0, 0), s0: 0, s1: 0, s1n: 0, fine: 0,
            sck: false, cs_n: true, mosi: false, miso: false, rst: true, busy: false,
            hist: VecDeque::new(), pending: VecDeque::new(), stable_from: 0, cache: None,
            profile, dut, dram,
            power: Power { dac_code: 0, buck_en: false, dut_sw: false, discharge: false,
                           dut_pg: false, ovp_mv: 5863 },
            id_lines: 0xFF, cycles: 0,
        };
        for _ in 0..32 {
            w.cycle();
        }
        w.rst = false;
        for _ in 0..32 {
            w.cycle();
        }
        w
    }

    fn powered(&self) -> bool {
        let mv = self.power.socket_mv();
        match &self.dut {
            Some(d) => mv >= d.v_min && mv <= d.v_max,
            None => mv > 0,
        }
    }

    /// The supply changed: whatever the pads read may change with it.
    fn power_changed(&mut self) {
        self.cache = None;
        if !self.powered() {
            self.dram.drive = false;
        }
    }

    // ------------------------------------------------------------ the clock
    /// One clock cycle: a rising edge at `now`, a falling edge half way.
    fn cycle(&mut self) {
        let t = self.now;
        let (d0, d1, oe) = self.d_old;
        let pins = self.sck as u32 | (self.cs_n as u32) << 1 | (self.mosi as u32) << 2
            | (self.power.dut_pg as u32) << 3 | (self.rst as u32) << 4;
        let (mut nd0, mut nd1, mut noe) = ([0u32; 4], [0u32; 4], [0u32; 4]);
        let r = unsafe {
            cosim_cycle(self.gw, words(self.s0).as_ptr(), words(self.s1).as_ptr(), pins,
                        nd0.as_mut_ptr(), nd1.as_mut_ptr(), noe.as_mut_ptr())
        };
        self.miso = r & 1 != 0;
        self.busy = r & 2 != 0;
        // input registers of the pad cells
        let rise = self.pad_in(t);
        self.s1 = self.s1n;
        self.s0 = rise;
        // the pads carry d0 in the first half of the cycle and d1 in the second
        self.push(t, d0, oe);
        self.s1n = self.pad_in(t + TICK_PS);
        self.push(t + TICK_PS, d1, oe);
        self.d_old = (wide(nd0), wide(nd1), wide(noe));
        let fine = (r >> 8 & 0x7F) as u64;
        if fine != self.fine {
            self.fine = fine;
            self.cache = None;
        }
        self.now = t + 2 * TICK_PS;
        self.cycles += 1;
    }

    /// Advance simulated time.
    pub fn run(&mut self, ps: u64) {
        let until = self.now + ps;
        while self.now < until {
            self.cycle();
        }
    }

    fn push(&mut self, t: u64, out: u128, oe: u128) {
        if let Some(last) = self.hist.back() {
            if last.out == out && last.oe == oe {
                return;
            }
        }
        let s = Snap { t, out, oe };
        self.hist.push_back(s);
        self.pending.push_back(s);
        while self.hist.len() > 1 && self.hist[1].t + HISTORY_PS < t {
            self.hist.pop_front();
        }
        self.unsettle(t + T_DRV_PS + T_RX_PS);
    }

    /// The pads may still change until `t` plus the longest input delay.
    fn unsettle(&mut self, t: u64) {
        self.stable_from = self.stable_from.max(t + 127 * FINE_PS + 1);
        self.cache = None;
    }

    /// What the FPGA drove on its pads at time `t`.
    fn fpga_at(&self, t: u64) -> Snap {
        self.hist.iter().rev().find(|s| s.t <= t).copied().unwrap_or_default()
    }

    // ------------------------------------------------------- the cartridge
    fn line(s: &Snap, io: u8, idle: bool) -> bool {
        if s.oe >> io & 1 == 1 { s.out >> io & 1 == 1 } else { idle }
    }

    /// The 24 control lines on the socket side of the drivers, in sense order.
    fn ctl_levels(&self, s: &Snap) -> u32 {
        let driving = !Self::line(s, ramspec::IO_CTL_DRV_EN_N, true) && self.powered();
        let mut level = 0u32;
        for (k, io) in CTL_SENSE_ORDER.iter().enumerate() {
            if driving && Self::line(s, *io, true) {
                level |= 1 << k;
            }
        }
        if let Some(d) = &self.dut {
            for f in &d.faults {
                match *f {
                    Fault::CtlStuck(k, v) => level = level & !(1 << k) | (v as u32) << k,
                    Fault::CtlShort(a, b) => {
                        let w = level >> a & level >> b & 1;
                        level = level & !(1 << a) & !(1 << b) | w << a | w << b;
                    }
                    _ => {}
                }
            }
        }
        level
    }

    /// Data lines the cartridge drives onto the socket: (mask, value).
    fn dq_driven(&self, s: &Snap) -> (u64, u64) {
        if !self.powered() {
            return (0, 0);
        }
        let mut lanes = 0u64;
        for k in 0..5u8 {
            if !Self::line(s, ramspec::IO_DQ_DRV_EN0_N + k, true) {
                lanes |= 0xFF << (8 * k);
            }
        }
        let mask = s.oe as u64 & ALL40 & lanes;
        (mask, s.out as u64 & mask)
    }

    fn pulls(s: &Snap) -> u64 {
        let even = 0x55_5555_5555u64;
        (if Self::line(s, ramspec::IO_PULL_A, true) { even } else { 0 })
            | (if Self::line(s, ramspec::IO_PULL_B, true) { !even & ALL40 } else { 0 })
    }

    /// Data lines at the socket at time `ts`.
    fn dq_socket(&mut self, ts: u64) -> u64 {
        let s = self.fpga_at(ts.saturating_sub(T_DRV_PS));
        self.dram_until(ts);
        let (fm, fv) = self.dq_driven(&s);
        let (dm, dv) = self.dram_out(ts);
        let pull = Self::pulls(&s);
        fv | dv & dm & !fm | pull & !(fm | dm)
    }

    /// Levels on the FPGA pads at time `t`, as the input cells see them.
    fn pad_in(&mut self, t: u64) -> u128 {
        let tp = t.saturating_sub(self.fine * FINE_PS);
        if tp >= self.stable_from {
            if let Some(c) = self.cache {
                return c;
            }
        }
        let own = self.fpga_at(tp);
        let ts = tp.saturating_sub(T_RX_PS);
        let s = self.fpga_at(ts.saturating_sub(T_DRV_PS));
        let mut pad = self.cache.unwrap_or(0);
        // data lines through the receive buffers
        if !Self::line(&s, ramspec::IO_DQ_RX_EN_N, true) && self.power.socket_mv() > 0 {
            pad = pad & !(ALL40 as u128) | self.dq_socket(ts) as u128;
        } else {
            self.dram_until(ts);
        }
        // one sense group on SENSE0..7
        let sel: Vec<u8> = (0..5u8)
            .filter(|g| !Self::line(&s, ramspec::IO_SENSE_SEL0_N + g, true))
            .collect();
        let byte = match sel.as_slice() {
            [g] if *g < 3 => (self.ctl_levels(&s) >> (8 * g)) as u8,
            [3] => self.id_lines,
            [_] => 0,
            _ => 0xFF,
        };
        let s0 = ramspec::IO_SENSE0;
        pad = pad & !(0xFFu128 << s0) | (byte as u128) << s0;
        // a pad the FPGA drives reads its own level
        pad = pad & !own.oe | own.out & own.oe;
        if tp >= self.stable_from {
            self.cache = Some(pad);
        }
        pad
    }

    // ------------------------------------------------------------ the DRAM
    /// Let the DRAM see every change that reaches the socket up to `ts`.
    fn dram_until(&mut self, ts: u64) {
        while let Some(s) = self.pending.front().copied() {
            if s.t + T_DRV_PS > ts {
                break;
            }
            self.pending.pop_front();
            self.dram_event(s.t + T_DRV_PS, &s);
        }
        if let Some(off) = self.dram.off_at {
            if off <= ts {
                self.dram.drive = false;
                self.dram.off_at = None;
            }
        }
    }

    fn refresh_row(&mut self, row: usize, t: u64) {
        let d = self.dut.as_ref().unwrap();
        if let Some(ret) = d.retention_ps {
            if t - self.dram.last_ref[row] > ret {
                for c in 0..1usize << d.col_bits {
                    self.dram.mem[row << d.col_bits | c] ^= d.width_mask; // charge lost
                }
            }
        }
        self.dram.last_ref[row] = t;
    }

    fn dram_event(&mut self, t: u64, s: &Snap) {
        let Some(d) = self.dut.clone() else { return };
        if !self.powered() {
            return;
        }
        let pr = self.profile;
        let lv = self.ctl_levels(s);
        let bit = |k: u32| lv >> k & 1 == 1;
        let ras_n = bit(16 + (pr.ras as u32).trailing_zeros());
        let cas_n = bit(20 + (pr.cas as u32).trailing_zeros());
        let we_n = bit(12 + (pr.we as u32).trailing_zeros());
        let oe_n = pr.oe != 0 && bit(14 + (pr.oe as u32).trailing_zeros());
        let mut addr = lv & 0xFFF;
        for f in &d.faults {
            if let Fault::AddrLineLow(l) = f {
                addr &= !(1 << l);
            }
        }
        let (was_ras, was_cas) = (self.dram.ras_n, self.dram.cas_n);
        self.dram.ras_n = ras_n;
        self.dram.cas_n = cas_n;
        self.dram.oe_n = oe_n;

        if was_ras && !ras_n {
            self.dram.t_ras = t;
            if !cas_n {
                let row = self.dram.cbr_row;
                self.refresh_row(row, t);       // CAS before RAS
                self.dram.cbr_row = (row + 1) & ((1 << d.row_bits) - 1);
            } else {
                self.dram.row = addr as usize & ((1 << d.row_bits) - 1);
                let row = self.dram.row;
                self.refresh_row(row, t);
            }
        }
        if was_cas && !cas_n && !ras_n && !was_ras {
            let col = addr as usize & ((1 << d.col_bits) - 1);
            let a = self.dram.row << d.col_bits | col;
            if !we_n {
                // early write: the device stores what is on its inputs
                let (fm, fv) = self.dq_driven(s);
                let din = fv | Self::pulls(s) & !fm;
                let mut v = din;
                if d.split0 { v = (din & 1) << 1 }
                if d.split1 { v = v & !(1u64 << 33) | (din >> 32 & 1) << 33 }
                self.dram.mem[a] = v & d.width_mask;
                self.dram.drive = false;
                self.dram.off_at = None;
            } else {
                let mut v = self.dram.mem[a];
                for f in &d.faults {
                    match *f {
                        Fault::StuckBit { addr, bit, value } if addr as usize == a => {
                            v = v & !(1 << bit) | (value as u64) << bit;
                        }
                        Fault::DqShort(x, y) => {
                            let w = v >> x & v >> y & 1;
                            v = v & !(1 << x) & !(1 << y) | w << x | w << y;
                        }
                        _ => {}
                    }
                }
                self.dram.value = v;
                self.dram.t_valid = (self.dram.t_ras + d.t_rac_ps).max(t + d.t_cac_ps);
                self.dram.drive = true;
                self.dram.off_at = None;
                let tv = self.dram.t_valid;
                self.unsettle(tv + T_RX_PS);
            }
        }
        // outputs turn off: FPM when CAS rises, EDO when RAS and CAS are both high
        let cas_rose = !was_cas && cas_n;
        let ras_rose = !was_ras && ras_n;
        if self.dram.drive && ((cas_rose && (!d.edo || ras_n)) || (ras_rose && d.edo && cas_n)) {
            self.dram.off_at = Some(t + T_OFF_PS);
            self.unsettle(t + T_OFF_PS + T_RX_PS);
        }
    }

    /// Data outputs of the DRAM at time `ts`: (mask, value). Before the
    /// access time the outputs carry the complement of the cell, the worst
    /// case for a tester.
    fn dram_out(&self, ts: u64) -> (u64, u64) {
        let Some(d) = &self.dut else { return (0, 0) };
        if !self.dram.drive || self.dram.oe_n || !self.powered() {
            return (0, 0);
        }
        let v = if ts >= self.dram.t_valid { self.dram.value } else { !self.dram.value };
        (d.width_mask, v & d.width_mask)
    }

    // ------------------------------------------------------------- the SPI
    fn spi_byte(&mut self, out: u8) -> u8 {
        let mut back = 0u8;
        for n in (0..8).rev() {
            self.mosi = out >> n & 1 == 1;
            self.run(SPI_HALF_PS);
            back = back << 1 | self.miso as u8;     // mode 0: sampled on the rising edge
            self.sck = true;
            self.run(SPI_HALF_PS);
            self.sck = false;
        }
        back
    }
}

// ------------------------------------------------- embedded-hal front ends
/// Shared handle on the simulated world.
#[derive(Clone)]
pub struct Sim(pub Rc<RefCell<World>>);

impl Sim {
    pub fn new(profile: &'static Profile, dut: Option<Dut>) -> Self {
        Sim(Rc::new(RefCell::new(World::new(profile, dut))))
    }

    /// The board as the firmware builds it, on simulated buses and pins.
    #[allow(clippy::type_complexity)]
    pub fn board(&self) -> rt_hal::Board<FpgaSpi, I2cBus, Pin, Pin, Pin, Pin, Pin, Clock> {
        let pin = |which| Pin { sim: self.clone(), which };
        rt_hal::Board::new(FpgaSpi(self.clone()), I2cBus(self.clone()), pin(Which::BuckEn),
                           pin(Which::DutSw), pin(Which::Discharge), pin(Which::OvpN),
                           pin(Which::DutPg), Clock(self.clone()))
    }
}

#[derive(Debug)]
pub struct Never;

impl spi::Error for Never {
    fn kind(&self) -> spi::ErrorKind { spi::ErrorKind::Other }
}
impl i2c::Error for Never {
    fn kind(&self) -> i2c::ErrorKind { i2c::ErrorKind::Other }
}
impl digital::Error for Never {
    fn kind(&self) -> digital::ErrorKind { digital::ErrorKind::Other }
}

/// The FPGA register port on the shared SPI bus.
pub struct FpgaSpi(Sim);

impl spi::ErrorType for FpgaSpi {
    type Error = Never;
}

impl SpiDevice for FpgaSpi {
    fn transaction(&mut self, ops: &mut [Operation<'_, u8>]) -> Result<(), Never> {
        let mut w = self.0 .0.borrow_mut();
        w.cs_n = false;
        w.run(2 * SPI_HALF_PS);
        for op in ops {
            match op {
                Operation::Write(b) => b.iter().for_each(|x| { w.spi_byte(*x); }),
                Operation::Read(b) => b.iter_mut().for_each(|x| *x = w.spi_byte(0)),
                Operation::Transfer(r, t) => {
                    for (i, x) in r.iter_mut().enumerate() {
                        *x = w.spi_byte(t.get(i).copied().unwrap_or(0));
                    }
                }
                Operation::TransferInPlace(b) => b.iter_mut().for_each(|x| *x = w.spi_byte(*x)),
                Operation::DelayNs(ns) => w.run(*ns as u64 * 1000),
            }
        }
        w.run(2 * SPI_HALF_PS);
        w.cs_n = true;
        w.run(2 * SPI_HALF_PS);
        Ok(())
    }
}

/// The I2C bus with the DAC (MCP4725) and the current monitor (INA219).
pub struct I2cBus(Sim);

impl i2c::ErrorType for I2cBus {
    type Error = Never;
}

impl I2c for I2cBus {
    fn transaction(&mut self, address: u8, ops: &mut [i2c::Operation<'_>]) -> Result<(), Never> {
        let mut w = self.0 .0.borrow_mut();
        let mut pointer = 0u8;
        for op in ops {
            match (address, op) {
                (0x60, i2c::Operation::Write(b)) if b.len() == 2 => {
                    w.power.dac_code = (b[0] as u16 & 0x0F) << 8 | b[1] as u16; // fast mode
                    w.power_changed();
                }
                (0x40, i2c::Operation::Write(b)) if !b.is_empty() => pointer = b[0],
                (0x40, i2c::Operation::Read(b)) if b.len() == 2 => {
                    let reg: u16 = match pointer {
                        // bus voltage: 4 mV per count, left-aligned by three bits
                        0x02 => ((w.power.rail_mv() / 4) as u16) << 3,
                        // shunt voltage: 10 uV per count across 20 milliohms
                        0x01 => {
                            let ma = match (&w.dut, w.power.socket_mv() > 0) {
                                (Some(d), true) => d.current_ma as u32,
                                _ => 0,
                            };
                            (ma * 20 / 10) as u16
                        }
                        _ => 0,
                    };
                    b.copy_from_slice(&reg.to_be_bytes());
                }
                _ => return Err(Never),
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub enum Which {
    BuckEn,
    DutSw,
    Discharge,
    OvpN,
    DutPg,
}

pub struct Pin {
    sim: Sim,
    which: Which,
}

impl digital::ErrorType for Pin {
    type Error = Never;
}

impl Pin {
    fn set(&mut self, high: bool) {
        let mut w = self.sim.0.borrow_mut();
        match self.which {
            Which::BuckEn => w.power.buck_en = high,
            Which::DutSw => w.power.dut_sw = high,
            Which::Discharge => w.power.discharge = high,
            Which::DutPg => w.power.dut_pg = high,
            Which::OvpN => {}
        }
        w.power_changed();
    }
}

impl OutputPin for Pin {
    fn set_low(&mut self) -> Result<(), Never> {
        self.set(false);
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Never> {
        self.set(true);
        Ok(())
    }
}

impl InputPin for Pin {
    fn is_high(&mut self) -> Result<bool, Never> {
        let w = self.sim.0.borrow();
        Ok(match self.which {
            // the switch-enable node: the request, unless the comparator vetoes it
            Which::OvpN => w.power.dut_sw && !w.power.tripped(),
            Which::BuckEn => w.power.buck_en,
            Which::DutSw => w.power.dut_sw,
            Which::Discharge => w.power.discharge,
            Which::DutPg => w.power.dut_pg,
        })
    }
    fn is_low(&mut self) -> Result<bool, Never> {
        self.is_high().map(|h| !h)
    }
}

/// Delays advance simulated time.
pub struct Clock(Sim);

impl DelayNs for Clock {
    fn delay_ns(&mut self, ns: u32) {
        self.0 .0.borrow_mut().run(ns as u64 * 1000);
    }
}
