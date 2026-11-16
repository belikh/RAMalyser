#!/bin/sh
# Tolerance run on the DUT rail set point and the over-voltage comparator:
# 2000 Monte Carlo units (fixed seed) and all 8192 worst-case corners of the
# 13 parts in tol.inc. One line per check; exit status 1 if any FAIL.
#
# DC only. The tolerances of the regulator, the DAC and the comparator are
# datasheet limits written from memory (ASSUMED in tol.inc). Ripple and
# load steps are not in the model; RIPPLE below stands in for them.
# A random unit draws every part uniformly between its limits, which is
# harsher than real production spread. A corner puts every part at a limit.
set -eu
cd "$(dirname "$0")"
command -v ngspice >/dev/null || { echo "ngspice not found"; exit 2; }
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
RIPPLE=0.05      # ASSUMED ripple plus overshoot the comparator must not react to, V
fail=0
chk() { if awk "BEGIN { exit !($2) }"; then echo "PASS  $1: $3"; else echo "FAIL  $1: $3"; fail=1; fi; }

# columns after the tag: 2 code 0, 3..7 codes for 5.5 5.0 3.6 3.3 2.6 V, 8 trip open, 9 trip grounded
ngspice -b tol_mc.cir 2>/dev/null | awk '$1 == "u" && NF == 9' > "$tmp/mc"
ngspice -b tol_corner.cir 2>/dev/null | awk '$1 == "u" && NF == 9' > "$tmp/co"
[ "$(wc -l < "$tmp/mc")" -eq 2000 ] && [ "$(wc -l < "$tmp/co")" -eq 8192 ] || { echo "simulation did not finish"; exit 2; }

# stat FILE EXPR -> "min max mean share" of an awk expression over the units;
# share is the fraction of units for which the expression is below LIMIT
stat() { awk -v lim="${3:-0}" "{ v = $2; if (NR == 1 || v < lo) lo = v; if (NR == 1 || v > hi) hi = v; s += v; if (v < lim) b++ }
    END { printf \"%.4f %.4f %.4f %.4f\", lo, hi, s / NR, b / NR }" "$1"; }
get() { echo "$1" | cut -d' ' -f"$2"; }
f() { awk -v v="$1" -v s="${2:-1}" -v p="${3:-3}" 'BEGIN { printf "%.*f", p, v * s }'; }

# ---- (a) the top of the rail
m=$(stat "$tmp/mc" '$2' 5.5); c=$(stat "$tmp/co" '$2' 5.5)
chk "5.5 V reachable on every unit" "$(get "$c" 1) >= 5.5" \
    "rail at DAC code 0 is $(f "$(get "$c" 1)") .. $(f "$(get "$c" 2)") V at the corners, $(f "$(get "$m" 1)") .. $(f "$(get "$m" 2)") V in 2000 random units; $(f "$(get "$m" 4)" 100 1) % of those stop short of 5.5 V"
echo "INFO  highest the rail can be set: $(f "$(get "$c" 2)") V (worst corner), $(f "$(get "$m" 2)") V (random units)"

# ---- (b) comparator trip points
for s in "8 open 3.85" "9 grounded 5.863"; do
    set -- $s
    m=$(stat "$tmp/mc" "\$$1"); c=$(stat "$tmp/co" "\$$1")
    echo "INFO  trip, strap $2: nominal $3 V; $(f "$(get "$c" 1)") .. $(f "$(get "$c" 2)") V at the corners, $(f "$(get "$m" 1)") .. $(f "$(get "$m" 2)") V in random units"
done

# ---- (c) false trips: the comparator of a unit against that unit's own rail
m=$(stat "$tmp/mc" '$9 - $3' "$RIPPLE"); c=$(stat "$tmp/co" '$9 - $3' "$RIPPLE")
chk "no false trip at 5.5 V, 5 V cartridge" "$(get "$c" 1) > $RIPPLE" \
    "trip minus rail is $(f "$(get "$c" 1)" 1000 0) mV at the worst corner, $(f "$(get "$m" 1)" 1000 0) mV at the worst random unit (needs more than $(f "$RIPPLE" 1000 0) mV)"
m=$(stat "$tmp/mc" '$8 - $5' "$RIPPLE"); c=$(stat "$tmp/co" '$8 - $5' "$RIPPLE")
chk "no false trip at 3.6 V, 3.3 V cartridge" "$(get "$c" 1) > $RIPPLE" \
    "trip minus rail is $(f "$(get "$c" 1)" 1000 0) mV at the worst corner, $(f "$(get "$m" 1)" 1000 0) mV at the worst random unit; $(f "$(get "$m" 4)" 100 1) % of random units have less than $(f "$RIPPLE" 1000 0) mV"
m=$(stat "$tmp/mc" '$9 - $2'); c=$(stat "$tmp/co" '$9 - $2')
echo "INFO  5 V cartridge, DAC code 0: trip minus rail is $(f "$(get "$c" 1)" 1000 0) .. $(f "$(get "$c" 2)" 1000 0) mV at the corners, $(f "$(get "$m" 1)" 1000 0) .. $(f "$(get "$m" 2)" 1000 0) mV in random units (nominal 263 mV)"
# a 5 V set point reaching a 3.3 V cartridge is what the strap is for
ABSMAX=4.6       # ASSUMED absolute maximum supply of 3.3 V DRAM, V
c=$(stat "$tmp/co" '$8')
chk "3.3 V cartridge is cut off below its absolute maximum" "$(get "$c" 2) < $ABSMAX" "highest trip with the strap open $(f "$(get "$c" 2)") V, limit $ABSMAX V"

# ---- (d) open-loop error of dac_code_for(); the firmware refuses to close
#      the switch if the rail is more than 5 % from the set point (rt-hal, vdut_enable)
for s in "3 5.5" "4 5.0" "5 3.6" "6 3.3" "7 2.6"; do
    set -- $s
    m=$(stat "$tmp/mc" "(\$$1 - $2) / $2 * 100" -5); c=$(stat "$tmp/co" "(\$$1 - $2) / $2 * 100" -5)
    out=$(stat "$tmp/mc" "((\$$1 - $2) / $2 > 0.05 || (\$$1 - $2) / $2 < -0.05) ? -1 : 1")
    worst=$(awk -v a="$(get "$c" 1)" -v b="$(get "$c" 2)" 'BEGIN { a = a < 0 ? -a : a; print (a > b ? a : b) }')
    chk "set point $2 V inside the firmware's 5 % window" "$worst < 5" \
        "error $(f "$(get "$c" 1)" 1 2) .. $(f "$(get "$c" 2)" 1 2) % at the corners ($(f "$(get "$c" 1)" "$(awk "BEGIN { print $2 * 10 }")" 0) .. $(f "$(get "$c" 2)" "$(awk "BEGIN { print $2 * 10 }")" 0) mV), $(f "$(get "$m" 1)" 1 2) .. $(f "$(get "$m" 2)" 1 2) % in random units, $(f "$(get "$out" 4)" 100 1) % of them outside the window"
done
exit $fail
