# Architecture

A base unit and slide-in cartridges. The base is 3.3 V only and fast; each
cartridge (an adapter board in a printed shell) carries whatever its memory
family needs.

```
 USB-C PD 9 V ──► bucks: 3.3 V, 5 V, 1.1 V ──► LDO 2.5 V
        │
        └─► DUT buck 2.5–5.8 V (DAC-set) ─► shunt + INA219 ─► P-FET switch ─► VDUT
                                 over-voltage comparator ─┘ (cuts the switch)

 RP2350B ── SPI ──► ECP5 FPGA (timing engine) ══ 116 lines, 22 Ω ══► card-edge slot ══► cartridge
   │  USB, display, keys, buzzer            ▲
   └─ loads the FPGA bitstream at power-up  └ 25 MHz oscillator, PLL to 162.5 MHz
```

## Timing engine (`gateware/`)

- One tick is half a clock period: 3.077 ns at 162.5 MHz. Every bus line
  leaves through a DDR output register and DQ lines return through DDR input
  registers, so RAS, CAS, WE, the address multiplex point and the read strobe
  are placed to one tick.
- A per-pin input delay (about 25 ps per step, 127 steps) interpolates the
  read strobe between ticks. 127 steps cover 3.2 ns, more than one tick, so
  the strobe position is continuous.
- The engine runs *march elements*: for each address in a range, up to six
  read/write operations, each a full RAS/CAS cycle. It compares against a
  generated pattern (solid, checkerboard or address-derived), counts errors,
  and keeps the first failing address, the data read there and the union of
  failing bits. Refresh cycles (RAS-only or CAS-before-RAS) are interleaved.
- All 116 lines can also be driven and read statically, which the wiring
  test uses.
- The MCU reads and writes 32-bit registers over SPI
  (`firmware/rt-core/src/regs.rs`).

Access time is found by moving the strobe earlier until reads fail, then
subtracting the calibrated delay of the adapter's buffers.

## Bus (`spec/ramspec.py`, `spec/generated/bus.md`)

One 164-contact card edge. The cartridge carries gold fingers on its front
edge; the base carries a right-angle socket under the console (Samtec
PCIE-164-02-F-D-RA). The cartridge lies flat, 5 mm above the base board, and
slides in from the rear on two rails.

The connector is the PCI Express x16 part because it is cheap and stocked
everywhere, but only mechanically: the pin assignment is this project's own.
**Never put a cartridge in a PC's PCIe slot** (its DUT supply contacts would
see 12 V), and do not put a PCIe card in the tester.

Contacts: 116 FPGA lines, 24 grounds (one ground position per six signal
positions), 8 DUT-rail contacts, 3.3 V and 5 V, two I2C buses (adapter ID
and SPD) and sideband signals. Every FPGA line is 3.3 V and **not 5 V
tolerant**. This style of socket is rated for a limited number of
insertions (tens to low hundreds); treat it as a wear part.

Rules for an adapter:

1. No socket pin may connect to a bus signal contact except through a buffer.
   `pcb/boards/common.py: check_buffered` fails the build otherwise.
2. An adapter that needs more than 3.8 V on VDUT grounds `STRAP_5V_OK`.
   Without it the base's comparator cuts the DUT switch above 3.85 V.
3. The base tristates every line while `DUT_PG` is low.

116 lines were chosen so a 168-pin SDRAM DIMM (109 signals) fits later
without changing the base. The ground count is modest for 133 MHz; that
cartridge will need its signal integrity checked on the bench.

## 5 V cartridges

Drive path: 74LVC8T245 with its B side powered from VDUT, so drive levels
follow the DUT supply during margin tests. Receive path: 74LVC245A powered
from 3.3 V, whose inputs are 5 V tolerant with TTL thresholds, which is what
NMOS and CMOS DRAM outputs are specified against. Both sit on the same FPGA
line; the engine never enables both at once.

Every control line is read back through sense buffers (eight at a time) so a
stuck or shorted line is reported by name before any memory test runs.
Weak pulls on DQ (two nets, even and odd bits) let the tester tell a driven
line from a floating one: that is how missing bits, FPM versus EDO, and
adjacent shorts are detected.

## Power

- Input: USB-C PD, 9 V requested. With a 5 V-only source the DUT rail cannot
  reach 5 V; the firmware must refuse 5 V tests in that case.
- DUT rail: adjustable buck, set point from an I2C DAC into the feedback
  node (2.5–5.8 V), current measured by an INA219 on a 20 mΩ shunt,
  switched by a P-FET with soft start, discharged through 100 Ω when off.
- The module-power LED is wired to VDUT itself, not to firmware.
- 4116-class chips: -5 V and +12 V are made on the DIP adapter and reach only
  the 4116 socket. A comparator holds +12 V and +5 V off until VBB is below
  about -4.2 V.

## Wi-Fi module (optional)

J8 on the back of the base board takes a Seeed XIAO module (the footprint is
the same for every XIAO; the ESP32-C6 is the intended one). It is wired to
the four RP2350 pins that were spare:

| RP2350 | Net | XIAO pin |
|---|---|---|
| GPIO0 (UART0 TX) | `WIFI_RX` | D7 (RX) |
| GPIO1 (UART0 RX) | `WIFI_TX` | D6 (TX) |
| GPIO2 | `WIFI_IO0` | D0 |
| GPIO3 | `WIFI_IO1` | D1 |

The module's 5 V pin is fed from the 5 V rail through a Schottky diode, so
plugging a cable into the module's own USB port cannot drive the rail. Its
3.3 V pin is left open. There is no copper on any layer under the antenna.
No firmware uses the module yet: the link is a UART plus two spare lines,
and the protocol is still to be written.

## Firmware (`firmware/`)

`rt-core` holds all test logic behind a small `Platform` trait and runs on a
PC against a software model of the engine and a DRAM (`cargo test --features
sim`). `rt-fw` is the RP2350 binary that implements `Platform` on the real
hardware.
