* Auxiliary supplies of the RAMalyser DIP cartridge (revision A): the
* switched +5 V feed, the -5 V charge pump, the VBB-good comparator, the
* +12 V boost and the two switches that gate +12 V and +5 V to the 4116
* socket. Component values are those of the board definition (rails sheet).
* Lines marked ASSUMED are not from the board files or a checked datasheet.
*
* LIMIT: neither converter is simulated as a switching circuit.
*  - The TPS60403 charge pump is a source of -V(P5_SW) behind a fixed
*    output resistance. It works in both directions (it also pulls VBB
*    back towards zero when its input falls), which is how an unregulated
*    switched-capacitor inverter behaves; that is ASSUMED, not checked.
*  - The MT3608 boost is a current source that regulates its 0.6 V feedback
*    pin, limited by input power, and takes that power from P5_SW. The
*    inductor and diode are present as the passive path they also are.
*    Ripple, loop dynamics and any internal soft start are not represented.
*
* Nodes: slot5 (+5 V at the slot contact)  p5sw (P5_SW)  vbb (VBB_N5V)
*        v12raw (V12_RAW)  vdd (VDD_12V)  vcc (VCC_4116)  vdut  v33
*        auxen (AUX_EN)  auxok (AUX_OK)  q5g q12g q12d qvg  vdiv vref fb

.param chip=1          ; 1 = a 4116 in the socket, 0 = empty socket
.param idd=35m         ; ASSUMED 4116 operating IDD (datasheet maximum, from memory)
.param ibb=200u        ; ASSUMED 4116 IBB (datasheet maximum, from memory)
.param icc=4m          ; ASSUMED 4116 ICC with its output loaded
.param rout=15         ; ASSUMED TPS60403 output resistance at 5 V in, typical
.param cpon=1          ; 1 = charge pump works, 0 = failed
.param vstart=1.6      ; TPS60403 minimum input voltage
.param iqcp=65u        ; ASSUMED TPS60403 supply current
.param eff=0.85        ; ASSUMED MT3608 efficiency at 12 V out
.param ilimin=2        ; ASSUMED average input current available (switch limit 4 A peak)
.param gmb=20           ; ASSUMED gain of the behavioural boost, A/V
.param ven=1.5         ; ASSUMED MT3608 enable threshold
.param v33v=3.3        ; +3V3 at the cartridge
.param r68=68k
.param r100=100k
.param r120=120k
.param r10=10k
.param esr=5m          ; ASSUMED ceramic capacitor ESR
.param kpn=0.1         ; ASSUMED 2N7002 transconductance parameter, A/V^2
.param vtn=1.6         ; ASSUMED 2N7002 threshold, typical (datasheet 1.0 .. 2.5 V)

* ---- +5 V feed switch (AUX_EN)
Mq5   p5sw q5g slot5 slot5 PAO3401
Rq5g  q5g slot5 100k
Mq5n  q5g auxen 0 0 N2N7002
Raen  auxen 0 100k
Cp1   p5sw cp1 10u
Rcp1  cp1 0 {esr}
Cp2   p5sw cp2 1u
Rcp2  cp2 0 {esr}
* i(Vx) in the test decks is the current into the cartridge's +5 V contact

* ---- -5 V charge pump (behavioural), 10u reservoir, 47k bleed
Icpgo 0 cpgo 1m                              ; 1 V unless a test deck forces cpgo low
Rcpgo cpgo 0 1k
Bcpen cpen 0 V = ({cpon} > 0.5 && v(cpgo) > 0.5) ? 0.5 * (1 + tanh((v(p5sw) - {vstart}) / 0.05)) : 0
* current out of VBB while the pump runs: (VBB - (-P5_SW)) / Rout, either sign
Bcp   vbb 0 I = v(cpen) * (v(vbb) + v(p5sw)) / {rout}
Bcpin p5sw 0 I = v(cpen) * (sqrt(((v(vbb) + v(p5sw)) / {rout})^2 + 1e-10) + {iqcp})
Cvbb  vbb cvb 10u
Rcvb  cvb 0 {esr}
Cvbs  vbb 0 100n                             ; at the socket
Rvbb  vbb 0 47k

* ---- VBB-good comparator (LMV331, open drain, 10k pull-up to +3V3)
Rd1   v33 vdiv {r68}
Rd2   vdiv vbb {r100}
Rr1   v33 vref {r120}
Rr2   vref 0 {r10}
Scmp  auxok 0 vdiv vref SWCMP
Rpu   auxok v33 10k
.model SWCMP SW(VT=0 VH=0 RON=50 ROFF=1G)    ; ASSUMED output resistance; no delay, no offset
.model SWON  SW(VT=0.5 VH=0 RON=1m ROFF=1G)

* ---- +12 V boost (behavioural) with its real inductor, diode and divider
Lb    p5sw lbm 22u
Rlb   lbm bsw 0.2                            ; ASSUMED inductor resistance
Db    bsw v12raw DSS14
.model DSS14 D(IS=5u N=1.1 RS=0.05 CJO=110p) ; ASSUMED SS14 Schottky
Cv12  v12raw c12 10u
Rc12  c12 0 {esr}
Rf1   v12raw fb 191k
Rf2   fb 0 10k
* enables are smooth steps 50 mV wide, so the solver can follow them
Bben  ben 0 V = 0.25 * (1 + tanh((v(auxok) - {ven}) / 0.05)) * (1 + tanh((v(p5sw) - 2) / 0.05))
Bbo   0 v12raw I = v(ben) * max(0, min({gmb} * (0.6 - v(fb)), {eff} * {ilimin} * v(p5sw) / max(v(v12raw), 1)))
Bbi   p5sw 0 I = v(ben) * max(0, min({gmb} * (0.6 - v(fb)), {eff} * {ilimin} * v(p5sw) / max(v(v12raw), 1))) * max(v(v12raw), 1) / ({eff} * max(v(p5sw), 0.5))

* ---- +12 V switch, gated by AUX_OK
Mq12  vddx q12g v12raw v12raw PAO3401
Vs12  vddx vdd 0                             ; probe: current through the +12 V switch
Rq12a q12g v12raw 100k
Rq12b q12g q12d 100k
Mq12n q12d auxok 0 0 N2N7002
Rvdd  vdd 0 10k
Cvdd  vdd 0 1.1u                             ; 1u + 100n at the socket
Evsg12 vsg12 0 v12raw q12g 1                 ; probe: source-gate voltage of the +12 V switch

* ---- 4116 VCC switch, gated by AUX_OK
Mqv   vcc qvg vdut vdut PAO3401
Rqvg  qvg vdut 100k
Mqvn  qvg auxok 0 0 N2N7002
Rvcc  vcc 0 10k
Cvcc  vcc 0 100n

* ---- one 4116 in the socket (resistive, ASSUMED)
Rchd  vdd 0 {chip > 0.5 ? 12/idd : 1G}
Rchb  vbb 0 {chip > 0.5 ? 5/ibb : 1G}
Rchc  vcc 0 {chip > 0.5 ? 5/icc : 1G}

* AO3401A and 2N7002 as in ../rail.inc before the base's revision
.model PAO3401 PMOS(LEVEL=1 VTO=-0.9 KP=7 RD=12m RS=8m CGSO=1.2n CGDO=0.12n W=1 L=1)
.model N2N7002 NMOS(LEVEL=1 VTO={vtn} KP={kpn} RD=1 RS=0.5 CGSO=30p CGDO=5p W=1 L=1)
