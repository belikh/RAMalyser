#!/bin/sh
# usage: one.sh name [iverilog -P options]
cd "$(dirname "$0")"; name=$1; shift
iverilog -g2012 -DSIM -I ../../spec/generated -o build/$name.vvp "$@" ../rtl/tester_core.v ../rtl/spi_regs.v ../rtl/io_cells.v dram_model.v tb_core.v && time vvp -n build/$name.vvp | tail -6
