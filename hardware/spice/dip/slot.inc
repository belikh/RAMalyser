* What the base presents at the slot. ASSUMED unless noted.
* +5 V: the base's regulator (a source behind a resistance) with its two
* 22u capacitors, then the bottom-layer pour (47 mOhm, from the base's
* supply report), the slot contact and the loop inductance of that path.
.param r5path=47m      ; pour, from pcb/fab/base/rt_base-supply-report.txt
.param r5cont=30m      ; ASSUMED contact resistance of one slot contact pair
.param l5path=150n     ; ASSUMED loop inductance, about 200 mm of pour over a plane
.param v5set=5.0
.param r5reg=50m       ; ASSUMED output impedance of the regulator over the first milliseconds
V5reg reg5 0 {v5set}
R5reg reg5 base5 {r5reg}
Cb5   base5 cb5 44u
Rcb5  cb5 0 2.5m
R5p   base5 s5a {r5path}
L5p   s5a s5b {l5path}
R5c   s5b slot5x {r5cont}
