---
title: Fit the Wi-Fi module
summary: Solder a Seeed XIAO ESP32-C6 to the back of the base board so the tester can join a network.
order: 30
icon: wifi
audience: users
status: draft
---

The module is optional. Without it the tester works over USB. No firmware uses the module yet.

## Parts

| Part | Quantity |
| --- | --- |
| Seeed XIAO ESP32-C6 | 1 |
| 1 x 7 pin header, 2.54 mm | 2 |

## Steps

1. Hold a XIAO against J8 on the back of the base board. The antenna end points to the rear edge and the USB-C end points to the front.
2. Check that pin D0 sits on the square pad. The two rows are mirrored on the board because the module mounts on the back.
3. Solder the two pin headers to the module.
4. Solder the header pins to J8 from the top of the board.
5. Put the board into the tray. The module sits in the opening in the tray floor.
6. Check that the module does not stand below the tray floor. The module is about 6.7 mm tall on pin headers and the space is 7.4 mm.

:::warning Measure before you solder
The 6.7 mm height is an estimate. Measure your module on its headers first.
:::

:::see-also
- [[reference/wifi-module]]
- [[start/build]]
:::
