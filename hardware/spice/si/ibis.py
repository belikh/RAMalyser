"""Reads the parts of an IBIS file this simulation uses, and writes them as
ngspice sub-circuits. The file is data: nothing in it is executed.

A model becomes:
  receiver  pin vcc          C_comp, the ground clamp and the power clamp
  driver    pin vcc ku kd    the same plus pull-up and pull-down tables, each
                             scaled by a 0..1 control voltage (ku, kd)
The control voltages ramp over the time the [Ramp] entry gives for the
20 % to 80 % swing, stretched to the full swing. This is the two-table
method of IBIS 2; the V-t waveform tables are not used.
"""
import re

NUM = re.compile(r"^[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?")
SUFFIX = {"f": 1e-15, "p": 1e-12, "n": 1e-9, "u": 1e-6, "m": 1e-3, "k": 1e3}


def num(tok):
    if tok.upper() == "NA":
        return None
    m = NUM.match(tok)
    if not m:
        raise ValueError(tok)
    v = float(m.group(0))
    rest = tok[m.end():]
    return v * SUFFIX.get(rest[:1].lower(), 1.0) if rest else v


def parse(path):
    """{'package': {component: {R,L,C: (typ,min,max)}}, 'models': {name: {...}}}"""
    out = {"package": {}, "models": {}}
    comp = model = table = None
    in_pkg = False
    for raw in open(path, encoding="latin-1"):
        line = raw.split("|")[0].rstrip()
        if not line.strip():
            continue
        m = re.match(r"^\[([^\]]+)\]\s*(.*)$", line)
        if m:
            key, rest = m.group(1).strip().lower(), m.group(2).strip()
            table, in_pkg = None, False
            if key == "component":
                comp = rest
                out["package"][comp] = {}
            elif key == "package":
                in_pkg = True
            elif key == "model":
                model = out["models"].setdefault(rest, {"tables": {}})
            elif model is not None and key in ("pulldown", "pullup", "gnd clamp", "power clamp"):
                table = model["tables"].setdefault(key, [])
            elif model is not None and key == "ramp":
                table = "ramp"
            elif model is not None and key == "voltage range":
                model["vcc"] = [num(t) for t in rest.split()[:3]]
            elif key in ("pin", "end"):
                model = model if key == "pin" else None
            continue
        tok = line.split()
        if in_pkg and comp and tok[0] in ("R_pkg", "L_pkg", "C_pkg"):
            out["package"][comp][tok[0][0]] = [num(t) for t in tok[1:4]]
        elif model is not None and table == "ramp" and tok[0] in ("dV/dt_r", "dV/dt_f"):
            model[tok[0]] = [tuple(float(x) for x in t.split("/")) for t in tok[1:4]]
        elif model is not None and isinstance(table, list) and len(tok) >= 4:
            try:
                table.append([num(t) for t in tok[:4]])
            except ValueError:
                pass
        elif model is not None and tok[0] == "C_comp":
            model["c_comp"] = [num(t) for t in tok[1:4]]
        elif model is not None and tok[0] == "Model_type":
            model["type"] = tok[1]
    return out


def _pwl(expr, rows, col):
    pts = [(v, r[col] if r[col] is not None else r[1]) for r in rows for v in [r[0]]]
    pts.sort()
    body = ", ".join(f"{v:.5g}, {i:.5g}" for v, i in pts)
    # held at the end values outside the table: a straight-line continuation
    # of the last two rows can cross zero and give the pin a false resting point
    return f"pwl(min(max({expr}, {pts[0][0]:.5g}), {pts[-1][0]:.5g}), {body})"


def subckt(name, model, corner=0, driver=False):
    """ngspice text. corner: 0 typ, 1 min (slow, weak), 2 max (fast, strong)."""
    col = 1 + corner
    t = model["tables"]
    cc = model["c_comp"][corner] or model["c_comp"][0]
    lines = [f".subckt {name} pin vcc" + (" ku kd" if driver else ""),
             f"Cc pin 0 {cc:.4g}"]
    if t.get("gnd clamp"):
        lines.append("Bgc pin 0 I = " + _pwl("v(pin)", t["gnd clamp"], col))
    if t.get("power clamp"):
        lines.append("Bpc pin 0 I = " + _pwl("v(vcc)-v(pin)", t["power clamp"], col))
    if driver:
        lines.append("Bpd pin 0 I = v(kd) * " + _pwl("v(pin)", t["pulldown"], col))
        lines.append("Bpu pin 0 I = v(ku) * " + _pwl("v(vcc)-v(pin)", t["pullup"], col))
    lines.append(f".ends {name}")
    # ngspice wants continuation lines for long expressions
    text = []
    for l in lines:
        while len(l) > 900:
            cut = l.rfind(", ", 0, 900) + 1
            text.append(l[:cut])
            l = "+ " + l[cut:].lstrip()
        text.append(l)
    return "\n".join(text)


def ramp_times(model, corner=0):
    """(rise, fall) full-swing times in seconds from the 20-80 % ramp."""
    r, f = model["dV/dt_r"][corner], model["dV/dt_f"][corner]
    return r[1] / 0.6, f[1] / 0.6
