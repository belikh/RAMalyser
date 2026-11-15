//! RP2350B firmware entry point. NOT COMPILED OR RUN YET: see README.md.
//! Pin numbers follow the base board schematic (sheet "mcu").
#![no_std]
#![no_main]

use core::fmt::Write as _;

use embassy_executor::Spawner;
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::i2c::{self, I2c};
use embassy_rp::spi::{self, Spi};
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::{bind_interrupts, peripherals::USB};
use embassy_time::Delay;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embedded_hal_bus::spi::ExclusiveDevice;
use heapless::String;
use panic_halt as _;
use rt_core::{console, regs, Platform};
use rt_hal::{ecp5, Board};
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
});

/// Built by gateware/build.sh.
static BITSTREAM: &[u8] = include_bytes!("../../../gateware/build/top.bit");

/// Adapter buffer round trip until per-adapter calibration exists (bench).
const DEFAULT_CAL_PS: u32 = 10_000;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // FPGA configuration port: SPI1 on GP10 (CCLK), GP11 (MOSI), GP12 (MISO), GP13 (SN)
    let mut cfg_spi_cfg = spi::Config::default();
    cfg_spi_cfg.frequency = 8_000_000;
    let cfg_bus = Spi::new_blocking(p.SPI1, p.PIN_10, p.PIN_11, p.PIN_12, cfg_spi_cfg);
    let mut cfg_dev = ExclusiveDevice::new(cfg_bus, Output::new(p.PIN_13, Level::High), Delay)
        .unwrap();
    let mut programn = Output::new(p.PIN_14, Level::High);
    let mut initn = Input::new(p.PIN_15, Pull::Up);
    let mut done = Input::new(p.PIN_9, Pull::Up);
    let configured =
        ecp5::configure(&mut cfg_dev, &mut programn, &mut initn, &mut done, &mut Delay, BITSTREAM);

    // FPGA registers: SPI0 on GP18 (SCK), GP19 (MOSI), GP16 (MISO), GP17 (CS)
    let mut reg_cfg = spi::Config::default();
    reg_cfg.frequency = 4_000_000;
    let reg_bus = Spi::new_blocking(p.SPI0, p.PIN_18, p.PIN_19, p.PIN_16, reg_cfg);
    let reg_dev = ExclusiveDevice::new(reg_bus, Output::new(p.PIN_17, Level::High), Delay).unwrap();

    // I2C0 on GP4 (SDA), GP5 (SCL): DAC, INA219, adapter ID EEPROM
    let i2c = I2c::new_blocking(p.I2C0, p.PIN_5, p.PIN_4, i2c::Config::default());

    let mut board = Board::new(
        reg_dev,
        i2c,
        Output::new(p.PIN_36, Level::Low), // DUT buck enable
        Output::new(p.PIN_37, Level::Low), // DUT switch request
        Output::new(p.PIN_38, Level::Low), // discharge
        Input::new(p.PIN_39, Pull::None),  // switch-enable node (low = over-voltage veto)
        Output::new(p.PIN_22, Level::Low), // DUT_PG
        Delay,
    );

    // USB serial console
    let driver = Driver::new(p.USB, Irqs);
    let mut usb_cfg = embassy_usb::Config::new(0x2E8A, 0x000A); // placeholder VID/PID: get your own
    usb_cfg.manufacturer = Some("Jupiter");
    usb_cfg.product = Some("RAMalyser console");
    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    static STATE: StaticCell<State> = StaticCell::new();
    let mut builder = embassy_usb::Builder::new(
        driver,
        usb_cfg,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        &mut [],
        CONTROL_BUF.init([0; 64]),
    );
    let mut class = CdcAcmClass::new(&mut builder, STATE.init(State::new()), 64);
    let mut usb = builder.build();

    let console = async {
        let mut line: String<96> = String::new();
        let mut out: String<1024> = String::new();
        let mut buf = [0u8; 64];
        loop {
            class.wait_connection().await;
            let _ = writeln!(out, "RAMalyser. FPGA: {:?}, engine id {:#010x}", configured,
                             board.reg_read(regs::ID));
            loop {
                for chunk in out.as_bytes().chunks(64) {
                    if class.write_packet(chunk).await.is_err() {
                        break;
                    }
                }
                out.clear();
                let n = match class.read_packet(&mut buf).await {
                    Ok(n) => n,
                    Err(_) => break,
                };
                for &b in &buf[..n] {
                    if b == b'\r' || b == b'\n' {
                        if !line.is_empty() {
                            // tests block here; USB is not serviced meanwhile
                            console::execute(&mut board, line.as_str(), DEFAULT_CAL_PS, &mut out);
                            line.clear();
                        }
                    } else if line.push(b as char).is_err() {
                        line.clear();
                    }
                }
            }
        }
    };
    embassy_futures::join::join(usb.run(), console).await;
}
