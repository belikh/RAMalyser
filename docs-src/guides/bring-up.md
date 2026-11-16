---
title: Bring up a new unit
summary: The checks to make on a new base unit and its cartridges, in order, before any memory goes into a socket.
order: 10
icon: check
audience: users
status: draft
---

Revision A has never been built. Do each step in order and stop at the first one that fails.

:::danger No memory until the supply is proven
Do not fit a memory module until you have swept the DUT supply and seen the over-voltage cut-off work.
:::

## Before you order

1. Print each socket footprint at 1:1 and lay the real part on it: 72-pin SIMM, 30-pin SIMM, ZIF sockets and ZIP strips. The SIMM and ZIP patterns come from published dimensions.
2. Check each DIP and ZIP pin map marked `UNVERIFIED` in `spec/ramspec.py` against the manufacturer's datasheet.
3. Check every part number and its stock in the assembler's BOM tool.
4. Check the part rotations in the assembler's placement preview.
5. Check the card-edge socket footprint against the socket you buy. The pattern is from Samtec's drawing for PCIE-164-02-x-D-RA.
6. Order the cartridges with hard-gold fingers and a bevelled tongue.
7. Check that the PD controller you fit (CH224A) has the CH224K pinout the schematic uses.
8. Check the fabricator's limits against the table in [[start/build#fabrication-limits]].
9. If you fit the Wi-Fi module, check J8 against a real XIAO first. See [[guides/fit-wifi-module]].

## Base board without power

1. Measure the resistance from each rail to ground: `VBUS`, `+3V3`, `+5V`, `+1V1`, `+2V5`, `RP_1V1` and `VDUT`.
2. Stop if any rail reads as a short.

## Base board, first power

Use a bench supply set to 5 V with a 100 mA limit on `VBUS`.

1. Check that `+3V3`, `+1V1`, `+2V5` and `RP_1V1` are in range and that `+5V` follows the input.
2. Hold BOOT and connect USB. The board appears as a USB drive.
3. Load the firmware.
4. Check that the FPGA configures: `DONE` goes high, the ID register reads `0x52414D31` and the PLL locks.
5. Connect a PD charger. `VBUS` goes to 9 V.
6. Repeat step 5 with several chargers and with a 5 V-only source.

## DUT supply

Test with no cartridge first, then with a dummy load.

1. Sweep the DAC. The output goes from 2.5 V to 5.8 V and rises with every step. Record the table.
2. Leave `STRAP_5V_OK` open and raise the output. The switch drops out above about 3.85 V.
3. Ground `STRAP_5V_OK` and raise the output. The switch drops out above about 5.86 V.
4. Measure ripple and step response at 0 A, 1 A and 2 A.
5. Compare the current reading with a meter.
6. Short the output and measure the trip time.
7. Measure the discharge time after switch-off.
8. Measure the switch temperature at 2 A.

## Timing

1. Run the loopback with no [[term:DUT]] at 3.0 V, 5.0 V and 5.5 V. Record the launch-to-capture delay of each line.
2. Store that table in the cartridge EEPROM. It is the calibration table.
3. Measure the input-delay step size against one [[term:Tick]].
4. Measure RAS to data-valid on a known module with an oscilloscope. Compare it with the access time the tester reports. The target is within 3 ns.
5. Measure edge rates and ground bounce with 36 DQ lines switching.

## Memory

1. Test known-good and known-bad parts in every socket: each size, parity and non-parity, FPM and EDO, 5 V and 3.3 V SIMMs, and each DIP family.
2. For the 4116, watch the three rails on an oscilloscope at power-on and power-off.
3. Pull the -5 V pump and check that +12 V and +5 V stay off.
4. Insert a chip backwards. The current limit trips and nothing overheats.
5. Loop the tests for thirty minutes at the highest current.
6. Repeat on a second unit and compare the results.

## Cartridge slot

1. Check the insertion force and the alignment on the rails.
2. Check that the shell clears the console when the cartridge is seated.
3. Measure the contact resistance of the DUT-rail contacts at 2 A, and their temperature rise.
4. Pull a cartridge with the DUT supply on. The firmware drops the rail when `ADP_PRESENT_N` rises.

## Limits of revision A

| Missing | State |
| --- | --- |
| 168-pin SDRAM DIMMs | The base has the lines and the SPD bus. The cartridge and the synchronous engine do not exist. |
| ECC-optimised 72-pin SIMMs | Different pinout, no socket map. |
| Access time from CAS, page-mode cycle time | Not measured separately. |
| EMC | No work done. A unit sold assembled needs it. |

:::see-also
- [[start/build]]
- [[concepts/architecture]]
:::
