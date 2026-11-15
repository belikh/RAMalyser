//! What can be plugged in: socket profiles and the geometry they allow.

/// How one socket presents a DUT to the engine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    pub name: &'static str,
    /// DQ lines the tester drives during writes.
    pub drive: u64,
    /// DQ lines compared on reads.
    pub mask: u64,
    /// Data-in on DQ0, data-out on DQ1 (1-bit DRAM chips).
    pub split0: bool,
    /// Parity-in on DQ32, parity-out on DQ33 (30-pin SIMM).
    pub split1: bool,
    pub ras: u8,
    pub cas: u8,
    pub we: u8,
    pub oe: u8,
    /// Most row / column address bits the socket can carry.
    pub max_row_bits: u8,
    pub max_col_bits: u8,
    /// Nominal supply in millivolts.
    pub vdut_mv: u16,
    /// Needs the -5 V / +12 V auxiliary rails (4116 class).
    pub aux_rails: bool,
    /// Refresh interval for the whole array, milliseconds.
    pub refresh_ms: u16,
    /// Slowest access time this socket has to cope with, nanoseconds.
    pub max_t_rac_ns: u16,
}

const fn bits(n: u32) -> u64 {
    (1u64 << n) - 1
}

/// 72-pin SIMM, bank 0 (RAS0/RAS2 pair), 32 data bits plus 4 parity.
pub const SIMM72: Profile = Profile {
    name: "72-pin SIMM", drive: bits(36), mask: bits(36), split0: false, split1: false,
    ras: 0b0101, cas: 0b1111, we: 0b01, oe: 0, max_row_bits: 12, max_col_bits: 12,
    vdut_mv: 5000, aux_rails: false, refresh_ms: 16, max_t_rac_ns: 120,
};
/// Second bank of a two-bank 72-pin SIMM (RAS1/RAS3).
pub const SIMM72_BANK1: Profile = Profile { name: "72-pin SIMM bank 1", ras: 0b1010, ..SIMM72 };

/// 30-pin SIMM / SIPP: 8 data bits, optional parity with separate in/out.
pub const SIMM30: Profile = Profile {
    name: "30-pin SIMM", drive: 0xFF | 1 << 32, mask: 0xFF | 1 << 33, split0: false,
    split1: true, ras: 0b0001, cas: 0b0011, we: 0b01, oe: 0, max_row_bits: 12,
    max_col_bits: 12, vdut_mv: 5000, aux_rails: false, refresh_ms: 8, max_t_rac_ns: 150,
};

const X1: Profile = Profile {
    name: "", drive: 0b01, mask: 0b10, split0: true, split1: false, ras: 1, cas: 1, we: 1,
    oe: 0, max_row_bits: 8, max_col_bits: 8, vdut_mv: 5000, aux_rails: false, refresh_ms: 2,
    max_t_rac_ns: 200,
};
const X4: Profile = Profile {
    name: "", drive: 0xF, mask: 0xF, split0: false, split1: false, ras: 1, cas: 1, we: 1,
    oe: 1, max_row_bits: 9, max_col_bits: 9, vdut_mv: 5000, aux_rails: false, refresh_ms: 4,
    max_t_rac_ns: 150,
};

pub const DIP_4116: Profile = Profile {
    name: "4116 (16K x1)", max_row_bits: 7, max_col_bits: 7, aux_rails: true,
    max_t_rac_ns: 300, ..X1
};
pub const DIP_4164_41256: Profile = Profile {
    name: "4164 / 41256", max_row_bits: 9, max_col_bits: 9, refresh_ms: 4, ..X1
};
pub const DIP_4416_4464: Profile = Profile {
    name: "4416 / 4464", max_row_bits: 8, max_col_bits: 8, ..X4
};
pub const DIP_411000: Profile = Profile {
    name: "411000 (1M x1)", max_row_bits: 10, max_col_bits: 10, refresh_ms: 8, ..X1
};
pub const DIP_44256_514400: Profile = Profile {
    name: "44256 / 514400", max_row_bits: 10, max_col_bits: 10, refresh_ms: 8, ..X4
};

pub const ALL: [&Profile; 7] = [
    &SIMM72, &SIMM30, &DIP_4116, &DIP_4164_41256, &DIP_4416_4464, &DIP_411000,
    &DIP_44256_514400,
];

/// Geometry found by probing.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Geometry {
    pub row_bits: u8,
    pub col_bits: u8,
    /// DQ lines that respond.
    pub width_mask: u64,
}

impl Geometry {
    pub fn words(&self) -> u32 {
        1u32 << (self.row_bits + self.col_bits)
    }

    pub fn data_bits(&self) -> u32 {
        self.width_mask.count_ones()
    }

    /// Capacity in bits.
    pub fn bits(&self) -> u64 {
        self.words() as u64 * self.data_bits() as u64
    }
}

/// 72-pin SIMM presence-detect decode (IBM table). `pd` bit n = PD(n+1),
/// 1 = open, 0 = grounded. Returns (megabytes, access time in ns) as a hint
/// only: many modules carry wrong or vendor-specific codes.
pub fn simm72_presence(pd: u8) -> Option<(u16, u16)> {
    let size = match pd & 0b0011 {
        0b00 => 4,      // PD1 G, PD2 G: 1M words
        0b01 => 2,      // PD1 O, PD2 G: 512K
        0b10 => 1,      // PD1 G, PD2 O: 256K
        _ => 8,         // both open: 2M
    };
    let speed = match (pd >> 2) & 0b11 {
        0b00 => 100,    // PD3 G, PD4 G
        0b01 => 80,     // PD3 O, PD4 G
        0b10 => 70,     // PD3 G, PD4 O
        _ => 60,        // both open
    };
    if pd & 0b1111 == 0b1111 {
        return None;    // all open: no code (or 8 MB / 60 ns, ambiguous)
    }
    Some((size, speed))
}
