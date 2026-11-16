---
title: Architecture
summary: How the base unit, the timing engine and the cartridges work together to drive a memory module and measure it.
order: 10
icon: cpu
audience: developers
status: draft
---

A [[term:Base unit]] works at 3.3 V only. Each [[term:Cartridge]] carries what its memory family needs: sockets, level buffers and any extra supply rails.

```text title="Signal and power path"
 USB-C PD 9 V --> bucks: 3.3 V, 5 V, 1.1 V --> LDO 2.5 V
        |
        +--> DUT buck 2.3 to 5.6 V (DAC-set) --> shunt + INA219 --> P-FET switch --> VDUT
                                 over-voltage comparator --+ (cuts the switch)

 RP2350B -- SPI --> ECP5 FPGA (timing engine) == 116 lines, 22 ohm ==> card-edge slot ==> cartridge
   |  USB, display, keys, buzzer            ^
   +- loads the FPGA bitstream at power-up  +- 25 MHz oscillator, PLL to 162.5 MHz
```

## Timing engine

The [[term:Timing engine]] is in `gateware/`.

- One [[term:Tick]] is half a clock period: 3.077 ns at 162.5 MHz.
- Every bus line leaves through a DDR output register. DQ lines return through DDR input registers.
- RAS, CAS, WE, the address multiplex point and the read strobe are each placed to one tick.
- A per-pin input delay of about 25 ps per step, with 127 steps, moves the read strobe between ticks. The 127 steps cover 3.2 ns, which is more than one tick.
- The engine runs [[term:March element]] passes. Each address gets up to six read or write operations, and each operation is a full RAS and CAS cycle.
- The engine compares what it reads with a generated pattern: solid, checkerboard or address-derived.
- It counts errors and keeps the first failing address, the data read there and the union of failing bits.
- It interleaves refresh cycles, RAS-only or CAS-before-RAS.
- It can drive and read all 116 lines statically. The wiring test uses that.
- The processor reads and writes 32-bit registers over SPI. `firmware/rt-core/src/regs.rs` lists them.

## Access time

The tester moves the read strobe earlier until reads fail. It then subtracts the calibrated delay of the cartridge's buffers. The result is the access time of the module.

## 5 V cartridges

| Path | Part | Supply | Reason |
| --- | --- | --- | --- |
| Drive | 74LVC8T245 | B side from [[term:VDUT]] | Drive levels follow the DUT supply during margin tests |
| Receive | 74LVC245A | 3.3 V | Its inputs tolerate 5 V and have TTL thresholds, as NMOS and CMOS DRAM outputs specify |

Both buffers sit on the same FPGA line. The engine never enables both at once.

Sense buffers read every control line back, eight at a time. The tester names a stuck or shorted line before any memory test runs.

Weak pulls on DQ, one net for even bits and one for odd bits, show whether a line is driven or floating. The tester uses that to find missing bits, to tell FPM from EDO and to find shorts between neighbours.

## Power

| Part | Behaviour |
| --- | --- |
| Input | USB-C PD, 9 V requested. From a 5 V-only source the DUT rail cannot reach 5 V, so the firmware must refuse 5 V tests |
| DUT rail | Adjustable buck. An I2C DAC feeds the feedback node to set 2.3 V to 5.6 V. The firmware uses 2.6 V to 5.5 V |
| Current | INA219 across a 20 milliohm shunt |
| Switch | P-FET with soft start. A 100 ohm resistor discharges the rail when it is off |
| Over-voltage | A comparator cuts the switch above 3.85 V, or above 5.86 V when the cartridge grounds `STRAP_5V_OK` |
| Module-power LED | Wired to VDUT itself. Firmware does not control it |
| 4116 rails | The DIP cartridge makes -5 V and +12 V, and they reach only the 4116 socket. A comparator holds +12 V and +5 V off until the -5 V rail is below about -4.2 V |

## Firmware

| Crate | Role |
| --- | --- |
| `rt-core` | All test logic, the console and the front-panel screens, behind the `Platform` and `Canvas` traits. It runs on a PC against a software model of the engine and a DRAM |
| `rt-hal` | Board drivers: FPGA registers, DUT supply, display, keys, LEDs and buzzer |
| `rt-fw` | The RP2350 binary. It wires the pins to those drivers and serves the keys, the USB console and the Wi-Fi link in one loop. It has not been compiled yet |

The Wi-Fi module has its own firmware in `wifi/`. It serves the page in `web/` and passes commands to the tester over a serial link.

:::see-also
- [[reference/bus]]
- [[reference/wifi-module]]
- [[develop/check]]
:::
