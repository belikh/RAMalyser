//! Hardware-independent test logic for the RAMalyser memory tester.
//!
//! Everything here talks to the outside world through the [`Platform`]
//! trait: register access to the FPGA timing engine, the DUT supply and a
//! delay. The RP2350 firmware implements it over SPI/I2C; the `sim` module
//! implements it in software so the algorithms run under `cargo test`.
#![cfg_attr(not(any(test, feature = "sim")), no_std)]

pub mod console;
pub mod device;
pub mod ramspec;
pub mod regs;
pub mod report;
#[cfg(any(test, feature = "sim"))]
pub mod sim;
pub mod tests;
pub mod timing;
pub mod ui;

/// What the test logic needs from the hardware.
pub trait Platform {
    fn reg_read(&mut self, addr: u8) -> u32;
    fn reg_write(&mut self, addr: u8, value: u32);
    fn delay_us(&mut self, us: u32);
    /// Set the DUT supply in millivolts (does not switch it on).
    fn set_vdut_mv(&mut self, mv: u16);
    /// Switch the DUT supply on or off; returns false if the rail did not
    /// come up in range (short, over-voltage cut-off, adapter missing).
    fn vdut_enable(&mut self, on: bool) -> bool;
    /// DUT supply current in milliamps.
    fn vdut_current_ma(&mut self) -> u16;
}

/// 40-bit data word helpers (the engine handles DQ0..DQ39).
pub fn write40<P: Platform>(p: &mut P, lo: u8, v: u64) {
    p.reg_write(lo, v as u32);
    p.reg_write(lo + 1, (v >> 32) as u32 & 0xFF);
}

pub fn read40<P: Platform>(p: &mut P, lo: u8) -> u64 {
    p.reg_read(lo) as u64 | ((p.reg_read(lo + 1) as u64 & 0xFF) << 32)
}
