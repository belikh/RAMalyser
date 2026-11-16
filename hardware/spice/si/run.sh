#!/bin/sh
# Signal integrity of the bus lines, FPGA to memory socket and back.
# One PASS, FAIL or INFO line per check; exit status 1 if any FAIL.
#
# Line lengths come from lengths.json, taken from the routed boards.
# The level shifter and the receive buffer use Texas Instruments' IBIS
# models, fetched into models/ on the first run; without them a generic
# driver is used and the output says so. The FPGA pad and the DRAM are
# generic models. See the ASSUMED lines and LIMITS OF THE METHOD in si.py.
# Takes a few minutes: about 450 ngspice runs.
set -eu
cd "$(dirname "$0")"
command -v ngspice >/dev/null || { echo "ngspice not found"; exit 2; }
./fetch.sh | sed 's/^/INFO  fetch: /'
exec python3 -I si.py
