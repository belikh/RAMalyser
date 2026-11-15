//! Line-oriented command console, used over USB serial and by the host CLI.
//!
//! Commands:
//!   list                      socket profiles
//!   basic <n>                 quick test on profile n
//!   full <n>                  extensive test on profile n
//!   id                        engine ID and tick period
//!   peek <reg> / poke <reg> <value>   raw register access (hex)

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

/// Execute one command line; output goes to `out`. `cal_ps` is the
/// calibrated adapter delay. Returns false for an unknown command.
pub fn execute<P: Platform, W: Write>(p: &mut P, line: &str, cal_ps: u32, out: &mut W) -> bool {
    let mut it = line.split_whitespace();
    let cmd = match it.next() {
        Some(c) => c,
        None => return true,
    };
    let profile = |arg: Option<&str>| {
        arg.and_then(parse_num).and_then(|n| device::ALL.get(n as usize).copied())
    };
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
