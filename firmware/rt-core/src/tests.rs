//! The memory tests, composed from engine "march elements".

use crate::device::{Geometry, Profile};
use crate::ramspec;
use crate::regs::*;
use crate::report::{Failure, Mode, Phase, Report, WireFault};
use crate::timing::{self, Timing};
use crate::{read40, write40, Platform};

// march operations: bit 0 = write, bit 1 = inverted data
pub const R0: u8 = 0b00;
pub const R1: u8 = 0b10;
pub const W0: u8 = 0b01;
pub const W1: u8 = 0b11;

pub const PAT_SOLID: u8 = 0;
pub const PAT_CHECKER: u8 = 1;
pub const PAT_ADDRESS: u8 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ElemResult {
    pub errors: u32,
    pub fail_addr: u32,
    pub fail_bits: u64,
    pub fail_data: u64,
    pub last: u64,
    pub last2: u64,
}

/// A configured test session on one socket profile.
pub struct Session<'a, P: Platform> {
    pub p: &'a mut P,
    pub profile: &'static Profile,
    pub geom: Geometry,
    pub timing: Timing,
    /// Round-trip delay of the adapter buffers and wiring, from calibration.
    pub cal_ps: u32,
    refresh: bool,
}

impl<'a, P: Platform> Session<'a, P> {
    pub fn new(p: &'a mut P, profile: &'static Profile, cal_ps: u32) -> Self {
        let geom = Geometry {
            row_bits: profile.max_row_bits,
            col_bits: profile.max_col_bits,
            width_mask: profile.mask,
        };
        let timing = Timing::relaxed(profile.max_t_rac_ns as u32);
        Session { p, profile, geom, timing, cal_ps, refresh: true }
    }

    fn ctrl(&self) -> u32 {
        CTRL_ENGINE | if self.refresh { CTRL_RFSH_EN } else { 0 }
    }

    /// Write geometry, masks, line selects and timing to the engine.
    pub fn configure(&mut self) {
        let pr = self.profile;
        self.p.reg_write(GEOM, self.geom.row_bits as u32 | (self.geom.col_bits as u32) << 4);
        write40(self.p, DRV_LO, pr.drive);
        write40(self.p, MASK_LO, self.geom.width_mask & pr.mask);
        self.p.reg_write(SEL, pr.ras as u32 | (pr.cas as u32) << 4 | (pr.we as u32) << 8
            | (pr.oe as u32) << 10 | (pr.split0 as u32) << 12 | (pr.split1 as u32) << 13);
        let tick_ps = timing::apply(self.p, &self.timing);
        // one refresh cycle per interval/rows, expressed in clk cycles (2 ticks)
        let rows = 1u32 << self.geom.row_bits;
        let period_ps = pr.refresh_ms as u64 * 1_000_000_000 / rows as u64;
        let cycles = (period_ps / (2 * tick_ps as u64)).clamp(64, 0xFF_FFFF) as u32;
        self.p.reg_write(RFSH_PERIOD, cycles);
        self.p.reg_write(CTRL, self.ctrl());
    }

    pub fn set_pattern(&mut self, pat: u64) {
        write40(self.p, PAT_LO, pat);
    }

    /// Weak pull direction on DQ lines the DUT is not driving.
    pub fn set_pulls(&mut self, high: bool) {
        let a = ramspec::IO_PULL_A as u32;
        let b = ramspec::IO_PULL_B as u32;
        let word = (a / 32) as u8;
        debug_assert_eq!(a / 32, b / 32);
        let bits = 1u32 << (a % 32) | 1u32 << (b % 32);
        let out = self.p.reg_read(GPIO_OUT + word);
        let oe = self.p.reg_read(GPIO_OE + word);
        self.p.reg_write(GPIO_OUT + word, if high { out | bits } else { out & !bits });
        self.p.reg_write(GPIO_OE + word, oe | bits);
    }

    /// Run one march element over [start, end] and collect the result.
    pub fn element(&mut self, start: u32, end: u32, down: bool, pat: u8, ops: &[u8]) -> ElemResult {
        debug_assert!(!ops.is_empty() && ops.len() <= 6);
        let mut word = ops.len() as u32 | (down as u32) << 3 | (pat as u32) << 4;
        for (i, op) in ops.iter().enumerate() {
            word |= (*op as u32 & 3) << (8 + 2 * i);
        }
        self.p.reg_write(ADDR_START, start);
        self.p.reg_write(ADDR_END, end);
        self.p.reg_write(MARCH, word);
        self.p.reg_write(CTRL, self.ctrl() | CTRL_START);
        let mut guard = 0u32;
        while self.p.reg_read(STATUS) & STATUS_BUSY != 0 {
            self.p.delay_us(200);
            guard += 1;
            if guard > 3_000_000 {
                self.p.reg_write(CTRL, self.ctrl() | CTRL_ABORT);
                break;
            }
        }
        ElemResult {
            errors: self.p.reg_read(ERR_COUNT),
            fail_addr: self.p.reg_read(FAIL_ADDR),
            fail_bits: read40(self.p, FAIL_BITS_LO),
            fail_data: read40(self.p, FAIL_DATA_LO),
            last: read40(self.p, LAST_LO),
            last2: read40(self.p, SMP2_LO),
        }
    }

    fn last_addr(&self) -> u32 {
        self.geom.words() - 1
    }

    fn run(&mut self, phase: Phase, steps: &[(bool, u8, &[u8])]) -> Option<Failure> {
        let end = self.last_addr();
        let mut total = 0u32;
        let mut first: Option<ElemResult> = None;
        let mut bits = 0u64;
        for (down, pat, ops) in steps {
            let r = self.element(0, end, *down, *pat, ops);
            if r.errors != 0 {
                total = total.saturating_add(r.errors);
                bits |= r.fail_bits;
                first.get_or_insert(r);
            }
        }
        first.map(|r| Failure { phase, errors: total, addr: r.fail_addr, bits, read: r.fail_data })
    }

    /// March C-: {any(w0); up(r0,w1); up(r1,w0); down(r0,w1); down(r1,w0); any(r0)}.
    /// Detects stuck-at, transition, address-decoder and coupling faults (10n).
    pub fn march_c_minus(&mut self, phase: Phase, pat: u8) -> Option<Failure> {
        self.run(phase, &[
            (false, pat, &[W0]), (false, pat, &[R0, W1]), (false, pat, &[R1, W0]),
            (true, pat, &[R0, W1]), (true, pat, &[R1, W0]), (false, pat, &[R0]),
        ])
    }

    /// Write and verify every cell twice with a pattern and its complement.
    pub fn pattern_pass(&mut self, phase: Phase, pat: u8) -> Option<Failure> {
        self.run(phase, &[(false, pat, &[W0]), (false, pat, &[R0]),
                          (false, pat, &[W1]), (false, pat, &[R1])])
    }

    // ---------------------------------------------------------- detection
    /// Find which DQ lines a device drives: a line is present if it reads 0
    /// against pull-ups after writing 0 and 1 against pull-downs after writing 1.
    pub fn detect_width(&mut self) -> u64 {
        let full = self.profile.mask;
        self.geom.width_mask = full;
        self.configure();
        self.set_pattern(0);
        self.set_pulls(true);
        self.element(0, 0, false, PAT_SOLID, &[W0]);
        let zeros = !self.element(0, 0, false, PAT_SOLID, &[R0]).last;
        self.set_pulls(false);
        self.element(0, 0, false, PAT_SOLID, &[W1]);
        let ones = self.element(0, 0, false, PAT_SOLID, &[R1]).last;
        self.set_pulls(true);
        self.geom.width_mask = zeros & ones & full;
        self.geom.width_mask
    }

    /// Find the number of row and column address bits by aliasing: if writing
    /// at an address with one more bit set overwrites address 0, that
    /// address line is not decoded by the device.
    pub fn detect_size(&mut self) -> (u8, u8) {
        self.geom.row_bits = self.profile.max_row_bits;
        self.geom.col_bits = self.profile.max_col_bits;
        self.configure();
        self.set_pattern(0x55_5555_5555);
        let cb = self.profile.max_col_bits as u32;
        // An address line is decoded if writing the complement at `addr`
        // leaves address 0 alone. Judge by majority so a few bad data bits
        // (found later by the pattern tests) do not spoil the size.
        let probe = |s: &mut Self, addr: u32| -> bool {
            s.element(0, 0, false, PAT_SOLID, &[W0]);
            s.element(addr, addr, false, PAT_SOLID, &[W1]);
            let r = s.element(0, 0, false, PAT_SOLID, &[R0]);
            let width = (s.geom.width_mask & s.profile.mask).count_ones();
            r.fail_bits.count_ones() * 2 <= width
        };
        let mut cols = 0u8;
        while (cols as u32) < cb && probe(self, 1 << cols) {
            cols += 1;
        }
        let mut rows = 0u8;
        while rows < self.profile.max_row_bits && probe(self, 1 << (cb + rows as u32)) {
            rows += 1;
        }
        self.geom.row_bits = rows;
        self.geom.col_bits = cols;
        self.configure();
        (rows, cols)
    }

    /// EDO parts keep driving after CAS rises while RAS is low; FPM parts
    /// float, so the pull-ups win.
    pub fn detect_mode(&mut self) -> Mode {
        let saved = self.timing;
        self.timing = saved.edo_probe();
        self.configure();
        self.set_pattern(0);
        self.set_pulls(true);
        self.element(0, 0, false, PAT_SOLID, &[W0]);
        let r = self.element(0, 0, false, PAT_SOLID, &[R0]);
        self.timing = saved;
        self.configure();
        if r.last2 & self.geom.width_mask == 0 { Mode::Edo } else { Mode::Fpm }
    }

    /// Measure the RAS access time: move the read strobe earlier until reads
    /// fail, first in whole ticks, then with the fine input delay. Returns
    /// nanoseconds, or None if the device fails even at the slowest timing.
    pub fn measure_t_rac(&mut self) -> Option<u16> {
        let span = self.last_addr().min(255);
        self.set_pattern(0x5A_5A5A_5A5A);
        self.configure();
        self.element(0, span, false, PAT_ADDRESS, &[W0]);
        let tick_ps = timing::tick_ps(self.p);
        let ras_f = timing::ns_to_ticks(self.p, self.timing.ras_f);
        let latest = timing::ns_to_ticks(self.p, self.timing.smp);
        let earliest = timing::ns_to_ticks(self.p, self.timing.cas_f) + 1;
        let t = self.timing;
        let pass = |s: &mut Self, tick: u32, fine: u32| -> bool {
            timing::apply_strobe_ticks(s.p, &t, tick, fine);
            s.element(0, span, false, PAT_ADDRESS, &[R0]).errors == 0
        };
        if !pass(self, latest, 0) {
            timing::apply(self.p, &t);
            return None;
        }
        // binary search for the first passing tick
        let (mut lo, mut hi) = (earliest, latest); // hi passes
        while lo < hi {
            let mid = (lo + hi) / 2;
            if pass(self, mid, 0) { hi = mid } else { lo = mid + 1 }
        }
        // refine: the input delay moves the strobe earlier in 25 ps steps
        let max_fine = (tick_ps / FINE_STEP_PS).min(FINE_MAX);
        let (mut flo, mut fhi) = (0u32, max_fine); // largest passing fine in [flo, fhi]
        while flo < fhi {
            let mid = (flo + fhi + 1) / 2;
            if pass(self, hi, mid) { flo = mid } else { fhi = mid - 1 }
        }
        timing::apply(self.p, &t);
        let ps = (hi - ras_f) as i64 * tick_ps as i64 - (flo * FINE_STEP_PS) as i64
            - self.cal_ps as i64;
        Some(((ps.max(0) + 500) / 1000) as u16)
    }

    /// Longest refresh pause (from `steps`, milliseconds) after which the
    /// array still reads back. None if even the first step fails.
    pub fn retention(&mut self, steps: &[u32]) -> Option<u32> {
        let end = self.last_addr();
        let mut best = None;
        self.set_pattern(0x55_5555_5555);
        for &ms in steps {
            self.refresh = true;
            self.configure();
            self.element(0, end, false, PAT_CHECKER, &[W0]);
            self.refresh = false;
            self.p.reg_write(CTRL, self.ctrl());
            self.p.delay_us(ms * 1000);
            let ok = self.element(0, end, false, PAT_CHECKER, &[R0]).errors == 0;
            self.refresh = true;
            self.p.reg_write(CTRL, self.ctrl());
            if !ok {
                break;
            }
            best = Some(ms);
        }
        best
    }

    // -------------------------------------------------------------- wiring
    /// Drive each control line through the adapter's drivers and read it back
    /// through the sense buffers: finds stuck lines and shorts between lines.
    pub fn wiring_test(&mut self) -> [Option<WireFault>; 8] {
        let mut faults = [None; 8];
        let mut n = 0;
        let mut push = |f: WireFault, faults: &mut [Option<WireFault>; 8]| {
            if n < 8 && !faults.contains(&Some(f)) {
                faults[n] = Some(f);
                n += 1;
            }
        };
        self.p.reg_write(CTRL, 0); // engine off: lines follow the GPIO registers
        let level = |v: bool| -> Lines {
            let mut l = Lines::default();
            for io in CTL_SENSE_ORDER.iter() {
                l.set(*io, v);
            }
            l
        };
        // 1. stuck lines: everything high, then everything low
        let high = self.drive_and_sense(&level(true));
        let low = self.drive_and_sense(&level(false));
        let mut stuck = [false; 24];
        for k in 0..24usize {
            if high >> k & 1 == 0 {
                stuck[k] = true;
                push(WireFault::StuckLow(k as u8), &mut faults);
            } else if low >> k & 1 == 1 {
                stuck[k] = true;
                push(WireFault::StuckHigh(k as u8), &mut faults);
            }
        }
        // 2. shorts: walk a zero, then a one, across the lines that can move
        for pass in 0..2 {
            for i in 0..24usize {
                if stuck[i] {
                    continue;
                }
                let mut lines = level(pass == 0);
                lines.set(CTL_SENSE_ORDER[i], pass != 0);
                let back = self.drive_and_sense(&lines);
                for k in 0..24usize {
                    let want = if k == i { pass != 0 } else { pass == 0 };
                    if stuck[k] || (back >> k & 1 == 1) == want {
                        continue;
                    }
                    if k != i {
                        let (a, b) = if i < k { (i, k) } else { (k, i) };
                        push(WireFault::Short(a as u8, b as u8), &mut faults);
                    }
                }
            }
        }
        // leave the lines idle (all high) and hand back to the engine
        self.drive_and_sense(&level(true));
        self.configure();
        faults
    }

    /// Put the control lines on the bus, then read all three sense groups.
    fn drive_and_sense(&mut self, lines: &Lines) -> u32 {
        let mut back = 0u32;
        for g in 0..3u8 {
            let mut l = *lines;
            for io in [ramspec::IO_DQ_DRV_EN0_N, ramspec::IO_DQ_DRV_EN1_N,
                       ramspec::IO_DQ_DRV_EN2_N, ramspec::IO_DQ_DRV_EN3_N,
                       ramspec::IO_DQ_DRV_EN4_N, ramspec::IO_DQ_RX_EN_N] {
                l.set(io, true);
            }
            l.set(ramspec::IO_CTL_DRV_EN_N, false);
            for s in 0..5u8 {
                l.set(ramspec::IO_SENSE_SEL0_N + s, s != g);
            }
            l.set(ramspec::IO_PULL_A, true);
            l.set(ramspec::IO_PULL_B, true);
            for w in 0..4u8 {
                self.p.reg_write(GPIO_OUT + w, l.out[w as usize]);
                self.p.reg_write(GPIO_OE + w, l.oe[w as usize]);
            }
            self.p.delay_us(20);
            let s0 = ramspec::IO_SENSE0 as u32;
            let lo = self.p.reg_read(GPIO_IN + (s0 / 32) as u8) as u64;
            let hi = self.p.reg_read(GPIO_IN + (s0 / 32) as u8 + 1) as u64;
            let byte = ((lo | hi << 32) >> (s0 % 32)) as u32 & 0xFF;
            back |= byte << (8 * g);
        }
        back
    }

    /// Read the eight ID / presence-detect lines of sense group 3.
    pub fn read_id(&mut self) -> u8 {
        let mut l = Lines::default();
        for s in 0..5u8 {
            l.set(ramspec::IO_SENSE_SEL0_N + s, s != 3);
        }
        l.set(ramspec::IO_CTL_DRV_EN_N, true);
        for w in 0..4u8 {
            self.p.reg_write(GPIO_OUT + w, l.out[w as usize]);
            self.p.reg_write(GPIO_OE + w, l.oe[w as usize]);
        }
        self.p.delay_us(20);
        let s0 = ramspec::IO_SENSE0 as u32;
        let lo = self.p.reg_read(GPIO_IN + (s0 / 32) as u8) as u64;
        let hi = self.p.reg_read(GPIO_IN + (s0 / 32) as u8 + 1) as u64;
        ((lo | hi << 32) >> (s0 % 32)) as u8
    }
}

/// IO line numbers of the 24 control lines in sense order.
pub const CTL_SENSE_ORDER: [u8; 24] = [
    ramspec::IO_MA0, ramspec::IO_MA1, ramspec::IO_MA2, ramspec::IO_MA3, ramspec::IO_MA4,
    ramspec::IO_MA5, ramspec::IO_MA6, ramspec::IO_MA7, ramspec::IO_MA8, ramspec::IO_MA9,
    ramspec::IO_MA10, ramspec::IO_MA11, ramspec::IO_WE0_N, ramspec::IO_WE1_N,
    ramspec::IO_OE0_N, ramspec::IO_OE1_N, ramspec::IO_RAS0_N, ramspec::IO_RAS1_N,
    ramspec::IO_RAS2_N, ramspec::IO_RAS3_N, ramspec::IO_CAS0_N, ramspec::IO_CAS1_N,
    ramspec::IO_CAS2_N, ramspec::IO_CAS3_N,
];

/// GPIO image of the 116 bus lines.
#[derive(Clone, Copy, Default)]
struct Lines {
    out: [u32; 4],
    oe: [u32; 4],
}

impl Lines {
    fn set(&mut self, io: u8, high: bool) {
        let (w, b) = ((io / 32) as usize, io % 32);
        self.oe[w] |= 1 << b;
        if high { self.out[w] |= 1 << b } else { self.out[w] &= !(1 << b) }
    }
}

// ------------------------------------------------------------- full tests
fn power_up<P: Platform>(s: &mut Session<P>, mv: u16, rep: &mut Report) -> bool {
    s.p.set_vdut_mv(mv);
    if !s.p.vdut_enable(true) {
        rep.power_fault = true;
        rep.current_ma = s.p.vdut_current_ma();
        s.p.vdut_enable(false);
        return false;
    }
    s.p.delay_us(500); // DRAMs need 100-200 us after power, then 8 RAS cycles
    s.configure();
    s.element(0, 7, false, PAT_SOLID, &[R0]);
    true
}

/// The quick test: wiring, identification, speed, and every cell written and
/// verified twice.
pub fn basic_test<P: Platform>(p: &mut P, profile: &'static crate::device::Profile,
                               cal_ps: u32) -> Report {
    let mut rep = Report { profile: profile.name, ..Report::default() };
    let mut s = Session::new(p, profile, cal_ps);
    if !power_up(&mut s, profile.vdut_mv, &mut rep) {
        return rep;
    }
    rep.wire_faults = s.wiring_test();
    if rep.wire_faults.iter().any(|w| w.is_some()) {
        s.p.vdut_enable(false);
        return rep;
    }
    if s.detect_width() == 0 {
        rep.no_device = true;
        s.p.vdut_enable(false);
        return rep;
    }
    s.detect_size();
    rep.geometry = s.geom;
    if s.geom.row_bits == 0 || s.geom.col_bits == 0 {
        rep.no_device = true;
        s.p.vdut_enable(false);
        return rep;
    }
    rep.mode = Some(s.detect_mode());
    rep.t_rac_ns = s.measure_t_rac();
    rep.current_ma = s.p.vdut_current_ma();
    s.set_pattern(0x55_5555_5555);
    rep.failure = s.pattern_pass(Phase::Pattern, PAT_SOLID);
    s.p.vdut_enable(false);
    rep
}

/// The long test: the basic test plus March C-, checkerboard, address-in-data,
/// supply margins, voltage bounce and retention.
pub fn extensive_test<P: Platform>(p: &mut P, profile: &'static crate::device::Profile,
                                   cal_ps: u32) -> Report {
    let mut rep = basic_test(p, profile, cal_ps);
    if !rep.passed() {
        return rep;
    }
    let geom = rep.geometry;
    let mut s = Session::new(p, profile, cal_ps);
    s.geom = geom;
    let nominal = profile.vdut_mv;
    let (low, high) = (nominal - nominal / 10, nominal + nominal / 10);
    if !power_up(&mut s, nominal, &mut rep) {
        return rep;
    }
    s.set_pattern(0);
    let mut fail = s.march_c_minus(Phase::March, PAT_SOLID);
    if fail.is_none() {
        s.set_pattern(0x55_5555_5555);
        fail = s.march_c_minus(Phase::Checkerboard, PAT_CHECKER);
    }
    if fail.is_none() {
        s.set_pattern(0);
        fail = s.pattern_pass(Phase::AddressInData, PAT_ADDRESS);
    }
    for (mv, phase) in [(low, Phase::VoltageLow), (high, Phase::VoltageHigh)] {
        if fail.is_none() {
            s.p.set_vdut_mv(mv);
            s.p.delay_us(20_000);
            s.set_pattern(0x33_3333_3333);
            fail = s.march_c_minus(phase, PAT_CHECKER);
        }
    }
    if fail.is_none() {
        // bounce: write at the low limit, read at the high limit, and back
        let end = s.geom.words() - 1;
        s.set_pattern(0x69_6969_6969);
        for (wv, rv) in [(low, high), (high, low)] {
            s.p.set_vdut_mv(wv);
            s.p.delay_us(20_000);
            s.element(0, end, false, PAT_ADDRESS, &[W0]);
            s.p.set_vdut_mv(rv);
            s.p.delay_us(20_000);
            let r = s.element(0, end, false, PAT_ADDRESS, &[R0]);
            if r.errors != 0 && fail.is_none() {
                fail = Some(Failure { phase: Phase::Bounce, errors: r.errors,
                                      addr: r.fail_addr, bits: r.fail_bits, read: r.fail_data });
            }
        }
    }
    s.p.set_vdut_mv(nominal);
    s.p.delay_us(20_000);
    if fail.is_none() {
        let base = profile.refresh_ms as u32;
        let steps = [base, base * 2, base * 4, base * 8, base * 16, base * 32];
        rep.retention_ms = s.retention(&steps);
        if rep.retention_ms.is_none() {
            fail = Some(Failure { phase: Phase::Retention, errors: 1, addr: 0, bits: 0, read: 0 });
        }
    }
    rep.failure = fail;
    s.p.vdut_enable(false);
    rep
}
