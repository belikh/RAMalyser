#!/bin/sh
# Gate-level regression: scenarios of run.sh against the synthesised
# ECP5 netlist of the whole top level (PLL, reset, SPI, core, 116 pad cells)
# instead of the Verilog source.
#
# The netlist comes from the same yosys step as the bitstream build. LUTs,
# flip-flops, carry chains and I/O buffers use yosys's ECP5 simulation
# models. The four primitives yosys only has as black boxes (PLL, the two
# DDR cells and the input delay) use the stand-ins in ecp5_prims_sim.v: read
# its header for what they do not model. The clock is 162.5 MHz from the
# PLL, so one tick is 3.077 ns (2.5 ns in run.sh), and the "too slow" part
# is slower to stay outside the wider window.
set -eu
cd "$(dirname "$0")"
[ -d "$HOME/.local/opt/oss-cad-suite/bin" ] && PATH="$HOME/.local/opt/oss-cad-suite/bin:$PATH"
python3 ../../spec/ramspec.py --emit ../../spec/generated >/dev/null
INC=../../spec/generated
CELLS=$(yosys-config --datdir)/ecp5
mkdir -p build
yosys -q -l build/gate_yosys.log -p "read_verilog -I$INC ../rtl/top_ecp5.v ../rtl/tester_core.v \
    ../rtl/spi_regs.v ../rtl/io_cells.v; synth_ecp5 -top top_ecp5; \
    write_verilog -noattr build/top_gate.v"
echo "netlist: $(grep -c '^  LUT4 \|^  TRELLIS_FF \|^  CCU2C ' build/top_gate.v) logic cells," \
     "$(grep -c '^  ODDRX1F \|^  IDDRX1F \|^  DELAYF \|^  EHXPLLL ' build/top_gate.v) pad and clock primitives"
SRC="build/top_gate.v $CELLS/cells_sim.v ecp5_prims_sim.v dram_model.v tb_core.v"
names=""
run() {
    name=$1; shift
    names="$names $name"
    ( iverilog -g2012 -DGATE -I "$INC" -I "$CELLS" -s tb_core -o "build/gate_$name.vvp" "$@" $SRC &&
      vvp -n "build/gate_$name.vvp" ) > "build/gate_$name.log" 2>&1 &
}
# iverilog simulates about 100 clocks of the 5 000-cell netlist per second
# of processor time, so the work is cut down: a 16-cell memory, one of the
# three pattern modes, a faster SPI clock and a shorter speed sweep. The
# default is four scenarios (about 12 minutes of processor time);
# "run_gate.sh all" runs all 14 (about 47 minutes of processor time).
G="-Ptb_core.AW=2 -Ptb_core.PATMODES=1 -Ptb_core.SPI_HALF=30.0 -Ptb_core.STUCK_ADDR=5"
run good_fpm          $G
run stuck_bit         $G -Ptb_core.STUCK_EN=1 -Ptb_core.EXPECT_FAIL=1
run speed_60          $G -Ptb_core.MODE=1 -Ptb_core.T_RAC=60.0 -Ptb_core.SWEEP_FROM=18
run detect_edo        $G -Ptb_core.MODE=2 -Ptb_core.EDO=1
if [ "${1:-}" = "all" ]; then
run good_edo          $G -Ptb_core.EDO=1
run stuck_bit_high    $G -Ptb_core.STUCK_EN=1 -Ptb_core.STUCK_VAL=1 -Ptb_core.EXPECT_FAIL=1
run addr_line_stuck   $G -Ptb_core.ADDR_STUCK_LINE=1 -Ptb_core.EXPECT_FAIL=1
run dq_short          $G -Ptb_core.DQ_SHORT_A=1 -Ptb_core.DQ_SHORT_B=2 -Ptb_core.EXPECT_FAIL=1
run too_slow          $G -Ptb_core.T_RAC=150.0 -Ptb_core.EXPECT_FAIL=1
run speed_70          $G -Ptb_core.MODE=1 -Ptb_core.T_RAC=70.0 -Ptb_core.SWEEP_FROM=22
run speed_83          $G -Ptb_core.MODE=1 -Ptb_core.T_RAC=83.0 -Ptb_core.SWEEP_FROM=26
run detect_fpm        $G -Ptb_core.MODE=2
run retention         $G -Ptb_core.MODE=3 -Ptb_core.T_RET=200000.0
run abort             $G -Ptb_core.MODE=4 -Ptb_core.AW=3
fi
wait
fail=0
for name in $names; do
    if grep -q '^PASS' "build/gate_$name.log" && ! grep -q '^FAIL\|rror:' "build/gate_$name.log"; then
        echo "ok    $name: $(grep '^PASS' "build/gate_$name.log")"
    else
        echo "FAIL  $name: $(tail -3 "build/gate_$name.log" | tr '\n' ' ')"; fail=1
    fi
done
exit $fail
