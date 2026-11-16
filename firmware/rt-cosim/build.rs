//! Compile the gateware with Verilator and link it in. Needs `verilator`
//! (OSS CAD Suite) and a C++ compiler; no crates.
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run(cmd: &mut Command) {
    let status = cmd.status().unwrap_or_else(|e| panic!("cannot run {cmd:?}: {e}"));
    assert!(status.success(), "{cmd:?} failed");
}

fn main() {
    let here = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let rtl = here.join("../../gateware/rtl");
    let spec = here.join("../../spec");
    let suite = PathBuf::from(env::var("HOME").unwrap_or_default()).join(".local/opt/oss-cad-suite/bin");
    let mut path = env::var("PATH").unwrap_or_default();
    if suite.is_dir() {
        path = format!("{}:{path}", suite.display());
    }
    // the bus line numbers the gateware is built with
    run(Command::new("python3").arg(spec.join("ramspec.py")).arg("--emit").arg(spec.join("generated")));
    let mdir = out.join("v");
    run(Command::new("verilator").env("PATH", &path)
        .args(["--cc", "--build", "-O3", "--x-assign", "0", "--x-initial", "0", "-Wno-fatal",
               "-Wno-WIDTH", "-Wno-UNUSED", "--top-module", "cosim_top", "-CFLAGS", "-O2 -fPIC"])
        .arg(format!("-I{}", spec.join("generated").display()))
        .arg("--Mdir").arg(&mdir)
        .arg(here.join("rtl/cosim_top.v"))
        .arg(rtl.join("tester_core.v")).arg(rtl.join("spi_regs.v")));
    // the include directory of the Verilator that built the model
    let root = String::from_utf8(Command::new("verilator").env("PATH", &path).arg("--getenv")
        .arg("VERILATOR_ROOT").output().expect("verilator --getenv").stdout).unwrap();
    let inc = Path::new(root.trim()).join("include");
    let obj = out.join("shim.o");
    run(Command::new("g++").args(["-c", "-O2", "-fPIC", "-std=c++17"])
        .arg(format!("-I{}", mdir.display())).arg(format!("-I{}", inc.display()))
        .arg(here.join("src/shim.cpp")).arg("-o").arg(&obj));
    let lib = out.join("libcosimshim.a");
    let _ = std::fs::remove_file(&lib);
    run(Command::new("ar").arg("crs").arg(&lib).arg(&obj));

    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-search=native={}", mdir.display());
    println!("cargo:rustc-link-lib=static=cosimshim");
    println!("cargo:rustc-link-lib=static=Vcosim_top");
    println!("cargo:rustc-link-lib=static=verilated");
    println!("cargo:rustc-link-lib=dylib=stdc++");
    println!("cargo:rustc-link-lib=dylib=pthread");
    for f in ["rtl/cosim_top.v", "src/shim.cpp", "../../gateware/rtl/tester_core.v",
              "../../gateware/rtl/spi_regs.v", "../../spec/ramspec.py"] {
        println!("cargo:rerun-if-changed={}", here.join(f).display());
    }
}
