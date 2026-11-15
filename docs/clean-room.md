# Clean-room record

The tester does for asynchronous DRAM what earlier commercial memory testers
did, as described in their published manuals, without using any of their
implementation.

## What was used

- An earlier manufacturer's manuals (archived copies) and published teardown notes,
  as a description of behaviour: test phases, what is detected, what is
  measured and how results are reported.
- Public standards and datasheet facts: SIMM pinouts, presence-detect codes,
  DRAM timing parameters, march test algorithms from the literature.
- Published patents, for the EDO/FPM detection method (sample after CAS
  rises).

The full list with URLs is kept in the private repository.

## What was not used

- No firmware, PAL/CPLD contents or PC software of any earlier tester was downloaded,
  read, disassembled or run. Dumps exist on archive.org; they were left alone.
- No schematic of the original exists publicly, and none was reconstructed
  from a unit. The electronics here are a new design around current parts.
- No text from the manuals is reproduced in this repository.
- No files were taken from other testers' repositories. Two open DIP-chip
  testers were read for ideas (one socket per chip family; retention
  testing); both are share-alike licensed, so nothing was copied.

## Rule for contributors

If you have studied the original firmware or logic, do not contribute test
algorithms, timing tables or UI text here.
