* The DUT regulator of the base board as built (pcb/boards/base.py, power
* sheet): AP63300, 4.7 uH (FNR4020S4R7MT, 98 mOhm), 10 uF + 100 nF in,
* 2 x 22 uF out, feedback 100k to the rail, 20k to ground, 100k to the DAC.
* Parameters come from run.param: vin vset iload kc (output capacitance
* left after DC bias, 1 = none lost) and the regulator's start state.
.include buck.inc
.include run.param
.param v0={min(vset, vin - 0.4)}       ; where the output starts
Vin   src 0 {vin}
Rsrc  src vin 50m                 ; ASSUMED source and connector resistance
Cin1  vin ci1 6u ic={vin}         ; 10 uF 25 V X5R at 9 V: about 60 % left, ASSUMED
Rci1  ci1 0 5m
Cin2  vin 0 100n ic={vin}
X1    vin sw fb buck comp0={0.3 + max(0.93, iload + 0.5 + 0.8e6 * min(1, v0/vin) / 500k) / 5} ss0=1
L1    sw l1 4.7u ic={iload}
Rl1   l1 out 98m
Co1   out c1 {22u*kc} ic={v0}
Rc1   c1 0 3m                     ; ASSUMED ESR
Co2   out c2 {22u*kc} ic={v0}
Rc2   c2 0 3m
Rf1   out fb 100k
Rf2   fb 0 20k
Rf3   fb dac 100k
.options method=gear reltol=1e-3
