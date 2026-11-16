---
title: Supported memory
summary: The modules and chips RAMalyser tests, the cartridge and socket each one uses, and what a test reports.
order: 10
icon: memory
audience: users
status: draft
---

## Modules and chips

| Memory | Cartridge | Socket |
| --- | --- | --- |
| 72-pin SIMM: FPM and EDO, parity and non-parity, one or two banks, 5 V and 3.3 V | SIMM | XU1 |
| 30-pin SIMM | SIMM | XU2 |
| 30-pin SIPP | SIMM | XU3 |
| 4116, with its -5 V and +12 V rails | DIP and ZIP | ZIF |
| 4164, 41256 | DIP and ZIP | ZIF |
| 4416, 4464 | DIP and ZIP | ZIF |
| 411000 | DIP and ZIP | ZIF |
| 44256, 514256, 514400 | DIP and ZIP | ZIF, and ZIP-20 strips for ZIP packages |

Some DIP and ZIP pin maps are marked `UNVERIFIED` in `spec/ramspec.py`. Check them against a datasheet before you trust a result.

## What a test reports

| Step | Result |
| --- | --- |
| Identify | Size, width, FPM or EDO |
| Wiring | Each stuck, shorted or missing line, by name |
| Access time | Nanoseconds, to one [[term:Tick]] with interpolation |
| Pattern and march | First failing address, the data read there, every failing bit |
| Supply margin | The same tests at a low and a high [[term:VDUT]] |
| Retention | Errors after a pause without refresh |

## Not supported

168-pin SDRAM DIMMs and ECC-optimised 72-pin SIMMs are not supported. See [[guides/bring-up#limits-of-revision-a]].

:::see-also
- [[reference/bus]]
- [[concepts/architecture]]
:::
