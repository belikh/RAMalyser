* DUT supply rail of the RAMalyser base board (revision A).
* Component values are those of the board definition (power sheet).
* Lines marked ASSUMED are not from the board files or a checked datasheet.
*
* LIMIT: the buck regulator (AP63300) is not simulated as a switching
* converter. It is a current source that follows its 0.8 V feedback
* reference into the real output capacitors and feedback network, with a
* current limit and no ability to sink. Loop dynamics, ripple and the
* inductor are not represented.
*
* Nodes: vraw (buck output, VDUT_RAW)  vsh (after the shunt, VDUT_SH)
*        vdut (switched rail)          vcart (rail on the cartridge)
*        dac (DAC_OUT)  buck_en  swen (DUT_SW_EN)  dis (DUT_DISCH)
*        strap (STRAP_5V_OK)  ovpn (OVP_N)  g (DUT_G)  gx (DUT_GX)

.param ilim=3.5        ; ASSUMED buck current limit, A
.param gmbuck=2000     ; ASSUMED loop gain of the behavioural regulator, A/V
.param tss=0.3m        ; ASSUMED soft-start time constant of the reference
.param ccart=87u       ; cartridge bulk: SIMM 47u + 4 x 10u (DIP: 47u)
.param rconn=10m       ; ASSUMED card-edge contact and plane resistance
.param esr=5m          ; ASSUMED ceramic capacitor ESR
.param kpn=15          ; ASSUMED AO3400A transconductance parameter, A/V^2
.param vtn=1.05        ; AO3400A threshold, typical (datasheet range 0.65 .. 1.45 V)
.param iload=0

V3v3 v33 0 3.3

* ---- buck, behavioural
Bref  refraw 0 V = 0.8 * (v(buck_en) > 1.2 ? 1 : 0)
Rss   refraw ref 1k
Css   ref 0 {tss/1k}
Bbuck 0 vraw I = max(0, min({ilim}, {gmbuck} * (v(ref) - v(fb))))
Co1   vraw co1 22u
Rco1  co1 0 {esr}
Co2   vraw co2 22u
Rco2  co2 0 {esr}
Rben  buck_en 0 100k
* feedback: 100k to the rail, 20k to ground, 100k to the DAC
Rfb1  vraw fb 100k
Rfb2  fb 0 20k
Rfb3  fb dac 100k

* ---- current shunt (INA219 across it)
Rsh   vraw vsh 20m
Eish  ish 0 vraw vsh 50                      ; probe: shunt current, 1 V = 1 A

* ---- high-side switch
* The capacitor from gate to drain sets the rise of the output: the gate
* current through Rg charges it while the output moves. Switch-off goes
* through Rpu and the diode and is not slowed by Rg. Rpu is fed from ahead
* of the shunt, so its current is not counted as module current.
Mq1   vdut g vsh vsh PAO3401
Rpu   vraw gx 4.7k
Rg    g gx 100k
Dg    gx g DSCH
Cgd   g vdut 10n
Mq2   gx ovpn 0 0 NAO3400
.model DSCH D(IS=1u N=1.05 RS=0.4 CJO=110p)  ; ASSUMED small Schottky (B5819W class), 0.3 V at 1 mA
Rsw   swen ovpn 10k
Rovpd ovpn 0 100k
Evsg  vsg 0 vsh g 1                          ; probe: source-gate voltage of the switch

* ---- over-voltage comparator (LMV331, open drain on OVP_N)
Rr1   v33 ovpref 10k
Rr2   ovpref 0 10k
Rd1   vraw ovpdiv 100k
Rd2   ovpdiv 0 75k
Rd3   ovpdiv strap 82k
Scmp  ovpn 0 ovpdiv ovpref SWCMP
.model SWCMP SW(VT=0 VH=0 RON=50 ROFF=1G)   ; ASSUMED output resistance; no delay, no offset

* ---- discharge
Mq3   dischr dis 0 0 NAO3400
Rdis  vdut dischr 100
Rdpd  dis 0 100k
Cv    vdut cv 22u
Rcv   cv 0 {esr}

* ---- other loads on the switched rail
Rled  vdut leda 1k
Dled  leda 0 DLED
.model DLED D(IS=1e-18 N=2 RS=5)             ; ASSUMED red LED, about 1.8 V
* VDUT_SENSE monitor (cartridge ties VDUT_SENSE to VDUT): 100k + 47k
Rm1   vcart adc 100k
Rm2   adc 0 47k

* ---- cartridge
Rconn vdut vcart {rconn}
Ccart vcart cc {ccart}
Rcc   cc 0 {2*esr}
Iload vcart 0 {iload}

* AO3401A: Vgs(th) -0.9 V typ, Rds(on) about 50 mOhm at -4.5 V (datasheet
* typical values); the split between channel and series resistance, and
* the capacitances, are ASSUMED.
.model PAO3401 PMOS(LEVEL=1 VTO=-0.9 KP=7 RD=12m RS=8m CGSO=1.2n CGDO=0.12n W=1 L=1)
.model NAO3400 NMOS(LEVEL=1 VTO={vtn} KP={kpn} RD=10m RS=8m CGSO=0.6n CGDO=0.08n W=1 L=1)
