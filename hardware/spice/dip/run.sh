#!/bin/sh
# Run the DIP cartridge's auxiliary-supply simulations with ngspice and
# print one line per check. PASS/FAIL compare the model with what a
# three-rail 4116 needs: VBB (-5 V) on before VDD (+12 V) and VCC (+5 V),
# and off after them. INFO lines are numbers with no limit attached.
# Exit status 1 if any FAIL.
#
# Both converters are behavioural models and the MOSFETs are generic
# level-1 models: see the ASSUMED lines in dip.inc and slot.inc. Nothing
# here has been compared with a built board.
set -eu
cd "$(dirname "$0")"
command -v ngspice >/dev/null || { echo "ngspice not found"; exit 2; }
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
fail=0

sim() { ngspice -b "$1" 2>&1; }
val() { awk -v k="$1" '$1 == k && $2 == "=" { print $3; exit }'; }
chk() { if awk "BEGIN { exit !($2) }"; then echo "PASS  $1: $3"; else echo "FAIL  $1: $3"; fail=1; fi; }
ex() { awk "BEGIN { print $1 }"; }
f() { awk -v v="$1" -v s="${2:-1}" -v p="${3:-2}" 'BEGIN { printf "%.*f", p, v * s }'; }

# 4116 limits: VBB -5 V, VDD +12 V and VCC +5 V, each +-10 % (datasheet
# operating range, from memory: ASSUMED).
VBB_MIN=-4.5
# "Present" for the order checks: more than half its value. The chip is
# destroyed by VDD without VBB, not by a VBB that is a volt short for a
# millisecond; that second case has its own checks. ASSUMED.
VBB_PRESENT=-2.5

# the weak corner: divider resistors 1 % the wrong way, +3V3 2 % low, the
# slot's +5 V 5 % low, twice the charge pump's output resistance
mkdir "$tmp/weak"; cp ./*.cir slot.inc "$tmp/weak/"
sed 's/^\.param r68=68k/.param r68=68.68k/; s/^\.param r100=100k/.param r100=99k/;
     s/^\.param r120=120k/.param r120=118.8k/; s/^\.param r10=10k/.param r10=10.1k/;
     s/^\.param v33v=3.3 /.param v33v=3.234 /; s/^\.param rout=15 /.param rout=30 /' dip.inc > "$tmp/weak/dip.inc"
sed -i 's/^\.param v5set=5.0/.param v5set=4.75/' "$tmp/weak/slot.inc"

# ---- 0. idle
o=$(sim d0_idle.cir)
vi=$(echo "$o" | awk '$1 == "v(vbb)" { print $3 }')
chk "VBB pin with the supplies off" "$vi < 0.3" \
    "$(f "$vi") V: the comparator's divider pulls the -5 V rail above ground through 168k"

# ---- 1. interlock threshold
thr() {  # label, directory
    o=$(cd "$2" && sim d1_threshold.cir)
    t=$(echo "$o" | val trip)
    chk "interlock$1" "$(echo "$o" | val vdd_m4) < 0.5 && $(echo "$o" | val vcc_m4) < 0.5 && $t < -3.9" \
        "AUX_OK rises with VBB below $(f "$t") V; +12 V and VCC are off above it"
    echo "INFO  comparator input$1: $(f "$(echo "$o" | val vdiv_m5)" 1000 0) mV with VBB at -5 V, $(f "$(echo "$o" | val vdiv_m525)" 1000 0) mV at -5.25 V (LMV331 common-mode range starts at -100 mV)"
}
thr "" .
thr " (weak corner)" "$tmp/weak"

# ---- 2. power-up
up() {  # label, directory
    o=$(cd "$2" && sim d2_powerup.cir)
    a=$(echo "$o" | val vbb_at_vdd); b=$(echo "$o" | val vbb_at_vcc)
    tv=$(echo "$o" | val t_vbb); td=$(echo "$o" | val t_vdd); tc=$(echo "$o" | val t_vcc)
    chk "power-up order$1" "$a < $VBB_PRESENT && $b < $VBB_PRESENT" \
        "VBB is at $(f "$a") V when +12 V passes 0.5 V and $(f "$b") V when VCC does"
    chk "power-up, VBB in tolerance first$1" "$a < $VBB_MIN && $b < $VBB_MIN" \
        "+12 V comes $(f "$(ex "$td - 0.001")" 1e6 0) us and VCC $(f "$(ex "$tc - 0.001")" 1e6 0) us after AUX_EN; VBB reaches $VBB_MIN V after $(f "$(ex "$tv - 0.001")" 1e6 0) us"
    ip=$(echo "$o" | val i_peak); s=$(echo "$o" | val t_i10_s); e=$(echo "$o" | val t_i10_e)
    chk "inrush into the +5 V switch$1" "$ip < 4.0" \
        "peak $(f "$ip" 1 1) A (AO3401A: 4 A continuous, 30 A pulsed), above 10 A for $(f "$(ex "${e:-0} - ${s:-0}")" 1e6 1) us; +5 V dips to $(f "$(echo "$o" | val slot5_min)") V at the slot and $(f "$(echo "$o" | val base5_min)") V at the base's regulator"
    echo "INFO  +12 V switch$1: peak $(f "$(echo "$o" | val i12_peak)") A into 1.1 uF, gate drive $(f "$(echo "$o" | val vsg12_max)") V (AO3401A limit 12 V)"
    vb=$(echo "$o" | val vbb_end); vd=$(echo "$o" | val vdd_end); vc=$(echo "$o" | val vcc_end)
    chk "rails with one 4116$1" "$vb < -4.5 && $vb > -5.5 && $vd > 10.8 && $vd < 13.2 && $vc > 4.5 && $vc < 5.5" \
        "VBB $(f "$vb") V, VDD $(f "$vd") V, VCC $(f "$vc") V; $(f "$(echo "$o" | val i5_end)" 1000 0) mA from the slot's +5 V at $(f "$(echo "$o" | val p5_end)") V"
    echo "INFO  +12 V raw rail$1: peak $(f "$(echo "$o" | val v12raw_pk)") V"
}
up "" .
up " (weak corner)" "$tmp/weak"
# empty socket: regulation with no load
mkdir "$tmp/empty"; cp ./*.cir slot.inc "$tmp/empty/"
sed 's/^\.param chip=1 /.param chip=0 /' dip.inc > "$tmp/empty/dip.inc"
o=$(cd "$tmp/empty" && sim d2_powerup.cir)
vb=$(echo "$o" | val vbb_end); vd=$(echo "$o" | val vdd_end)
chk "rails with the socket empty" "$vb < -4.5 && $vb > -5.5 && $vd > 10.8 && $vd < 13.2" "VBB $(f "$vb") V, VDD $(f "$vd") V"

# ---- 3. power-down by AUX_EN
o=$(sim d3_powerdown.cir)
a=$(echo "$o" | val vbb_at_vdd); b=$(echo "$o" | val vbb_at_vcc)
chk "power-down order" "$a < $VBB_PRESENT && $b < $VBB_PRESENT" \
    "VBB is at $(f "$a") V when +12 V falls through 0.5 V and $(f "$b") V when VCC does"
chk "power-down, VBB in tolerance to the end" "$a < $VBB_MIN && $b < $VBB_MIN" \
    "the boost empties the +5 V feed to $(f "$(echo "$o" | val p5_at_ok)") V before AUX_OK drops ($(f "$(ex "$(echo "$o" | val t_ok_off) - 0.030")" 1000 2) ms after AUX_EN) and the charge pump follows it; +12 V is gone after $(f "$(ex "$(echo "$o" | val t_vdd_off) - 0.030")" 1000 1) ms; VBB is $(f "$(echo "$o" | val vbb_late)") V 39 ms later"

# ---- 4. cartridge pulled under power
pull() {  # label, directory
    o=$(cd "$2" && sim d4_pulled.cir)
    a=$(echo "$o" | val vbb_at_vdd); b=$(echo "$o" | val vbb_at_vcc)
    chk "cartridge pulled$1" "$a < $VBB_PRESENT && $b < $VBB_PRESENT" \
        "VBB is at $(f "$a") V when +12 V falls through 0.5 V ($(f "$(ex "$(echo "$o" | val t_vdd_off) - 0.030")" 1000 2) ms after the pull) and $(f "$b") V when VCC does ($(f "$(ex "$(echo "$o" | val t_vcc_off) - 0.030")" 1000 2) ms)"
}
pull "" .
pull " (socket empty)" "$tmp/empty"

# ---- 5. the -5 V supply fails
o=$(sim d5_vbbfail.cir)
chk "charge pump never starts" "$(echo "$o" | val a_vdd_max) < 0.5 && $(echo "$o" | val a_vcc_max) < 0.5" \
    "+12 V stays at $(f "$(echo "$o" | val a_vdd_max)") V and VCC at $(f "$(echo "$o" | val a_vcc_max)") V; VBB sits at $(f "$(echo "$o" | val a_vbb_end)") V, the raw boost rail at $(f "$(echo "$o" | val a_v12raw)") V"
o=$(sim d5b_vbbstop.cir)
a=$(echo "$o" | val vbb_at_vdd)
chk "charge pump stops under power" "$a < $VBB_PRESENT" \
    "VBB is at $(f "$a") V when +12 V falls through 0.5 V, $(f "$(ex "$(echo "$o" | val t_vdd_off) - 0.030")" 1000 1) ms after the pump stops"
exit $fail
