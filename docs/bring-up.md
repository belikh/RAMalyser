# Bring-up and bench validation

Revision A has never been built. The boards are netlist-correct and pass
design-rule checks; the gateware and test logic pass in simulation. Nothing
here has been measured. Build five sets, not a batch, and work through this
list. Items marked **must** gate everything after them.

## Before ordering

- **must** Print each socket footprint 1:1 and offer up the real part:
  72-pin SIMM, 30-pin SIMM, ZIF sockets, ZIP strips. The SIMM and ZIP
  patterns were drawn from published dimensions, not from the datasheet of
  the socket you will buy.
- **must** Check the DIP and ZIP pin maps flagged in `spec/ramspec.py`
  (`UNVERIFIED`) against manufacturer datasheets.
- Confirm every LCSC number and stock level in the assembler's BOM tool, and
  the part rotations in its placement preview.
- Check the card-edge socket footprint against the socket bought (the
  pattern is from Samtec's drawing for PCIE-164-02-x-D-RA) and that the
  cartridge sits 5.0 mm above the base when seated.
- Order cartridges with hard-gold fingers and a bevelled tongue.
- If you fit the Wi-Fi module: check J8 against a real XIAO before
  soldering (the rows are mirrored because it mounts on the back), and that
  it clears the tray opening.
- Confirm the PD controller fitted (CH224A) matches the CH224K pinout the
  schematic assumes.
- Confirm the fabricator's limits for 0.2 mm drills / 0.45 mm vias and
  0.09 mm minimum track and gap on the 6-layer base, and 0.1 mm track /
  0.127 mm gap on the 4-layer cartridges.
- Fill in the parts listed without an LCSC number in each board's
  `fab-report.txt` (the 4 x 0603 resistor arrays among them).

## Base board, unpowered

- Resistance from each rail to ground: VBUS, +3V3, +5V, +1V1, +2V5, RP_1V1,
  VDUT. No shorts.

## Base board, first power (bench supply, 100 mA limit, 5 V on VBUS)

- +3V3, +1V1, +2V5, RP_1V1 in range; +5V follows the input.
- USB enumerates in BOOTSEL; load firmware; the FPGA configures (DONE high,
  ID register reads 0x52414D31, PLL locked).
- With a PD charger: VBUS goes to 9 V; try several chargers and a 5 V-only
  source.

## DUT supply (no adapter, then a dummy load)

- Sweep the DAC: output 2.5 to 5.8 V, monotonic; record the table.
- Without `STRAP_5V_OK` grounded the switch drops out above about 3.85 V;
  with it grounded, above about 5.86 V.
- Ripple and step response at 0, 1 and 2 A; current reading against a meter;
  trip time into a short; discharge time; switch temperature at 2 A.

## Timing

- Loopback (no DUT): launch-to-capture delay per line at 3.0, 5.0 and 5.5 V.
  This is the calibration table; store it in the adapter EEPROM.
- Measure the input-delay step size against one tick.
- Scope RAS to data-valid on a known module and compare with the reported
  access time. Target is +/-3 ns; this is a target, not a result.
- Edge rates and ground bounce with 36 DQ switching.

## Memory

- Known-good and known-bad parts for every socket: sizes, parity and
  non-parity, FPM and EDO, 5 V and 3.3 V SIMMs, each DIP family.
- 4116: scope the three rails at power-on and power-off; pull the -5 V pump
  and check +12 V and +5 V stay off.
- A chip inserted backwards: current limit trips, nothing overheats.
- Thirty minutes of looped testing at the highest current; repeatability
  across units.

## Cartridge slot

- Insertion force, alignment on the rails, and that the shell clears the
  console when seated.
- Contact resistance on the DUT-rail contacts at 2 A; temperature rise.
- Behaviour if a cartridge is pulled with the DUT supply on (it should not
  be; the firmware must drop the rail when ADP_PRESENT_N rises).

## Not designed for yet

- 168-pin SDRAM DIMMs (the base has the lines and the SPD bus; the adapter
  and the synchronous engine are future work).
- ECC-optimised 72-pin SIMMs (different pinout).
- Access time from CAS and page-mode cycle time are not measured separately.
- No EMC work has been done. A product sold assembled needs it.
