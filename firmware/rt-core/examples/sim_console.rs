//! The tester's console against the software engine and DRAM model, on
//! standard input and output. `web/dev.py` runs it behind the web
//! interface, so the page can be tried without hardware.
//!
//!     cargo run -p rt-core --features sim --example sim_console
//!
//! Lines starting with `!` choose the simulated memory:
//!     !good   a 4 MB FPM 70 ns SIMM (the default)      !edo    the same, EDO 60 ns
//!     !empty  an empty socket                           !cell   one bad cell
//!     !short  CAS0 shorted to CAS1 on the cartridge
use std::io::{BufRead, Write};

use rt_core::console;
use rt_core::sim::{Fault, SimDut, SimPlatform};

fn dut(kind: &str) -> Option<SimDut> {
    let mut d = SimDut::new(10, 10, (1u64 << 36) - 1, 70.0);
    match kind {
        "empty" => return None,
        "edo" => {
            d.edo = true;
            d.t_rac_ns = 60.0;
        }
        "cell" => d.faults.push(Fault::StuckBit { addr: 0x1234, bit: 5, value: false }),
        "short" => d.faults.push(Fault::CtlShort(20, 21)),
        _ => {}
    }
    Some(d)
}

fn main() {
    let mut kind = String::from("good");
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        if let Some(k) = line.strip_prefix('!') {
            kind = k.trim().to_string();
            println!("{{\"ok\":true,\"memory\":\"{kind}\"}}");
        } else {
            let mut p = SimPlatform::new(dut(&kind));
            let mut out = String::new();
            console::execute(&mut p, &line, 9000, &mut out);
            print!("{out}");
        }
        std::io::stdout().flush().unwrap();
    }
}
