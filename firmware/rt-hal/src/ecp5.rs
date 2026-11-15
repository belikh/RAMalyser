//! Load a bitstream into the ECP5 through its slave-SPI configuration port
//! (CFG[2:0] = 001). Command set from Lattice's sysCONFIG documentation.
//! UNTESTED ON HARDWARE.

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::{Operation, SpiDevice};

const ISC_ENABLE: [u8; 4] = [0xC6, 0, 0, 0];
const ISC_DISABLE: [u8; 4] = [0x26, 0, 0, 0];
const LSC_BITSTREAM_BURST: [u8; 4] = [0x7A, 0, 0, 0];
const LSC_READ_STATUS: [u8; 4] = [0x3C, 0, 0, 0];
const STATUS_DONE: u32 = 1 << 8;
const STATUS_FAIL: u32 = 1 << 13;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigError {
    /// INITN stayed low after PROGRAMN was released.
    InitTimeout,
    Spi,
    /// The device reported a configuration failure or DONE never rose.
    NotDone(u32),
}

/// Configure the FPGA from a complete bitstream held in memory (the
/// firmware embeds it). The burst command and the whole bitstream go out in
/// one chip-select frame, as the device requires.
pub fn configure<SPI, PRG, INIT, DONE, D>(spi: &mut SPI, programn: &mut PRG, initn: &mut INIT,
                                          done: &mut DONE, delay: &mut D, bitstream: &[u8])
                                          -> Result<(), ConfigError>
where
    SPI: SpiDevice,
    PRG: OutputPin,
    INIT: InputPin,
    DONE: InputPin,
    D: DelayNs,
{
    let _ = programn.set_low();
    delay.delay_ms(1);
    let _ = programn.set_high();
    let mut waited = 0;
    while initn.is_low().unwrap_or(true) {
        delay.delay_ms(1);
        waited += 1;
        if waited > 100 {
            return Err(ConfigError::InitTimeout);
        }
    }
    delay.delay_ms(1);
    spi.write(&ISC_ENABLE).map_err(|_| ConfigError::Spi)?;
    delay.delay_ms(1);
    spi.transaction(&mut [Operation::Write(&LSC_BITSTREAM_BURST), Operation::Write(bitstream)])
        .map_err(|_| ConfigError::Spi)?;
    spi.write(&ISC_DISABLE).map_err(|_| ConfigError::Spi)?;
    delay.delay_ms(10);
    let mut st = [0u8; 4];
    spi.transaction(&mut [Operation::Write(&LSC_READ_STATUS), Operation::Read(&mut st)])
        .map_err(|_| ConfigError::Spi)?;
    let status = u32::from_be_bytes(st);
    if status & STATUS_FAIL != 0 || status & STATUS_DONE == 0 || done.is_low().unwrap_or(true) {
        return Err(ConfigError::NotDone(status));
    }
    Ok(())
}
