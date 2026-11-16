---
title: Build and check
summary: The tools to install and the five commands that check the spec, the gateware, the firmware logic and the enclosure.
order: 10
icon: terminal
audience: developers
status: stable
---

## Tools

| Tool | Used for |
| --- | --- |
| Python 3 | The spec and the enclosure |
| Icarus Verilog | Gateware simulation |
| OSS CAD Suite: yosys, nextpnr-ecp5, ecppack | The bitstream |
| Rust | Firmware |
| uv | The enclosure's Python environment |
| gh | Fetches the logo for the enclosure. If `gh` has no access, the build makes the case with a blank top |

## Check the spec

```sh title="Bus and pin maps"
python3 spec/ramspec.py --check
```

```text title="Output"
spec ok: 116 IO lines, 90 legacy roles, 8 socket maps, DIMM needs 109 lines
```

## Simulate the gateware

```sh title="13 scenarios"
gateware/sim/run.sh
```

Each scenario prints one line. The run takes several minutes.

```text title="Last lines of the output"
ok    speed_83: PASS: speed within one tick
ok    detect_fpm: PASS: mode detected
ok    detect_edo: PASS: mode detected
ok    retention: PASS: retention loss detected (64 errors)
```

A scenario that fails prints a line that starts with `FAIL`, and the script exits with status 1.

## Build the bitstream

```sh title="Bitstream"
gateware/build.sh
```

The script tries placement seeds until one meets timing and prints `seed N meets timing`. It fails if no seed reaches 162.5 MHz.

## Test the firmware logic

```sh title="32 tests"
cd firmware
./update-design.sh
cargo test --features sim
```

`update-design.sh` downloads the design system release and generates the display's palette and fonts into `rt-hal/src/generated/`. It needs `gh` and `uv`. Run it once, and again to take a new release.

```text title="Output"
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The three groups are the test algorithms, the JSON console and the front-panel screens. They run `rt-core` against a software model of the engine and a DRAM. `rt-fw`, the RP2350 binary, has not been compiled yet.

## Preview the display

```sh title="Draw every screen to a PPM file"
cd firmware
cargo run -p rt-hal --features host --example lcd_preview -- /tmp/lcd
```

```text title="Output"
1-select
2-testing
3-pass
4-fail
5-details
6-no-device
7-about
```

The files use the same fonts and colours as the display.

## Web interface

```sh title="Build the page and try it against the simulated tester"
python3 web/build.py
python3 web/dev.py 8080
```

```text title="Output"
14 files in web/dist, 140129 bytes gzipped in web_assets.h
http://localhost:8080/
```

`web/build.py` downloads the Jupiter stylesheet, fonts and logo with `gh`. `web/dev.py` answers the same requests as the Wi-Fi module and runs the firmware's console against its software model. Post `good`, `edo`, `empty`, `cell` or `short` to `/api/memory` to change the simulated memory.

## Wi-Fi module firmware

```sh title="Build with PlatformIO"
python3 web/build.py
cd wifi
pio run
```

This firmware has not been compiled yet.

## Build the enclosure

```sh title="Enclosure and fit checks"
cd enclosure
uv venv
uv pip install -e .
.venv/bin/python enclosure.py --check
```

```text title="Last line of the output"
enclosure checks passed
```

The command writes the STEP and STL files to `enclosure/out/`.

## Documentation

The pages are Markdown in `docs-src`. `jupiter-docs` from [belikh/docs](https://github.com/belikh/docs) generates `docs/*.md` and the site from them. Edit `docs-src` only.

```sh title="Check and build the pages"
git clone https://github.com/belikh/docs .jupiter-docs
(cd .jupiter-docs && npm ci && npm run update:design)
export JUPITER_DESIGN_DIR="$PWD/.jupiter-docs/vendor/design"
node .jupiter-docs/packages/docs/bin/jupiter-docs.mjs check
node .jupiter-docs/packages/docs/bin/jupiter-docs.mjs build
```

```text title="Output of the check"
16 page(s) OK
```

Commit `docs-src` and the regenerated `docs` together. `.jupiter-docs` and `dist` are in `.gitignore`.

## Workflows

| Workflow | Runs on | Checks |
| --- | --- | --- |
| `docs.yaml` | Push to `main` and pull request that touch the pages | `jupiter-docs check`, `build --check` and the site build |

:::see-also
- [[concepts/architecture]]
- [[start/build]]
:::
