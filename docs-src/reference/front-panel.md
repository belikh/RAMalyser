---
title: Front panel
summary: The five screens of the display, what each of the six keys does on each screen, and the LEDs and the buzzer.
order: 12
icon: monitor
audience: users
status: draft
---

The display is 320 x 240 pixels. Every screen has the name `RAMalyser` at the top left, the screen name at the top right and the active keys on the bottom two lines.

## Keys

| Screen | ESC | F1 | F2 | F3 | F4 | F5 |
| --- | --- | --- | --- | --- | --- | --- |
| Select | | Up | Down | Quick test | Full test | About |
| Testing | | | | | | |
| Result | Back to Select | Run again | Details, after a `FAIL` | | | |
| Details | Back to Result | | | | | |
| About | Back to Select | | | | | |

The list wraps: F1 on the first profile moves to the last one.

## Screens

| Screen | Shows |
| --- | --- |
| Select | The seven profiles. A bar marks the chosen one. The 4116 line carries `-5 V +12 V` |
| Testing | The profile and the word `Testing`. No key works until the test ends |
| Result | `PASS`, `FAIL` or `NO DEVICE`, then the profile, the first fault if there is one, the size, the mode with access time and current, and the retention after a full test |
| Details | The supply fault, each wiring fault by line name, and for a memory fault the phase, error count, first address, bad bits, data read and the failing data lines |
| About | Firmware version, engine ID, whether the engine answers, the tick period and the number of profiles |

A test started from the USB console or the web interface also shows on the display.

## Result lines

```text title="A memory fault"
FAIL
72-pin SIMM
Pattern: 2 errors
4 MB  1024K x36
FPM  70 ns  120 mA
```

```text title="Its details"
72-pin SIMM
Phase     Pattern
Errors    2
First at  0x1234
Bad bits  0x0000020020
Read      0xf555575555
Lines     DQ5 DQ17
```

## LEDs and buzzer

| Part | Behaviour |
| --- | --- |
| Green LED | On while a `PASS` is on the display |
| Red LED | On while a `FAIL` is on the display |
| Module-power LED | On while the DUT supply is on. It is wired to the supply and firmware does not control it |
| Buzzer | One beep after a pass. Three beeps after a fail or no device |

## Type and colour

The display uses JetBrains Mono at 14, 20 and 40 pixels and the dark colour roles of the Jupiter design system. `firmware/update-design.sh` generates both from the design system release.

:::see-also
- [[guides/test-a-module]]
- [[reference/console]]
:::
