---
title: RAMalyser
summary: A tester for the SIMM, SIPP, DIP and ZIP memory of machines from the 1970s to the 1990s.
---

RAMalyser identifies a memory module, checks its wiring, measures its access time in nanoseconds and runs pattern, march, supply-margin and retention tests. It reports each failure by address and data bit.

A [[term:Base unit]] holds the processor, the [[term:Timing engine]] and the power supplies. Each memory family has a [[term:Cartridge]] that slides into the base unit from the rear.

:::warning Revision A has never been built
The boards pass electrical and design-rule checks and the logic passes in simulation. No hardware exists and nothing has been measured. Work through [[guides/bring-up]] before you fit a memory module.
:::

## Where to go

| You want to | Read |
| --- | --- |
| Test a memory module | [[guides/test-a-module]] |
| Test from a browser | [[guides/use-the-web-interface]] |
| Look up a screen, a key or a message | [[reference/front-panel]] and [[reference/console]] |
| Order the boards, print the case and put a unit together | [[start/build]] |
| Power a new unit for the first time | [[guides/bring-up]] |
| Add Wi-Fi | [[guides/fit-wifi-module]] |
| See which modules and chips it tests | [[reference/memory]] |
| Design a cartridge | [[reference/bus]] |
| Understand how it measures | [[concepts/architecture]] |
| Run the checks on the source | [[develop/check]] |
