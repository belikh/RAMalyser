//! The firmware's own tests, run through the real drivers and the real
//! gateware against a pin-level cartridge and DRAM.
use rt_core::device::{self, Profile};
use rt_core::report::{Mode, Phase, Report, WireFault};
use rt_core::tests::{basic_test, extensive_test};
use rt_cosim::{Dut, Fault, Sim, PATH_PS};

fn run(profile: &'static Profile, dut: Option<Dut>, long: bool) -> (Report, Sim) {
    let sim = Sim::new(profile, dut);
    let mut board = sim.board();
    let rep = if long { extensive_test(&mut board, profile, PATH_PS) }
              else { basic_test(&mut board, profile, PATH_PS) };
    {
        let w = sim.0.borrow();
        eprintln!("{}: {:.1} ms simulated, {} clock cycles, tRAC {:?}", profile.name,
                  w.now as f64 / 1e9, w.cycles, rep.t_rac_ns);
    }
    (rep, sim)
}

fn basic(profile: &'static Profile, dut: Dut) -> Report {
    run(profile, Some(dut), false).0
}

const X4: &Profile = &device::DIP_44256_514400;
const X1: &Profile = &device::DIP_4164_41256;

fn x1(rows: u8, cols: u8, t_rac: u32) -> Dut {
    Dut { split0: true, ..Dut::new(rows, cols, 0b10, t_rac) }
}

#[test]
fn good_4bit_fpm_chip() {
    let rep = basic(X4, Dut::new(6, 6, 0xF, 80));
    assert!(rep.passed(), "{rep:?}");
    assert_eq!((rep.geometry.row_bits, rep.geometry.col_bits), (6, 6));
    assert_eq!(rep.geometry.width_mask, 0xF);
    assert_eq!(rep.mode, Some(Mode::Fpm));
    assert_eq!(rep.current_ma, 120);
}

#[test]
fn good_1bit_chip_with_separate_in_and_out() {
    let rep = basic(X1, x1(7, 6, 120));
    assert!(rep.passed(), "{rep:?}");
    assert_eq!((rep.geometry.row_bits, rep.geometry.col_bits), (7, 6));
    assert_eq!(rep.geometry.width_mask, 0b10);
}

#[test]
fn good_30_pin_simm_with_parity() {
    let dut = Dut { split1: true, ..Dut::new(6, 6, 0xFF | 1 << 33, 70) };
    let rep = basic(&device::SIMM30, dut);
    assert!(rep.passed(), "{rep:?}");
    assert_eq!(rep.geometry.width_mask, 0xFF | 1 << 33);
    assert_eq!(rep.t_rac_ns, Some(70));
}

#[test]
fn good_72_pin_simm_36_bits_edo() {
    let dut = Dut { edo: true, ..Dut::new(5, 5, 0xF_FFFF_FFFF, 60) };
    let rep = basic(&device::SIMM72, dut);
    assert!(rep.passed(), "{rep:?}");
    assert_eq!(rep.geometry.width_mask, 0xF_FFFF_FFFF);
    assert_eq!(rep.mode, Some(Mode::Edo));
    let t = rep.t_rac_ns.expect("speed measured");
    assert!((58..=62).contains(&t), "measured {t} ns for a 60 ns part");
}

/// Regression: a start that arrives during an idle refresh cycle used to be
/// dropped, and the firmware then read the previous element's results. The
/// 72-pin profile refreshes most often, so repeat single-word elements on it.
#[test]
fn start_during_refresh_is_not_lost() {
    use rt_core::tests::{Session, PAT_SOLID, R0, R1, W0, W1};
    use rt_core::Platform;
    let pr = &device::SIMM72;
    let sim = Sim::new(pr, Some(Dut::new(5, 5, 0xF_FFFF_FFFF, 60)));
    let mut b = sim.board();
    b.set_vdut_mv(5000);
    assert!(b.vdut_enable(true));
    let mut s = Session::new(&mut b, pr, PATH_PS);
    s.configure();
    for n in 0..400u32 {
        let (w, r, want) = if n % 2 == 0 { (W0, R0, 0) } else { (W1, R1, 0xF_FFFF_FFFF) };
        s.element(n % 32, n % 32, false, PAT_SOLID, &[w]);
        let got = s.element(n % 32, n % 32, false, PAT_SOLID, &[r]);
        assert_eq!((got.errors, got.last & 0xF_FFFF_FFFF), (0, want), "element {n}");
    }
}

#[test]
fn access_time_is_measured_within_two_ns() {
    for t in [60u32, 70, 100, 150] {
        let rep = basic(X4, Dut::new(5, 5, 0xF, t));
        let got = rep.t_rac_ns.expect("speed measured") as i32;
        assert!((got - t as i32).abs() <= 2, "{t} ns part measured as {got} ns");
    }
}

#[test]
fn stuck_bit_is_found_at_its_address() {
    let mut dut = Dut::new(6, 6, 0xF, 80);
    dut.faults.push(Fault::StuckBit { addr: 1234, bit: 2, value: false });
    let f = basic(X4, dut).failure.expect("failure reported");
    assert_eq!((f.addr, f.bits), (1234, 0b0100));
}

#[test]
fn shorted_data_lines_are_found() {
    let mut dut = Dut::new(6, 6, 0xF, 80);
    dut.faults.push(Fault::DqShort(0, 3));
    let rep = basic(X4, dut);
    assert!(!rep.passed(), "{rep:?}");
}

#[test]
fn dead_address_line_halves_the_size() {
    let mut dut = Dut::new(6, 6, 0xF, 80);
    dut.faults.push(Fault::AddrLineLow(5));
    let rep = basic(X4, dut);
    assert_eq!((rep.geometry.row_bits, rep.geometry.col_bits), (5, 5), "{rep:?}");
}

#[test]
fn shorted_control_lines_stop_the_test() {
    let mut dut = Dut::new(6, 6, 0xF, 80);
    dut.faults.push(Fault::CtlShort(3, 4));
    let rep = basic(X4, dut);
    assert!(rep.wire_faults.contains(&Some(WireFault::Short(3, 4))), "{rep:?}");
    assert!(rep.geometry.row_bits == 0, "the device must not be exercised: {rep:?}");
}

#[test]
fn stuck_control_line_is_named() {
    let mut dut = Dut::new(6, 6, 0xF, 80);
    dut.faults.push(Fault::CtlStuck(16, false));
    let rep = basic(X4, dut);
    assert!(rep.wire_faults.contains(&Some(WireFault::StuckLow(16))), "{rep:?}");
}

#[test]
fn empty_socket_reports_no_device() {
    let (rep, _) = run(X4, None, false);
    assert!(rep.no_device, "{rep:?}");
}

#[test]
fn part_slower_than_the_socket_allows_fails() {
    let rep = basic(X4, Dut::new(5, 5, 0xF, 260));
    assert!(!rep.passed(), "{rep:?}");
}

#[test]
fn overcurrent_is_a_power_fault_and_the_rail_goes_off() {
    let dut = Dut { current_ma: 2600, ..Dut::new(5, 5, 0xF, 80) };
    let (rep, sim) = run(X4, Some(dut), false);
    assert!(rep.power_fault, "{rep:?}");
    let w = sim.0.borrow();
    assert!(!w.power.buck_en && !w.power.dut_sw && !w.power.dut_pg);
}

#[test]
fn rail_is_off_after_a_passing_test() {
    let (rep, sim) = run(X4, Some(Dut::new(5, 5, 0xF, 80)), false);
    assert!(rep.passed());
    let w = sim.0.borrow();
    assert!(!w.power.buck_en && !w.power.dut_sw && !w.power.dut_pg);
    assert_eq!(w.power.socket_mv(), 0);
}

#[test]
fn extensive_test_passes_and_finds_the_retention_time() {
    // 4 ms refresh interval on this socket; the cells hold for 20 ms
    let dut = Dut { retention_ps: Some(20_000_000_000), v_min: 4300, v_max: 5700,
                    ..x1(5, 5, 100) };
    let (rep, _) = run(X1, Some(dut), true);
    assert!(rep.passed(), "{rep:?}");
    assert_eq!(rep.retention_ms, Some(16));
}

#[test]
fn part_that_fails_at_low_supply_is_caught_by_the_margin_test() {
    let dut = Dut { v_min: 4700, ..Dut::new(5, 5, 0xF, 80) };
    let (rep, _) = run(X4, Some(dut), true);
    let f = rep.failure.expect("failure reported");
    assert_eq!(f.phase, Phase::VoltageLow, "{rep:?}");
}
