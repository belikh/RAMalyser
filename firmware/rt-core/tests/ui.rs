//! The front-panel interface against the software engine + DRAM model.
use rt_core::device;
use rt_core::report::Verdict;
use rt_core::sim::{Fault, SimDut, SimPlatform};
use rt_core::tests::{basic_test, extensive_test};
use rt_core::ui::{Action, Canvas, Font, Info, Ink, Key, Screen, Ui, H, W};

/// Records what was drawn and checks that nothing leaves the screen.
#[derive(Default)]
struct Paper {
    texts: Vec<(u16, u16, String, Ink)>,
}

impl Canvas for Paper {
    fn fill(&mut self, x: u16, y: u16, w: u16, h: u16, _ink: Ink) {
        assert!(x + w <= W && y + h <= H, "fill off screen: {x},{y} {w}x{h}");
    }
    fn text(&mut self, x: u16, y: u16, s: &str, font: Font, fg: Ink, _bg: Ink) {
        let (cw, ch) = self.cell(font);
        assert!(x + cw * s.len() as u16 <= W, "text off the right edge: {s:?} at {x}");
        assert!(y + ch <= H, "text off the bottom: {s:?} at {y}");
        self.texts.push((x, y, s.to_string(), fg));
    }
    fn cell(&self, font: Font) -> (u16, u16) {
        match font {
            Font::Small => (8, 17),
            Font::Body => (12, 24),
            Font::Big => (24, 48),
        }
    }
}

fn info() -> Info {
    Info { version: "0.1.0", engine_id: 0x52414d31, engine_ok: true, tick_ps: 3077 }
}

fn drawn(ui: &Ui) -> String {
    let mut p = Paper::default();
    ui.draw(&mut p);
    p.texts.iter().map(|t| t.2.as_str()).collect::<Vec<_>>().join("|")
}

fn simm(t: f64) -> SimDut {
    SimDut::new(10, 10, (1u64 << 36) - 1, t)
}

fn run(ui: &mut Ui, dut: Option<SimDut>, key: Key) {
    match ui.key(key) {
        Action::Run { profile, full } => {
            assert_eq!(ui.screen(), Screen::Running);
            assert!(drawn(ui).contains("Testing"));
            let mut p = SimPlatform::new(dut);
            let pr = device::ALL[profile];
            let r = if full { extensive_test(&mut p, pr, 9000) } else { basic_test(&mut p, pr, 9000) };
            ui.finish(r);
        }
        Action::None => panic!("the key did not start a test"),
    }
}

#[test]
fn every_profile_is_listed_and_selectable() {
    let mut ui = Ui::new(info());
    let text = drawn(&ui);
    for pr in device::ALL.iter() {
        assert!(text.contains(pr.name), "{} missing from {text}", pr.name);
    }
    for want in 1..device::ALL.len() {
        assert_eq!(ui.key(Key::F2), Action::None);
        assert_eq!(ui.selected(), want);
    }
    ui.key(Key::F2);
    assert_eq!(ui.selected(), 0, "the list wraps");
    ui.key(Key::F1);
    assert_eq!(ui.selected(), device::ALL.len() - 1);
}

#[test]
fn quick_test_of_a_good_module_shows_pass() {
    let mut ui = Ui::new(info());
    run(&mut ui, Some(simm(70.0)), Key::F3);
    assert_eq!(ui.verdict(), Some(Verdict::Pass));
    let text = drawn(&ui);
    assert!(text.contains("PASS") && text.contains("4 MB  1024K x36") && text.contains("70 ns"),
            "{text}");
    assert!(!text.contains("F2 Details"), "no details on a pass: {text}");
    assert_eq!(ui.key(Key::F2), Action::None);
    assert_eq!(ui.screen(), Screen::Result);
    ui.key(Key::Esc);
    assert_eq!(ui.screen(), Screen::Select);
    assert_eq!(ui.verdict(), None);
}

#[test]
fn full_test_reports_retention() {
    let mut ui = Ui::new(info());
    run(&mut ui, Some(simm(70.0)), Key::F4);
    let text = drawn(&ui);
    assert!(text.contains("PASS") && text.contains("Holds 512 ms"), "{text}");
}

#[test]
fn empty_socket_says_no_device() {
    let mut ui = Ui::new(info());
    run(&mut ui, None, Key::F3);
    assert_eq!(ui.verdict(), Some(Verdict::NoDevice));
    let text = drawn(&ui);
    assert!(text.contains("NO DEVICE") && text.contains("Nothing answers"), "{text}");
}

#[test]
fn bad_cell_fails_and_details_name_the_line() {
    let mut dut = simm(70.0);
    dut.faults.push(Fault::StuckBit { addr: 0x1234, bit: 5, value: false });
    let mut ui = Ui::new(info());
    run(&mut ui, Some(dut), Key::F3);
    assert_eq!(ui.verdict(), Some(Verdict::Fail));
    let text = drawn(&ui);
    assert!(text.contains("FAIL") && text.contains("Pattern: 1 errors"), "{text}");
    ui.key(Key::F2);
    assert_eq!(ui.screen(), Screen::Detail);
    let text = drawn(&ui);
    assert!(text.contains("First at  0x1234") && text.contains("DQ5"), "{text}");
    ui.key(Key::Esc);
    assert_eq!(ui.screen(), Screen::Result);
}

#[test]
fn wiring_fault_is_named() {
    let mut dut = simm(70.0);
    dut.faults.push(Fault::CtlShort(20, 21));
    let mut ui = Ui::new(info());
    run(&mut ui, Some(dut), Key::F3);
    let text = drawn(&ui);
    assert!(text.contains("FAIL") && text.contains("CAS0 shorted to CAS1"), "{text}");
}

#[test]
fn again_repeats_the_same_test() {
    let mut ui = Ui::new(info());
    ui.key(Key::F2);
    run(&mut ui, None, Key::F4);
    assert_eq!(ui.key(Key::F1), Action::Run { profile: 1, full: true });
}

#[test]
fn about_shows_the_engine() {
    let mut ui = Ui::new(info());
    ui.key(Key::F5);
    let text = drawn(&ui);
    assert!(text.contains("Engine 0x52414d31") && text.contains("Tick 3077 ps"), "{text}");
    ui.key(Key::Esc);
    assert_eq!(ui.screen(), Screen::Select);
}

#[test]
fn all_bits_bad_still_fits_the_detail_screen() {
    let mut ui = Ui::new(info());
    let mut p = SimPlatform::new(Some(simm(70.0)));
    let mut r = basic_test(&mut p, device::ALL[0], 9000);
    r.failure = Some(rt_core::report::Failure {
        phase: rt_core::report::Phase::March, errors: 123456, addr: 0xFFFFF,
        bits: (1u64 << 40) - 1, read: 0,
    });
    ui.finish(r);
    ui.key(Key::F2);
    drawn(&ui); // the canvas asserts that nothing is drawn off the screen
}
