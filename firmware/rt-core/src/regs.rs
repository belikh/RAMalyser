//! Register map of the FPGA timing engine (see gateware/rtl/tester_core.v).

pub const ID: u8 = 0x00;
pub const CTRL: u8 = 0x01;
pub const STATUS: u8 = 0x02;
pub const GEOM: u8 = 0x03;
pub const TIM0: u8 = 0x04;
pub const TIM1: u8 = 0x05;
pub const TIM2: u8 = 0x06;
pub const RFSH_PERIOD: u8 = 0x07;
pub const ADDR_START: u8 = 0x08;
pub const ADDR_END: u8 = 0x09;
pub const MARCH: u8 = 0x0A;
pub const PAT_LO: u8 = 0x0B;
pub const PAT_HI: u8 = 0x0C;
pub const MASK_LO: u8 = 0x0D;
pub const MASK_HI: u8 = 0x0E;
pub const DRV_LO: u8 = 0x0F;
pub const DRV_HI: u8 = 0x10;
pub const SEL: u8 = 0x11;
pub const ERR_COUNT: u8 = 0x12;
pub const FAIL_ADDR: u8 = 0x13;
pub const FAIL_BITS_LO: u8 = 0x14;
pub const FAIL_BITS_HI: u8 = 0x15;
pub const FAIL_DATA_LO: u8 = 0x16;
pub const FAIL_DATA_HI: u8 = 0x17;
pub const SMP2_LO: u8 = 0x18;
pub const SMP2_HI: u8 = 0x19;
pub const LAST_LO: u8 = 0x1A;
pub const LAST_HI: u8 = 0x1B;
pub const TICK_PS: u8 = 0x1C;
pub const GPIO_OUT: u8 = 0x20; // four words, IO0..IO115
pub const GPIO_OE: u8 = 0x24;
pub const GPIO_IN: u8 = 0x28;

pub const ID_VALUE: u32 = 0x5241_4D31; // "RAM1"

pub const CTRL_START: u32 = 1 << 0;
pub const CTRL_ABORT: u32 = 1 << 1;
pub const CTRL_RFSH_EN: u32 = 1 << 2;
pub const CTRL_CBR: u32 = 1 << 3;
pub const CTRL_ENGINE: u32 = 1 << 4;
pub const CTRL_ROW_FAST: u32 = 1 << 5;

pub const STATUS_BUSY: u32 = 1 << 0;
pub const STATUS_ERR: u32 = 1 << 1;
pub const STATUS_DONE: u32 = 1 << 2;
pub const STATUS_DUT_PG: u32 = 1 << 3;

/// Fine input-delay step of the FPGA I/O cell, picoseconds (nominal).
pub const FINE_STEP_PS: u32 = 25;
pub const FINE_MAX: u32 = 127;
