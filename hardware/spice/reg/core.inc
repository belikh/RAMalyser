* The 1.1 V FPGA core regulator as built (pcb/boards/base.py, power sheet):
* AP63200 (500 kHz, 125 / 68 mOhm, 2.8 A peak limit, 450 mA light-load
* peak), 2.2 uH, 10 uF + 100 nF in, 2 x 22 uF out, feedback 30k / 80k6,
* no feed-forward capacitor (the datasheet's table has 100 pF).
.include buck.inc
.include run.param
.param v0={vset}
Vin   src 0 {vin}
Rsrc  src vin 50m                 ; ASSUMED
Cin1  vin ci1 6u ic={vin}         ; 10 uF 25 V X5R at 9 V, ASSUMED 60 % left
Rci1  ci1 0 5m
Cin2  vin 0 100n ic={vin}
X1    vin sw fb buck rhs=125m rls=68m ipk=2.8 ipfm=0.45 gcs=3
+ comp0={0.3 + max(0.45, iload + 0.45 + 0.8e6 * (vset/vin) / 500k) / 3} ss0=1
L1    sw l1 2.2u ic={iload}
Rl1   l1 out 60m                  ; ASSUMED: the catalogue entry gives no resistance
Co1   out c1 {22u*kc} ic={vset}
Rc1   c1 0 3m
Co2   out c2 {22u*kc} ic={vset}
Rc2   c2 0 3m
Rf1   out fb 30k
Rf2   fb 0 80.6k
.options method=gear reltol=1e-3
