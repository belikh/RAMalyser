---
title: Web interface and module API
summary: The requests the Wi-Fi module answers, the JSON form of a test report and the serial link between the module and the tester.
order: 35
icon: code
audience: developers
status: draft
---

## Requests

The module listens on port 80.

| Request | Body | Answer |
| --- | --- | --- |
| `GET /` and the page's files | None | The web interface, gzipped |
| `GET /api/state` | None | `{"busy":false,"result":null,"wifi":{"mode":"sta","ssid":"...","ip":"..."}}` |
| `POST /api/command` | `list`, `id`, `basic N` or `full N` | `202` and `{"accepted":true}`. `409` while a test runs. `400` for anything else |
| `POST /api/wifi` | Form fields `ssid` and `password` | `{"saved":true}`, then the module restarts |

A client posts a command, then reads `/api/state` until `busy` is false. `result` then holds the tester's answer. The module answers `{"ok":false,"error":"the tester did not answer"}` after 30 minutes without one.

`peek` and `poke` are not accepted over the network. They stay on the USB console.

`wifi.mode` is `sta` when the module has joined a network and `ap` when it has opened its own.

## Answers from the tester

Each `json` command gets one line.

| Command | Answer |
| --- | --- |
| `json list` | `{"ok":true,"profiles":[{"n":0,"name":"72-pin SIMM","aux_rails":false}, ...]}` |
| `json id` | `{"ok":true,"engine":"0x52414d31","engine_ok":true,"tick_ps":3077}` |
| `json basic 0`, `json full 0` | `{"ok":true,"test":"basic","report":{...}}` |
| Anything else | `{"ok":false,"error":"unknown command"}` or `{"ok":false,"error":"no such profile"}` |

## Report

```json title="json basic 0, a good 4 MB SIMM"
{"ok":true,"test":"basic","report":{"profile":"72-pin SIMM","passed":true,"verdict":"pass","no_device":false,"power_fault":false,"bits":37748736,"words":1048576,"width":36,"row_bits":10,"col_bits":10,"mode":"FPM","t_rac_ns":70,"retention_ms":null,"current_ma":120,"wire_faults":[],"failure":null}}
```

| Field | Type | Meaning |
| --- | --- | --- |
| `profile` | String | The profile name |
| `passed` | Boolean | True for a pass |
| `verdict` | String | `pass`, `fail` or `no_device` |
| `no_device`, `power_fault` | Boolean | Nothing answered; the supply tripped or did not come up |
| `bits`, `words`, `width` | Number | Capacity in bits, word count and data width. 0 when the size is unknown |
| `row_bits`, `col_bits` | Number | Address bits |
| `mode` | String or null | `FPM` or `EDO` |
| `t_rac_ns` | Number or null | Access time from RAS |
| `retention_ms` | Number or null | Longest pause without refresh that read back correctly. Full test only |
| `current_ma` | Number | Supply current |
| `wire_faults` | Array | `{"kind":"stuck_low","line":"MA3"}`, `{"kind":"stuck_high","line":"WE0"}` or `{"kind":"short","line":"CAS0","other":"CAS1"}` |
| `failure` | Object or null | `{"phase":"Pattern","errors":1,"addr":4660,"bits":"0x0000000020","read":"0xfaaaaaaa8a"}` |

`bits` and `read` inside `failure` are hexadecimal strings, because they hold 40 bits.

## Serial link

| Item | Value |
| --- | --- |
| Signals | Module D6 to RP2350 GPIO1, module D7 from RP2350 GPIO0 |
| Format | 115200 baud, 8 data bits, no parity, 1 stop bit |
| Request | `json`, a space, the command and a line feed |
| Answer | One line of JSON and a line feed |

The tester runs the same console on this link and on USB.

## Files

| Path | Contents |
| --- | --- |
| `web/src/` | The page: `index.html`, `app.js`, `app.css` |
| `web/build.py` | Fetches the design system release and builds `web/dist/` and `wifi/src/web_assets.h` |
| `web/dev.py` | Serves the page on a PC with a simulated tester behind it |
| `wifi/` | The module firmware, a PlatformIO project |

The page uses the Jupiter stylesheet, fonts and logo from the design system release. `web/build.py` downloads them at each build.

:::see-also
- [[guides/use-the-web-interface]]
- [[reference/console]]
- [[reference/wifi-module]]
:::
