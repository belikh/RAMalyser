* Switching model of the AP63200 / AP63203 / AP63205 / AP63300 buck regulators.
*
* THIS IS NOT THE MANUFACTURER'S MODEL. Diodes Incorporated publishes none
* that could be fetched for these parts (see run.sh), so the regulator is
* built here from its datasheet: fixed-frequency peak-current-mode control,
* ideal switches with the datasheet on-resistances, cycle-by-cycle peak
* limit, pulse skipping at light load with a clamped peak current, no
* reverse inductor current, 4 ms soft start, and no maximum duty cycle (the
* parts hold the high-side switch on in drop-out).
*
* From the datasheets (DS42002 rev 3-2, DS41326 rev 3-2): fsw, rhs, rls,
* ipk, ipfm, izc, vref, tss, minimum on-time 80 ns.
* ASSUMED, because the datasheets do not give them: the error amplifier
* and its compensation (gm, rc, cc, chf), the current-sense gain (gcs), the
* slope compensation (se), the level at which pulses are skipped. Loop
* results (overshoot, settling, stability) therefore show what a
* reasonable loop does with the board's parts, not what the real part does.
* Not modelled: frequency spreading, the bootstrap refresh, the
* output over-voltage pull-down of the AP63300 (the low-side switch is
* turned on when the output is 10 % high; whether it then sinks current is
* not stated, so the model does not sink), thermal effects, switching loss.
*
* Pins: vin sw fb. Ground is node 0. comp0 presets the compensation
* capacitor for a start at the set point (ss0=1) instead of a soft start.

.subckt buck vin sw fb fsw=500k rhs=75m rls=40m ipk=4.5 ipfm=0.93 izc=0.06 vref=0.8
+ tss=4m ss0=1 comp0=0.5 gm=200u rc=68k cc=330p chf=10p gcs=5 voff=0.3 se=0.8e6
.param tper={1/fsw}
* reference with soft start
Vref ref 0 PWL(0 {vref*ss0} {tss} {vref})
* error amplifier (ASSUMED) and its clamps
Bea   0 compi I = max(-20u, min(20u, {gm} * (v(ref) - v(fb))))
Rcomp compi cz {rc}
Ccomp cz 0 {cc} ic={comp0}
Chf   compi 0 {chf} ic={comp0}
Dchi  compi chi DCL
Vchi  chi 0 {voff + (ipk + se*tper)/gcs}
Dclo  clo compi DCL
Vclo  clo 0 {voff + 0.6*ipfm/gcs}
.model DCL D(IS=1e-12 N=0.1)
* clock, slope ramp, minimum on-time
Vclk  clk 0 PULSE(0 1 0 5n 5n 60n {tper})
Vramp ramp 0 PULSE(0 {se*tper} 0 {tper-10n} 5n 5n {tper})
Vblk  blank 0 PULSE(0 1 80n 5n 5n {tper-95n} {tper})
* current sense: high-side switch current, low-side (freewheel) current
Rsh   vin vx 1m
Rsl   ls 0 1m
Bihs  ihs 0 V = (v(vin) - v(vx)) * 1000
Bils  ils 0 V = -v(ls) * 1000
* peak command: the compensation voltage less the slope ramp, held between
* the light-load peak and the current limit
Bcmd  cmd 0 V = max({ipfm}, min({ipk}, (v(compi) - {voff}) * {gcs} - v(ramp)))
* a pulse starts on the clock unless the loop asks for less than the skip level
Bset  set 0 V = (v(clk) > 0.5 && (v(compi) - {voff}) * {gcs} > {ipfm} && v(fb) < {1.1*vref}) ? 1 : 0
Brst  rst 0 V = (v(q) > 0.5 && v(blank) > 0.5 && v(ihs) > v(cmd)) ? 1 : 0
Vone  one 0 1
Sset  q one set 0 SLS
Srst  q 0 rst 0 SLR
Cq    q 0 20p ic=0
Rq    q 0 1G
* low side: on while the high side is off, until the current reaches zero
Eqd   qb 0 q 0 1
Rqd   qb qd 1k
Cqd   qd 0 30p ic=0
Bzs   zs 0 V = (v(q) < 0.2 && v(qd) < 0.2 && v(ils) < {izc}) ? 1 : 0
Szs   zc one zs 0 SLS
Szr   zc 0 set 0 SLR
Czc   zc 0 20p ic=1
Rzc   zc 0 1G
Blsg  lsg 0 V = (v(q) < 0.2 && v(zc) < 0.5) ? 1 : 0
Shs   vx sw q 0 SHS
Sls   sw ls lsg 0 SLSW
Dls   ls sw DBODY
Csw   sw 0 100p
Rsw   sw 0 200k
.model SLS  SW(VT=0.5 VH=0.1 RON=50 ROFF=1G)
.model SLR  SW(VT=0.5 VH=0.1 RON=5 ROFF=1G)
.model SHS  SW(VT=0.7 VH=0.1 RON={rhs} ROFF=10Meg)
.model SLSW SW(VT=0.5 VH=0.1 RON={rls} ROFF=10Meg)
.model DBODY D(IS=1e-9 N=1.2 RS=0.05)
.ends
