---
title: Test a module
summary: Fit a cartridge and a memory module, run a test from the front panel and read the result.
order: 5
icon: play
audience: users
status: draft
---

:::warning Revision A has never been built
These steps follow the firmware as written. Nobody has run them on hardware. The firmware logic runs against a software model of the tester.
:::

## Fit the cartridge and the module

1. Check that the module-power LED is off.
2. Slide the [[term:Cartridge]] in from the rear, sockets up, until the gold fingers seat.
3. Put the module or chip in its socket. [[reference/memory]] lists the socket for each type.
4. Connect the tester to a USB-C PD charger.

:::danger Check the orientation of a chip
A chip inserted backwards draws a large current. Match pin 1 of the chip to the pin 1 mark of the socket before you close the lever.
:::

:::note 5 V memory needs a 9 V port
The tester asks the USB-C port for 9 V. A port that gives only 5 V cannot bring the DUT rail to 5 V, so 5 V memory cannot be tested from it.
:::

## Run a test

The display lists the memory profiles. The bottom two lines show what each key does.

1. Press F1 or F2 to move to the profile that matches your memory.
2. Press F3 for the quick test or F4 for the full test.
3. Wait while the display shows `Testing`. Keep the module in its socket.

| Test | Checks |
| --- | --- |
| Quick | Wiring, identification, access time, every cell written and verified twice |
| Full | The quick test, then March C-, checkerboard, address-in-data, supply margins, voltage bounce and retention |

## Read the result

| Display | LED | Buzzer | Meaning |
| --- | --- | --- | --- |
| `PASS` | Green | One beep | The memory passed. The lines below give its size, mode, access time and current |
| `FAIL` | Red | Three beeps | Something is wrong. The second line names the first fault |
| `NO DEVICE` | Off | Three beeps | No memory answers in that socket |

```text title="A pass after a full test"
PASS
72-pin SIMM
4 MB  1024K x36
FPM  70 ns  120 mA
Holds 512 ms
```

1. After a `FAIL`, press F2 for the details: the phase, the error count, the first failing address and the failing data lines.
2. Press F1 to run the same test again.
3. Press ESC to go back to the list.

[[reference/front-panel]] describes every screen.

## Remove the module

1. Wait for the result. The DUT supply is off when the module-power LED is off.
2. Take the module out.
3. Pull the cartridge out to the rear if you want to change it.

:::see-also
- [[guides/use-the-web-interface]]
- [[reference/front-panel]]
- [[reference/console]]
:::
