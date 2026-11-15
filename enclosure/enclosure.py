"""3D-printed enclosure for the RAMalyser memory tester.

Printed parts, all generated from spec/mech.py:
  tray       holds the base board on six bosses; tall side walls form the
             cartridge bay, open at the rear; USB-C cut-out in the rear wall
  rail_left, rail_right
             sit on the base board along the sides of the bay; the cartridge
             rides on them, 5 mm above the board, into the card-edge socket
  console    cover over the socket and the front strip: slot for the
             cartridge in its rear wall, display window, key holes, LED
             holes, buzzer vent, the logo raised on its top face
  keycap     plunger that reaches a tactile switch through the console (x6)
  cartridge_simm, cartridge_dip
             shell screwed to each adapter board, making it a cartridge:
             covers the electronics, window for the sockets, ribbed grips,
             recessed label panel; the gold-finger tongue projects in front

    .venv/bin/python enclosure.py            # writes out/*.step and out/*.stl
    .venv/bin/python enclosure.py --check    # also runs the fit checks

Styling follows late-80s/90s workstation hardware (Sun SPARCstation): a low
boxy chassis, every outer edge and every opening chamfered, bands of
horizontal ribs along the sides and the front fascia. The Jupiter logo
stands 0.6 mm proud of the console's top face; it is taken from the
design-system release at build time (never copied into this repository).

Two ways to print the tray and console:

  flat      `tray.stl`, `console.stl`: the right way up, no supports (the
            console lid bridges about 50 mm). The logo is then the last three
            layers, so one filament change colours it.
  at 45 deg `tray_45.stl`, `console_45.stl`: tilted back 45 degrees about the
            long axis, standing on the chamfered rear-bottom edge. The top
            and front faces then both slope upwards, so layer lines and a
            slicer texture (fuzzy skin) cover them evenly. Use a brim;
            supports land only on hidden faces. The logo is no longer a
            layer change in this orientation: paint it, or use a
            multi-material printer.

Rails print flat side down, keycaps collar down, cartridge shells top face
down.
"""
from __future__ import annotations

import sys
from pathlib import Path

import subprocess

from build123d import (Align, Axis, Box, Cone, Cylinder, Face, Location, Part, Plane, Pos, Rot,
                       chamfer, export_step, export_stl, extrude, import_svg, mirror, trace)

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "spec"))
import mech  # noqa: E402

OUT = Path(__file__).resolve().parent / "out"

# ---- parameters (mm)
WALL = 2.4
FLOOR = 2.4
GAP = 0.6                 # clearance around the board
BOSS_H = 5.0              # board underside above the floor (THT pins, bottom parts)
BOSS_D = 7.5
SCREW_D = 3.4             # M3 clearance
HEAD_D = 6.5              # M3 cap-head counterbore
PILOT_D = 2.9             # M3 thread-forming pilot
TRAY_RIM = 3.0            # wall height above the board's top face under the console
CONSOLE_Y0 = 97.6         # console rear face: just behind the socket mouth
CONSOLE_TOP = mech.LCD_HEIGHT + 1.6     # inside face of the console lid above the board
LID = 2.0
KEY_HOLE = 7.0
KEY_STEM = 6.4
LED_HOLE = 3.0
CH = 1.0                  # chamfer on outer edges
RIM = 0.6                 # chamfer on the rims of openings
CART_LID = 2.0
CART_WALL = 2.0
LABEL_DEPTH = 0.4
SLOT_CLEAR = 0.4          # clearance around the cartridge board in the console slot

BW, BH = mech.BASE_SIZE
BOARD_Z = FLOOR + BOSS_H                    # underside of the board
TOP_Z = BOARD_Z + mech.PCB_THICKNESS        # top face of the board
CARD_Z = TOP_Z + mech.ADAPTER_STACK         # underside of a seated cartridge board
CARD_TOP = CARD_Z + mech.PCB_THICKNESS
CART_IN = CARD_TOP + mech.ADAPTER_PART_HEIGHT + 0.6    # inside face of the shell lid
CART_TOP = CART_IN + CART_LID
BAY_WALL_Z = CART_TOP                       # bay side walls rise to the shell's top
OUTER_W = BW + 2 * (GAP + WALL)
OUTER_H = BH + 2 * (GAP + WALL)
OX = -(GAP + WALL)                          # enclosure origin in board coordinates
OY = -(GAP + WALL)

MIN = (Align.MIN, Align.MIN, Align.MIN)
CEN = (Align.CENTER, Align.CENTER, Align.MIN)


# Board coordinates have y running towards the user with x to the right when
# seen from above, which is left-handed once z points up. The solids are
# therefore built with y negated, so the printed case is not a mirror image.
def box(x, y, z, w, h, d) -> Part:
    """Axis-aligned box from its board-coordinate corner (x, y), size w x h x d."""
    return Pos(x, -(y + h), z) * Box(w, h, d, align=MIN)


def cyl(x, y, z, dia, height) -> Part:
    return Pos(x, -y, z) * Cylinder(dia / 2, height, align=CEN)


def cbox(x, y, z, w, h, d, c=CH) -> Part:
    """Box with every edge chamfered."""
    b = Box(w, h, d, align=MIN)
    return Pos(x, -(y + h), z) * chamfer(b.edges(), c)


def rim_rect(x, y, z_top, w, h, c=RIM) -> Part:
    """Cutter that chamfers the rim of a rectangular opening in a top face."""
    b = Box(w + 2 * c, h + 2 * c, c, align=MIN)
    b = chamfer(b.edges().group_by(Axis.Z)[0], c * 0.98)
    return Pos(x - c, -(y + h + c), z_top - c) * b


def rim_round(x, y, z_top, dia, c=RIM) -> Part:
    """Cutter that chamfers the rim of a round hole in a top face."""
    return Pos(x, -y, z_top - c) * Cone(dia / 2, dia / 2 + c, c, align=CEN)


RIB_W, RIB_D, RIB_PITCH = 1.2, 0.7, 3.0
DESIGN_DIR = Path(__file__).resolve().parent / ".design"
LOGO = DESIGN_DIR / "package" / "logo" / "jupiter-logo-light.svg"
BADGE_W, BADGE_RELIEF = 24.0, 0.6
LOGO_SVG_WIDTH = 757.0    # drawn width of the logo in its own units
MIN_LINE = 0.5           # thinnest line a 0.4 mm nozzle prints reliably


def fetch_logo() -> bool:
    """Get the logo from the current design-system release (build-time fetch
    into a git-ignored directory)."""
    if LOGO.exists():
        return True
    DESIGN_DIR.mkdir(exist_ok=True)
    try:
        subprocess.run(["gh", "release", "download", "--repo", "belikh/design", "--pattern",
                        "belikh-jupiter-graphics-*.tgz", "--dir", str(DESIGN_DIR), "--clobber"],
                       check=True, capture_output=True, timeout=120)
        tgz = sorted(DESIGN_DIR.glob("belikh-jupiter-graphics-*.tgz"))[-1]
        subprocess.run(["tar", "xzf", str(tgz), "-C", str(DESIGN_DIR), "package/logo"],
                       check=True)
    except Exception as exc:            # offline, or no access to the release
        print("logo not available:", exc)
    return LOGO.exists()


def logo_faces():
    """Logo outlines scaled to BADGE_W, lower-left corner at the origin.
    Returns (faces, width, height) or None if the logo cannot be fetched."""
    if not fetch_logo():
        return None
    faces = []
    for shp in import_svg(str(LOGO)):
        # the importer leaves a placement on each shape that scale() ignores;
        # drop it so every outline is in the same plain coordinates
        shp = shp.located(Location())
        if isinstance(shp, Face):
            faces.append(shp)                       # wordmark: filled outlines
        else:
            # planet and lines: 4-unit strokes, widened if needed so they still
            # print at this size
            faces.extend(trace(shp, max(4.0, MIN_LINE * LOGO_SVG_WIDTH / BADGE_W)).faces())
    # with the placement dropped the outlines are in SVG orientation (y down)
    # and some face downwards: flip them upright and make every face look up
    upright = []
    for f in faces:
        for g in mirror(f, Plane.XZ).faces():
            upright.append(-g if g.normal_at().Z < 0 else g)
    faces = upright
    bbs = [f.bounding_box() for f in faces]
    x0 = min(b.min.X for b in bbs); x1 = max(b.max.X for b in bbs)
    y0 = min(b.min.Y for b in bbs); y1 = max(b.max.Y for b in bbs)
    k = BADGE_W / (x1 - x0)
    placed = [Pos(-x0 * k, -y0 * k, 0) * f.scale(k) for f in faces]
    return placed, BADGE_W, (y1 - y0) * k


def ribs_x(x, y_face, z0, z1, length, into: float) -> list[Part]:
    """Horizontal grooves along x on a wall whose outer face is at y_face;
    `into` is +1 if the wall's material lies at larger board y."""
    out = []
    z = z0
    while z + RIB_W <= z1:
        y = y_face if into > 0 else y_face - RIB_D
        out.append(box(x, y - 0.01, z, length, RIB_D + 0.02, RIB_W))
        z += RIB_PITCH
    return out


def ribs_y(x_face, y, z0, z1, length, into: float) -> list[Part]:
    out = []
    z = z0
    while z + RIB_W <= z1:
        x = x_face if into > 0 else x_face - RIB_D
        out.append(box(x - 0.01, y, z, RIB_D + 0.02, length, RIB_W))
        z += RIB_PITCH
    return out


def tray() -> Part:
    low = TOP_Z + TRAY_RIM                       # wall height under the console
    bay_len = CONSOLE_Y0 - OY                    # tall walls run from the rear to the console
    body = cbox(OX, OY, 0, OUTER_W, OUTER_H, low)
    for x in (OX, BW + GAP):                     # tall side walls of the cartridge bay
        body += cbox(x, OY, 0, WALL, bay_len, BAY_WALL_Z)
    body -= box(-GAP, -GAP, FLOOR, BW + 2 * GAP, BH + 2 * GAP, BAY_WALL_Z)
    # the rear wall stays below the cartridge so it can slide in over it
    body -= box(-GAP, OY - 0.1, CARD_Z - 0.8, BW + 2 * GAP, WALL + 0.2, BAY_WALL_Z)
    for i, (x, y) in enumerate(mech.BASE_HOLES):
        body += cyl(x, y, FLOOR, BOSS_D, BOSS_H)
        if i < 4:                                # bay: screws come down through rail and board
            body -= cyl(x, y, FLOOR - 1.0, PILOT_D, BOSS_H + 1.1)
        else:                                    # console: screws come up from below
            body -= cyl(x, y, 0, SCREW_D, FLOOR + BOSS_H)
            body -= cyl(x, y, 0, HEAD_D, 3.2)
    # USB-C mouth through the rear wall
    ux, _ = mech.USB_C
    uw, uh = mech.USB_C_SIZE
    body -= box(ux - uw / 2 - 0.5, OY - 0.1, TOP_Z - 0.3, uw + 1.0, WALL + GAP + 0.2, uh + 0.8)
    # opening under the optional Wi-Fi module on the back of the board: the
    # module is taller than the gap under the board, and its USB-C port and
    # antenna are reached through here
    assert mech.XIAO_HEIGHT <= BOSS_H + FLOOR, "Wi-Fi module would stand proud of the tray"
    wx, wy, ww, wh = mech.XIAO_WINDOW
    body -= box(wx, wy, -0.1, ww, wh, FLOOR + 0.2)
    # workstation-style rib bands along both sides
    for g in ribs_y(OX, 6.0, 2.4, BAY_WALL_Z - 1.6, CONSOLE_Y0 - 12.0, +1) + \
            ribs_y(OX + OUTER_W, 6.0, 2.4, BAY_WALL_Z - 1.6, CONSOLE_Y0 - 12.0, -1):
        body -= g
    return body


RAIL_W = 7.6
RAIL_Y0, RAIL_Y1 = 1.0, 96.5


def rail(right: bool) -> Part:
    """Runner for one side of the bay. It lies on the base board; the
    cartridge board rides on its top face and is guided sideways by its lip."""
    h = mech.ADAPTER_STACK - 0.15                # a little under the card: the socket sets the height
    x0 = BW - 0.6 - RAIL_W if right else 0.6
    length = RAIL_Y1 - RAIL_Y0
    body = cbox(x0, RAIL_Y0, TOP_Z, RAIL_W, length, h, 0.6)
    # lip between the board edge and the tray wall, up to the top of the shell
    ax = mech.ADAPTER_ORIGIN[0]
    lip_w = ax - 0.6 - 0.35                      # leaves 0.35 mm beside the cartridge
    lx = BW - 0.6 - lip_w if right else 0.6
    body += cbox(lx, RAIL_Y0, TOP_Z, lip_w, length, CART_TOP - TOP_Z, 0.6)
    # lead-in at the rear end
    body -= Pos(x0 + RAIL_W / 2, -RAIL_Y0, TOP_Z + h) * Rot(45, 0, 0) * Box(RAIL_W + 1, 3.0, 3.0)
    for x, y in mech.BASE_HOLES[:4]:
        if (x > BW / 2) == right:
            body -= cyl(x, y, TOP_Z - 0.1, SCREW_D, h + 0.2)
            body -= cyl(x, y, TOP_Z + h - 3.0, HEAD_D, 3.1)     # head sits below the running face
    return body


TUBE_IN = KEY_HOLE + 2.0   # guide tube bore: room for the keycap collar
LOGO_AT = (6.0, 101.5)     # left edge and rear edge of the logo on the lid (board coords)


def console() -> Part:
    """Cover for the socket and the front strip. Sits on the tray walls and
    is held by the two front screws, which pass up through the tray and
    board into its posts. The cartridge enters through the slot in its rear
    wall. Everything inside starts at the open bottom plane, so it prints
    lid-up with nothing in mid-air."""
    z0 = TOP_Z + TRAY_RIM                    # open bottom plane (on the tray rim)
    z_in = TOP_Z + CONSOLE_TOP               # inside face of the lid
    z_top = z_in + LID
    depth = BH + GAP + WALL - CONSOLE_Y0
    body = cbox(OX, CONSOLE_Y0, z0, OUTER_W, depth, z_top - z0)
    body -= box(OX + WALL, CONSOLE_Y0 + WALL, z0, OUTER_W - 2 * WALL, depth - 2 * WALL, z_in - z0)
    # cartridge slot through the rear wall: the board, full width
    ax, aw = mech.ADAPTER_ORIGIN[0], mech.ADAPTER_SIZE[0]
    body -= box(ax - SLOT_CLEAR, CONSOLE_Y0 - 0.1, CARD_Z - SLOT_CLEAR, aw + 2 * SLOT_CLEAR,
                WALL + 0.2, mech.PCB_THICKNESS + 2 * SLOT_CLEAR)
    # display window
    wx, wy, ww, wh = mech.LCD_WINDOW
    body -= box(wx, wy, z_in - 0.1, ww, wh, LID + 0.2)
    body -= rim_rect(wx, wy, z_top, ww, wh)
    # keys: full-height guide tubes; the lid hole is narrower than the bore so
    # the keycap's collar is trapped underneath
    for x, y in mech.KEYS:
        body += cyl(x, y, z0, TUBE_IN + 2.4, z_in - z0)
        body -= cyl(x, y, z0 - 0.1, TUBE_IN, z_in - z0 + 0.1)
        body -= cyl(x, y, z_in - 0.1, KEY_HOLE, LID + 0.2)
        body -= rim_round(x, y, z_top, KEY_HOLE)
    # LED light holes and buzzer vent
    for x, y in mech.LEDS:
        body -= cyl(x, y, z_in - 0.1, LED_HOLE, LID + 0.2)
        body -= rim_round(x, y, z_top, LED_HOLE, 0.4)
    bx, by = mech.BUZZER
    for dx in (-3.0, 0.0, 3.0):
        body -= box(bx + dx - 0.6, by - 3.5, z_in - 0.1, 1.2, 7.0, LID + 0.2)
    # screw posts above the two front board holes
    for x, y in mech.BASE_HOLES[4:]:
        body += cyl(x, y, z0, BOSS_D, z_in - z0)
        body -= cyl(x, y, z0 - 0.1, PILOT_D, 10.0)
    # front fascia: a full-width rib band
    y_front = BH + GAP + WALL
    for g in ribs_x(4.0, y_front, z0 + 2.4, z_top - 2.4, BW - 8.0, -1):
        body -= g
    # logo, raised on the top face, reading from the front
    lf = logo_faces()
    if lf is not None:
        faces, _w, h = lf
        lx, ly = LOGO_AT
        art = faces[0]
        for f in faces[1:]:
            art = art + f
        body += extrude(Pos(lx, -(ly + h), z_top - 0.2) * art, BADGE_RELIEF + 0.2)
    return body


def keycap() -> Part:
    """Plunger: a stem through the lid, standing 1.5 mm proud, and a collar
    that rides in the guide tube and cannot pass the lid hole. Fit the six
    keycaps into the console from below before screwing it down."""
    stem_len = CONSOLE_TOP + LID - mech.KEY_ACTUATOR_HEIGHT + 1.5
    stem = Cylinder(KEY_STEM / 2, stem_len, align=CEN)
    stem = chamfer(stem.edges().group_by(Axis.Z)[-1], 0.6)
    return stem + Cylinder((TUBE_IN - 0.6) / 2, 1.6, align=CEN)


CART_FRONT = 97.0          # shell ends here (adapter y); the board and tongue go on into the slot


def cartridge(kind: str) -> Part:
    """Shell for one adapter board, built in place (board coordinates, with
    the cartridge seated). It is as wide as the board, so the rails guide
    board and shell together; a skirt wraps the rear edge as a grip."""
    ax, ay = mech.ADAPTER_ORIGIN
    aw, _ah = mech.ADAPTER_SIZE
    rear = ay - CART_WALL                        # skirt behind the board's rear edge
    length = ay + CART_FRONT - rear
    body = cbox(ax, rear, CARD_Z, aw, length, CART_TOP - CARD_Z)
    # hollow above the board; walls stand on the board's edges
    body -= box(ax + CART_WALL, ay + CART_WALL, CARD_TOP, aw - 2 * CART_WALL,
                CART_FRONT - 2 * CART_WALL, CART_IN - CARD_TOP)
    # the board itself (the shell sits on it; the rear skirt hangs behind it)
    body -= box(ax - 0.1, ay - 0.15, CARD_Z - 0.1, aw + 0.2, CART_FRONT + 0.3,
                mech.PCB_THICKNESS + 0.1)
    # socket window
    for wx, wy, ww, wh in mech.ADAPTER_WINDOWS[kind]:
        body -= box(ax + wx, ay + wy, CART_IN - 0.1, ww, wh, CART_LID + 0.2)
        body -= rim_rect(ax + wx, ay + wy, CART_TOP, ww, wh)
    win_end = max(wy + wh for _wx, wy, _ww, wh in mech.ADAPTER_WINDOWS[kind])
    # corner posts onto the board, screw holes through the lid
    for hx, hy in mech.ADAPTER_HOLES:
        body += cyl(ax + hx, ay + hy, CARD_TOP, BOSS_D, CART_IN - CARD_TOP)
        body -= cyl(ax + hx, ay + hy, CARD_TOP - 0.1, SCREW_D, CART_TOP - CARD_TOP + 0.2)
        body -= cyl(ax + hx, ay + hy, CART_TOP - 1.2, HEAD_D, 1.3)
    # recessed label panel between the window and the front, clear of the posts
    label_y = ay + win_end + 3.0
    label_h = ay + CART_FRONT - 3.0 - label_y
    if label_h > 8.0:
        body -= box(ax + 14.0, label_y, CART_TOP - LABEL_DEPTH, aw - 28.0, label_h,
                    LABEL_DEPTH + 0.1)
    # grip ribs across the lid along both side edges
    y = ay + 14.0
    while y + RIB_W < ay + CART_FRONT - 14.0:
        for gx in (ax - 0.01, ax + aw - 7.0):
            body -= box(gx, y, CART_TOP - RIB_D, 7.01, RIB_W, RIB_D + 0.1)
        y += RIB_PITCH
    return body


def adapter_board(kind: str = "") -> Part:
    """The cartridge's circuit board, seated: body plus tongue."""
    ax, ay = mech.ADAPTER_ORIGIN
    aw, ah = mech.ADAPTER_SIZE
    tx, tw, tl = mech.TONGUE
    return (box(ax, ay, CARD_Z, aw, ah, mech.PCB_THICKNESS)
            + box(ax + tx, ay + ah, CARD_Z, tw, tl, mech.PCB_THICKNESS))


def adapter_keepout(kind: str) -> Part:
    """Cartridge board, the parts on it and the socket zone, for checks."""
    ax, ay = mech.ADAPTER_ORIGIN
    aw, ah = mech.ADAPTER_SIZE
    v = adapter_board()
    parts_zone = box(ax + 4.0, ay + 4.0, CARD_TOP, aw - 8.0, CART_FRONT - 8.0,
                     mech.ADAPTER_PART_HEIGHT)
    for hx, hy in mech.ADAPTER_HOLES:            # nothing is placed where the posts land
        parts_zone -= cyl(ax + hx, ay + hy, CARD_TOP - 0.1, BOSS_D + 0.4, 10.0)
    v += parts_zone
    for wx, wy, ww, wh in mech.ADAPTER_WINDOWS[kind]:
        v += box(ax + wx + 0.8, ay + wy + 0.8, CARD_TOP, ww - 1.6, wh - 1.6, 40.0)
    return v


def board_keepout() -> Part:
    """The base board plus the tallest parts on it, for interference checks."""
    v = box(0, 0, BOARD_Z, BW, BH, mech.PCB_THICKNESS)
    k = mech.RAIL_KEEPOUT
    v += box(k, 2.0, TOP_Z, BW - 2 * k, CONSOLE_Y0 - 4.0, mech.MAX_HEIGHT_UNDER_ADAPTER)
    sx, sy, sw, sh = mech.SLOT_BODY
    v += box(sx, sy, TOP_Z, sw, sh, mech.SLOT_HEIGHT)
    mx, my, mw, mh = mech.LCD_MODULE
    v += box(mx, my, TOP_Z, mw, mh, mech.LCD_HEIGHT)
    return v


FOOT = 2.0   # legs of the chamfer the tilted prints stand on


def foot_chamfer(y_board: float, z: float) -> Part:
    """Prism that cuts a 45-degree flat along a rear-bottom outer edge."""
    side = FOOT * 2 ** 0.5
    return Pos(OX + OUTER_W / 2, -y_board, z) * Rot(45, 0, 0) * Box(OUTER_W + 1, side, side)


def tilted(p: Part) -> Part:
    """Part turned back 45 degrees about its long axis and set on the bed."""
    t = Rot(-45, 0, 0) * p
    return Pos(0, 0, -t.bounding_box().min.Z) * t


def volume(p) -> float:
    return p.volume if p is not None else 0.0


def checks(parts: dict[str, Part]) -> list[str]:
    import trimesh
    errs = []

    def clash(a_name, a, b_name, b):
        v = volume(a & b)
        if v > 1e-3:
            errs.append(f"{a_name} intersects {b_name}: {v:.1f} mm^3")

    keep = board_keepout()
    fixed = ("tray", "console", "rail_left", "rail_right")
    for name in fixed:
        clash(name, parts[name], "the base board or its parts", keep)
    for i, a in enumerate(fixed):
        for b in fixed[i + 1:]:
            clash(a, parts[a], b, parts[b])
    for kind in ("simm", "dip"):
        cart = parts[f"cartridge_{kind}"]
        clash(f"cartridge_{kind}", cart, "its own board, parts or sockets", adapter_keepout(kind))
        clash(f"cartridge_{kind}", cart, "the base board or its parts", keep)
        for name in fixed:
            clash(name, parts[name], f"the seated {kind} cartridge", cart)
            clash(name, parts[name], f"the {kind} cartridge board", adapter_keepout(kind))
        # the cartridge must be able to slide: swept back along the bay it
        # still clears everything fixed
        for back in (30.0, 60.0, 100.0):
            moved = Pos(0, back, 0) * (cart + adapter_board())
            for name in fixed:
                clash(name, parts[name], f"the {kind} cartridge {back:.0f} mm out", moved)
    # the tongue reaches the socket mouth and the shell stops short of the console
    ay, ah = mech.ADAPTER_ORIGIN[1], mech.ADAPTER_SIZE[1]
    if abs(ay + ah - mech.SLOT_MOUTH_Y) > 0.05:
        errs.append("cartridge shoulder does not meet the socket mouth")
    if ay + CART_FRONT > CONSOLE_Y0 - 0.3:
        errs.append("cartridge shell would hit the console")
    # the cartridge rests on the rails, not on the base parts
    if abs((TOP_Z + mech.ADAPTER_STACK - 0.15) - (CARD_Z - 0.15)) > 1e-6:
        errs.append("rail height does not match the card height")
    # the USB mouth must be open: a probe through the rear wall hits nothing
    ux, _ = mech.USB_C
    probe = box(ux - 4.0, OY - 1.0, TOP_Z + 0.2, 8.0, WALL + GAP + 2.0, 2.8)
    if volume(parts["tray"] & probe) > 1e-3:
        errs.append("USB-C cut-out is obstructed")
    for x, y in mech.KEYS:
        probe = cyl(x, y, TOP_Z + mech.KEY_ACTUATOR_HEIGHT, KEY_STEM - 0.4, 40.0)
        if volume(parts["console"] & probe) > 1e-3:
            errs.append(f"key hole at {x},{y} is obstructed")
    wx, wy, ww, wh = mech.LCD_WINDOW
    probe = box(wx + 0.5, wy + 0.5, TOP_Z + mech.LCD_HEIGHT, ww - 1.0, wh - 1.0, 20.0)
    if volume(parts["console"] & probe) > 1e-3:
        errs.append("display window is obstructed")
    if not LOGO.exists():
        errs.append("logo missing: design-system release not reachable")
    for name in list(parts) + ["tray_45", "console_45"]:
        m = trimesh.load(OUT / f"{name}.stl")
        if not m.is_watertight:
            errs.append(f"{name}.stl is not watertight")
        x, y, z = m.extents
        if max(x, y) > 250 or z > 200:
            errs.append(f"{name} does not fit a 250 mm printer bed: {x:.0f}x{y:.0f}x{z:.0f}")
    return errs


def build_parts() -> dict[str, Part]:
    return {"tray": tray(), "console": console(), "rail_left": rail(False),
            "rail_right": rail(True), "keycap": keycap(),
            "cartridge_simm": cartridge("simm"), "cartridge_dip": cartridge("dip")}


def main() -> int:
    OUT.mkdir(exist_ok=True)
    parts = build_parts()
    for name, p in parts.items():
        export_step(p, str(OUT / f"{name}.step"))
        export_stl(p, str(OUT / f"{name}.stl"))
        bb = p.bounding_box()
        print(f"{name}: {bb.size.X:.1f} x {bb.size.Y:.1f} x {bb.size.Z:.1f} mm, "
              f"{p.volume / 1000:.1f} cm^3")
    for name in ("tray", "console"):
        t = tilted(parts[name])
        export_stl(t, str(OUT / f"{name}_45.stl"))
        bb = t.bounding_box()
        print(f"{name}_45: {bb.size.X:.1f} x {bb.size.Y:.1f} x {bb.size.Z:.1f} mm on the bed")
    if "--check" in sys.argv:
        errs = checks(parts)
        for e in errs:
            print("ENCLOSURE ERROR:", e)
        if errs:
            return 1
        print("enclosure checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
