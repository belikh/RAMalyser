#!/bin/sh
# Switching simulation of the base board's regulators and the impedance of
# the FPGA supplies, with ngspice. One PASS, FAIL or INFO line per check;
# exit status 1 if any FAIL.
#
#   hardware/spice/reg/run.sh           all checks (about 30 switching runs, several minutes)
#   hardware/spice/reg/run.sh pdn       only the FPGA supply impedance (seconds)
#
# The regulator is this project's own model built from the datasheets, not
# the manufacturer's: Diodes Incorporated's product pages for the AP63300
# and AP632xx link no SPICE model, and its model index refuses automated
# requests. Nothing is downloaded. Capacitor models are typical figures,
# not the makers' files. Every such value is marked ASSUMED in buck.inc,
# dut.inc, core.inc and pdn.cir. Nothing here has been compared with a
# built board.
set -eu
cd "$(dirname "$0")"
command -v ngspice >/dev/null || { echo "ngspice not found"; exit 2; }
# KEEP=dir keeps the simulator output there; with the output already in it
# the simulations are not run again, only the checks.
if [ -n "${KEEP:-}" ]; then tmp=$KEEP; mkdir -p "$tmp"; else tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT; fi
fail=0
jobs=${JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)}

val() { awk -v k="$1" '$1 == k && $2 == "=" { print $3; exit }' "$tmp/$2.out"; }
chk() { if awk "BEGIN { exit !($2) }"; then echo "PASS  $1: $3"; else echo "FAIL  $1: $3"; fail=1; fi; }
ex() { awk "BEGIN { print ($1) }"; }
f() { awk -v v="$1" -v s="${2:-1}" -v p="${3:-3}" 'BEGIN { printf "%.*f", p, v * s }'; }

# job: name  rail  deck  "vin vset iload kc"  sed-script for the @...@ marks
job() { printf '%s|%s|%s|%s|%s\n' "$1" "$2" "$3" "$4" "$5" >> "$tmp/jobs"; }
ripple() {  # name rail "vin vset iload kc" span-ms
    e=$(ex "$4 * 1e-3"); t0=$(ex "$4 * 0.6e-3")
    job "$1" "$2" a_ripple.cir "$3" "s/@TEND@/$e/g; s/@T0@/$t0/; s/@P1@/$(ex "$e - 8e-6")/; s/@P2@/$(ex "$e - 6e-6")/g; s/@P3@/$(ex "$e - 4e-6")/g; s/@P4@/$(ex "$e - 2e-6")/g; s/@P5@/$e/"
}
cat > "$tmp/work.sh" <<'W'
#!/bin/sh
# one job: a private directory, the decks, the parameters, ngspice
IFS='|' read -r name rail deck par sedx <<X
$1
X
d="$TMPD/$name.d"; mkdir "$d"
cp buck.inc "$d/"; cp "$rail" "$d/rail.inc"
set -- $par
printf '.param vin=%s vset=%s iload=%s kc=%s\n' "$1" "$2" "$3" "$4" > "$d/run.param"
sed "$sedx" "$deck" > "$d/deck.cir"
(cd "$d" && ngspice -b deck.cir > "../$name.out" 2>&1) || true
rm -rf "$d"
W
run_jobs() { [ -f "$tmp/done" ] && return 0; TMPD="$tmp" xargs -P "$jobs" -d '\n' -n 1 sh "$tmp/work.sh" < "$tmp/jobs"; : > "$tmp/jobs"; : > "$tmp/done"; }

# output capacitance left under DC bias. ASSUMED: typical curve of a 22 uF
# 25 V X5R 0805 (CL21A226MAQNNNE class): -10 % at 1.1 V, -30 % at 2.6 V,
# -40 % at 3.3 V, -60 % at 5 V and above. The maker's curve was not fetched.
derate() { awk -v v="$1" 'BEGIN { print (v < 1.5 ? 0.9 : v < 3.0 ? 0.7 : v < 4.0 ? 0.6 : 0.4) }'; }

pdn() {
    ngspice -b pdn.cir > "$tmp/pdn.out" 2>&1 || true
    for rail in 1v1 3v3 2v5; do
        zt=$(val "zt_$rail" pdn); zm=$(val "zmax_$rail" pdn)
        fm=$(awk -v k="zmax_$rail" '$1 == k { print $5; exit }' "$tmp/pdn.out")
        zh=$(val "zhf_$rail" pdn)
        chk "FPGA supply impedance, $rail, 1 kHz to 100 MHz" "$zm <= $zt" \
            "highest $(f "$zm" 1000 0) mOhm at $(f "$fm" 1e-6 2) MHz, target $(f "$zt" 1000 0) mOhm"
        echo "INFO  $rail at 500 MHz: $(f "$zh" 1000 0) mOhm (the board's capacitors no longer act there; the package and die do)"
    done
    echo "INFO  1v1: $(f "$(val zlf_1v1 pdn)" 1000 0) mOhm at 1 kHz (the track resistance), $(f "$(val zmid_1v1 pdn)" 1000 0) mOhm at 10 MHz, above target from $(f "$(val fx_1v1 pdn)" 1e-6 0) MHz up"
    dr=$(val drop_1v1 pdn); vb=$(val vball_1v1 pdn); ic=$(val icore pdn)
    chk "1.1 V at the FPGA balls" "$vb >= 1.045" \
        "$(f "$vb") V at $(f "$ic" 1000 0) mA with the regulator 1.5 % low: $(f "$dr" 1000 0) mV lost in 205 mOhm of track; the FPGA needs 1.045 V, reached at $(f "$(val ilimit_1v1 pdn)" 1000 0) mA"
}

if [ "${1:-}" = pdn ]; then pdn; exit $fail; fi

# ---- 1. DUT rail, 9 V input: ripple and regulation
: > "$tmp/jobs"
for v in 2.6 3.3 5.0 5.5; do
    ripple "r9_${v}_0.1" dut.inc "9 $v 0.1 1" 3
    ripple "r9_${v}_1" dut.inc "9 $v 1 1" 1.2
    ripple "r9_${v}_2" dut.inc "9 $v 2 1" 1.2
    ripple "d9_${v}_2" dut.inc "9 $v 2 $(derate $v)" 1.2
done
ripple "d9_5.0_0.1" dut.inc "9 5.0 0.1 $(derate 5.0)" 3
# ---- 2. the same from a 5 V source (a charger that gives no 9 V)
for c in "3.3 2" "5.0 1" "5.0 2" "5.5 1"; do
    set -- $c; ripple "r5_$1_$2" dut.inc "5 $1 $2 1" 1.2
done
# ---- 3. load steps
for v in 5.0 3.3; do
    job "s_${v}_n" dut.inc a_step.cir "9 $v 0.1 1" "s/@I0@/0.1/g; s/@I1@/1/g"
    job "s_${v}_d" dut.inc a_step.cir "9 $v 0.1 $(derate $v)" "s/@I0@/0.1/g; s/@I1@/1/g"
done
# ---- 4. set-point steps, and the fall with nothing connected
job "sp" dut.inc a_setpoint.cir "9 3.3 0.02 1" ""
job "fall_n" dut.inc a_fall.cir "9 5.0 0 1" ""
job "fall_d" dut.inc a_fall.cir "9 5.0 0 0.5" ""
# ---- 5. the 1.1 V core rail
ripple "c_0.05" core.inc "9 1.0978 0.05 0.9" 3
ripple "c_0.3" core.inc "9 1.0978 0.3 0.9" 1.2
job "c_step" core.inc a_step.cir "9 1.0978 0.05 0.9" "s/@I0@/0.05/g; s/@I1@/0.4/g"
run_jobs

echo "---- DUT rail from 9 V"
ipk2=0
for v in 2.6 3.3 5.0 5.5; do
    for i in 0.1 1 2; do
        n="r9_${v}_$i"; a=$(val vavg $n); hi=$(val vmax $n); lo=$(val vmin $n)
        [ -n "$a" ] || { echo "FAIL  $v V at $i A: the simulation did not finish"; fail=1; continue; }
        rp=$(ex "$hi - $lo")
        chk "ripple, $v V at $i A" "$rp < 0.02 * $v && ($a - $v) < 0.01 * $v && ($v - $a) < 0.01 * $v" \
            "$(f "$rp" 1000 1) mV peak to peak, mean $(f "$a") V, inductor peak $(f "$(val ipk $n)" 1 2) A"
    done
    n="r9_${v}_2"
    p=$(val ipk $n); ipk2=$(ex "$p > $ipk2 ? $p : $ipk2")
    n="d9_${v}_2"; rp=$(ex "$(val vmax $n) - $(val vmin $n)")
    chk "ripple, $v V at 2 A, capacitors derated to $(derate $v)" "$rp < 0.02 * $v" "$(f "$rp" 1000 1) mV peak to peak"
done
n=d9_5.0_0.1; rp=$(ex "$(val vmax $n) - $(val vmin $n)")
chk "ripple, 5.0 V at 0.1 A, capacitors derated to $(derate 5.0)" "$rp < 0.02 * 5.0" "$(f "$rp" 1000 1) mV peak to peak (pulse skipping)"
# cycle-to-cycle peak current at the highest duty cycle
n=r9_5.5_2
alt=$(awk -v a="$(val p1 $n)" -v b="$(val p2 $n)" -v c="$(val p3 $n)" -v d="$(val p4 $n)" 'BEGIN { x = a - b; y = b - c; z = c - d; if (x < 0) x = -x; if (y < 0) y = -y; if (z < 0) z = -z; m = x; if (y > m) m = y; if (z > m) m = z; print m }')
chk "current loop at 61 % duty (5.5 V, 2 A)" "$alt < 0.05" "peaks of four cycles in a row differ by $(f "$alt" 1000 0) mA (slope compensation is ASSUMED)"

echo "---- inductor of the DUT rail (FNR4020S4R7MT)"
# The catalogue entry gives two currents, 2.5 A and 2 A, without saying
# which is saturation and which is heating. The regulator's datasheet asks
# for a DC rating 35 % above the load: 2.7 A for 2 A.
chk "inductor peak at 2 A against the smaller catalogue current" "$ipk2 <= 2.0" "peak $(f "$ipk2" 1 2) A, catalogue 2.0 A and 2.5 A"
chk "inductor rating against the regulator datasheet's rule" "2.0 >= 2.7" "rated 2.0 A (or 2.5 A), 2.7 A asked for a 2 A load"
echo "INFO  into a short the regulator limits at 4.5 A peak, above either catalogue current"

echo "---- DUT rail from a 5 V source"
for c in "3.3 2" "5.0 1" "5.0 2" "5.5 1"; do
    set -- $c; n="r5_$1_$2"; a=$(val vavg $n)
    echo "INFO  5 V source, set $1 V at $2 A: rail $(f "$a") V ($(f "$(ex "100 * ($a - $1) / $1")" 1 1) %)"
done
a=$(val vavg r5_5.0_1)
chk "5 V source: a 5.0 V set point at 1 A" "$a > 0.98 * 5.0" \
    "rail $(f "$a") V: the regulator is in drop-out, so a 5 V module cannot be tested from a source that gives no 9 V"

echo "---- load step 0.1 A -> 1 A -> 0.1 A"
for v in 5.0 3.3; do
    for k in n d; do
        n="s_${v}_$k"; pre=$(val vpre $n); dip=$(val vdip $n); pk=$(val vpk $n)
        [ "$k" = n ] && lab="" || lab=", capacitors derated to $(derate $v)"
        rec=""
        for w in "w1 0.1" "w2 0.2" "w3 0.3" "w4 0.5"; do
            set -- $w; x=$(val $1 $n)
            if [ -z "$rec" ] && awk "BEGIN { d = $x - $(val vend $n); if (d < 0) d = -d; exit !(d < 0.005 * $v) }"; then rec=$2; fi
        done
        chk "load step at $v V$lab" "($pre - $dip) < 0.05 * $v && ($pk - $pre) < 0.05 * $v" \
            "dip $(f "$(ex "$pre - $dip")" 1000 0) mV, overshoot $(f "$(ex "$pk - $pre")" 1000 0) mV, back within 0.5 % $([ -n "$rec" ] && echo "inside $rec ms" || echo "after more than 0.5 ms")"
    done
done

echo "---- set point"
tu=$(val t_up sp); vu=$(val v_up sp); v55=$(val v_55 sp); t55=$(val t_55 sp); td=$(val t_dn sp); td2=$(val t_dn2 sp)
chk "3.3 V -> 5.0 V in one DAC step, overshoot under 5 %" "$vu < 5.0 * 1.05" "reaches 4.95 V after $(f "$(ex "$tu - 0.001")" 1000 2) ms, highest $(f "$vu") V: the inductor is driven to its current limit and its energy ends up in the capacitors"
chk "5.0 V -> 5.5 V stays under the comparator" "$v55 < 5.863" "highest $(f "$v55") V, comparator at 5.863 V; reaches 5.45 V after $(f "$(ex "$t55 - 0.005")" 1000 2) ms"
echo "INFO  5.5 V -> 3.3 V at 20 mA: within 10 % after $(f "$(ex "$td - 0.009")" 1000 1) ms, 3.35 V after $(f "$(ex "$td2 - 0.009")" 1000 1) ms (the load alone discharges the capacitors)"
echo "INFO  5.0 V -> 3.3 V with nothing connected: under 3.85 V after $(f "$(val t_385 fall_n)" 1 2) s, within 10 % after $(f "$(val t_363 fall_n)" 1 2) s (with half the capacitance: $(f "$(val t_385 fall_d)" 1 2) s and $(f "$(val t_363 fall_d)" 1 2) s)"

echo "---- 1.1 V core rail"
for i in 0.05 0.3; do
    n="c_$i"; a=$(val vavg $n); hi=$(val vmax $n); lo=$(val vmin $n)
    chk "ripple, 1.1 V at $i A" "$hi < 1.155 && $lo > 1.045" "$(f "$(ex "$hi - $lo")" 1000 1) mV peak to peak, $(f "$lo") V to $(f "$hi") V at the regulator, inductor peak $(f "$(val ipk $n)" 1 2) A"
done
n=c_step; pre=$(val vpre $n); dip=$(val vdip $n); pk=$(val vpk $n)
chk "load step 0.05 A -> 0.4 A -> 0.05 A at 1.1 V" "$dip > 1.045 && $pk < 1.155" "$(f "$dip") V to $(f "$pk") V at the regulator (limits 1.045 V and 1.155 V)"

echo "---- FPGA supplies"
pdn
exit $fail
