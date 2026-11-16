---
title: Use the web interface
summary: Run tests from a browser, over the Wi-Fi module or over a USB cable, and see each result with its history.
order: 8
icon: globe
audience: users
status: draft
---

The page works in two ways. With the Wi-Fi module fitted, any browser on your network opens it. Without the module, Chrome or Edge on a computer talks to the tester over USB.

:::warning Not run on hardware
The page runs against a software model of the tester. The Wi-Fi module firmware has not been compiled or run.
:::

## Over Wi-Fi, first time

1. Fit the module and load its firmware. See [[guides/fit-wifi-module]].
2. Power the tester. The module opens a network named `RAMalyser-` and four characters.
3. Join that network from a phone or a computer. The page opens, or open `http://192.168.4.1/`.
4. Scroll to **Wi-Fi**, enter your network name and password, and select **Save and restart**.
5. Join your own network again and open `http://ramalyser.local/`.

If the module cannot join your network within 20 seconds, it opens its own network again.

:::danger The module has no password
Anyone on the same network can run tests and change the Wi-Fi settings. The module's own network is open. Use it on a network you trust.
:::

## Over USB

1. Build the page. See [[develop/check#web-interface]].
2. Serve the page from your computer.

```sh title="Serve the built page"
python3 -m http.server --directory web/dist 8000
```

3. Open `http://localhost:8000/` in Chrome or Edge.
4. Connect the tester with a USB-C cable.
5. Select **Connect over USB** and choose `RAMalyser console`.

## Run a test

1. Choose the memory under **Memory**.
2. Select **Quick test** or **Full test**.
3. Wait for the result. The buttons are off while a test runs.

The page shows `PASS`, `FAIL` or `NO DEVICE` with the size, address bits, mode, access time, supply current and retention. After a fail it names the fault and lists the failing data lines. **This session** keeps one row per test until you close the page.

## Messages

| Message | Cause | Fix |
| --- | --- | --- |
| `Looking for the tester` | The page is starting | Wait a moment |
| `No tester connected` | The page is not served by a module | Select **Connect over USB** |
| `This browser cannot use USB serial` | The browser has no Web Serial | Use Chrome or Edge on a computer, or open the module's address |
| `The tester does not answer` | The link between the module and the tester is broken, or the tester is off | Check the module's seating and power-cycle the tester |
| `Lost the tester: the tester is busy with another test` | Someone else started a test | Wait for it to end |
| `Could not open the port` | Another program has the serial port | Close the terminal program |

:::see-also
- [[reference/web-api]]
- [[guides/test-a-module]]
:::
