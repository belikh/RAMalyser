# wifi: firmware for the Wi-Fi module

**Status: written, not compiled, not run.** No ESP32 toolchain was on the
development machine and no module has been fitted. Expect to fix details on
the first build.

The module is a Seeed XIAO ESP32-C6 on J8 of the base board. It serves the
web interface in `../web` and passes commands to the tester over a serial
link. The documentation has the steps: `docs/guides/use-the-web-interface.md`
and `docs/reference/web-api.md`.

    python3 ../web/build.py     # makes src/web_assets.h from the page
    pio run -t upload           # through the module's own USB-C port

The build needs PlatformIO with the pioarduino platform (see
`platformio.ini`).
