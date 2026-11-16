//! Draw every screen of the front-panel interface to PPM files, with the
//! same fonts and colours the display uses.
//!
//!     cargo run -p rt-hal --features host --example lcd_preview -- out-dir
use rt_core::device;
use rt_core::sim::{Fault, SimDut, SimPlatform};
use rt_core::tests::{basic_test, extensive_test};
use rt_core::ui::{Action, Info, Key, Ui};
use rt_hal::display::MemCanvas;

fn simm() -> SimDut {
    SimDut::new(10, 10, (1u64 << 36) - 1, 70.0)
}

fn save(dir: &str, name: &str, ui: &Ui) {
    let mut c = MemCanvas::new();
    ui.draw(&mut c);
    std::fs::write(format!("{dir}/{name}.ppm"), c.ppm()).unwrap();
    println!("{name}");
}

fn test(ui: &mut Ui, dut: Option<SimDut>, key: Key) {
    if let Action::Run { profile, full } = ui.key(key) {
        let mut p = SimPlatform::new(dut);
        let pr = device::ALL[profile];
        ui.finish(if full { extensive_test(&mut p, pr, 9000) } else { basic_test(&mut p, pr, 9000) });
    }
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    std::fs::create_dir_all(&dir).unwrap();
    let info = Info { version: env!("CARGO_PKG_VERSION"), engine_id: 0x52414d31, engine_ok: true,
                      tick_ps: 3077 };
    let mut ui = Ui::new(info);
    ui.key(Key::F2);
    ui.key(Key::F2);
    save(&dir, "1-select", &ui);
    let mut ui = Ui::new(info);
    ui.key(Key::F3);
    save(&dir, "2-testing", &ui);
    let mut ui = Ui::new(info);
    test(&mut ui, Some(simm()), Key::F4);
    save(&dir, "3-pass", &ui);
    let mut ui = Ui::new(info);
    let mut bad = simm();
    bad.faults.push(Fault::StuckBit { addr: 0x1234, bit: 5, value: false });
    bad.faults.push(Fault::StuckBit { addr: 0x1234, bit: 17, value: true });
    test(&mut ui, Some(bad), Key::F3);
    save(&dir, "4-fail", &ui);
    ui.key(Key::F2);
    save(&dir, "5-details", &ui);
    let mut ui = Ui::new(info);
    test(&mut ui, None, Key::F3);
    save(&dir, "6-no-device", &ui);
    let mut ui = Ui::new(info);
    ui.key(Key::F5);
    save(&dir, "7-about", &ui);
}
