---
title: Clean-room record
summary: What the design drew on, what it did not use, and the rule for anyone who contributes.
order: 20
icon: shield
audience: maintainers
status: stable
---

RAMalyser does for asynchronous DRAM what earlier commercial memory testers did, as their published manuals describe. It uses none of their implementation.

## Sources used

- An earlier manufacturer's manuals (archived copies) and published teardown notes. They describe behaviour: test phases, what is detected, what is measured and how results are reported.
- Public standards and datasheet facts: SIMM pinouts, presence-detect codes, DRAM timing parameters and march test algorithms from the literature.
- Published patents, for the EDO and FPM detection method (sample after CAS rises).

The private repository keeps the full list with URLs.

## Sources left alone

- Firmware, PAL or CPLD contents and PC software of any earlier tester. Nobody downloaded, read, disassembled or ran them. Dumps exist on archive.org.
- Schematics of an earlier tester. None is public, and nobody reconstructed one from a unit. The electronics are a new design around current parts.
- Text from the manuals. This repository reproduces none of it.
- Files from other testers' repositories. Two open DIP-chip testers gave ideas: one socket per chip family, and retention testing. Both have share-alike licences, so nobody copied from them.

## Rule for contributors

If you have studied the firmware or logic of an earlier tester, do not contribute test algorithms, timing tables or UI text.

:::see-also
- [[concepts/architecture]]
:::
