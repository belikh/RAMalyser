//! Base-board drivers written against `embedded-hal` 1.0, so they do not
//! depend on the MCU: FPGA register access, the DUT supply (DAC set point,
//! switch, discharge, current sense) and ECP5 configuration.
//!
//! UNTESTED ON HARDWARE: revision A has not been built. Register formats
//! follow the parts' datasheets; values marked "bench" need measuring.
#![no_std]

#[cfg(feature = "host")]
extern crate std;

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::i2c::I2c;
use embedded_hal::spi::{Operation, SpiDevice};
use rt_core::Platform;

pub mod display;
pub mod ecp5;

const DAC_ADDR: u8 = 0x60; // MCP4725A0, A0 low
const INA_ADDR: u8 = 0x40; // INA219, A0 = A1 = GND
const INA_SHUNT: u8 = 0x01;
const INA_BUS: u8 = 0x02;

/// DUT rail set point. With R1 = 100k (rail to FB), R2 = 20k (FB to GND)
/// and R3 = 100k (FB to DAC), Vout = 0.8 * (1 + R1/R2) + (0.8 - Vdac) * R1/R3.
pub const VDUT_AT_DAC_ZERO_MV: i32 = 5600;
pub const DAC_REF_MV: i32 = 3300;
/// Firmware limits. The rail cannot be set above 5.6 V in hardware, and the
/// comparator cuts the switch above that if the regulator fails.
pub const VDUT_MIN_MV: u16 = 2600;
pub const VDUT_MAX_MV: u16 = 5500;
/// Trip current, milliamps.
pub const CURRENT_LIMIT_MA: u16 = 2000;
const SHUNT_MILLIOHM: u32 = 20;

pub fn dac_code_for(mv: u16) -> u16 {
    let mv = mv.clamp(VDUT_MIN_MV, VDUT_MAX_MV) as i32;
    let vdac = (VDUT_AT_DAC_ZERO_MV - mv).clamp(0, DAC_REF_MV);
    (vdac * 4095 / DAC_REF_MV) as u16
}

pub struct Board<SPI, I2C, EN, SW, DIS, OVP, PG, D> {
    pub fpga: SPI,
    pub i2c: I2C,
    /// DUT buck enable.
    pub buck_en: EN,
    /// DUT high-side switch request (the comparator can still veto it).
    pub dut_sw: SW,
    pub discharge: DIS,
    /// Reads the switch-enable node: low while the over-voltage comparator trips.
    pub ovp_n: OVP,
    /// DUT_PG to the FPGA and adapter: bus lines are released while low.
    pub dut_pg: PG,
    pub delay: D,
    target_mv: u16,
}

impl<SPI, I2C, EN, SW, DIS, OVP, PG, D> Board<SPI, I2C, EN, SW, DIS, OVP, PG, D>
where
    SPI: SpiDevice,
    I2C: I2c,
    EN: OutputPin,
    SW: OutputPin,
    DIS: OutputPin,
    OVP: InputPin,
    PG: OutputPin,
    D: DelayNs,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(fpga: SPI, i2c: I2C, buck_en: EN, dut_sw: SW, discharge: DIS, ovp_n: OVP,
               dut_pg: PG, delay: D) -> Self {
        let mut b = Board { fpga, i2c, buck_en, dut_sw, discharge, ovp_n, dut_pg, delay,
                            target_mv: 0 };
        b.rail_off();
        b
    }

    fn rail_off(&mut self) {
        let _ = self.dut_pg.set_low();
        let _ = self.dut_sw.set_low();
        let _ = self.buck_en.set_low();
        let _ = self.discharge.set_high();
        self.delay.delay_ms(50);
        let _ = self.discharge.set_low();
    }

    fn ina(&mut self, reg: u8) -> Option<u16> {
        let mut b = [0u8; 2];
        self.i2c.write_read(INA_ADDR, &[reg], &mut b).ok()?;
        Some(u16::from_be_bytes(b))
    }

    /// Voltage on the supply side of the switch, millivolts.
    pub fn vdut_raw_mv(&mut self) -> Option<u16> {
        self.ina(INA_BUS).map(|r| (r >> 3) * 4)
    }
}

impl<SPI, I2C, EN, SW, DIS, OVP, PG, D> Platform for Board<SPI, I2C, EN, SW, DIS, OVP, PG, D>
where
    SPI: SpiDevice,
    I2C: I2c,
    EN: OutputPin,
    SW: OutputPin,
    DIS: OutputPin,
    OVP: InputPin,
    PG: OutputPin,
    D: DelayNs,
{
    fn reg_read(&mut self, addr: u8) -> u32 {
        let mut data = [0u8; 4];
        let cmd = [addr & 0x7F];
        let ok = self.fpga.transaction(&mut [Operation::Write(&cmd), Operation::Read(&mut data)]);
        if ok.is_err() { 0 } else { u32::from_be_bytes(data) }
    }

    fn reg_write(&mut self, addr: u8, value: u32) {
        let v = value.to_be_bytes();
        let _ = self.fpga.write(&[0x80 | (addr & 0x7F), v[0], v[1], v[2], v[3]]);
    }

    fn delay_us(&mut self, us: u32) {
        self.delay.delay_us(us);
    }

    fn set_vdut_mv(&mut self, mv: u16) {
        self.target_mv = mv.clamp(VDUT_MIN_MV, VDUT_MAX_MV);
        let code = dac_code_for(self.target_mv);
        // MCP4725 fast mode: two bytes, power-down bits clear
        let _ = self.i2c.write(DAC_ADDR, &[(code >> 8) as u8 & 0x0F, code as u8]);
    }

    fn vdut_enable(&mut self, on: bool) -> bool {
        if !on {
            self.rail_off();
            return true;
        }
        let _ = self.discharge.set_low();
        let _ = self.buck_en.set_high();
        self.delay.delay_ms(10); // bench: buck soft-start time
        // regulated before the DUT ever sees it: within 5 % of the set point
        let raw = self.vdut_raw_mv().unwrap_or(0) as i32;
        let want = self.target_mv as i32;
        if (raw - want).abs() * 20 > want {
            self.rail_off();
            return false;
        }
        let _ = self.dut_sw.set_high();
        self.delay.delay_ms(20); // bench: gate RC soft start
        let vetoed = self.ovp_n.is_low().unwrap_or(true);
        if vetoed || self.vdut_current_ma() > CURRENT_LIMIT_MA {
            self.rail_off();
            return false;
        }
        let _ = self.dut_pg.set_high();
        true
    }

    fn vdut_current_ma(&mut self) -> u16 {
        // shunt register LSB is 10 uV
        match self.ina(INA_SHUNT) {
            Some(r) => {
                let uv = (r as i16).max(0) as u32 * 10;
                (uv / SHUNT_MILLIOHM) as u16
            }
            None => u16::MAX, // sensor not answering: treat as a fault
        }
    }
}
