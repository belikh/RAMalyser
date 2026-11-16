* Tolerance model of the DUT rail set point and the over-voltage comparator
* (base board, power sheet). DC only: the regulator is an ideal amplifier
* that holds its feedback pin at the reference.
*
* The decks that include this file alter the element values from the
* tolerances below; the values here are nominal.
*
* Tolerances. Resistors are the board's 1 % parts. The rest are datasheet
* limits written down from memory and not re-checked against the PDFs:
* treat them as ASSUMED.
.csparam tol_r=0.01
.csparam tol_ref=0.015
.csparam tol_33=0.02
.csparam dac_off=0.0075
.csparam dac_gain=0.02
.csparam cmp_vos=7m
* tol_r     0402 thick film, 1 %
* tol_ref   ASSUMED AP63300 feedback reference, 0.8 V +/- 1.5 % over temperature
* tol_33    ASSUMED AP63203 fixed 3.3 V output, +/- 2 % (it is the DAC's reference)
* dac_off   ASSUMED MCP4725 offset error, +/- 0.75 % of full scale
* dac_gain  ASSUMED MCP4725 gain error, +/- 2 % of full scale
* cmp_vos   ASSUMED LMV331 input offset, +/- 7 mV

V33  v33 0 3.3
Vref ref 0 0.8

* Set point. The DAC output is code/4096 of its supply, with gain and
* offset error, and cannot leave the supply range.
* Vcode: code / 4096.  Vgain: 1 + gain error.  Voff: offset as a fraction of full scale.
Vcode code 0 0
Vgain gain 0 1
Voff  off 0 0
Bdac  dac 0 V = max(0, min(v(v33), v(v33) * (v(code) * v(gain) + v(off))))
Ereg  rail 0 ref fb 1e6
R1    rail fb 100k
R2    fb 0 20k
R3    fb dac 100k

* Comparator: the rail voltage at which OVP_DIV meets OVP_REF plus the
* offset. An ideal amplifier finds it. Rst is 1G for an open strap and
* 1m for a grounded one.
Rr1   v33 cref 10k
Rr2   cref 0 10k
Vos   cref crefo 0
Rd1   trip div 100k
Rd2   div 0 75k
Rd3   div st 82k
Rst   st 0 1G
Etrip trip 0 crefo div 1e6
