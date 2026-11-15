"""Shaded preview of the assembled enclosure (software z-buffer, no GPU).

    .venv/bin/python render.py            # writes out/render.png

Seen from the rear: the tray and rails, the console with its slot, and the
SIMM cartridge part-way in. The logo relief is drawn in the accent colour read from the logo file.
"""
from __future__ import annotations

import re

import numpy as np
from build123d import Pos
from PIL import Image

import enclosure as e
import mech

W, H, SS = 1800, 1150, 2            # output size and supersampling factor
BODY = (0.80, 0.79, 0.76)           # warm light grey
KEY = (0.55, 0.55, 0.57)
CART = (0.36, 0.36, 0.38)           # charcoal, like a game cartridge
RAIL = (0.62, 0.62, 0.60)
GOLD = (0.86, 0.68, 0.25)
BOARD = (0.10, 0.32, 0.20)
BG_TOP, BG_BOT = np.array([0.96, 0.96, 0.97]), np.array([0.84, 0.85, 0.87])


def accent() -> tuple[float, float, float]:
    """Accent colour as used in the logo (falls back to mid grey)."""
    try:
        cols = re.findall(r"#([0-9a-fA-F]{6})", e.LOGO.read_text())
        counts = {c: cols.count(c) for c in set(cols)}
        c = sorted(counts, key=counts.get)[-2]          # second most used: the accent
        return tuple(int(c[i:i + 2], 16) / 255 for i in (0, 2, 4))
    except Exception:
        return (0.4, 0.4, 0.4)


def mesh(part, tol=0.08):
    verts, tris = part.tessellate(tol, 0.3)
    v = np.array([(p.X, p.Y, p.Z) for p in verts])
    return v[np.array(tris)]


SLID_OUT = 38.0       # how far the cartridge is drawn pulled back from the slot


def scene():
    items = []
    console = e.console()
    lid_top = e.TOP_Z + e.CONSOLE_TOP + e.LID
    items.append((mesh(e.tray()), BODY, None))
    items.append((mesh(console), BODY, lid_top + 0.05))   # above the lid: the logo
    bw, bh = mech.BASE_SIZE
    items.append((mesh(e.box(0, 0, e.BOARD_Z, bw, bh, mech.PCB_THICKNESS)), BOARD, None))
    for right in (False, True):
        items.append((mesh(e.rail(right)), RAIL, None))
    # a cartridge part-way into the slot, its gold-finger tongue leading
    back = Pos(0, SLID_OUT, 0)
    items.append((mesh(back * e.adapter_board()), BOARD, None))
    tx, tw, tl = mech.TONGUE
    ax, ay = mech.ADAPTER_ORIGIN
    fingers = e.box(ax + tx + 0.6, ay + mech.ADAPTER_SIZE[1] + 2.6, e.CARD_TOP, tw - 1.2, 4.2, 0.06)
    items.append((mesh(back * fingers), GOLD, None))
    items.append((mesh(back * e.cartridge("simm")), CART, None))
    cap = e.keycap()
    z = e.TOP_Z + mech.KEY_ACTUATOR_HEIGHT
    for x, y in mech.KEYS:
        items.append((mesh(Pos(x, -y, z) * cap), KEY, None))
    return items


def render(path):
    w, h = W * SS, H * SS
    eye = np.array([-60.0, 150.0, 250.0])           # from behind and above the rear-left corner
    target = np.array([85.0, -70.0, 6.0])
    fwd = target - eye
    fwd /= np.linalg.norm(fwd)
    right = np.cross(fwd, [0, 0, 1.0])
    right /= np.linalg.norm(right)
    up = np.cross(right, fwd)
    f = 2.15 * w / 2                               # focal length in pixels
    light = np.array([-0.35, -0.55, 0.76])
    light /= np.linalg.norm(light)
    fill = np.array([0.6, -0.2, 0.3])
    fill /= np.linalg.norm(fill)

    ys = np.linspace(0, 1, h)[:, None, None]
    img = np.broadcast_to(BG_TOP * (1 - ys) + BG_BOT * ys, (h, w, 3)).copy()
    zbuf = np.full((h, w), np.inf)
    acc = np.array(accent())

    for tris, colour, split_z in scene():
        rel = tris - eye
        cam = np.stack([rel @ right, rel @ up, rel @ fwd], axis=-1)        # (n, 3, 3)
        n = np.cross(tris[:, 1] - tris[:, 0], tris[:, 2] - tris[:, 0])
        ln = np.linalg.norm(n, axis=1)
        ok = ln > 1e-12
        n[ok] /= ln[ok][:, None]
        facing = np.einsum("ij,ij->i", n, tris[:, 0] - eye) < 0
        shade = (0.34 + 0.62 * np.clip(n @ light, 0, 1) + 0.16 * np.clip(n @ fill, 0, 1))
        px = w / 2 + f * cam[..., 0] / cam[..., 2]
        py = h / 2 - f * cam[..., 1] / cam[..., 2]
        for i in np.nonzero(ok & facing)[0]:
            x, y, z = px[i], py[i], cam[i, :, 2]
            x0, x1 = int(max(np.floor(x.min()), 0)), int(min(np.ceil(x.max()), w - 1))
            y0, y1 = int(max(np.floor(y.min()), 0)), int(min(np.ceil(y.max()), h - 1))
            if x0 > x1 or y0 > y1:
                continue
            gx, gy = np.meshgrid(np.arange(x0, x1 + 1) + 0.5, np.arange(y0, y1 + 1) + 0.5)
            d = (y[1] - y[2]) * (x[0] - x[2]) + (x[2] - x[1]) * (y[0] - y[2])
            if abs(d) < 1e-12:
                continue
            a = ((y[1] - y[2]) * (gx - x[2]) + (x[2] - x[1]) * (gy - y[2])) / d
            b = ((y[2] - y[0]) * (gx - x[2]) + (x[0] - x[2]) * (gy - y[2])) / d
            c = 1 - a - b
            inside = (a >= 0) & (b >= 0) & (c >= 0)
            depth = 1 / (a / z[0] + b / z[1] + c / z[2] + 1e-30)
            zb = zbuf[y0:y1 + 1, x0:x1 + 1]
            hit = inside & (depth < zb)
            if not hit.any():
                continue
            col = np.array(colour)
            if split_z is not None and tris[i, :, 2].max() > split_z:
                col = acc
            zb[hit] = depth[hit]
            img[y0:y1 + 1, x0:x1 + 1][hit] = np.clip(col * shade[i], 0, 1)

    out = img.reshape(H, SS, W, SS, 3).mean(axis=(1, 3))
    Image.fromarray((out * 255).astype(np.uint8)).save(path)


if __name__ == "__main__":
    e.OUT.mkdir(exist_ok=True)
    render(e.OUT / "render.png")
    print("written", e.OUT / "render.png")
