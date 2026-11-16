.. raw:: html

   <p align="right">
     <picture>
       <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/belikh/design/latest/packages/graphics/logo/jupiter-logo-dark.svg">
       <img alt="jupiter.au" src="https://raw.githubusercontent.com/belikh/design/latest/packages/graphics/logo/jupiter-logo-light.svg" width="260">
     </picture>
   </p>

   <h1>RAMalyser</h1>

.. meta::
   :description: RAMalyser tests 72-pin and 30-pin SIMMs, SIPPs and DIP and ZIP DRAM chips from machines of the 1970s to 1990s.

RAMalyser tests the memory used in machines of the 1970s to 1990s. It is a new design built from current parts.

`Documentation`_ · `Memory types`_ · `Contents`_ · `Build and check`_ · `Licence`_

.. raw:: html

   <table>
     <tr align="left"><th>Warning</th></tr>
     <tr><td>

Revision A has never been built. The boards pass electrical and design-rule checks and the logic passes in simulation. No hardware exists and nothing has been measured. Read `Bring up a new unit <docs/guides/bring-up.md>`_ before you order anything.

.. raw:: html

   </td></tr>
   </table>

|

.. raw:: html

   <p align="center"><picture><source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/rule/end-dark.svg"><img alt="" src="https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/rule/end-light.svg" width="50%"></picture></p>

|

Documentation
-------------

The guides and references are at `docs.jupiter.au/ramalyser <https://docs.jupiter.au/ramalyser/>`_ and, as plain Markdown, in `docs/ <docs/README.md>`_.

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Page
     - Contents
   * - `Test a module <docs/guides/test-a-module.md>`_
     - Fit a cartridge and a module, run a test and read the result
   * - `Console commands <docs/reference/console.md>`_
     - Every command, the report format and each message
   * - `Use the web interface <docs/guides/use-the-web-interface.md>`_
     - Run tests from a browser, over Wi-Fi or USB
   * - `Build a unit <docs/start/build.md>`_
     - Boards, fabrication limits, hand-fitted parts, enclosure and assembly
   * - `Bring up a new unit <docs/guides/bring-up.md>`_
     - The checks to make before any memory goes into a socket
   * - `Supported memory <docs/reference/memory.md>`_
     - Modules and chips, and what a test reports
   * - `Cartridge bus <docs/reference/bus.md>`_
     - The 164-contact connector and the rules for a cartridge
   * - `Architecture <docs/concepts/architecture.md>`_
     - The timing engine, the cartridges, power and firmware

|

Memory types
------------

- 72-pin SIMMs: FPM and EDO, parity and non-parity, one or two banks
- 30-pin SIMMs and SIPPs
- DIP and ZIP DRAM chips: 4116 (with its -5 V and +12 V rails), 4164, 41256, 4416, 4464, 411000, 44256/514256, 514400

Each memory family has a slide-in cartridge. The cartridge seats in a card-edge slot under the console.

The tester identifies the module (size, width, FPM or EDO) and checks its wiring. It measures the access time in nanoseconds. It runs pattern, march, supply-margin and retention tests, and reports each failure by address and data bit.

|

.. raw:: html

   <p align="center"><picture><source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/rule/end-dark.svg"><img alt="" src="https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/rule/end-light.svg" width="50%"></picture></p>

|

Contents
--------

.. |code-l| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/code-light.svg#gh-light-mode-only
   :width: 20
.. |code-d| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/code-dark.svg#gh-dark-mode-only
   :width: 20
.. |wifi-l| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/wifi-light.svg#gh-light-mode-only
   :width: 20
.. |wifi-d| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/wifi-dark.svg#gh-dark-mode-only
   :width: 20
.. |file-l| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/file-light.svg#gh-light-mode-only
   :width: 20
.. |file-d| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/file-dark.svg#gh-dark-mode-only
   :width: 20
.. |cpu-l| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/cpu-light.svg#gh-light-mode-only
   :width: 20
.. |cpu-d| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/cpu-dark.svg#gh-dark-mode-only
   :width: 20
.. |rust-l| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/rust-light.svg#gh-light-mode-only
   :width: 20
.. |rust-d| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/rust-dark.svg#gh-dark-mode-only
   :width: 20
.. |image-l| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/image-light.svg#gh-light-mode-only
   :width: 20
.. |image-d| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/image-dark.svg#gh-dark-mode-only
   :width: 20
.. |topology-l| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/topology-light.svg#gh-light-mode-only
   :width: 20
.. |topology-d| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/topology-dark.svg#gh-dark-mode-only
   :width: 20
.. |book-l| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/book-light.svg#gh-light-mode-only
   :width: 20
.. |book-d| image:: https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/icon/book-dark.svg#gh-dark-mode-only
   :width: 20

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Directory
     - Contents
   * - |file-l| |file-d| ``spec/``
     - The electrical and mechanical interface as data: the base-to-adapter bus, socket pin maps, FPGA ball assignment and enclosure dimensions
   * - |cpu-l| |cpu-d| ``gateware/``
     - ECP5 timing engine in Verilog, a simulation suite with a fault-injecting DRAM model, and the bitstream build
   * - |rust-l| |rust-d| ``firmware/``
     - ``rt-core`` (test logic, console and front-panel screens, tested on a PC), ``rt-hal`` (board drivers), ``rt-cosim`` (the firmware running against the real gateware in simulation) and ``rt-fw`` (RP2350 binary, compiles, not yet run)
   * - |code-l| |code-d| ``web/``
     - The web interface, served by the Wi-Fi module or used over USB from Chrome
   * - |wifi-l| |wifi-d| ``wifi/``
     - Firmware for the optional Wi-Fi module (ESP32-C6, not yet compiled)
   * - |image-l| |image-d| ``enclosure/``
     - Parametric 3D-printed case (build123d) with STEP and STL output
   * - |topology-l| |topology-d| ``hardware/schematics/``
     - Schematic PDFs of the base unit and the cartridges
   * - |topology-l| |topology-d| ``hardware/spice/``
     - ngspice simulation of the DUT supply
   * - |book-l| |book-d| ``docs/``
     - Generated documentation. The source is ``docs-src/``

PCB layouts and fabrication files are not published.

|

.. raw:: html

   <p align="center"><picture><source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/rule/end-dark.svg"><img alt="" src="https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/rule/end-light.svg" width="50%"></picture></p>

|

Build and check
---------------

The build needs Python 3, Icarus Verilog, OSS CAD Suite (yosys, nextpnr-ecp5, ecppack, verilator), a C++ compiler, ngspice, Rust and uv.

.. code-block:: sh

   python3 spec/ramspec.py --check                      # bus and pin maps
   gateware/sim/run.sh                                  # 13 simulation scenarios
   gateware/build.sh                                    # bitstream, fails on timing
   (cd firmware && cargo test --features sim)           # 32 tests of the firmware logic
   (cd firmware/rt-cosim && cargo test)                 # 17 scenarios: firmware on the real gateware
   hardware/spice/run.sh                                # DUT supply checks
   (cd enclosure && uv venv && uv pip install -e . && .venv/bin/python enclosure.py --check)

The enclosure build fetches the logo from the design-system release with ``gh``. If ``gh`` has no access, the build makes the case with a blank top.

|

.. raw:: html

   <p align="center"><picture><source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/rule/end-dark.svg"><img alt="" src="https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/rule/end-light.svg" width="50%"></picture></p>

|

Licence
-------

You may use, copy and change the project for non-commercial purposes. Commercial sale is reserved to the copyright holder. See `LICENSE.md <LICENSE.md>`_ and `CLA.md <CLA.md>`_.

|

.. raw:: html

   <p align="center"><picture><source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/rule/end-dark.svg"><img alt="" src="https://raw.githubusercontent.com/belikh/design/latest/packages/github/graphics/rule/end-light.svg" width="50%"></picture></p>

|

In memory of David "Dubi" Feinstein (1954-2019) and his Innoventions RAMCHECK and SIMCHECK II testers.

.. raw:: html

   <p align="right"><sub>jupiter.au</sub></p>
