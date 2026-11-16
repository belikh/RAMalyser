#!/usr/bin/env python3
"""Signal integrity of the bus between the FPGA and the memory socket.

    ./run.sh            # fetches the vendor models if absent, then runs this

Four questions, on the SIMM cartridge (the longest lines of the two):
  1. FPGA pad -> 22 ohm -> slot -> level-shifter input
  2. level shifter -> 22 ohm -> socket -> DRAM pin, at 5 V and 3.3 V
  3. DRAM data out -> receive buffer -> slot -> FPGA pad
  4. how long each of these takes, and how far the lines differ

Line lengths, layers and via counts are those of the routed boards
(lengths.json, taken from the routed boards). Everything marked ASSUMED below is
not from a board file, a vendor model or a datasheet that was read.

LIMITS OF THE METHOD
- A net is one lossless line with the length-weighted impedance of its
  layers. Layer changes inside a net are not separate sections.
- A net with several pads is a main line to the pad of interest, with the
  rest of its copper as one open stub joined at the middle of the main line.
- No crosstalk, no ground bounce, no simultaneous switching.
- IBIS models are used by the two-table method (I-V tables switched by a
  linear ramp), not the V-t tables.
Nothing here has been compared with a built board.
"""
from __future__ import annotations

import bisect
import concurrent.futures
import json
import math
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import ibis  # noqa: E402

OUT = HERE / "out"
SERIES = 22.0                      # the resistor arrays on both boards
VIA_C = 0.4e-12                    # ASSUMED capacitance of one through via
T0 = 2e-9                          # the driver starts to switch here
TEND = 60e-9
QUICK = 25e-9                      # enough for a delay figure

# ---------------------------------------------------------------- stack-ups
# ASSUMED: JLCPCB-like builds. Base, 6 layers 1.6 mm: 0.0994 mm prepreg under
# the outer layers, 0.55 mm cores, 0.1088 mm prepreg between the two middle
# layers. Cartridges, 4 layers 1.6 mm: 0.2104 mm prepreg, 1.065 mm core.
ER = 4.4                           # ASSUMED FR-4, both builds
T_OUT, T_IN = 0.035, 0.0152        # copper, mm


def microstrip(h, w, t=T_OUT, er=ER):
    """IPC-2141 surface microstrip: ohms, ps per mm."""
    return (87 / math.sqrt(er + 1.41) * math.log(5.98 * h / (0.8 * w + t)),
            3.3356 * math.sqrt(0.475 * er + 0.67))


def stripline_offset(a, d, w, t=T_IN, er=ER):
    """IPC-2141 asymmetric stripline, a = distance to the nearer plane, d to the farther."""
    return (80 / math.sqrt(er) * math.log(1.9 * (2 * a + t) / (0.8 * w + t)) * (1 - a / (4 * (a + d + t))),
            3.3356 * math.sqrt(er))


def embedded_far(h, w, t=T_IN, er=ER):
    """A track on an inner layer whose only solid plane is h away, with its
    own layer's pour beside it. ASSUMED: the microstrip formula, less 10 %
    for the dielectric above and 10 % for the pour alongside."""
    z, _ = microstrip(h, w, t, er)
    return z * 0.8, 3.3356 * math.sqrt(er)


W_BASE, W_CART = 0.127, 0.15       # signal track widths in the board rules
LAYER = {
    "base": {"F.Cu": microstrip(0.0994, W_BASE), "B.Cu": microstrip(0.0994, W_BASE),
             # In2 and In3 face each other across 0.1088 mm; each sees the other's ground pour
             "In2.Cu": stripline_offset(0.1088, 0.55, W_BASE),
             "In3.Cu": stripline_offset(0.1088, 0.55, W_BASE)},
    "cart": {"F.Cu": microstrip(0.2104, W_CART),          # over the In1 ground plane
             "B.Cu": microstrip(0.2104, W_CART),          # over the In2 VDUT pour, where there is one
             "In2.Cu": embedded_far(1.065 + T_IN, W_CART)},   # ground plane is a core away
}


def line(net, kind, frac=1.0):
    """(Z0, delay s, via capacitance) of a net, or of the first frac of it."""
    tot = sum(net["layers"].values()) or 1e-9
    z = sum(l * LAYER[kind][k][0] for k, l in net["layers"].items()) / tot
    td = sum(l * LAYER[kind][k][1] for k, l in net["layers"].items()) * 1e-12 * frac
    return z, max(td, 1e-12), net["vias"] * VIA_C * frac


# ------------------------------------------------------------------- parts
def generic(name, ron, cc, driver=True):
    """A driver that is a resistance to each rail and a diode to ground."""
    s = [f".subckt {name} pin vcc" + (" ku kd" if driver else ""), f"Cc pin 0 {cc:g}",
         "Dg 0 pin DESD", ".model DESD D(IS=1e-12 N=1 RS=3)"]
    if driver:
        s += [f"Bpd pin 0 I = v(kd) * v(pin) / {ron}", f"Bpu pin 0 I = v(ku) * (v(pin) - v(vcc)) / {ron}"]
    return "\n".join(s + [f".ends {name}"])


class Parts:
    def __init__(self):
        self.notes = []
        self.sub = {}
        self.ramp = {}
        self.clamp = {}
        self.pkg = {}
        self._ibis("lvc8t245", "ti-lvc8t245/scem494.ibs", "LVC8T245_PW",
                   {"33": "LVC8T245_IO_33", "50": "LVC8T245_IO_50"}, {"33": 14.0, "50": 10.0})
        self._ibis("lvc245a", "ti-lvc245a/sn74lvc245a.ibs", "LVC245A_PW",
                   {"33": "LVC245A_IO_33"}, {"33": 12.0})
        # ECP5 pad. Datasheet: 5 pF pin capacitance, 8 mA at 0.4 V (50 ohm at most).
        # ASSUMED: 25 ohm typical, 1.2 ns full swing with SLEWRATE=FAST, 3 nH and
        # 0.5 pF for the caBGA256 ball, a diode to ground and none to VCCIO.
        for c in (0, 2):
            self.sub[("ecp5", "33", c)] = generic(f"ecp5_33_{c}", 25.0, 5e-12)
            self.ramp[("ecp5", "33", c)] = (1.2e-9, 1.2e-9)
        self.pkg["ecp5"] = (0.1, 3e-9, 0.5e-12)
        self.notes.append("ECP5 pad: generic model (vendor IBIS not fetched), ASSUMED 25 ohm, 1.2 ns")
        # DRAM data output. ASSUMED: 30 ohm, 2 ns, 7 pF; no vendor model exists for old DRAMs.
        for v in ("33", "50"):
            for c in (0, 2):
                self.sub[("dram", v, c)] = generic(f"dram_{v}_{c}", 30.0, 7e-12)
                self.ramp[("dram", v, c)] = (2e-9, 2e-9)
        self.pkg["dram"] = (0.1, 4e-9, 0.5e-12)

    def _ibis(self, key, rel, comp, models, ron):
        f = HERE / "models" / rel
        if f.exists():
            d = ibis.parse(f)
            self.pkg[key] = tuple(d["package"][comp][k][0] for k in "RLC")
            for v, mname in models.items():
                m = d["models"][mname]
                for c in (0, 2):
                    self.sub[(key, v, c)] = ibis.subckt(f"{key}_{v}_{c}", m, c, True)
                    self.ramp[(key, v, c)] = ibis.ramp_times(m, c)
                self.clamp[(key, v)] = sorted((r[0], r[1]) for r in m["tables"]["gnd clamp"])
            self.notes.append(f"{key}: vendor IBIS model {f.name}")
        else:
            self.pkg[key] = (0.04, 2.7e-9, 0.32e-12)
            for v in models:
                for c in (0, 2):
                    self.sub[(key, v, c)] = generic(f"{key}_{v}_{c}", ron[v], 6e-12)
                    self.ramp[(key, v, c)] = (1.2e-9, 1.2e-9)
            self.notes.append(f"{key}: vendor model missing, generic model used (ASSUMED {ron} ohm, 1.2 ns)")

    def clamp_ma(self, key, v, volts):
        """Current the ground clamp of a pin carries at a voltage, mA."""
        tab = self.clamp.get((key, v))
        if not tab or volts >= 0:
            return 0.0
        xs = [p[0] for p in tab]
        i = min(max(bisect.bisect_left(xs, volts), 1), len(xs) - 1)
        (x0, y0), (x1, y1) = tab[i - 1], tab[i]
        return abs(y0 + (y1 - y0) * (volts - x0) / (x1 - x0)) * 1e3


# ----------------------------------------------------------------- netlist
class Deck:
    def __init__(self, parts, title):
        self.p, self.l, self.n, self.used = parts, [f"* {title}"], 0, set()
        self.tend = TEND

    def u(self, pre):
        self.n += 1
        return f"{pre}{self.n}"

    def tline(self, a, b, z, td, cvia=0.0):
        """Lossless line a -> b; the via capacitance sits in the middle."""
        if td < 60e-12:                  # too short to matter as a line, and a line this short
            self.l += [f"{self.u('L')} {a} {b} {z * td:.4g}",          # would force tiny time steps
                       f"{self.u('C')} {b} 0 {td / z + cvia:.4g}"]
            return
        if cvia > 0:
            m = self.u("n")
            self.l += [f"{self.u('T')} {a} 0 {m} 0 Z0={z:.1f} TD={td / 2:.4g}",
                       f"{self.u('C')} {m} 0 {cvia:.3g}",
                       f"{self.u('T')} {m} 0 {b} 0 Z0={z:.1f} TD={td / 2:.4g}"]
        else:
            self.l.append(f"{self.u('T')} {a} 0 {b} 0 Z0={z:.1f} TD={td:.4g}")

    def stub(self, a, z, td, cend=1e-15):
        e = self.u("n")
        self.tline(a, e, z, td)
        self.l.append(f"{self.u('C')} {e} 0 {cend:.3g}")
        return e

    def pin(self, key, v, corner, ext, vcc, drive=None):
        """A part's pin on node ext. drive: None (receiver), 'r' or 'f'. Returns the die node."""
        die = self.u("die")
        r, l, c = self.p.pkg[key]
        mid = self.u("n")
        self.l += [f"{self.u('R')} {ext} {mid} {r:g}", f"{self.u('L')} {mid} {die} {l:g}",
                   f"{self.u('C')} {ext} 0 {c:g}"]
        name = f"{key}_{v}_{corner}"
        self.used.add((key, v, corner))
        ku, kd = self.u("ku"), self.u("kd")
        if drive is None:
            self.l += [f"V{ku} {ku} 0 0", f"V{kd} {kd} 0 0"]
        else:
            tr, tf = self.p.ramp[(key, v, corner)]
            t = tr if drive == "r" else tf
            up = f"PWL(0 0 {T0:g} 0 {T0 + t:g} 1)"
            dn = f"PWL(0 1 {T0:g} 1 {T0 + t:g} 0)"
            self.l += [f"V{ku} {ku} 0 {up if drive == 'r' else dn}",
                       f"V{kd} {kd} 0 {dn if drive == 'r' else up}"]
        self.l.append(f"X{self.u('x')} {die} {vcc} {ku} {kd} {name}")
        # a 5 V tolerant pin draws nothing above its supply, so without this the
        # starting point of a line held high is not defined
        self.l.append(f"{self.u('R')} {die} 0 1Meg")
        if drive == "f":                 # starts high: tell the solver so
            volts = next(x.split()[3] for x in self.l if x.split()[1:2] == [vcc] and x[0] == "V")
            self.l.append(f".nodeset v({die})={volts} v({ext})={volts}")
        return die

    def text(self, probes):
        subs = "\n".join(self.p.sub[k] for k in sorted(self.used))
        ctl = (".control\nset wr_singlescale\nset wr_vecnames\n"
               f"tran 20p {self.tend:g}\nwrdata OUTFILE {' '.join(f'v({p})' for p in probes)}\n.endc\n.end\n")
        return "\n".join(self.l) + "\n" + subs + "\n" + ctl


def run(name, deck, probes):
    f = OUT / f"{name}.cir"
    data = OUT / f"{name}.txt"
    f.write_text(deck.text(probes).replace("OUTFILE", str(data)))
    r = subprocess.run(["ngspice", "-b", str(f)], capture_output=True, text=True, timeout=300)
    if not data.exists():
        raise RuntimeError(f"ngspice failed on {f.name}:\n{r.stdout[-800:]}{r.stderr[-800:]}")
    rows = [[float(x) for x in ln.split()] for ln in data.read_text().splitlines()[1:] if ln.strip()]
    data.unlink()
    return [r[0] for r in rows], {p: [r[i + 1] for r in rows] for i, p in enumerate(probes)}


# ---------------------------------------------------------------- analysis
def crossings(t, v, level):
    out = []
    for i in range(1, len(v)):
        if (v[i - 1] - level) * (v[i] - level) < 0 or (v[i] == level and v[i - 1] != level):
            out.append(t[i - 1] + (t[i] - t[i - 1]) * (level - v[i - 1]) / (v[i] - v[i - 1])
                       if v[i] != v[i - 1] else t[i])
    return out


def look(t, v, edge, lo, hi, mid):
    """Figures of one edge at a receiver."""
    n = {lv: len(crossings(t, v, lv)) for lv in (lo, mid, hi)}
    first = crossings(t, v, mid)
    valid = crossings(t, v, hi if edge == "r" else lo)
    return {"max": max(v), "min": min(v),
            "delay": (first[0] - T0) if first else None,           # from the start of the driver's edge
            "valid": (valid[-1] - T0) if valid else None,          # last entry into the valid band
            "once": all(c == 1 for c in n.values()), "n": n}


# --------------------------------------------------------------- the lines
def load_lines():
    # per-net copper lengths, taken from the routed boards
    d = json.loads((HERE / "lengths.json").read_text())
    return d["rt_base"], d["rt_simm_adapter"], d["rt_dip_adapter"]


LEGACY = {}                                             # role -> IO number, as spec/ramspec.py
for i in range(40):
    LEGACY[f"DQ{i}"] = i
for i in range(12):
    LEGACY[f"MA{i}"] = 40 + i
for i in range(4):
    LEGACY[f"RAS{i}_N"] = 52 + i
    LEGACY[f"CAS{i}_N"] = 56 + i
LEGACY.update({"WE0_N": 60, "WE1_N": 61, "OE0_N": 62, "OE1_N": 63})


def klass(role):
    return "DQ" if role.startswith("DQ") else "MA" if role.startswith("MA") else role[:-3].rstrip("0123456789")


# ASSUMED loads at the module: (heavy, light) pF behind the socket contact.
# Heavy: a 72-pin module with many chips on a line; light: a small module.
LOAD = {"MA": (50e-12, 15e-12), "WE": (50e-12, 15e-12), "RAS": (25e-12, 10e-12),
        "CAS": (25e-12, 10e-12), "OE": (25e-12, 10e-12), "DQ": (10e-12, 7e-12)}
SOCKET_L = 4e-9                    # ASSUMED SIMM socket contact
MODULE = (60.0, 30 * 6.5e-12)      # ASSUMED trace on the module: 60 ohm, 30 mm
SLOT = (1.5e-9, 0.5e-12, 0.3e-12)  # ASSUMED slot contact: series L, C at the socket, C of the finger


def target(snet):
    """The socket pad a test drives: the 72-pin socket if the net reaches it."""
    pads = {k: v for k, v in snet["to"].items() if k.startswith("XU") and v is not None}
    for k in sorted(pads):
        if k.startswith("XU1."):
            return k, pads[k]
    k = max(pads, key=pads.get)
    return k, pads[k]


def stage1(parts, base, cart, role, rser, edge, corner, tend=TEND):
    """FPGA pad -> FIO -> R -> IO -> slot -> F line -> level-shifter A pin."""
    n = LEGACY[role]
    d = Deck(parts, f"stage 1 {role} R={rser} {edge}")
    d.tend = tend
    d.l.append("Vcc vcc 0 3.3")
    d.pin("ecp5", "33", corner, "ball", "vcc", edge)
    d.tline("ball", "ra", *line(base[f"FIO{n}"], "base"))
    d.l += [f"Rser ra rb {max(rser, 0.01)}", "Crn rb 0 0.5p"]
    d.tline("rb", "slot", *line(base[f"IO{n}"], "base"))
    d.l += [f"Cslot slot 0 {SLOT[1]:g}", f"Lslot slot edge {SLOT[0]:g}", f"Cedge edge 0 {SLOT[2]:g}"]
    d.tline("edge", "rx", *line(cart[f"F_{role}"], "cart"))
    die = d.pin("lvc8t245", "33", corner, "rx", "vcc")
    if role.startswith("DQ"):                            # the receive buffer's output shares the line
        d.pin("lvc245a", "33", corner, "rx", "vcc")
    t, v = run(f"s1_{role}_{int(rser)}_{edge}_{corner}", d, ["rx", die])
    return look(t, v["rx"], edge, 0.8, 2.0, 1.5), min(v[die])


def stage2(parts, cart, role, vdut, edge, corner, heavy, rser=SERIES, tend=TEND):
    """Level shifter B pin -> D line -> R -> S line -> socket -> module -> DRAM pin."""
    v = "50" if vdut > 4 else "33"
    dnet, snet = cart[f"D_{role}"], cart[f"S_{role}"]
    pad, dist = target(snet)
    d = Deck(parts, f"stage 2 {role} {vdut} V {edge}")
    d.tend = tend
    d.l.append(f"Vcc vcc 0 {vdut}")
    d.pin("lvc8t245", v, corner, "drv", "vcc", edge)
    zc, tpd = LAYER["cart"]["F.Cu"]
    rn = next(x for k, x in dnet["to"].items() if k.startswith("RN"))
    d.tline("drv", "ra", zc, rn * tpd * 1e-12)
    others = [x for k, x in dnet["to"].items() if k.startswith("U") and x and x > rn]
    if others:                                           # the receive buffer's input hangs off this node
        e = d.stub("ra", zc, (max(others) - rn) * tpd * 1e-12)
        d.l.append("V33 v33 0 3.3")
        d.pin("lvc245a", "33", corner, e, "v33")
    d.l += [f"Rser ra rb {max(rser, 0.01)}", "Crn rb 0 0.5p"]
    z, td, cv = line(snet, "cart", dist / snet["mm"])
    zr, tdr, _ = line(snet, "cart", max(snet["mm"] - dist, 0.1) / snet["mm"])
    d.tline("rb", "mid", z, td / 2, cv / 2)
    d.stub("mid", zr, tdr, 2e-12)                        # other sockets and the sense buffer
    d.tline("mid", "sock", z, td / 2, cv / 2)
    d.l.append(f"Lsock sock mod {SOCKET_L:g}")
    d.tline("mod", "dram", *MODULE)
    d.l.append(f"Cload dram 0 {LOAD[klass(role)][0 if heavy else 1]:g}")
    t, w = run(f"s2_{role}_{v}_{edge}_{corner}_{int(heavy)}_{int(rser)}", d, ["dram"])
    return look(t, w["dram"], edge, 0.8, 2.4, 1.5)


def stage3a(parts, cart, role, vdut, edge, corner, tend=TEND):
    """DRAM data pin -> module -> socket -> S line -> R -> receive buffer input."""
    v = "50" if vdut > 4 else "33"
    dnet, snet = cart[f"D_{role}"], cart[f"S_{role}"]
    pad, dist = target(snet)
    d = Deck(parts, f"stage 3a {role} {vdut} V {edge}")
    d.tend = tend
    d.l += [f"Vcc vcc 0 {vdut}", "V33 v33 0 3.3"]
    d.pin("dram", v, corner, "dram", "vcc", edge)
    d.tline("dram", "mod", *MODULE)
    d.l.append(f"Lsock mod sock {SOCKET_L:g}")
    z, td, cv = line(snet, "cart", dist / snet["mm"])
    zr, tdr, _ = line(snet, "cart", max(snet["mm"] - dist, 0.1) / snet["mm"])
    d.tline("sock", "mid", z, td / 2, cv / 2)
    d.stub("mid", zr, tdr, 2e-12)
    d.tline("mid", "rb", z, td / 2, cv / 2)
    d.l += [f"Rser rb ra {SERIES}", "Crn rb 0 0.5p"]
    zc, tpd = LAYER["cart"]["F.Cu"]
    rn = next(x for k, x in dnet["to"].items() if k.startswith("RN"))
    far = max(x for k, x in dnet["to"].items() if k.startswith("U") and x)
    d.tline("ra", "off", zc, rn * tpd * 1e-12)
    d.pin("lvc8t245", v, corner, "off", "vcc")           # the driver, switched off
    d.tline("ra", "rx", zc, max(far - rn, 0.1) * tpd * 1e-12)
    die = d.pin("lvc245a", "33", corner, "rx", "v33")
    t, w = run(f"s3a_{role}_{v}_{edge}_{corner}", d, ["rx", die])
    return look(t, w["rx"], edge, 0.8, 2.0, 1.5), min(w[die])


def stage3b(parts, base, cart, role, rser, edge, corner, tend=TEND, rsrc=0.0):
    """Receive buffer output -> F line -> slot -> IO -> R -> FIO -> FPGA pad."""
    n = LEGACY[role] if role in LEGACY else None
    d = Deck(parts, f"stage 3b {role} R={rser} {edge}")
    d.tend = tend
    d.l.append("Vcc vcc 0 3.3")
    d.pin("lvc245a", "33", corner, "drv", "vcc", edge)
    d.pin("lvc8t245", "33", corner, "drv", "vcc")        # the level shifter's input on the same pad group
    d.l.append(f"Rsrc drv drs {max(rsrc, 0.01)}")        # not on the board: a what-if
    d.tline("drs", "edge", *line(cart[f"F_{role}"], "cart"))
    d.l += [f"Cedge edge 0 {SLOT[2]:g}", f"Lslot edge slot {SLOT[0]:g}", f"Cslot slot 0 {SLOT[1]:g}"]
    d.tline("slot", "rb", *line(base[f"IO{n}"], "base"))
    d.l += [f"Rser rb ra {max(rser, 0.01)}", "Crn rb 0 0.5p"]
    d.tline("ra", "ball", *line(base[f"FIO{n}"], "base"))
    die = d.pin("ecp5", "33", corner, "ball", "vcc")
    t, w = run(f"s3b_{role}_{int(rser)}_{edge}_{corner}_{int(rsrc)}_{int(tend * 1e9)}", d, [die])
    return look(t, w[die], edge, 0.8, 2.0, 1.5)


# ------------------------------------------------------------------ report
fail = 0


def say(kind, name, text):
    global fail
    if kind == "FAIL":
        fail = 1
    print(f"{kind:<5} {name}: {text}")


def chk(name, ok, text):
    say("PASS" if ok else "FAIL", name, text)


def ns(x):
    return "none" if x is None else f"{x * 1e9:.2f} ns"


def main():
    OUT.mkdir(exist_ok=True)
    parts = Parts()
    base, cart, dip = load_lines()
    for n in parts.notes:
        say("INFO", "model", n)
    for kind, lay in LAYER.items():
        say("INFO", f"lines, {kind}", ", ".join(f"{k} {z:.0f} ohm {t:.2f} ps/mm" for k, (z, t) in lay.items()))
    pool = concurrent.futures.ThreadPoolExecutor(max_workers=os.cpu_count() or 4)

    roles = [r for r in LEGACY if f"F_{r}" in cart]
    total = {r: base[f"FIO{LEGACY[r]}"]["mm"] + base[f"IO{LEGACY[r]}"]["mm"] + cart[f"F_{r}"]["mm"] for r in roles}
    short, long_ = min(total, key=total.get), max(total, key=total.get)
    say("INFO", "FPGA to level shifter", f"{len(roles)} lines, {total[short]:.0f} mm ({short}) to {total[long_]:.0f} mm ({long_})")

    # ---- 1. FPGA pad to the level shifter's input
    j = {(role, rser, e): pool.submit(stage1, parts, base, cart, role, rser, e, 2)
         for role in (short, long_) for rser in (0.0, 22.0, 33.0, 47.0) for e in "rf"}
    for role in (short, long_):
        for rser in (0.0, 22.0, 33.0, 47.0):
            (r, rmin), (f, fmin) = j[(role, rser, "r")].result(), j[(role, rser, "f")].result()
            ima = parts.clamp_ma("lvc8t245", "33", min(rmin, fmin))
            text = (f"{role} {total[role]:.0f} mm, {rser:.0f} ohm: {f['min']:.2f} V to {r['max']:.2f} V at the pin, "
                    f"clamp {ima:.1f} mA, valid after {ns(max(r['valid'] or 9, f['valid'] or 9))}, "
                    f"each threshold crossed once: {'yes' if r['once'] and f['once'] else 'no'}")
            if rser == SERIES:
                chk("1 level-shifter input", r["max"] <= 6.5 and (f["min"] >= -0.5 or ima <= 50) and r["once"] and f["once"], text)
            else:
                say("INFO", "1 other series value", text)

    # ---- 2. level shifter to the DRAM pin
    # lines that reach a socket (the spare data lines and three strobes do not)
    sock = [r for r in roles if any(k.startswith("XU") and x is not None
                                    for k, x in cart[f"S_{r}"]["to"].items())]
    say("INFO", "level shifter to socket", f"{len(sock)} lines reach a socket; not wired to one: "
        + ", ".join(r for r in roles if r not in sock))
    strobes = [r for r in sock if klass(r) in ("RAS", "CAS", "WE", "OE")]
    dist = {r: target(cart[f"S_{r}"])[1] for r in sock}

    def pick(k):
        rs = [r for r in sock if klass(r) == k]
        return sorted({min(rs, key=dist.get), max(rs, key=dist.get)}, key=dist.get)
    cases = [(r, v, h) for r in strobes + pick("MA") + pick("DQ") for v in (5.0, 3.3) for h in (True, False)]
    jobs = {(r, v, h, e): pool.submit(stage2, parts, cart, r, v, e, 2, h) for r, v, h in cases for e in "rf"}
    worst = {}
    for (r, v, h) in cases:
        a, b = jobs[(r, v, h, "r")].result(), jobs[(r, v, h, "f")].result()
        k = (klass(r), v)
        w = worst.setdefault(k, {"max": -9, "min": 9, "bad": [], "valid": 0, "rmax": "", "rmin": ""})
        if a["max"] > w["max"]:
            w["max"], w["rmax"] = a["max"], f"{r}, {'heavy' if h else 'light'} load"
        if b["min"] < w["min"]:
            w["min"], w["rmin"] = b["min"], f"{r}, {'heavy' if h else 'light'} load"
        w["valid"] = max(w["valid"], a["valid"] or 9, b["valid"] or 9)
        for e, x in (("rising", a), ("falling", b)):
            if not x["once"]:
                w["bad"].append(f"{r} {e} {'heavy' if h else 'light'} {x['n']}")
    for (k, v), w in sorted(worst.items(), key=lambda kv: (-kv[0][1], kv[0][0])):
        chk(f"2 DRAM pin limits, {k} at {v:g} V", w["min"] >= -1.0 and w["max"] <= v + 1.0,
            f"{w['min']:.2f} V ({w['rmin']}) to {w['max']:.2f} V ({w['rmax']}); allowed -1.0 V to {v + 1:.1f} V")
        if k in ("RAS", "CAS", "WE", "OE"):
            chk(f"2 strobe edges, {k} at {v:g} V", not w["bad"],
                "each of 0.8, 1.5 and 2.4 V crossed once on every edge" if not w["bad"] else
                f"{len(w['bad'])} edges cross a threshold more than once, for example {w['bad'][0]}")
        else:
            say("INFO", f"2 settling, {k} at {v:g} V", f"valid at the DRAM pin {ns(w['valid'])} after the driver starts")

    # what series value would it take? the worst strobe and the worst address line at 5 V
    wr = max((r for r in strobes), key=lambda r: jobs[(r, 5.0, False, "r")].result()["max"])
    j = {(rser, e): pool.submit(stage2, parts, cart, wr, 5.0, e, 2, False, rser)
         for rser in (22.0, 33.0, 47.0, 68.0) for e in "rf"}
    for rser in (22.0, 33.0, 47.0, 68.0):
        a, b = j[(rser, "r")].result(), j[(rser, "f")].result()
        say("INFO", "2 series value", f"{wr}, 5 V, light load, {rser:.0f} ohm: {b['min']:.2f} V to {a['max']:.2f} V, "
            f"crossed once: {'yes' if a['once'] and b['once'] else 'no'}, valid after {ns(max(a['valid'] or 9, b['valid'] or 9))}")

    # ---- 3. the way back
    dq = [r for r in sock if klass(r) == "DQ"]
    j = {(r, v, e): pool.submit(stage3a, parts, cart, r, v, e, 2) for r in pick("DQ") for v in (5.0, 3.3) for e in "rf"}
    for v in (5.0, 3.3):
        lo, hi, ima, val = 9, -9, 0, 0
        for r in pick("DQ"):
            (a, amin), (b, bmin) = j[(r, v, "r")].result(), j[(r, v, "f")].result()
            lo, hi = min(lo, b["min"]), max(hi, a["max"])
            ima = max(ima, parts.clamp_ma("lvc245a", "33", min(amin, bmin)))
            val = max(val, a["valid"] or 9, b["valid"] or 9)
        chk(f"3 receive buffer input, {v:g} V module", hi <= 6.5 and (lo >= -0.5 or ima <= 50),
            f"{lo:.2f} V to {hi:.2f} V, clamp {ima:.1f} mA, valid after {ns(val)}; allowed -0.5 V to 6.5 V or 50 mA in the clamp")
    fl = {r: total[r] for r in dq}
    ends = (min(fl, key=fl.get), max(fl, key=fl.get))
    j = {(role, rser, e): pool.submit(stage3b, parts, base, cart, role, rser, e, 2)
         for role in ends for rser in (0.0, 22.0, 33.0, 47.0) for e in "rf"}
    for role in ends:
        for rser in (0.0, 22.0, 33.0, 47.0):
            a, b = j[(role, rser, "r")].result(), j[(role, rser, "f")].result()
            text = (f"{role} {total[role]:.0f} mm, {rser:.0f} ohm: {b['min']:.2f} V to {a['max']:.2f} V at the pad, "
                    f"valid after {ns(max(a['valid'] or 9, b['valid'] or 9))}; allowed -0.5 V to 3.63 V")
            if rser == SERIES:
                chk("3 FPGA input", b["min"] >= -0.5 and a["max"] <= 3.63, text)
            else:
                say("INFO", "3 other series value", text)

    # a resistor at the receive buffer's output instead (there is none on the cartridge)
    j = {(rs, e): pool.submit(stage3b, parts, base, cart, ends[1], SERIES, e, 2, TEND, rs)
         for rs in (33.0, 47.0, 68.0) for e in "rf"}
    for rs in (33.0, 47.0, 68.0):
        a, b = j[(rs, "r")].result(), j[(rs, "f")].result()
        say("INFO", "3 resistor at the buffer", f"{ends[1]}, {rs:.0f} ohm at the receive buffer's output as well: "
            f"{b['min']:.2f} V to {a['max']:.2f} V at the FPGA pad, valid after {ns(max(a['valid'] or 9, b['valid'] or 9))}")

    # ---- how much hangs on the least certain figure, the cartridge's inner layer
    keep = LAYER["cart"]["In2.Cu"]
    LAYER["cart"]["In2.Cu"] = (LAYER["cart"]["F.Cu"][0], keep[1])
    ja = [pool.submit(stage2, parts, cart, wr, 5.0, e, 2, False) for e in "rf"]
    jb = [pool.submit(stage3b, parts, base, cart, ends[1], SERIES, e, 2) for e in "rf"]
    a, b, c, e = ja[0].result(), ja[1].result(), jb[0].result(), jb[1].result()
    LAYER["cart"]["In2.Cu"] = keep
    say("INFO", "sensitivity", f"with the cartridge's inner layer at {LAYER['cart']['F.Cu'][0]:.0f} ohm instead of {keep[0]:.0f} ohm: "
        f"{wr} at the DRAM pin {b['min']:.2f} V to {a['max']:.2f} V (5 V, light load); "
        f"{ends[1]} at the FPGA pad {e['min']:.2f} V to {c['max']:.2f} V")

    # ---- 4. delays, typical corner, every line
    j1 = {r: pool.submit(stage1, parts, base, cart, r, SERIES, "r", 0, QUICK) for r in roles}
    j2 = {(r, h): pool.submit(stage2, parts, cart, r, 5.0, "f", 0, h, SERIES, QUICK) for r in sock for h in (True, False)}
    j3a = {r: pool.submit(stage3a, parts, cart, r, 5.0, "r", 0, QUICK) for r in dq}
    j3b = {r: pool.submit(stage3b, parts, base, cart, r, SERIES, "r", 0, QUICK) for r in dq}
    d1 = {r: j1[r].result()[0]["delay"] for r in roles}
    d2 = {k: j.result()["delay"] for k, j in j2.items()}
    d3a = {r: j3a[r].result()[0]["delay"] for r in dq}
    d3b = {r: j3b[r].result()["delay"] for r in dq}

    def span(d, keys=None):
        vals = {k: v for k, v in d.items() if v is not None and (keys is None or k in keys)}
        a, b = min(vals, key=vals.get), max(vals, key=vals.get)
        return vals[a], vals[b], a, b
    lo, hi, a, b = span(d1)
    say("INFO", "4 FPGA to level shifter", f"{ns(lo)} ({a}) to {ns(hi)} ({b}), wiring and edge only")
    for h in (True, False):
        lo, hi, a, b = span({r: v for (r, hh), v in d2.items() if hh == h})
        say("INFO", f"4 level shifter to DRAM pin, {'heavy' if h else 'light'} load", f"{ns(lo)} ({a}) to {ns(hi)} ({b})")
    lo, hi, a, b = span(d3a)
    say("INFO", "4 DRAM pin to receive buffer", f"{ns(lo)} ({a}) to {ns(hi)} ({b})")
    lo, hi, a, b = span(d3b)
    say("INFO", "4 receive buffer to FPGA", f"{ns(lo)} ({a}) to {ns(hi)} ({b})")
    # out: a strobe to the module; back: a data line to the FPGA. Buffers not included.
    for h in (True, False):
        outs = {r: d1[r] + d2[(r, h)] for r in strobes if d1[r] and d2[(r, h)]}
        backs = {r: d3a[r] + d3b[r] for r in dq if d3a[r] and d3b[r]}
        o0, o1, b0, b1 = min(outs.values()), max(outs.values()), min(backs.values()), max(backs.values())
        say("INFO", f"4 wiring, {'heavy' if h else 'light'} load",
            f"strobe out {ns(o0)} to {ns(o1)}, data back {ns(b0)} to {ns(b1)}; "
            f"round trip {ns(o0 + b0)} to {ns(o1 + b1)}, spread {ns(o1 + b1 - o0 - b0)}")
        if h:
            heavy_rt = (o0 + b0, o1 + b1)
        else:
            light_rt = (o0 + b0, o1 + b1)
    spread = max(heavy_rt[1], light_rt[1]) - min(heavy_rt[0], light_rt[0])
    # Datasheet propagation delays at 3.3 V / 5 V, to add to the wiring: see BUFFER_NS.
    lo_b, hi_b = BUFFER_NS
    say("INFO", "4 with the buffers",
        f"round trip {(min(heavy_rt[0], light_rt[0]) * 1e9 + lo_b):.1f} ns to {(max(heavy_rt[1], light_rt[1]) * 1e9 + hi_b):.1f} ns; "
        "the whole-chain simulation assumes 9.0 ns for every line")
    # No limit is set for this in the project, so it is a figure, not a check.
    say("INFO", "4 line-to-line spread",
        f"{ns(spread)} between the fastest and the slowest round trip over all lines and both loads "
        "(wiring only). A speed reading is off by up to this much unless each line is calibrated")
    return fail


# Sum of the two buffers' propagation delays, minimum and maximum, ns, from
# the Texas Instruments datasheets (-40 to 85 C): SN74LVC8T245 A to B with
# VCCA 3.3 V and VCCB 5 V, 0.5 to 4.4 ns; SN74LVC245A at 3.3 V, 1.5 to 6.3 ns
# (typical 3.8 ns). With VCCB at 3.3 V the level shifter is 0.8 to 6.3 ns.
BUFFER_NS = (2.0, 10.7)

if __name__ == "__main__":
    sys.exit(main())
