---
title: Wi-Fi module connector
summary: The pins, supply and mechanical position of J8, the Seeed XIAO position on the back of the base board.
order: 30
icon: wifi
audience: developers
status: draft
---

J8 takes a Seeed XIAO module. Every XIAO has the same footprint. The ESP32-C6 is the intended module.

## Signals

| RP2350 pin | Net | XIAO pin |
| --- | --- | --- |
| GPIO0, UART0 TX | `WIFI_RX` | D7, RX |
| GPIO1, UART0 RX | `WIFI_TX` | D6, TX |
| GPIO2 | `WIFI_IO0` | D0 |
| GPIO3 | `WIFI_IO1` | D1 |

D2 to D5 and D8 to D10 are not connected.

## Supply

| XIAO pin | Connection |
| --- | --- |
| 5V | The 5 V rail through a Schottky diode (D5), with 10 µF to ground |
| 3V3 | Not connected. It is the output of the module's own regulator |
| GND | Ground |

The diode stops a cable in the module's USB port from driving the 5 V rail of the tester.

## Position

| Item | Value |
| --- | --- |
| Side | Back of the base board |
| Holes | Two rows of 7, 2.54 mm pitch, rows 15.24 mm apart |
| Pad 1 (D0) | x 112.38 mm, y 18.24 mm from the rear-left corner, seen from the top |
| Antenna end | Rear edge of the board |
| Copper under the antenna | None on any layer, 12.4 x 5.0 mm |
| Tray opening | 21 x 48 mm |

`spec/mech.py` holds these numbers as `XIAO_PIN1`, `XIAO_ROW_SPACING`, `XIAO_BODY`, `XIAO_ANTENNA`, `XIAO_HEIGHT` and `XIAO_WINDOW`.

:::see-also
- [[guides/fit-wifi-module]]
- [[concepts/architecture]]
:::
