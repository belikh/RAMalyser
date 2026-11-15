# rt-fw: RP2350B firmware

**Status: written, not compiled, not run.** This crate could not be built on
the development machine (no `thumbv8m.main-none-eabihf` target for the
distribution's Rust compiler), and the board it targets has not been made.
Expect to fix dependency versions and API details on first build.

All test logic, the command console and the board drivers live in `rt-core`
and `rt-hal`, which *are* compiled and tested (`cargo test --features sim`
in the parent directory). This crate only wires pins to those drivers.

    rustup target add thumbv8m.main-none-eabihf
    ../../gateware/build.sh                 # produces the FPGA bitstream it embeds
    cargo build --release
    picotool load -u -v -x -t elf target/thumbv8m.main-none-eabihf/release/rt-fw

What it does:

1. Brings up clocks, USB serial, SPI0 (FPGA registers, shared with the
   display), SPI1 (FPGA configuration), I2C0 (DAC, current monitor, adapter
   ID EEPROM).
2. Loads the embedded bitstream into the ECP5 and checks the engine ID.
3. Runs the `rt-core` console over USB serial: `list`, `basic <n>`,
   `full <n>`, `id`, `peek`, `poke`.

Not implemented yet: the display and keys (the console is the only UI),
adapter identification from the ID EEPROM, calibration storage, SPD access.
USB is not serviced while a test runs.
