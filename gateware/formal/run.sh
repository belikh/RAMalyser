#!/bin/sh
# Formal checks of the gateware with SymbiYosys (OSS CAD Suite).
#
#   prove   every property in *_props.vh holds for all inputs and all time
#           (k-induction, not a bounded search)
#   cover   the situations the properties talk about can occur
#   mutant  the RTL with one deliberate fault must fail the proof; this is
#           how the properties are known to be able to catch that fault
#
# Prints one PASS or FAIL line per check; exit status 1 if any fails.
set -eu
cd "$(dirname "$0")"
[ -d "$HOME/.local/opt/oss-cad-suite/bin" ] && PATH="$HOME/.local/opt/oss-cad-suite/bin:$PATH"
command -v sby >/dev/null || { echo "sby not found"; exit 2; }
python3 ../../spec/ramspec.py --emit ../../spec/generated >/dev/null
mkdir -p build
fail=0

task() {  # sby file, task, expected verdict (PASS or FAIL), id, label, properties file
    if sby -f -d "build/$4" "$1.sby" "$2" > "build/$4.log" 2>&1; then got=PASS; else got=FAIL; fi
    # a mutant only counts as caught if an assertion failed, not if the tool did
    if [ "$3" = FAIL ] && ! grep -q 'Assert failed' "build/$4.log"; then got=ERROR; fi
    if [ "$got" = "$3" ]; then
        case "$2:$3" in
        prove:PASS) echo "PASS  $5: proved by induction ($(grep -c 'assert (' "$6") assertions)" ;;
        cover:PASS) echo "PASS  $5: $(grep -c 'Reached cover statement' "build/$4.log") situations reached" ;;
        *)          echo "PASS  $5: caught by $(grep -m1 -o '[a-z_]*_props.vh:[0-9]*' "build/$4.log")" ;;
        esac
    else
        echo "FAIL  $5: expected $3, got $got (build/$4.log)"; fail=1
    fi
}

task tester_core prove PASS core_prove "core: properties" tester_core_props.vh
task tester_core cover PASS core_cover "core: reachability" tester_core_props.vh
task spi_regs    prove PASS spi_prove  "spi: properties" spi_regs_props.vh
task spi_regs    cover PASS spi_cover  "spi: reachability" spi_regs_props.vh

# ---- mutants: copies of tester_core.v with one fault each
mutant() {  # label, python expression that turns the source s into the mutant
    mkdir -p "build/src_$1"
    python3 - "$1" "$2" <<'EOF'
import pathlib, sys
s = pathlib.Path("../rtl/tester_core.v").read_text()
m = eval(sys.argv[2])
assert m != s, "mutation did not apply: " + sys.argv[1]
pathlib.Path(f"build/src_{sys.argv[1]}/tester_core.v").write_text(m)
sby = pathlib.Path("tester_core.sby").read_text().replace("../rtl/tester_core.v", f"build/src_{sys.argv[1]}/tester_core.v")
pathlib.Path(f"m_{sys.argv[1]}.sby").write_text(sby)
EOF
    task "m_$1" prove FAIL "m_$1" "mutant $1" tester_core_props.vh
    rm -f "m_$1.sby"
}
# a start that arrives during an idle refresh is dropped (the fault fixed in the RTL)
mutant start_dropped 's.replace("go_pend <= 1; st_busy <= 1; st_done <= 0;", "")'
# an abort pulse is only seen if it lands on the one clock that looks for it
mutant abort_dropped 's.replace("if (r_abort && st_busy) abort_pend <= 1;", "")'
# output enable asserted during write cycles as well
mutant oe_in_write   's.replace("oe0  <= rd_a &&", "oe0  <= cyc_a &&")'
# the bus stays driven when the DUT supply is off
mutant drive_no_power 's.replace("n_oe = {`N_IO{1\x27b0}};", "n_oe = r_gpio_oe;")'
exit $fail
