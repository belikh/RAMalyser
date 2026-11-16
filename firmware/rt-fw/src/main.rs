//! RP2350B firmware entry point. NOT COMPILED OR RUN YET: see README.md.
//! Pin numbers follow the base board schematic (sheets "mcu" and "ui").
//!
//! One loop serves four things: the keys and the display (`rt_core::ui`),
//! the USB serial console, and the link to the optional Wi-Fi module on
//! UART0. A test started from any of them shows on the display and on the
//! pass and fail LEDs. Tests block: nothing else is served while one runs.
#![no_std]
#![no_main]

use core::cell::RefCell;
use core::fmt::Write as _;

use embassy_executor::Spawner;
use embassy_futures::select::{select3, Either3};
use embassy_rp::gpio::{Input, Level, Output, Pull};
use embassy_rp::i2c::{self, I2c};
use embassy_rp::peripherals::{UART0, USB};
use embassy_rp::spi::{self, Spi};
use embassy_rp::uart::{self, BufferedInterruptHandler, BufferedUart};
use embassy_rp::usb::{Driver, InterruptHandler};
use embassy_rp::bind_interrupts;
use embassy_time::{Delay, Duration, Timer};
use embassy_usb::class::cdc_acm::{CdcAcmClass, State};
use embedded_hal_bus::spi::RefCellDevice;
use embedded_io_async::{Read as _, Write as _};
use heapless::String;
use panic_halt as _;
use rt_core::ui::{Action, Info, Ui};
use rt_core::{console, device, regs, timing, Platform};
use rt_hal::display::{Keys, Signals, St7789};
use rt_hal::{ecp5, Board};
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
    UART0_IRQ => BufferedInterruptHandler<UART0>;
});

/// Built by gateware/build.sh.
static BITSTREAM: &[u8] = include_bytes!("../../../gateware/build/top.bit");

/// Adapter buffer round trip until per-adapter calibration exists (bench).
const DEFAULT_CAL_PS: u32 = 10_000;
/// Link to the Wi-Fi module (see wifi/README.md).
const LINK_BAUD: u32 = 115_200;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // FPGA configuration port: SPI1 on GP10 (CCLK), GP11 (MOSI), GP12 (MISO), GP13 (SN)
    let mut cfg_spi_cfg = spi::Config::default();
    cfg_spi_cfg.frequency = 8_000_000;
    let cfg_bus = Spi::new_blocking(p.SPI1, p.PIN_10, p.PIN_11, p.PIN_12, cfg_spi_cfg);
    let cfg_cs = Output::new(p.PIN_13, Level::High);
    let mut cfg_dev = embedded_hal_bus::spi::ExclusiveDevice::new(cfg_bus, cfg_cs, Delay).unwrap();
    let mut programn = Output::new(p.PIN_14, Level::High);
    let mut initn = Input::new(p.PIN_15, Pull::Up);
    let mut done = Input::new(p.PIN_9, Pull::Up);
    let configured =
        ecp5::configure(&mut cfg_dev, &mut programn, &mut initn, &mut done, &mut Delay, BITSTREAM);

    // SPI0 on GP18 (SCK), GP19 (MOSI), GP16 (MISO) is shared by the FPGA
    // registers (CS GP17) and the display (CS GP23).
    let mut bus_cfg = spi::Config::default();
    bus_cfg.frequency = 4_000_000;
    static BUS: StaticCell<RefCell<Spi<'static, embassy_rp::peripherals::SPI0, spi::Blocking>>> =
        StaticCell::new();
    let bus = BUS.init(RefCell::new(Spi::new_blocking(p.SPI0, p.PIN_18, p.PIN_19, p.PIN_16,
                                                      bus_cfg)));
    let reg_dev = RefCellDevice::new(bus, Output::new(p.PIN_17, Level::High), Delay).unwrap();
    let lcd_dev = RefCellDevice::new(bus, Output::new(p.PIN_23, Level::High), Delay).unwrap();

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

    // Display: DC GP24, reset GP25, backlight GP26
    let mut lcd = St7789::new(lcd_dev, Output::new(p.PIN_24, Level::High),
                              Output::new(p.PIN_25, Level::High));
    lcd.init(&mut Delay);
    let _backlight = Output::new(p.PIN_26, Level::High);

    // Keys KEY0..KEY5 on GP27..GP32 (ESC, F1..F5), switches to ground
    let mut keys = Keys::new([
        Input::new(p.PIN_27, Pull::Up),
        Input::new(p.PIN_28, Pull::Up),
        Input::new(p.PIN_29, Pull::Up),
        Input::new(p.PIN_30, Pull::Up),
        Input::new(p.PIN_31, Pull::Up),
        Input::new(p.PIN_32, Pull::Up),
    ]);
    let mut signals = Signals {
        buzzer: Output::new(p.PIN_33, Level::Low),
        pass: Output::new(p.PIN_34, Level::Low),
        fail: Output::new(p.PIN_35, Level::Low),
    };

    let engine_id = board.reg_read(regs::ID);
    let mut ui = Ui::new(Info {
        version: env!("CARGO_PKG_VERSION"),
        engine_id,
        engine_ok: configured.is_ok() && engine_id == regs::ID_VALUE,
        tick_ps: timing::tick_ps(&mut board),
    });
    ui.draw(&mut lcd);

    // Wi-Fi module link: UART0, TX GP0 (WIFI_RX), RX GP1 (WIFI_TX)
    static LINK_TX: StaticCell<[u8; 2048]> = StaticCell::new();
    static LINK_RX: StaticCell<[u8; 128]> = StaticCell::new();
    let mut link_cfg = uart::Config::default();
    link_cfg.baudrate = LINK_BAUD;
    let mut link = BufferedUart::new(p.UART0, p.PIN_0, p.PIN_1, Irqs, LINK_TX.init([0; 2048]),
                                     LINK_RX.init([0; 128]), link_cfg);

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

    let serve = async {
        let mut usb_line: String<96> = String::new();
        let mut link_line: String<96> = String::new();
        let mut out: String<2048> = String::new();
        let mut usb_buf = [0u8; 64];
        let mut link_buf = [0u8; 32];
        let mut greeted = false;
        loop {
            match select3(Timer::after(Duration::from_millis(10)),
                          class.read_packet(&mut usb_buf), link.read(&mut link_buf)).await {
                // ---- every 10 ms: keys, and the greeting when a terminal opens
                Either3::First(()) => {
                    if class.dtr() && !greeted {
                        let _ = writeln!(out, "RAMalyser. FPGA: {:?}, engine id {:#010x}",
                                         configured, engine_id);
                        send_usb(&mut class, &mut out).await;
                    }
                    greeted = class.dtr();
                    if let Some(key) = keys.poll() {
                        match ui.key(key) {
                            Action::Run { profile, full } => {
                                signals.show(None);
                                ui.draw(&mut lcd);
                                let pr = device::ALL[profile];
                                let report = if full {
                                    rt_core::tests::extensive_test(&mut board, pr, DEFAULT_CAL_PS)
                                } else {
                                    rt_core::tests::basic_test(&mut board, pr, DEFAULT_CAL_PS)
                                };
                                let pass = report.passed();
                                ui.finish(report);
                                signals.beep(&mut Delay, pass);
                            }
                            Action::None => {}
                        }
                        signals.show(ui.verdict());
                        ui.draw(&mut lcd);
                    }
                }
                // ---- USB console
                Either3::Second(Ok(n)) => {
                    for &b in &usb_buf[..n] {
                        if let Some(line) = feed(&mut usb_line, b) {
                            command(&mut board, &mut ui, &mut lcd, &mut signals, line.as_str(),
                                    &mut out);
                            send_usb(&mut class, &mut out).await;
                        }
                    }
                }
                Either3::Second(Err(_)) => Timer::after(Duration::from_millis(50)).await,
                // ---- Wi-Fi module
                Either3::Third(Ok(n)) => {
                    for &b in &link_buf[..n] {
                        if let Some(line) = feed(&mut link_line, b) {
                            command(&mut board, &mut ui, &mut lcd, &mut signals, line.as_str(),
                                    &mut out);
                            let _ = link.write_all(out.as_bytes()).await;
                            out.clear();
                        }
                    }
                }
                Either3::Third(Err(_)) => {}
            }
        }
    };
    embassy_futures::join::join(usb.run(), serve).await;
}

/// Collect bytes into a line; returns the line when it is complete.
fn feed(line: &mut String<96>, b: u8) -> Option<String<96>> {
    if b == b'\r' || b == b'\n' {
        if line.is_empty() {
            return None;
        }
        let done = line.clone();
        line.clear();
        return Some(done);
    }
    if line.push(b as char).is_err() {
        line.clear();
    }
    None
}

/// Run one console line. A test also goes to the display and the LEDs.
fn command<P, C, PA, FA, BZ>(board: &mut P, ui: &mut Ui, lcd: &mut C,
                             signals: &mut Signals<PA, FA, BZ>, line: &str, out: &mut String<2048>)
where
    P: Platform,
    C: rt_core::ui::Canvas,
    PA: embedded_hal::digital::OutputPin,
    FA: embedded_hal::digital::OutputPin,
    BZ: embedded_hal::digital::OutputPin,
{
    if let Some((profile, full)) = console::test_request(line) {
        ui.begin(profile, full);
        signals.show(None);
        ui.draw(lcd);
        if let Some(report) = console::run_test(board, line, DEFAULT_CAL_PS, out) {
            let pass = report.passed();
            ui.finish(report);
            signals.beep(&mut Delay, pass);
        }
        signals.show(ui.verdict());
        ui.draw(lcd);
    } else {
        console::execute(board, line, DEFAULT_CAL_PS, out);
    }
}

async fn send_usb<'d, D: embassy_usb::driver::Driver<'d>>(class: &mut CdcAcmClass<'d, D>,
                                                          out: &mut String<2048>) {
    for chunk in out.as_bytes().chunks(63) {
        if class.write_packet(chunk).await.is_err() {
            break;
        }
    }
    out.clear();
}
