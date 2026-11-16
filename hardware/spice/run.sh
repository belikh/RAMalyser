#!/bin/sh
# Run the DUT-rail simulations with ngspice and print one line per check.
# PASS/FAIL compare the model with what the firmware and the parts expect;
# INFO lines are numbers with no limit attached. Exit status 1 if any FAIL.
#
# The buck regulator is a behavioural model and the MOSFETs are generic
# level-1 models: see the ASSUMED lines in rail.inc. Nothing here has been
# compared with a built board.
set -eu
cd "$(dirname "$0")"
command -v ngspice >/dev/null || { echo "ngspice not found"; exit 2; }
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
fail=0

sim() { ngspice -b "$1" 2>&1; }                       # $1 netlist
val() { awk -v k="$1" '$1 == k && $2 == "=" { print $3; exit }'; }   # measured value by name
chk() {  # name, condition (awk expression), text
    if awk "BEGIN { exit !($2) }"; then echo "PASS  $1: $3"; else echo "FAIL  $1: $3"; fail=1; fi
}
ex() { awk "BEGIN { print $1 }"; }                    # arithmetic
f() { awk -v v="$1" -v s="${2:-1}" -v p="${3:-3}" 'BEGIN { printf "%.*f", p, v * s }'; }

# ---- 1. set point against the DAC
# DAC voltages: code 0, then the firmware's codes for 5.5, 5.0, 3.3 and 2.6 V
# (dac_code_for in rt-hal), mid-scale, and full scale.
out=$(sim t1_setpoint.cir | awk '$1 == "v(vraw)" { print $3 }')
set -- $out
i=1
for want in 5.600 5.500 5.000 3.950 3.300 2.600 2.300; do
    got=$(echo "$out" | sed -n "${i}p")
    chk "set point $want V" "($got - $want) < 0.005 && ($want - $got) < 0.005" "rail $(f "$got") V"
    i=$((i + 1))
done
vmax=$(echo "$out" | sed -n 1p)

# ---- 2. over-voltage comparator
o=$(sim t2_ovp.cir)
t33=$(echo "$o" | val trip_open); t5=$(echo "$o" | val trip_5v)
chk "trip, strap open" "$t33 > 3.80 && $t33 < 3.90" "$(f "$t33") V, switch output above it $(f "$(echo "$o" | val above_open)") V"
chk "trip, strap grounded" "$t5 > 5.81 && $t5 < 5.91" "$(f "$t5") V, switch output above it $(f "$(echo "$o" | val above_5v)") V"
# The top of the 5 V margin range is 5.5 V. The regulator and its feedback
# resistors are within about 2 %, so the rail is built to reach 5.6 V and no
# more: 5.5 V can then be set on every unit. The comparator sits above that,
# within 0.3 V, to catch a regulator that has failed.
chk "5 V cartridge: rail limit" "$vmax <= 5.61 && $t5 > $vmax && $t5 - $vmax < 0.3" \
    "the rail reaches $(f "$vmax") V at DAC code 0; the comparator trips at $(f "$t5") V"
o=$(sim t2b_ovp_time.cir)
tt=$(echo "$o" | val t_trip); to=$(echo "$o" | val t_open); vc=$(echo "$o" | val vcart_max)
chk "trip to switch open" "($to - $tt) < 0.001" \
    "$(f "$(ex "$to - $tt")" 1000 1) ms after the comparator trips; cartridge saw $(f "$vc" 1 2) V of a 6.5 V fault"

# ---- 3. switch-on into the cartridge
inrush() {  # label, directory
    o=$(cd "$2" && sim t3_inrush.cir)
    ip=$(echo "$o" | val i_peak); rm=$(echo "$o" | val raw_min)
    s=$(echo "$o" | val t_over_2a_s); e=$(echo "$o" | val t_over_2a_e)
    t90=$(echo "$o" | val t_90); i30=$(echo "$o" | val i_30ms); v30=$(echo "$o" | val vcart_30ms)
    chk "inrush$1" "$ip < 2.0" "peak $(f "$ip" 1 1) A, above 2 A for $(f "$(ex "${e:-0} - ${s:-0}")" 1e6 0) us; supply side dips to $(f "$rm" 1 2) V"
    echo "INFO  rise$1: cartridge at 90 % $(f "$(ex "$t90 - 0.010")" 1000 2) ms after switch-on"
    chk "after 20 ms$1" "$i30 < 2.0 && $v30 > 4.75" "$(f "$i30" 1000 1) mA, $(f "$v30") V at the firmware's current check"
}
inrush "" .
# ---- 4. discharge
discharge() {  # label, directory
    o=$(cd "$2" && sim t4_discharge.cir)
    v=$(echo "$o" | val vcart_55ms); r=$(echo "$o" | val vraw_55ms); t=$(echo "$o" | val t_below_0v5)
    so=$(echo "$o" | val t_sw_open)
    chk "discharge$1" "$v < 0.5" "cartridge at $(f "$v" 1 2) V after the firmware's 50 ms${t:+ (below 0.5 V after $(f "$(ex "$t - 0.005")" 1000 1) ms)}; supply side left at $(f "$r" 1 2) V"
    echo "INFO  switch-off$1: the switch stays on $(f "$(ex "$so - 0.005")" 1000 1) ms after the request"
}
discharge "" .
# same two checks with AO3400A transistors at the weak end of the datasheet
# (threshold 1.45 V, a third of the typical transconductance) driven from 3.3 V logic
cp ./*.cir "$tmp/"
sed 's/^\.param kpn=15 /.param kpn=5 /; s/^\.param vtn=1.05 /.param vtn=1.45 /' rail.inc > "$tmp/rail.inc"
inrush " (weak AO3400A)" "$tmp"
discharge " (weak AO3400A)" "$tmp"

# ---- 5. shunt at the current limit
o=$(sim t5_shunt.cir)
sh=$(echo "$o" | awk '$1 == "v(vraw,vsh)" { print $3 }')
dr=$(echo "$o" | awk '$1 == "v(vraw,vcart)" { print $3 }')
vc=$(echo "$o" | awk '$1 == "v(vcart)" { print $3 }')
chk "shunt at 2 A" "$sh < 0.320" "$(f "$sh" 1000 1) mV, INA219 power-on range 320 mV (the firmware does not change it)"
echo "INFO  drop to the cartridge at 2 A: $(f "$dr" 1000 0) mV, module sees $(f "$vc") V of a 5.000 V set point (regulation is ahead of the shunt and switch)"
exit $fail
