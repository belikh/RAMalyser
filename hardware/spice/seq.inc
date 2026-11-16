* The base board's supplies around the DUT rail, for sequencing runs.
* Included after rail_seq.inc, which run_sequence.sh makes from rail.inc:
* the fixed 3.3 V source is taken out, the DUT regulator and the comparator
* are made to depend on their supplies, and the cartridge contact becomes a
* switch. Everything here is behavioural. Lines marked ASSUMED are not from
* the board files or a checked datasheet.
*
* Nodes added: vbus  v5  v11  v25  plug (1 = cartridge seated)
*              mcu (1 = firmware running)  fw_en fw_sw fw_dis (what the
*              firmware drives on DUT_BUCK_EN, DUT_SW_EN, DUT_DISCH)
*              dacf (DAC code / 4096)

.param uvlo=3.6        ; ASSUMED input voltage at which the AP632xx regulators start
.param drop=0.3        ; ASSUMED headroom a regulator needs at its input, V
.param tss33=1m        ; ASSUMED soft-start time constant of the fixed regulators
.param i33=0.3         ; ASSUMED load on 3.3 V, A
.param i5=0.1          ; ASSUMED load on 5 V, A
.param i11=0.15        ; ASSUMED load on 1.1 V, A
.param vbod=2.9        ; ASSUMED 3.3 V level below which the RP2350 is in reset
.param tboot=5m        ; ASSUMED time from reset release to the firmware driving its pins
.param eff=0.9         ; ASSUMED regulator efficiency

* ---- input: 2 x 22u at the connector, 10u at each of the four regulators
Cin  vbus cin 84u
Rcin cin 0 3m

* ---- 3.3 V (AP63203, enable tied to VBUS), 2 x 22u
Be33  e33 0 V = 0.5 * (1 + tanh((v(vbus) - {uvlo}) / 0.05))
Rs33  e33 s33 1k
Cs33  s33 0 {tss33/1k}
B33   0 v33x I = max(0, min(2.5, 2000 * (min(3.3 * v(s33), v(vbus) - {drop}) - v(v33))))
C33   v33 0 44u
R33   v33 0 {3.3/i33}
Vm33  v33x v33 0
Bi33  vbus 0 I = v(v33) * i(Vm33) / ({eff} * max(v(vbus), 1))

* ---- 5 V (AP63205): follows VBUS less the headroom until VBUS is high enough
B5    0 v5x I = max(0, min(2.5, 2000 * (min(5.0 * v(s33), v(vbus) - {drop}) - v(v5))))
C5    v5 0 44u
R5    v5 0 {5/i5}
Vm5   v5x v5 0
Bi5   vbus 0 I = v(v5) * i(Vm5) / ({eff} * max(v(vbus), 1))

* ---- 1.1 V (AP63200) and 2.5 V (AP2112K from 3.3 V)
B11   0 v11x I = max(0, min(2.5, 2000 * (min(1.1 * v(s33), v(vbus) - {drop}) - v(v11))))
C11   v11 0 44u
R11   v11 0 {1.1/i11}
Vm11  v11x v11 0
Bi11  vbus 0 I = v(v11) * i(Vm11) / ({eff} * max(v(vbus), 1))
B25   v25 0 V = min(2.5, max(0, v(v33) - 0.25))

* ---- input current of the DUT regulator (Vmdut is put in series with it in rail_seq.inc)
Bidut vbus 0 I = v(vraw) * i(Vmdut) / ({eff} * max(v(vbus), 1))

* ---- the RP2350: its pins are open until the firmware runs
Bmr   mraw 0 V = (v(v33) > {vbod} ? 1 : 0)
Rmr   mraw mdel {tboot/1u}
Cmr   mdel 0 1u
Bmcu  mcu 0 V = (v(mdel) > 0.63 && v(v33) > {vbod} ? 1 : 0)
.model SWPIN SW(VT=0.5 VH=0 RON=30 ROFF=1G)
Bpen  pen 0 V = v(fw_en) * v(v33) / 3.3
Bpsw  psw 0 V = v(fw_sw) * v(v33) / 3.3
Bpdis pdis 0 V = v(fw_dis) * v(v33) / 3.3
Sen   pen buck_en mcu 0 SWPIN
Ssw   psw swen mcu 0 SWPIN
Sdis  pdis dis mcu 0 SWPIN

* ---- DAC (MCP4725): a fraction of its supply
Bdac  dac 0 V = v(dacf) * v(v33)

* ---- cartridge strap: STRAP_5V_OK is grounded by a seated 5 V cartridge.
* The deck sets strap5: 1 = the cartridge grounds the strap, 0 = it leaves it open.
.model SWSTR SW(VT=0.5 VH=0 RON=1m ROFF=1G)
Bstr  strc 0 V = v(plug) * {strap5}
Sstr  strap 0 strc 0 SWSTR
