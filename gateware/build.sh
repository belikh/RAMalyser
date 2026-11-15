#!/bin/sh
# Build the ECP5 bitstream. Needs yosys, nextpnr-ecp5 and ecppack on PATH
# (OSS CAD Suite). Fails if 162.5 MHz timing is not met.
set -eu
cd "$(dirname "$0")"
[ -d "$HOME/.local/opt/oss-cad-suite/bin" ] && PATH="$HOME/.local/opt/oss-cad-suite/bin:$PATH"
mkdir -p build
python3 ../spec/ramspec.py --emit ../spec/generated >/dev/null
python3 ../spec/fpga_pins.py build/top.lpf
yosys -q -l build/yosys.log -p "read_verilog -I../spec/generated rtl/top_ecp5.v \
    rtl/tester_core.v rtl/spi_regs.v rtl/io_cells.v; synth_ecp5 -top top_ecp5 -json build/top.json"
# Placement is seed dependent; try a few seeds and keep the first that meets timing.
for seed in ${SEEDS:-1 2 3 4 5 6}; do
    nextpnr-ecp5 --25k --package CABGA256 --speed 6 --json build/top.json \
        --lpf build/top.lpf --textcfg build/top.config --freq 162.5 --timing-allow-fail \
        --seed "$seed" --report build/timing.json -l build/nextpnr.log -q 2>/dev/null
    if python3 -c "
import json,sys
r=json.load(open('build/timing.json'))['fmax']
sys.exit(0 if all(f['achieved']>=f['constraint'] for n,f in r.items() if 'clk25' not in n) else 1)"; then
        echo "seed $seed meets timing"; break
    fi
    echo "seed $seed misses timing"
done
ecppack --compress build/top.config build/top.bit
python3 - <<'EOF'
import json, sys
r = json.load(open("build/timing.json"))
for name, f in r["fmax"].items():
    if "clk25" in name:
        continue                     # input pin; only feeds the PLL
    print(f"{name}: {f['achieved']:.1f} MHz (need {f['constraint']:.0f})")
    if f["achieved"] < f["constraint"]:
        sys.exit("TIMING FAILED")
u = r["utilization"]
for k in ("TRELLIS_COMB", "TRELLIS_FF", "TRELLIS_IO", "IOLOGIC", "SIOLOGIC", "EHXPLLL"):
    if k in u:
        print(f"{k}: {u[k]['used']}/{u[k]['available']}")
EOF
ls -l build/top.bit
