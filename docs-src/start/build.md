---
title: Build a unit
summary: Order the three boards, print the enclosure, fit the hand-soldered parts and assemble a base unit with its cartridges.
order: 10
icon: play
audience: users
status: draft
---

## Boards

A unit is one base board and one board per [[term:Cartridge]].

| Board | Layers | Size | Assembly |
| --- | --- | --- | --- |
| Base | 6 | 160 x 150 mm | Parts on both sides. 16 FPGA decoupling capacitors are on the bottom. The FPGA is a BGA, so ask for X-ray inspection. |
| SIMM cartridge | 4 | 150 x 100 mm plus an 84.3 x 8.4 mm tongue | Parts on the top side |
| DIP and ZIP cartridge | 4 | 150 x 100 mm plus an 84.3 x 8.4 mm tongue | Parts on the top side |

PCB layouts and fabrication files are not published. The schematics are in `hardware/schematics/`.

## Fabrication limits

Check these against the fabricator's limits before you order.

| Board | Track | Clearance | Smallest via | Finish |
| --- | --- | --- | --- | --- |
| Base | 0.127 mm, necks to 0.09 mm | 0.1 mm | 0.45 mm pad, 0.2 mm hole | ENIG, 1.6 mm |
| Cartridges | 0.15 mm, necks to 0.1 mm | 0.127 mm | 0.55 mm pad, 0.25 mm hole | Hard gold on the edge fingers, 1.6 mm |

The base stack-up is top, ground, signal, signal, 3.3 V, bottom. The cartridge stack-up is top, ground, signal with the DUT rail, bottom.

Order the cartridges as gold-finger boards with a 20 degree bevel on the tongue. The key notch between contact positions 11 and 12 is part of the outline. Copper stays 0.2 mm from the board edge.

The series resistor arrays are the 4 x 0603 type: 0.8 mm pitch, 3.2 x 1.6 mm.

## Parts you fit by hand

These parts are through-hole. The machine-placement files leave them out.

| Part | Position | Check |
| --- | --- | --- |
| Right-angle card-edge socket, 164 contacts, Samtec PCIE-164-02-F-D-RA | Base J1 | Mouth towards the rear |
| 72-pin SIMM socket | SIMM cartridge XU1 | Compare the footprint with the part first |
| 30-pin SIMM socket | SIMM cartridge XU2 | New-old-stock only. Compare the footprint with the part first |
| 1 x 30 machined socket strip, 2.54 mm | SIMM cartridge XU3 | Takes SIPP modules |
| ZIF sockets, 16, 18 and 20 pin, 0.3 in | DIP cartridge XU1 to XU5 | 3M Textool 2xx-33xx pattern |
| Two 1 x 10 machined socket strips | DIP cartridge XU6 | ZIP-20 position |
| 1 x 8 socket for the display, SWD header, JTAG header, buzzer | Base | |
| Seeed XIAO ESP32-C6 on two 1 x 7 pin headers | Base J8, back of the board | Optional. See [[guides/fit-wifi-module]] |

## Other hardware

| Item | Quantity | Use |
| --- | --- | --- |
| 2.0 in 240 x 320 ST7789 SPI display with an 8-pin header | 1 | Plugs into J5 |
| M3 x 16 thread-forming screw | 4 | Down through each rail and the base board into the tray |
| M3 x 16 screw | 2 | Up through the tray and the board into the console posts |
| M3 x 6 screw | 4 per cartridge | Shell to cartridge board |
| USB-C PD charger, 9 V at 2 A or more | 1 | Power |

The display header order is GND, VCC, SCL, SDA, RES, DC, CS, BLK. Sellers use different pin orders, so compare the module with J5 before you solder.

## Enclosure

Print these parts in PETG or ABS with 0.2 mm layers and no supports.

| File in `enclosure/out/` | Quantity |
| --- | --- |
| `tray.stl` | 1 |
| `console.stl` | 1 |
| `rail_left.stl`, `rail_right.stl` | 1 each |
| `keycap.stl` | 6 |
| `cartridge_simm.stl`, `cartridge_dip.stl` | 1 per cartridge |

`tray_45.stl` and `console_45.stl` are the same parts tilted back 45 degrees, for a textured finish on the top face.

## Assemble

1. Put the base board into the tray.
2. Screw a rail down along each side of the bay with two M3 x 16 thread-forming screws.
3. Drop the six keycaps into the console from below.
4. Plug the display onto its header.
5. Put the console on and fit two M3 x 16 screws from below.
6. Screw each shell to its cartridge board with four M3 x 6 screws.
7. Label each cartridge "Not for a PC slot".

A cartridge slides in from the rear, sockets up, until the gold fingers seat.

:::danger Keep cartridges out of a PC
A cartridge fits a PCI Express x16 slot. Its DUT supply contacts would see 12 V there. Never put a cartridge in a PC, and never put a PCI Express card in the tester.
:::

:::warning Switch the DUT supply off first
Switch the DUT supply off before you insert or remove a cartridge.
:::

:::see-also
- [[guides/bring-up]]
- [[guides/fit-wifi-module]]
:::
