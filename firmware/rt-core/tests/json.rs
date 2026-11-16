//! The JSON form of the console, as the web interface reads it.
use rt_core::console;
use rt_core::sim::{Fault, SimDut, SimPlatform};

fn ask(dut: Option<SimDut>, line: &str) -> String {
    let mut p = SimPlatform::new(dut);
    let mut out = String::new();
    console::execute(&mut p, line, 9000, &mut out);
    assert_eq!(out.matches('\n').count(), 1, "one line per answer: {out:?}");
    out.trim_end().to_string()
}

fn simm() -> SimDut {
    SimDut::new(10, 10, (1u64 << 36) - 1, 70.0)
}

#[test]
fn list_and_id() {
    let l = ask(None, "json list");
    assert!(l.starts_with("{\"ok\":true,\"profiles\":[{\"n\":0,\"name\":\"72-pin SIMM\",\"aux_rails\":false}"), "{l}");
    assert!(l.contains("{\"n\":2,\"name\":\"4116 (16K x1)\",\"aux_rails\":true}"), "{l}");
    assert_eq!(ask(None, "json id"),
               "{\"ok\":true,\"engine\":\"0x52414d31\",\"engine_ok\":true,\"tick_ps\":3077}");
}

#[test]
fn good_module() {
    assert_eq!(ask(Some(simm()), "json basic 0"),
        "{\"ok\":true,\"test\":\"basic\",\"report\":{\"profile\":\"72-pin SIMM\",\"passed\":true,\
\"verdict\":\"pass\",\"no_device\":false,\"power_fault\":false,\"bits\":37748736,\"words\":1048576,\
\"width\":36,\"row_bits\":10,\"col_bits\":10,\"mode\":\"FPM\",\"t_rac_ns\":70,\"retention_ms\":null,\
\"current_ma\":120,\"wire_faults\":[],\"failure\":null}}");
}

#[test]
fn failures() {
    let mut d = simm();
    d.faults.push(Fault::StuckBit { addr: 0x1234, bit: 5, value: false });
    let j = ask(Some(d), "json basic 0");
    assert!(j.contains("\"verdict\":\"fail\"") && j.contains(
        "\"failure\":{\"phase\":\"Pattern\",\"errors\":1,\"addr\":4660,\"bits\":\"0x0000000020\",\"read\":\"0xfaaaaaaa8a\"}"), "{j}");
    let mut d = simm();
    d.faults.push(Fault::CtlShort(20, 21));
    let j = ask(Some(d), "json basic 0");
    assert!(j.contains("\"wire_faults\":[{\"kind\":\"short\",\"line\":\"CAS0\",\"other\":\"CAS1\"}]"), "{j}");
    let j = ask(None, "json basic 0");
    assert!(j.contains("\"verdict\":\"no_device\"") && j.contains("\"words\":0"), "{j}");
}

#[test]
fn errors() {
    assert_eq!(ask(None, "json basic 9"), "{\"ok\":false,\"error\":\"no such profile\"}");
    assert_eq!(ask(None, "json frob"), "{\"ok\":false,\"error\":\"unknown command\"}");
    assert_eq!(ask(None, "json"), "{\"ok\":false,\"error\":\"unknown command\"}");
}

#[test]
fn test_requests_are_recognised() {
    assert_eq!(console::test_request("basic 0"), Some((0, false)));
    assert_eq!(console::test_request("json full 6"), Some((6, true)));
    assert_eq!(console::test_request("full 7"), None);
    assert_eq!(console::test_request("list"), None);
    assert_eq!(console::test_request("json basic"), None);
    let mut p = SimPlatform::new(Some(simm()));
    let mut out = String::new();
    let r = console::run_test(&mut p, "basic 0", 9000, &mut out).unwrap();
    assert!(r.passed() && out.ends_with("PASS\n"), "{out}");
}
