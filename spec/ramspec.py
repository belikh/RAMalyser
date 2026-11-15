"""Single source of truth for the tester's electrical interface.

Everything that has to agree between gateware, firmware, schematics and docs
is defined here as plain data: the base<->adapter bus, the role each bus line
plays for a given adapter profile, and the pin map of every DUT socket.

Run `python3 ramspec.py --check` to validate, `--emit DIR` to write the
generated JSON / Verilog / Rust / Markdown views.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

# --------------------------------------------------------------------------
# Base <-> adapter bus: one PCI Express x16 card edge, 164 contacts.
# The adapter (cartridge) carries the gold fingers; the base carries a
# right-angle socket. Only the mechanical connector is PCIe: the pin
# assignment is this project's own and nothing here is PCIe compatible.
#   Contacts are named as on the connector: B1..B82 on the card's top side,
#   A1..A82 on its underside; the key notch sits between positions 11 and 12.
#   116 generic 3.3 V FPGA lines (IO0..IO115), never 5 V.
#   Adapters for 5 V memory MUST buffer every socket pin (see check_adapter).
# --------------------------------------------------------------------------
N_IO = 116
CONNECTORS = ("J1",)
POSITIONS = 82

MISC = [
    "ID_SDA", "ID_SCL",          # adapter identity EEPROM (0x50) on its own I2C bus
    "STRAP_5V_OK",               # adapter ties to GND to allow VDUT > 3.8 V
    "ADP_PRESENT_N",             # adapter ties to GND; base pulls up
    "DUT_PG",                    # base -> adapter: VDUT is on and in range
    "VDUT_SENSE",                # Kelvin sense of VDUT at the socket
    "SPD_SDA", "SPD_SCL",        # DUT-side I2C (SPD EEPROM on DIMMs), separate bus
    "SPD_WP",                    # DIMM SPD write-protect, adapter pulls to "protected"
    "AUX_EN",                    # base -> adapter: enable adapter aux rails (4116)
    "AUX_OK",                    # adapter -> base: aux rails sequenced and good
    "ADP_INT_N",
]
SPARE: list[str] = []


def _bus() -> dict[str, str]:
    """Contact name -> net, both sides of every position."""
    out: dict[str, str] = {}

    def put(pos: int, top: str, bottom: str) -> None:
        out[f"B{pos}"], out[f"A{pos}"] = top, bottom

    for pos in range(1, 5):
        put(pos, "VDUT", "VDUT")
    put(5, "+3V3", "+3V3")
    put(6, "+5V", "+5V")
    put(7, "GND", "GND")
    put(8, "GND", "GND")
    for i, pos in enumerate(range(9, 15)):          # 9..11 short side, 12..14 long side
        put(pos, MISC[2 * i], MISC[2 * i + 1])
    io = 0
    for k, pos in enumerate(range(15, POSITIONS + 1)):
        if k % 7 == 3:
            put(pos, "GND", "GND")
        else:
            put(pos, f"IO{io}", f"IO{io + 1}")
            io += 2
    assert io == N_IO, io
    return out


BUS = {"J1": _bus()}
CONTACTS = [f"{side}{pos}" for pos in range(1, POSITIONS + 1) for side in "BA"]

# --------------------------------------------------------------------------
# "Legacy" profile: how asynchronous DRAM adapters (SIMM, DIP) use IO lines.
# Drive path: 74LVC8T245 (B side = VDUT). Receive path: 74LVC245A at 3.3 V,
# giving TTL thresholds at any DUT voltage. Both share one FPGA line per DQ.
# --------------------------------------------------------------------------
LEGACY = {}
for i in range(40):
    LEGACY[f"DQ{i}"] = i                     # 0..31 data, 32..35 parity, 36..39 spare/ECC
for i in range(12):
    LEGACY[f"MA{i}"] = 40 + i
for i in range(4):
    LEGACY[f"RAS{i}_N"] = 52 + i
    LEGACY[f"CAS{i}_N"] = 56 + i
LEGACY["WE0_N"], LEGACY["WE1_N"] = 60, 61
LEGACY["OE0_N"], LEGACY["OE1_N"] = 62, 63
for i in range(5):
    LEGACY[f"DQ_DRV_EN{i}_N"] = 64 + i       # per byte lane driver enable (lane 4 = parity)
LEGACY["DQ_RX_EN_N"] = 69
LEGACY["CTL_DRV_EN_N"] = 70
for i in range(8):
    LEGACY[f"SENSE{i}"] = 71 + i             # read-back of control/ID lines, 8 at a time
for i in range(5):
    LEGACY[f"SENSE_SEL{i}_N"] = 79 + i
LEGACY["PULL_A"], LEGACY["PULL_B"] = 84, 85  # weak pull nodes: even / odd DQ bits
for i in range(4):
    LEGACY[f"ADP{i}"] = 86 + i               # adapter-specific (socket select etc.)
# IO90..IO115 are unused by legacy adapters (reserved for wide/synchronous DUTs).

CTL_SIGNALS = ([f"MA{i}" for i in range(12)] + [f"RAS{i}_N" for i in range(4)] +
               [f"CAS{i}_N" for i in range(4)] + ["WE0_N", "WE1_N", "OE0_N", "OE1_N"])

# Sense groups: SENSE_SELn_N low puts group n on SENSE0..7.
SENSE_GROUPS = [
    [f"MA{i}" for i in range(8)],
    ["MA8", "MA9", "MA10", "MA11", "WE0_N", "WE1_N", "OE0_N", "OE1_N"],
    [f"RAS{i}_N" for i in range(4)] + [f"CAS{i}_N" for i in range(4)],
    ["ID0", "ID1", "ID2", "ID3", "ID4", "ID5", "ID6", "ID7"],   # presence-detect etc.
    ["ID8", "ID9", "ID10", "ID11", "ID12", "ID13", "ID14", "ID15"],
]

# --------------------------------------------------------------------------
# DUT socket pin maps. Value is the tester signal name, a power net, or "NC".
# --------------------------------------------------------------------------

def _simm72() -> dict[int, str]:
    """72-pin SIMM, JEDEC / IBM "72 Pin SIMM Characteristics" (x32 and x36).

    Tester DQ numbering: DQ0..31 = data (byte n on CASn), DQ32..35 = parity
    for byte 0..3. ECC-optimised x36/x40 modules are not supported by this map.
    """
    m = {1: "GND", 10: "VDUT", 30: "VDUT", 39: "GND", 59: "VDUT", 72: "GND"}
    byte_pins = {
        0: [2, 4, 6, 8, 20, 22, 24, 26],
        1: [49, 51, 53, 55, 57, 61, 63, 65],
        2: [3, 5, 7, 9, 21, 23, 25, 27],
        3: [50, 52, 54, 56, 58, 60, 62, 64],
    }
    for b, pins in byte_pins.items():
        for i, p in enumerate(pins):
            m[p] = f"DQ{b * 8 + i}"
    m.update({36: "DQ32", 37: "DQ33", 35: "DQ34", 38: "DQ35"})
    for i, p in enumerate([12, 13, 14, 15, 16, 17, 18]):
        m[p] = f"MA{i}"
    m.update({28: "MA7", 31: "MA8", 32: "MA9", 19: "MA10", 29: "MA11"})
    m.update({44: "RAS0_N", 45: "RAS1_N", 34: "RAS2_N", 33: "RAS3_N"})
    m.update({40: "CAS0_N", 43: "CAS1_N", 41: "CAS2_N", 42: "CAS3_N"})
    m[47] = "WE0_N"
    # presence detect and otherwise-unused pins are only sensed, never driven
    m.update({67: "ID0", 68: "ID1", 69: "ID2", 70: "ID3", 11: "ID4",
              46: "ID5", 48: "ID6", 66: "ID7", 71: "ID8"})
    assert sorted(m) == list(range(1, 73))
    return m


def _simm30() -> dict[int, str]:
    """30-pin SIMM / SIPP. Parity: DP (in) on DQ32 drive, QP (out) on DQ33."""
    m = {1: "VDUT", 30: "VDUT", 9: "GND", 22: "GND"}
    for i, p in enumerate([3, 6, 10, 13, 16, 20, 23, 25]):
        m[p] = f"DQ{i}"
    for i, p in enumerate([4, 5, 7, 8, 11, 12, 14, 15, 17, 18, 19, 24]):
        m[p] = f"MA{i}"
    m.update({2: "CAS0_N", 21: "WE0_N", 27: "RAS0_N", 28: "CAS1_N",
              29: "DQ32", 26: "DQ33"})
    assert sorted(m) == list(range(1, 31))
    return m


def _dip(n: int, **kw: int | list[int]) -> dict[int, str]:
    m = {p: "NC" for p in range(1, n + 1)}
    for sig, pins in kw.items():
        for i, p in enumerate(pins if isinstance(pins, list) else [pins]):
            name = sig if not isinstance(pins, list) else f"{sig}{i}"
            m[p] = name
    return m


def _x1_16(vcc_pin: int, a7: int | None, a8: int | None, vbb: bool) -> dict[int, str]:
    """16-pin 1-bit DRAM family (4116 / 4164 / 41256). DIN on DQ0, DOUT on DQ1."""
    m = {p: "NC" for p in range(1, 17)}
    m.update({2: "DQ0", 3: "WE0_N", 4: "RAS0_N", 5: "MA0", 6: "MA2", 7: "MA1",
              10: "MA5", 11: "MA4", 12: "MA3", 13: "MA6", 14: "DQ1",
              15: "CAS0_N", 16: "GND"})
    m[vcc_pin] = "VDUT"
    if a7:
        m[a7] = "MA7"
    if a8:
        m[a8] = "MA8"
    if vbb:
        m[1] = "VBB_N5V"
        m[8] = "VDD_12V"
    return m


SOCKETS = {
    "SIMM72": _simm72(),
    "SIMM30": _simm30(),
    # 4116: VBB -5 V on 1, VDD +12 V on 8, VCC +5 V on 9 (VBB first on, last off)
    "DIP16_4116": _x1_16(vcc_pin=9, a7=None, a8=None, vbb=True),
    # 4164 / 41256: single +5 V on pin 8, A7 on 9, A8 on 1 (NC on 4164)
    "DIP16_4164_41256": _x1_16(vcc_pin=8, a7=9, a8=1, vbb=False),
    # 4416 / 4464 (x4, 18-pin)
    "DIP18_4416_4464": {
        1: "OE0_N", 2: "DQ0", 3: "DQ1", 4: "WE0_N", 5: "RAS0_N", 6: "MA6",
        7: "MA5", 8: "MA4", 9: "VDUT", 10: "MA7", 11: "MA3", 12: "MA2",
        13: "MA1", 14: "MA0", 15: "DQ2", 16: "CAS0_N", 17: "DQ3", 18: "GND"},
    # 411000 / 511000 (1M x1, 18-pin). DIN on DQ0, DOUT on DQ1. Pin 4 = TF (leave open)
    "DIP18_411000": {
        1: "DQ0", 2: "WE0_N", 3: "RAS0_N", 4: "NC", 5: "MA0", 6: "MA1",
        7: "MA2", 8: "MA3", 9: "VDUT", 10: "MA4", 11: "MA5", 12: "MA6",
        13: "MA7", 14: "MA8", 15: "MA9", 16: "CAS0_N", 17: "DQ1", 18: "GND"},
    # 44256 / 514256 (256K x4) and 514400 (1M x4, A9 on pin 5), 20-pin DIP
    "DIP20_44256_514400": {
        1: "DQ0", 2: "DQ1", 3: "WE0_N", 4: "RAS0_N", 5: "MA9", 6: "MA0",
        7: "MA1", 8: "MA2", 9: "MA3", 10: "VDUT", 11: "MA4", 12: "MA5",
        13: "MA6", 14: "MA7", 15: "MA8", 16: "OE0_N", 17: "CAS0_N",
        18: "DQ2", 19: "DQ3", 20: "GND"},
    # 44256 / 514256 / 514400 in 20-pin ZIP (zig-zag in-line)
    "ZIP20_44256_514400": {
        1: "OE0_N", 2: "CAS0_N", 3: "DQ2", 4: "DQ3", 5: "GND", 6: "DQ0",
        7: "DQ1", 8: "WE0_N", 9: "RAS0_N", 10: "MA9", 11: "MA0", 12: "MA1",
        13: "MA2", 14: "MA3", 15: "VDUT", 16: "MA4", 17: "MA5", 18: "MA6",
        19: "MA7", 20: "MA8"},
}

# Which map entries are unverified against a manufacturer datasheet in this
# repository's research notes. They must be checked before fabrication.
UNVERIFIED = {
    "DIP16_4116": "signal pins from memory; power pins 1/8/9/16 are sourced",
    "DIP16_4164_41256": "from memory",
    "DIP18_4416_4464": "from memory",
    "DIP18_411000": "from memory; no pin diagram was sourced",
    "DIP20_44256_514400": "514256 pins sourced; A9 on pin 5 for 514400 from memory",
    "ZIP20_44256_514400": "from memory; check against a 514256 ZIP datasheet",
}

# Signals a 168-pin SDR SDRAM DIMM needs: used only to prove the bus has room.
DIMM168_SIGNALS = ([f"DQ{i}" for i in range(64)] + [f"CB{i}" for i in range(8)] +
                   [f"A{i}" for i in range(14)] + ["BA0", "BA1"] +
                   [f"S{i}_N" for i in range(4)] + [f"DQMB{i}" for i in range(8)] +
                   ["RAS_N", "CAS_N", "WE_N"] + [f"CK{i}" for i in range(4)] +
                   ["CKE0", "CKE1"])

POWER_NETS = {"GND", "VDUT", "VBB_N5V", "VDD_12V", "NC"}


def bus_pin(name: str) -> tuple[str, str]:
    for conn, pins in BUS.items():
        for contact, net in pins.items():
            if net == name:
                return conn, contact
    raise KeyError(name)


def check() -> list[str]:
    errs = []
    flat = list(BUS["J1"].values())
    if len(flat) != 164 or sorted(BUS["J1"]) != sorted(CONTACTS):
        errs.append(f"bus has {len(flat)} contacts, expected 164")
    for i in range(N_IO):
        if flat.count(f"IO{i}") != 1:
            errs.append(f"IO{i} appears {flat.count(f'IO{i}')} times")
    for name in MISC:
        if flat.count(name) != 1:
            errs.append(f"{name} appears {flat.count(name)} times")
    want = {"GND": 24, "VDUT": 8, "+3V3": 2, "+5V": 2}
    for net, n in want.items():
        if flat.count(net) != n:
            errs.append(f"{net}: {flat.count(net)} contacts, expected {n}")
    # every signal position has a ground position within four positions
    gnd_pos = {int(c[1:]) for c, n in BUS["J1"].items() if n == "GND"}
    for c, n in BUS["J1"].items():
        if n.startswith("IO") and not any(abs(int(c[1:]) - g) <= 4 for g in gnd_pos):
            errs.append(f"{c} {n} has no ground within four positions")
    # legacy profile uses distinct lines inside the bus
    used = list(LEGACY.values())
    if len(set(used)) != len(used):
        errs.append("LEGACY profile assigns one IO line twice")
    if max(used) >= N_IO:
        errs.append("LEGACY profile exceeds bus width")
    # sense groups cover every control line exactly once
    sensed = [s for g in SENSE_GROUPS for s in g]
    for s in CTL_SIGNALS:
        if sensed.count(s) != 1:
            errs.append(f"{s} sensed {sensed.count(s)} times")
    for g in SENSE_GROUPS:
        if len(g) != 8:
            errs.append("sense group is not 8 wide")
    # sockets: every pin maps to a legacy signal, ID line or power net
    for name, m in SOCKETS.items():
        seen = {}
        for pin, sig in m.items():
            if sig in POWER_NETS:
                continue
            if sig not in LEGACY and not sig.startswith("ID"):
                errs.append(f"{name}.{pin}: unknown signal {sig}")
            if sig in seen:
                errs.append(f"{name}: {sig} on pins {seen[sig]} and {pin}")
            seen[sig] = pin
        if "VDUT" not in m.values() or "GND" not in m.values():
            errs.append(f"{name}: missing power or ground")
    if len(DIMM168_SIGNALS) + 4 > N_IO:   # keep 4 lines spare
        errs.append("168-pin DIMM does not fit the bus")
    return errs


def emit(out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    (out / "bus.json").write_text(json.dumps(
        {"bus": BUS, "legacy": LEGACY, "sense_groups": SENSE_GROUPS,
         "sockets": {k: {str(p): s for p, s in v.items()} for k, v in SOCKETS.items()},
         "unverified": UNVERIFIED}, indent=1))
    v = ["// Generated by spec/ramspec.py - do not edit.", "`ifndef RAMSPEC_VH",
         "`define RAMSPEC_VH", f"`define N_IO {N_IO}"]
    v += [f"`define IO_{k} {n}" for k, n in LEGACY.items()]
    v.append("`endif")
    (out / "ramspec.vh").write_text("\n".join(v) + "\n")
    r = ["// Generated by spec/ramspec.py - do not edit.",
         f"pub const N_IO: usize = {N_IO};"]
    r += [f"pub const IO_{k}: u8 = {n};" for k, n in LEGACY.items()]
    for name, m in SOCKETS.items():
        body = ", ".join(f'"{m[p]}"' for p in sorted(m))
        r.append(f"pub const {name}: [&str; {len(m)}] = [{body}];")
    (out / "ramspec.rs").write_text("\n".join(r) + "\n")
    md = ["# Base to adapter bus", "",
          "Generated from `spec/ramspec.py`. One PCI Express x16 card edge used as a "
          "mechanical connector only; the pin assignment is not PCIe. All IO lines are "
          "3.3 V only. B contacts are on the cartridge's top side, A contacts underneath; "
          "the key notch is between positions 11 and 12.", "",
          "| Pos | B (top) | A (underside) |", "|---|---|---|"]
    for pos in range(1, POSITIONS + 1):
        md.append(f"| {pos} | {BUS['J1'][f'B{pos}']} | {BUS['J1'][f'A{pos}']} |")
    md.append("")
    md += ["## Legacy (asynchronous DRAM) profile", "", "| Signal | IO line | Contact |",
           "|---|---|---|"]
    for k, n in LEGACY.items():
        c, p = bus_pin(f"IO{n}")
        md.append(f"| {k} | IO{n} | {p} |")
    (out / "bus.md").write_text("\n".join(md) + "\n")


if __name__ == "__main__":
    errors = check()
    for e in errors:
        print("SPEC ERROR:", e)
    if "--emit" in sys.argv:
        emit(Path(sys.argv[sys.argv.index("--emit") + 1]))
    if errors:
        sys.exit(1)
    print(f"spec ok: {N_IO} IO lines, {len(LEGACY)} legacy roles, "
          f"{len(SOCKETS)} socket maps, DIMM needs {len(DIMM168_SIGNALS)} lines")
