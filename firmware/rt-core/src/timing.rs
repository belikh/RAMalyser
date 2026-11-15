//! DRAM cycle timing in nanoseconds, converted to engine ticks.

use crate::{regs, Platform};

/// One read or write cycle. All times in nanoseconds from the start of the
/// cycle; RAS falls at `ras_f`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    pub ras_f: u32,
    pub mux: u32,    // address switches from row to column
    pub cas_f: u32,
    pub we_f: u32,   // early write: WE low before CAS
    pub smp: u32,    // read strobe
    pub smp2: u32,   // second strobe, after CAS rises (EDO check)
    pub cas_r: u32,
    pub ras_r: u32,
    pub end: u32,    // cycle length (RAS precharge follows ras_r)
    pub fine: u32,   // extra input delay steps (moves the strobe earlier)
}

impl Timing {
    /// Relaxed timing for parts no slower than `max_t_rac` ns: the strobe
    /// sits 50 ns beyond the worst-case access time (buffer delays included),
    /// with generous address setup, RAS-to-CAS delay and precharge.
    pub const fn relaxed(max_t_rac: u32) -> Timing {
        let ras_f = 6;
        let mux = ras_f + 10 + max_t_rac / 10;
        let cas_f = mux + 10;
        let smp = ras_f + max_t_rac + 50;
        let cas_r = smp + 5;
        Timing {
            ras_f, mux, cas_f, we_f: mux, smp, smp2: cas_r + 30, cas_r, ras_r: cas_r,
            end: cas_r + 40 + max_t_rac * 2 / 3, fine: 0,
        }
    }

    /// Same, but RAS stays low for 50 ns after CAS rises so EDO parts keep
    /// driving; the second strobe falls inside that window.
    pub const fn edo_probe(self) -> Timing {
        Timing { ras_r: self.cas_r + 50, end: self.end + 50, ..self }
    }

    pub fn with_strobe(self, smp: u32) -> Timing {
        Timing { smp, ..self }
    }
}

fn ticks(ns: u32, tick_ps: u32) -> u32 {
    ((ns * 1000 + tick_ps / 2) / tick_ps).min(255)
}

/// Program the engine. Returns the tick period in picoseconds.
pub fn apply<P: Platform>(p: &mut P, t: &Timing) -> u32 {
    let tick_ps = tick_ps(p);
    let k = |ns| ticks(ns, tick_ps);
    p.reg_write(regs::TIM0, k(t.ras_r) << 24 | k(t.cas_r) << 16 | k(t.cas_f) << 8 | k(t.mux));
    p.reg_write(regs::TIM1, k(t.smp2) << 24 | k(t.smp) << 16 | k(t.end) << 8 | k(t.ras_f));
    p.reg_write(regs::TIM2, k(t.we_f) << 8 | (t.fine & 0x7F));
    tick_ps
}

/// Program the strobe directly in ticks plus fine steps (used by the sweep).
pub fn apply_strobe_ticks<P: Platform>(p: &mut P, t: &Timing, smp_ticks: u32, fine: u32) {
    let tick_ps = tick_ps(p);
    let k = |ns| ticks(ns, tick_ps);
    p.reg_write(regs::TIM1, k(t.smp2) << 24 | (smp_ticks & 0xFF) << 16 | k(t.end) << 8 | k(t.ras_f));
    p.reg_write(regs::TIM2, k(t.we_f) << 8 | (fine & 0x7F));
}

pub fn tick_ps<P: Platform>(p: &mut P) -> u32 {
    match p.reg_read(regs::TICK_PS) {
        0 => 2500,
        v => v,
    }
}

pub fn ns_to_ticks<P: Platform>(p: &mut P, ns: u32) -> u32 {
    let t = tick_ps(p);
    ticks(ns, t)
}
