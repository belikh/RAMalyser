//! Line-oriented command console, used over USB serial and by the host CLI.
//!
//! Commands:
//!   list                      socket profiles
//!   basic <n>                 quick test on profile n
//!   full <n>                  extensive test on profile n
//!   id                        engine ID and tick period
//!   peek <reg> / poke <reg> <value>   raw register access (hex)
//!   json <list | id | basic <n> | full <n>>
//!                             the same, answered with one line of JSON
//!                             (the web interface and the Wi-Fi module use it)

use core::fmt::Write;

use crate::device;
use crate::regs;
use crate::tests::{basic_test, extensive_test};
use crate::timing;
use crate::Platform;

fn parse_num(s: &str) -> Option<u32> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x") {
        u32::from_str_radix(h, 16).ok()
    } else {
        s.parse().ok()
    }
}

/// The test a command line asks for, if it is one: (profile number, full).
/// The firmware uses it to put "Testing" on the display before the test
/// starts, and to show the result afterwards.
pub fn test_request(line: &str) -> Option<(usize, bool)> {
    let mut it = line.split_whitespace();
    let mut cmd = it.next()?;
    if cmd == "json" {
        cmd = it.next()?;
    }
    let full = match cmd {
        "basic" => false,
        "full" => true,
        _ => return None,
    };
    let n = parse_num(it.next()?)? as usize;
    (n < device::ALL.len()).then_some((n, full))
}

/// Run the test a line asks for and write the answer in the line's own
/// form (text or JSON). `None` if the line is not a test.
pub fn run_test<P: Platform, W: Write>(p: &mut P, line: &str, cal_ps: u32, out: &mut W)
                                       -> Option<crate::report::Report> {
    let (n, full) = test_request(line)?;
    let pr = device::ALL[n];
    let rep = if full { extensive_test(p, pr, cal_ps) } else { basic_test(p, pr, cal_ps) };
    if line.split_whitespace().next() == Some("json") {
        let _ = write!(out, "{{\"ok\":true,\"test\":\"{}\",\"report\":",
                       if full { "full" } else { "basic" });
        let _ = rep.write_json(out);
        let _ = writeln!(out, "}}");
    } else {
        let _ = write!(out, "{rep}");
    }
    Some(rep)
}

/// Execute one command line; output goes to `out`. `cal_ps` is the
/// calibrated adapter delay. Returns false for an unknown command.
pub fn execute<P: Platform, W: Write>(p: &mut P, line: &str, cal_ps: u32, out: &mut W) -> bool {
    if run_test(p, line, cal_ps, out).is_some() {
        return true;
    }
    let mut it = line.split_whitespace();
    let cmd = match it.next() {
        Some(c) => c,
        None => return true,
    };
    let profile = |arg: Option<&str>| {
        arg.and_then(parse_num).and_then(|n| device::ALL.get(n as usize).copied())
    };
    if cmd == "json" {
        return execute_json(p, it.next(), profile(it.clone().next()), cal_ps, out);
    }
    match cmd {
        "list" => {
            for (i, pr) in device::ALL.iter().enumerate() {
                let _ = writeln!(out, "{i}: {}{}", pr.name,
                                 if pr.aux_rails { "  (-5 V / +12 V rails)" } else { "" });
            }
        }
        "id" => {
            let id = p.reg_read(regs::ID);
            let _ = writeln!(out, "engine {:#010x} {}  tick {} ps", id,
                             if id == regs::ID_VALUE { "ok" } else { "UNEXPECTED" },
                             timing::tick_ps(p));
        }
        "basic" | "full" => match profile(it.next()) {
            Some(pr) => {
                let rep = if cmd == "basic" { basic_test(p, pr, cal_ps) }
                          else { extensive_test(p, pr, cal_ps) };
                let _ = write!(out, "{rep}");
            }
            None => {
                let _ = writeln!(out, "usage: {cmd} <profile number> (see `list`)");
            }
        },
        "peek" => match it.next().and_then(parse_num) {
            Some(a) if a < 0x80 => {
                let _ = writeln!(out, "{:#04x} = {:#010x}", a, p.reg_read(a as u8));
            }
            _ => {
                let _ = writeln!(out, "usage: peek <reg>");
            }
        },
        "poke" => match (it.next().and_then(parse_num), it.next().and_then(parse_num)) {
            (Some(a), Some(v)) if a < 0x80 => {
                p.reg_write(a as u8, v);
                let _ = writeln!(out, "ok");
            }
            _ => {
                let _ = writeln!(out, "usage: poke <reg> <value>");
            }
        },
        _ => {
            let _ = writeln!(out, "unknown command: {cmd}");
            return false;
        }
    }
    true
}

/// One JSON object on one line: `{"ok":true,...}` or `{"ok":false,"error":"..."}`.
fn execute_json<P: Platform, W: Write>(p: &mut P, cmd: Option<&str>,
                                       profile: Option<&'static device::Profile>, cal_ps: u32,
                                       out: &mut W) -> bool {
    match cmd {
        Some("list") => {
            let _ = write!(out, "{{\"ok\":true,\"profiles\":[");
            for (i, pr) in device::ALL.iter().enumerate() {
                let _ = write!(out, "{}{{\"n\":{i},\"name\":\"{}\",\"aux_rails\":{}}}",
                               if i == 0 { "" } else { "," }, pr.name, pr.aux_rails);
            }
            let _ = writeln!(out, "]}}");
        }
        Some("id") => {
            let id = p.reg_read(regs::ID);
            let _ = writeln!(out, "{{\"ok\":true,\"engine\":\"{:#010x}\",\"engine_ok\":{},\"tick_ps\":{}}}",
                             id, id == regs::ID_VALUE, timing::tick_ps(p));
        }
        Some(c @ ("basic" | "full")) => match profile {
            Some(pr) => {
                let rep = if c == "basic" { basic_test(p, pr, cal_ps) }
                          else { extensive_test(p, pr, cal_ps) };
                let _ = write!(out, "{{\"ok\":true,\"test\":\"{c}\",\"report\":");
                let _ = rep.write_json(out);
                let _ = writeln!(out, "}}");
            }
            None => {
                let _ = writeln!(out, "{{\"ok\":false,\"error\":\"no such profile\"}}");
            }
        },
        _ => {
            let _ = writeln!(out, "{{\"ok\":false,\"error\":\"unknown command\"}}");
            return false;
        }
    }
    true
}
