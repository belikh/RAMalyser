# Building a unit

Revision A is unbuilt: treat every quantity and part below as a starting
point and correct it against the first build.

## Boards

| Board | Layers | Size | Assembly |
|---|---|---|---|
| Base | 6 | 160 x 150 mm | SMD both sides (16 FPGA decoupling capacitors are on the bottom: 12 under the package, 4 just outside it); BGA, so X-ray inspection |
| SIMM cartridge | 4 | 150 x 100 mm plus an 84.3 x 8.4 mm tongue | SMD top side only |
| DIP/ZIP cartridge | 4 | as above | SMD top side only |

Fabrication notes:

- Base: stack-up top / ground / signal / signal / +3V3 / bottom. Tracks are
  0.127 mm with 0.1 mm clearance, and the rules allow the router to neck
  down to 0.09 mm; 0.45 mm vias with 0.2 mm drills (needed to escape the
  0.8 mm BGA); 1.6 mm, ENIG. Check these against the fabricator's
  multilayer limits before ordering.
- Cartridges: stack-up top / ground / signal + VDUT / bottom. Tracks are
  0.15 mm with necks down to 0.1 mm, 0.127 mm minimum clearance, copper no
  closer than 0.2 mm to the board edge, 0.5 mm vias with 0.25 mm drills.
- The series resistor arrays are 4 x 0603 (0.8 mm pitch, 3.2 x 1.6 mm), not
  the smaller 4 x 0402 type. They have no LCSC number filled in; each
  board's fab report lists every part that still needs one.
- Cartridges: **1.6 mm thick, hard gold on the edge fingers, 20 degree bevel
  on the tongue.** Order them as "gold fingers" boards. The key notch
  between positions 11 and 12 is part of the outline.

## Fitted by hand

These are through-hole and are not in the machine-placement files (see each
board's `hand-solder.csv`):

| Part | Where | Notes |
|---|---|---|
| Right-angle card-edge socket, 164 contacts (Samtec PCIE-164-02-F-D-RA) | base J1 | mouth towards the rear |
| 72-pin SIMM socket | SIMM cartridge XU1 | check the footprint against the part first |
| 30-pin SIMM socket | SIMM cartridge XU2 | new-old-stock only; check the footprint |
| 1x30 machined socket strip, 2.54 mm | SIMM cartridge XU3 | for SIPP modules |
| ZIF sockets, 16, 18 and 20 pin, 0.3 in | DIP cartridge XU1-XU5 | 3M Textool 2xx-33xx pattern |
| 2 x 1x10 machined socket strip | DIP cartridge XU6 | ZIP-20 position |
| 1x8 socket for the display, SWD and JTAG headers, buzzer | base | |

## Other hardware

- 2.0 in 240x320 ST7789 SPI display module with an 8-pin header
  (GND, VCC, SCL, SDA, RES, DC, CS, BLK). Pin order varies between sellers:
  check it against J5 before soldering.
- 4 x M3 x 16 thread-forming screws: down through each rail and the base
  board into the tray.
- 2 x M3 x 16 from below, through the tray and board into the console posts.
- 4 x M3 x 6 per cartridge: shell to board (self-tapping into the posts, or
  nuts under the board).
- USB-C PD charger that offers 9 V at 2 A or more.

## Enclosure

Printed parts: `tray`, `console`, `rail_left`, `rail_right`, six `keycap`,
and one `cartridge_*` shell per adapter. PETG or ABS, 0.2 mm layers, no
supports. See the header of `enclosure/enclosure.py` for print orientations,
including the 45 degree option for a textured finish.

Assembly order: board into the tray; rails screwed down along the sides of
the bay; keycaps into the console from below; display onto its header;
console on; two screws from below. Screw each shell to its adapter board.

A cartridge slides in from the rear, sockets up, until the gold fingers
seat. **Switch the DUT supply off before inserting or removing one.** Label
each cartridge: it fits a PC's PCIe slot mechanically and must never go in
one.

## Order of first power-up

Follow `bring-up.md`. Do not fit a memory module until the DUT supply has
been swept and the over-voltage cut-off has been seen to work.
