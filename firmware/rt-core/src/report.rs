//! Test results in a form both the display and the PC link can print.

use core::fmt;

use crate::device::Geometry;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Fpm,
    Edo,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WireFault {
    /// Control line reads back low/high whatever is driven.
    StuckLow(u8),
    StuckHigh(u8),
    /// Two control lines follow each other.
    Short(u8, u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Power,
    Wiring,
    Detect,
    Pattern,
    March,
    Checkerboard,
    AddressInData,
    Retention,
    VoltageLow,
    VoltageHigh,
    Bounce,
}

/// First failure of a memory test.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Failure {
    pub phase: Phase,
    pub errors: u32,
    pub addr: u32,
    /// Every DQ line that differed at least once.
    pub bits: u64,
    pub read: u64,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Report {
    pub profile: &'static str,
    pub geometry: Geometry,
    pub mode: Option<Mode>,
    /// Measured RAS access time, nanoseconds.
    pub t_rac_ns: Option<u16>,
    /// Longest refresh pause that still read back correctly, milliseconds.
    pub retention_ms: Option<u32>,
    pub current_ma: u16,
    pub wire_faults: [Option<WireFault>; 8],
    pub failure: Option<Failure>,
    pub no_device: bool,
    pub power_fault: bool,
}

impl Report {
    pub fn passed(&self) -> bool {
        !self.no_device && !self.power_fault && self.failure.is_none()
            && self.wire_faults.iter().all(|w| w.is_none())
    }
}

/// Control-line names in sense order (matches spec SENSE_GROUPS 0..2).
pub const CTL_NAMES: [&str; 24] = [
    "MA0", "MA1", "MA2", "MA3", "MA4", "MA5", "MA6", "MA7", "MA8", "MA9", "MA10", "MA11",
    "WE0", "WE1", "OE0", "OE1", "RAS0", "RAS1", "RAS2", "RAS3", "CAS0", "CAS1", "CAS2",
    "CAS3",
];

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.profile)?;
        if self.power_fault {
            return writeln!(f, "FAIL: supply fault ({} mA)", self.current_ma);
        }
        for w in self.wire_faults.iter().flatten() {
            match *w {
                WireFault::StuckLow(i) => writeln!(f, "FAIL: {} stuck low", CTL_NAMES[i as usize])?,
                WireFault::StuckHigh(i) => writeln!(f, "FAIL: {} stuck high", CTL_NAMES[i as usize])?,
                WireFault::Short(a, b) => writeln!(
                    f, "FAIL: {} shorted to {}", CTL_NAMES[a as usize], CTL_NAMES[b as usize])?,
            }
        }
        if self.no_device {
            return writeln!(f, "No device found");
        }
        let g = &self.geometry;
        let kbits = g.bits() / 1024;
        if kbits >= 8192 {
            write!(f, "{} MB", kbits / 8192)?;
        } else {
            write!(f, "{} Kbit", kbits)?;
        }
        writeln!(f, "  {}K x{}  ({}x{} addr)", g.words() / 1024, g.data_bits(),
                 g.row_bits, g.col_bits)?;
        if let Some(m) = self.mode {
            write!(f, "{}", if m == Mode::Edo { "EDO" } else { "FPM" })?;
        }
        if let Some(t) = self.t_rac_ns {
            write!(f, "  {} ns", t)?;
        }
        if let Some(r) = self.retention_ms {
            write!(f, "  retention >= {} ms", r)?;
        }
        writeln!(f, "  {} mA", self.current_ma)?;
        match &self.failure {
            None => writeln!(f, "PASS"),
            Some(x) => writeln!(
                f, "FAIL in {:?}: {} errors, first at {:#x}, bad bits {:#012x}, read {:#012x}",
                x.phase, x.errors, x.addr, x.bits, x.read),
        }
    }
}
