# rt-fw: RP2350B firmware

**Status: compiles and links, not run.** The board it targets has not been
made, so nothing here has executed on an RP2350. `memory.x` is the memory
map for the 16 MB flash on the base board.

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
3. Draws the front-panel interface (`rt_core::ui`) on the display and
   reads the six keys: choose a profile, run a quick or a full test, read
   the result and its details.
4. Runs the `rt-core` console over USB serial: `list`, `basic <n>`,
   `full <n>`, `id`, `peek`, `poke`, and the same with a `json` prefix.
5. Runs the same console on UART0 (115200 baud) for the optional Wi-Fi
   module (`../../wifi`).

A test started from the keys, the USB console or the Wi-Fi module shows on
the display, lights the pass or fail LED and sounds the buzzer.

Before building, run `../update-design.sh` once: it downloads the design
system release and generates the display's palette and fonts into
`../rt-hal/src/generated/` (not committed).

Not implemented yet: adapter identification from the ID EEPROM, calibration
storage, SPD access. Nothing is served while a test runs.

The parts of this file most likely to need changes on first build are the
`BufferedUart::new` argument order and the type of the shared SPI bus, which
differ between `embassy-rp` releases.
