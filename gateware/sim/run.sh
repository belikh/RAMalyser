#!/bin/sh
# Gateware regression: every scenario must print a PASS line and no FAIL line.
# Scenarios run in parallel; results are summarised at the end.
set -eu
cd "$(dirname "$0")"
python3 ../../spec/ramspec.py --emit ../../spec/generated >/dev/null
INC=../../spec/generated
SRC="../rtl/tester_core.v ../rtl/spi_regs.v ../rtl/io_cells.v dram_model.v tb_core.v"
mkdir -p build
names=""
run() {
    name=$1; shift
    names="$names $name"
    ( iverilog -g2012 -DSIM -I "$INC" -o "build/$name.vvp" "$@" $SRC &&
      vvp -n "build/$name.vvp" ) > "build/$name.log" 2>&1 &
}
run good_fpm
run good_edo          -Ptb_core.EDO=1
run stuck_bit         -Ptb_core.STUCK_EN=1 -Ptb_core.EXPECT_FAIL=1
run stuck_bit_high    -Ptb_core.STUCK_EN=1 -Ptb_core.STUCK_VAL=1 -Ptb_core.EXPECT_FAIL=1
run addr_line_stuck   -Ptb_core.ADDR_STUCK_LINE=2 -Ptb_core.EXPECT_FAIL=1
run dq_short          -Ptb_core.DQ_SHORT_A=1 -Ptb_core.DQ_SHORT_B=2 -Ptb_core.EXPECT_FAIL=1
run too_slow          -Ptb_core.T_RAC=120.0 -Ptb_core.EXPECT_FAIL=1
run speed_60          -Ptb_core.MODE=1 -Ptb_core.T_RAC=60.0
run speed_70          -Ptb_core.MODE=1 -Ptb_core.T_RAC=70.0
run speed_83          -Ptb_core.MODE=1 -Ptb_core.T_RAC=83.0
run detect_fpm        -Ptb_core.MODE=2
run detect_edo        -Ptb_core.MODE=2 -Ptb_core.EDO=1
run retention         -Ptb_core.MODE=3 -Ptb_core.T_RET=2000000.0
wait
fail=0
for name in $names; do
    if grep -q '^PASS' "build/$name.log" && ! grep -q '^FAIL\|rror:' "build/$name.log"; then
        echo "ok    $name: $(grep '^PASS' "build/$name.log")"
    else
        echo "FAIL  $name: $(tail -3 "build/$name.log" | tr '\n' ' ')"; fail=1
    fi
done
exit $fail
