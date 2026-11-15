//! Firmware algorithms against the software engine + DRAM model.
use rt_core::device::{self, Profile};
use rt_core::report::{Mode, Phase, WireFault};
use rt_core::sim::{Fault, SimDut, SimPlatform};
use rt_core::tests::{basic_test, extensive_test};

const CAL: u32 = 9000; // matches SimPlatform::path_ps

fn simm72_4mb(t_rac: f64) -> SimDut {
    // 1M x 36 (4 MB, parity): 10 row x 10 column bits
    SimDut::new(10, 10, (1u64 << 36) - 1, t_rac)
}

fn run(dut: Option<SimDut>, profile: &'static Profile) -> rt_core::report::Report {
    let mut p = SimPlatform::new(dut);
    basic_test(&mut p, profile, CAL)
}

#[test]
fn good_simm72_identified() {
    let r = run(Some(simm72_4mb(60.0)), &device::SIMM72);
    assert!(r.passed(), "{r}");
    assert_eq!((r.geometry.row_bits, r.geometry.col_bits), (10, 10));
    assert_eq!(r.geometry.data_bits(), 36);
    assert_eq!(r.mode, Some(Mode::Fpm));
    println!("{r}");
}

#[test]
fn non_parity_simm_is_32_bits_wide() {
    let r = run(Some(SimDut::new(9, 9, 0xFFFF_FFFF, 70.0)), &device::SIMM72);
    assert!(r.passed(), "{r}");
    assert_eq!(r.geometry.data_bits(), 32);
    assert_eq!((r.geometry.row_bits, r.geometry.col_bits), (9, 9));
}

#[test]
fn asymmetric_geometry_detected() {
    // 12 row x 10 column parts exist (16 MB SIMMs)
    let r = run(Some(SimDut::new(12, 10, 0xFFFF_FFFF, 60.0)), &device::SIMM72);
    assert!(r.passed(), "{r}");
    assert_eq!((r.geometry.row_bits, r.geometry.col_bits), (12, 10));
}

#[test]
fn speed_is_measured_within_one_ns() {
    for t in [50.0, 60.0, 70.0, 80.0, 100.0, 120.0] {
        let r = run(Some(simm72_4mb(t)), &device::SIMM72);
        let got = r.t_rac_ns.expect("speed") as f64;
        assert!((got - t).abs() <= 1.0, "model {t} ns measured {got} ns");
    }
}

#[test]
fn edo_detected() {
    let mut d = simm72_4mb(60.0);
    d.edo = true;
    assert_eq!(run(Some(d), &device::SIMM72).mode, Some(Mode::Edo));
}

#[test]
fn empty_socket_reports_no_device() {
    let r = run(None, &device::SIMM72);
    assert!(r.no_device && !r.passed());
}

#[test]
fn stuck_bit_found_with_address_and_bit() {
    let mut d = simm72_4mb(60.0);
    d.faults.push(Fault::StuckBit { addr: 0x1234, bit: 17, value: false });
    let r = run(Some(d), &device::SIMM72);
    let f = r.failure.expect("must fail");
    assert_eq!(f.addr, 0x1234);
    assert_eq!(f.bits, 1 << 17);
}

#[test]
fn shorted_data_lines_found() {
    let mut d = simm72_4mb(60.0);
    d.faults.push(Fault::DqShort(4, 5));
    let r = run(Some(d), &device::SIMM72);
    assert_eq!(r.failure.expect("must fail").bits, 0b11 << 4);
}

#[test]
fn device_address_line_fault_shrinks_or_fails() {
    let mut d = simm72_4mb(60.0);
    d.faults.push(Fault::AddrLineLow(3));
    let r = run(Some(d), &device::SIMM72);
    // an internal address fault shows up as a wrong (non power-of-two-complete) size
    assert!(!(r.geometry.row_bits == 10 && r.geometry.col_bits == 10 && r.passed()), "{r}");
}

#[test]
fn wiring_faults_named() {
    let mut d = simm72_4mb(60.0);
    d.faults.push(Fault::CtlStuck(5, false)); // MA5 stuck low
    let r = run(Some(d), &device::SIMM72);
    assert_eq!(r.wire_faults[0], Some(WireFault::StuckLow(5)));
    assert!(!r.passed());

    let mut d = simm72_4mb(60.0);
    d.faults.push(Fault::CtlShort(16, 20)); // RAS0 to CAS0
    let r = run(Some(d), &device::SIMM72);
    assert!(r.wire_faults.contains(&Some(WireFault::Short(16, 20))), "{r}");
}

#[test]
fn x1_chip_with_separate_din_dout() {
    // 41256: 9x9, data in on DQ0, data out on DQ1
    let mut d = SimDut::new(9, 9, 0b10, 120.0);
    d.split0 = true;
    let r = run(Some(d), &device::DIP_4164_41256);
    assert!(r.passed(), "{r}");
    assert_eq!((r.geometry.row_bits, r.geometry.col_bits), (9, 9));
    assert_eq!(r.geometry.bits(), 262_144);
    // a 4164 in the same socket is found as 8x8
    let mut d = SimDut::new(8, 8, 0b10, 150.0);
    d.split0 = true;
    let r = run(Some(d), &device::DIP_4164_41256);
    assert!(r.passed(), "{r}");
    assert_eq!(r.geometry.bits(), 65_536);
}

#[test]
fn simm30_with_parity() {
    let mut d = SimDut::new(10, 10, 0xFF | 1 << 33, 80.0);
    d.split1 = true;
    let r = run(Some(d), &device::SIMM30);
    assert!(r.passed(), "{r}");
    assert_eq!(r.geometry.data_bits(), 9);
}

#[test]
fn overcurrent_reported_as_power_fault() {
    let mut d = simm72_4mb(60.0);
    d.current_ma = 3000;
    let r = run(Some(d), &device::SIMM72);
    assert!(r.power_fault && !r.passed());
}

#[test]
fn extensive_passes_good_module_and_reports_retention() {
    let mut d = SimDut::new(8, 8, 0xFFFF_FFFF, 70.0);
    d.retention_us = Some(200_000); // 200 ms
    let mut p = SimPlatform::new(Some(d));
    let r = extensive_test(&mut p, &device::SIMM72, CAL);
    assert!(r.passed(), "{r}");
    // 16 ms base: 16, 32, 64, 128 pass; 256 fails
    assert_eq!(r.retention_ms, Some(128));
}

#[test]
fn extensive_catches_marginal_supply() {
    let mut d = SimDut::new(8, 8, 0xFFFF_FFFF, 70.0);
    d.v_min = 4700; // fails at 4.5 V
    let mut p = SimPlatform::new(Some(d));
    let r = extensive_test(&mut p, &device::SIMM72, CAL);
    assert_eq!(r.failure.expect("must fail").phase, Phase::VoltageLow);
}

#[test]
fn extensive_catches_leaky_cells() {
    let mut d = SimDut::new(8, 8, 0xFFFF_FFFF, 70.0);
    d.retention_us = Some(5_000); // loses data after 5 ms; spec is 16 ms
    let mut p = SimPlatform::new(Some(d));
    let r = extensive_test(&mut p, &device::SIMM72, CAL);
    assert_eq!(r.failure.expect("must fail").phase, Phase::Retention);
}

#[test]
fn console_runs_a_test_and_prints_a_report() {
    use rt_core::console::execute;
    let mut d = SimDut::new(9, 9, 0b10, 100.0);
    d.split0 = true;
    let mut p = SimPlatform::new(Some(d));
    let mut out = String::new();
    assert!(execute(&mut p, "list", CAL, &mut out));
    assert!(out.contains("3: 4164 / 41256"), "{out}");
    out.clear();
    assert!(execute(&mut p, "basic 3", CAL, &mut out));
    assert!(out.contains("256 Kbit") && out.contains("PASS") && out.contains("100 ns"), "{out}");
    println!("{out}");
    out.clear();
    assert!(execute(&mut p, "id", CAL, &mut out));
    assert!(out.contains("ok") && out.contains("3077 ps"), "{out}");
    assert!(!execute(&mut p, "frobnicate", CAL, &mut out));
}
