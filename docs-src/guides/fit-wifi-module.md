---
title: Fit the Wi-Fi module
summary: Solder a Seeed XIAO ESP32-C6 to the back of the base board so the tester can join a network.
order: 30
icon: wifi
audience: users
status: draft
---

The module is optional. Without it the tester works from its front panel and over USB.

## Parts

| Part | Quantity |
| --- | --- |
| Seeed XIAO ESP32-C6 | 1 |
| 1 x 7 pin header, 2.54 mm | 2 |

## Load the firmware

The firmware is in `wifi/`. It has not been compiled or run yet.

1. Install PlatformIO.
2. Build the page into the firmware.

```sh title="Build the page"
python3 web/build.py
```

3. Connect the module to your computer with its own USB-C port.
4. Build and load the firmware.

```sh title="Build and load"
cd wifi
pio run -t upload
```

The module opens a network named `RAMalyser-` and four characters when it has no network saved.

## Solder the module

1. Load the firmware into the module first. See the next section.
2. Hold a XIAO against J8 on the back of the base board. The antenna end points to the rear edge and the USB-C end points to the front.
3. Check that pin D0 sits on the square pad. The two rows are mirrored on the board because the module mounts on the back.
4. Solder the two pin headers to the module.
5. Solder the header pins to J8 from the top of the board.
6. Put the board into the tray. The module sits in the opening in the tray floor.
7. Check that the module does not stand below the tray floor. The module is about 6.7 mm tall on pin headers and the space is 7.4 mm.

:::warning Measure before you solder
The 6.7 mm height is an estimate. Measure your module on its headers first.
:::

:::see-also
- [[guides/use-the-web-interface]]
- [[reference/wifi-module]]
- [[start/build]]
:::
