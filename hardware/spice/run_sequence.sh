#!/bin/sh
# Sequencing runs on the base board's supplies with ngspice: power-up, a
# cartridge pulled out under power, a 3.3 V cartridge straight after a 5 V
# one, and the USB supply pulled. One line per check; exit 1 if any FAIL.
#
# The regulators are behavioural (seq.inc, and rail.inc with four lines
# changed below): no inductors, no ripple, no load-step overshoot. The
# ASSUMED lines in seq.inc list every value that is not from the board.
set -eu
cd "$(dirname "$0")"
command -v ngspice >/dev/null || { echo "ngspice not found"; exit 2; }
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
fail=0

# rail.inc for these runs: no fixed 3.3 V source; the DUT regulator needs its
# input (and its current is probed); the comparator works only while 3.3 V
# is above vcmp; the cartridge contact is a switch.
sed -e '/^V3v3  *v33 0 3.3/d' \
    -e 's|^Bbuck 0 vraw I = max(0, min({ilim}, {gmbuck} \* (v(ref) - v(fb))))|Bbuck 0 vrawx I = 0.5 * (1 + tanh((v(vbus) - {uvlo}) / 0.05)) * max(0, min(min({ilim}, {gmbuck} * (v(ref) - v(fb))), 20 * (v(vbus) - {drop} - v(vraw))))\nVmdut vrawx vraw 0|' \
    -e 's|^Scmp  ovpn 0 ovpdiv ovpref SWCMP|Bcmp  cmpc 0 V = (v(v33) > {vcmp} \&\& v(ovpdiv) > v(ovpref) ? 1 : -1)\nScmp  ovpn 0 cmpc 0 SWCMP|' \
    -e 's|^Rconn vdut vcart {rconn}|.model SWCON SW(VT=0.5 VH=0 RON={rconn} ROFF=1G)\nSconn vdut vcart plug 0 SWCON\nRcleak vcart 0 1MEG|' \
    rail.inc > "$tmp/rail_seq.inc"
# ASSUMED: below this the LMV331 output is taken as open (it is specified from 2.7 V)
echo ".param vcmp=1.8" >> "$tmp/rail_seq.inc"
for n in Vmdut Bcmp Sconn; do
    grep -q "^$n " "$tmp/rail_seq.inc" || { echo "rail.inc has changed: $n not placed"; exit 2; }
done
! grep -q '^V3v3' "$tmp/rail_seq.inc" || { echo "rail.inc has changed: 3.3 V source still there"; exit 2; }
cp seq.inc s[1-5]_*.cir "$tmp/"

sim() { (cd "$tmp" && ngspice -b "$1" 2>&1); }
val() { awk -v k="$1" '$1 == k && $2 == "=" { print $3; exit }'; }
chk() { if awk "BEGIN { exit !($2) }"; then echo "PASS  $1: $3"; else echo "FAIL  $1: $3"; fail=1; fi; }
ex() { awk "BEGIN { print $1 }"; }
f() { awk -v v="$1" -v s="${2:-1}" -v p="${3:-3}" 'BEGIN { printf "%.*f", p, v * s }'; }
need() { [ -n "$2" ] || { echo "FAIL  $1: the simulation gave no value"; fail=1; return 1; }; }

# ---- 1. power-up, no cartridge
o=$(sim s1_powerup.cir)
vd=$(echo "$o" | val vdut_max); vr=$(echo "$o" | val vraw_max); on=$(echo "$o" | val ovpn_max)
dp=$(echo "$o" | val dis_max_pre); en=$(echo "$o" | val en_max)
if need "power-up" "$vd"; then
    chk "power-up: DUT rail stays off" "$vd < 0.1 && $vr < 0.1" "switched rail at most $(f "$vd" 1000 1) mV, regulator output at most $(f "$vr" 1000 1) mV"
    chk "power-up: control lines stay low while the pins are open" "$on < 0.3 && $dp < 0.3 && $en < 0.3" \
        "switch request $(f "$on" 1000 1) mV, discharge $(f "$dp" 1000 1) mV, regulator enable $(f "$en" 1000 1) mV at most (the transistors need 650 mV)"
    echo "INFO  power-up: 3.3 V in regulation $(f "$(ex "$(echo "$o" | val t_33) - 0.001")" 1000 1) ms after VBUS appears; firmware drives its pins at $(f "$(ex "$(echo "$o" | val t_mcu) - 0.001")" 1000 1) ms"
    echo "INFO  power-up: the 5 V rail is $(f "$(echo "$o" | val v5_at_5)" 1 2) V while VBUS is 5 V and $(f "$(echo "$o" | val v5_at_9)" 1 2) V at 9 V; a 5 V-only source leaves it low"
fi

# ---- 2. cartridge pulled out at 5 V, 1 A
o=$(sim s2_unplug.cir)
to=$(echo "$o" | val t_open); tt=$(echo "$o" | val t_trip); re=$(echo "$o" | val t_retrip)
if need "unplug" "$to"; then
    chk "unplug: the comparator opens the switch" "($to - 0.08005) < 0.001" \
        "strap open lowers the trip point to 3.85 V; comparator trips $(f "$(ex "$tt - 0.08005")" 1e6 0) us and the switch is open $(f "$(ex "$to - 0.08005")" 1e6 0) us after the contact breaks"
    chk "unplug: the switch does not chatter" "\"$re\" == \"\"" "the comparator stays tripped (it senses ahead of the switch); switch request at the end $(f "$(echo "$o" | val ovpn_end)" 1000 0) mV"
    echo "INFO  unplug: no firmware reaction is modelled, because rt-fw does not read ADP_PRESENT_N. The regulator stays on at $(f "$(echo "$o" | val vraw_end)" 1 2) V and the slot contacts still carry $(f "$(echo "$o" | val vdut_end)" 1 2) V 320 ms later"
    echo "INFO  unplug: highest voltage on the regulator output after the load left $(f "$(echo "$o" | val vraw_max)") V (no inductor in the model, so no load-dump overshoot)"
fi

# ---- 3. a 3.3 V cartridge straight after a 5 V test
o=$(sim s3_hotplug33.cir)
v150=$(echo "$o" | val vraw_150); vc=$(echo "$o" | val vcart_151); vm=$(echo "$o" | val vcart_max); tr=$(echo "$o" | val t_release)
if need "3.3 V after 5 V" "$v150"; then
    chk "3.3 V after 5 V: the firmware's reading refuses the rail" "($v150 - 3.3) / 3.3 > 0.05" \
        "regulator output still $(f "$v150" 1 2) V when read 10 ms after enable, $(f "$(ex "($v150 - 3.3) / 3.3 * 100")" 1 0) % above the set point (limit 5 %)"
    chk "3.3 V after 5 V: the comparator holds the switch open" "$vc < 0.1 && $vm < 3.6" \
        "with the switch requested anyway the cartridge sees $(f "$vc" 1000 0) mV at first and $(f "$vm" 1 2) V at most"
    echo "INFO  3.3 V after 5 V: the comparator lets go $(f "$(ex "$tr - 0.150")" 1 2) s after the request, with the regulator output at $(f "$(echo "$o" | val vraw_at_rel)" 1 2) V; a test started sooner fails and must be repeated"
fi

# ---- 4. USB supply pulled
brown() {  # label, DAC volts, load ohms, strap5
    sed -e "s/^\.param dacv=.*/.param dacv=$2/" -e "s/^\.param rl=.*/.param rl=$3/" \
        -e "s/^\.param strap5=.*/.param strap5=$4/" "$tmp/s4_brownout.cir" > "$tmp/s4x.cir"
    o=$(sim s4x.cir)
    a=$(echo "$o" | val vcart_on); m=$(echo "$o" | val vcart_max)
    need "supply pulled, $1" "$a" || return 0
    need "supply pulled, $1" "$(echo "$o" | val t_33_low)" || return 0
    t=$(echo "$o" | val t_dut_low)
    chk "supply pulled, $1: the rail does not rise" "$m - $a < 0.05" \
        "$(f "$a" 1 2) V before, $(f "$m" 1 2) V at most after; 3.3 V rail below reset level after $(f "$(ex "$(echo "$o" | val t_33_low) - 0.080")" 1000 1) ms${t:+, cartridge below 0.5 V after $(f "$(ex "$t - 0.080")" 1000 1) ms}"
    [ -n "$t" ] || echo "INFO  supply pulled, $1: the cartridge still has $(f "$(echo "$o" | val vcart_end)" 1 2) V 320 ms later; nothing discharges it once the 3.3 V rail is gone"
}
brown "5 V at 1 A" 0.6 5 1
brown "5 V, idle module" 0.6 1000 1
brown "3.3 V at 0.5 A" 2.3 6.6 0
brown "3.3 V, idle module" 2.3 1000 0

# ---- 5. pad leakage on the switch request (RP2350 A2, erratum E9)
leak() {  # label, AO3400A threshold
    sed "s/^\.param vtn=1.05 /.param vtn=$2 /" "$tmp/rail_seq.inc" > "$tmp/rail_leak.inc"
    sed 's/^\.include rail_seq.inc/.include rail_leak.inc/' "$tmp/s5_padleak.cir" > "$tmp/s5x.cir"
    o=$(sim s5x.cir)
    l=$(echo "$o" | val ovpn_lvl); c=$(echo "$o" | val vcart_lvl)
    need "pad leakage, $1" "$l" || return 0
    chk "pad leakage, $1: the switch stays open" "$c < 0.1" \
        "120 uA into the 10k and 100k on the request line lifts it to $(f "$l" 1 2) V; cartridge at $(f "$c" 1 2) V with the switch not requested"
}
leak "typical transistor" 1.05
leak "low-threshold transistor" 0.65
exit $fail
