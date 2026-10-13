#!/usr/bin/env python3
"""What Episode 289's gold dots meant: a 25 s shot for the Coder v1 launch
film. A request runs on a plugin, the caller's payment comes in over
Lightning, and Bitcoin goes back out to the plugin's author and to
OpenAgents, each pulse labeled with its amount; the savings go to you.

  scripts/film/revenue-sharing.py OUT.mp4 [--frames-dir DIR] [--only SECONDS,...]

Beats (the timeline constants below hold the exact times):
  0-5       Someone asks Coder for help. System 1 catches the request and
            runs it on a plugin someone built (Explain this error); only a
            sliver goes on to the big model.
  5-10.5    The call is paid in Bitcoin over Lightning: 15 sats in, then
            split, 10 sats to the plugin's author and 5 to OpenAgents.
  10.5-15.5 The author keeps the whole fee. The first paid call bonus,
            1,000 sats, goes to the author out of OpenAgents' share.
  15.5-20.5 Less work for the big model: cost per fix $4.00 -> $1.84, and
            the $2.16 stays with you.
  20.5-25   A calm hold with steady calls, gold out to the author.

Numbers on screen and where they come from:
  15 sats = 5 sats endpoint + 10 sats author fee: the production quote for
    Explain this error on 2026-10-03, docs/payments/2026-10-03-end-to-end-demo.md
    (the first real paid run is still the owner's, so the screen says "demo
    payments").
  The split: crates/pay-ledger/rules/v1.toml and
    docs/payments/2026-10-02-central-receive-and-splits.md section 3: the
    author gets the plugin's whole fee, OpenAgents the endpoint price less the
    Lightning fees; bonuses are funded only from OpenAgents' share.
  1,000 sats first paid call bonus: rule v1 `first_paid_call_msat`.
  $4.00 vs $1.84 per accepted fix ($2.16 saved): docs/inference/briefed-agent-ab.md
    (21 issues x 3 runs, same model, 19/63 accepted each).

Style and colors follow scripts/film/system1-routing.py: noir canvas, the
request light (236,240,255) and the payment gold (255,206,84) from
crates/openagents-desktop/src/route_future.rs.

Fonts (licensed, not in the repo): FILM_FONTS (Geist, default
~/work/film-out/fonts) and BERKELEY_FONTS (default ~/Library/Fonts).
Needs Pillow, numpy and ffmpeg.
"""

import argparse
import math
import os
import random
import subprocess
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

ap = argparse.ArgumentParser()
ap.add_argument("out")
ap.add_argument("--frames-dir", help="also save a PNG every second here")
ap.add_argument("--only", help="render only these seconds to PNGs (comma list), no video")
args = ap.parse_args()

GEIST = Path(os.path.expanduser(os.environ.get("FILM_FONTS", "~/work/film-out/fonts")))
BERKELEY = Path(os.path.expanduser(os.environ.get("BERKELEY_FONTS", "~/Library/Fonts")))

W, H, FPS = 1920, 1080, 30
S = 2  # live layers are drawn at twice the size, then scaled down

CANVAS = (10, 10, 10)
CONTENT = (237, 237, 237)
SECONDARY = (129, 129, 129)
TERTIARY = (101, 101, 101)
LIGHT = (236, 240, 255)
GOLD = (255, 206, 84)
EDGE_A = 38


def font(name, size):
    path = GEIST / name if name.startswith("Geist") else BERKELEY / name
    return ImageFont.truetype(str(path), size)


g_cap = font("Geist-Regular.ttf", 34)
g_lab = font("Geist-Regular.ttf", 21)
g_node = font("Geist-Regular.ttf", 19)
g_small = font("Geist-Regular.ttf", 17)
g_tiny = font("Geist-Regular.ttf", 15)
m_amt = font("BerkeleyMono-Regular.ttf", 24)
m_big = font("BerkeleyMono-Regular.ttf", 50)
m_old = font("BerkeleyMono-Regular.ttf", 24)
m_mid = font("BerkeleyMono-Regular.ttf", 30)

LEFT = 118

# --- easing -------------------------------------------------------------------


def clamp(x, a=0.0, b=1.0):
    return max(a, min(b, x))


def ease(t):
    t = clamp(t)
    return t * t * (3 - 2 * t)


def fade(t, a, b):
    return ease((t - a) / (b - a))


def out_cubic(t):
    t = clamp(t)
    return 1 - (1 - t) ** 3


# --- the scene ------------------------------------------------------------------
YOU = (760.0, 600.0)
HUB = (1140.0, 600.0)        # Coder, with System 1 in front
R_S1 = 62.0
R_HUB = 30.0
MODEL = (1140.0, 330.0)      # System 2, the big model
PLUGIN = (1450.0, 450.0)
AUTHOR = (1690.0, 300.0)
OA = (1450.0, 820.0)

# --- the timeline (seconds) --------------------------------------------------------
CAPS = [
    (0.5, 5.0, ["Someone asks Coder for help.", "It runs on a plugin someone built."]),
    (5.3, 10.4, ["They pay for the call", "in Bitcoin, over Lightning."]),
    (10.7, 15.4, ["The author keeps the whole fee.", "Bonuses come out of our share."]),
    (15.7, 20.5, ["Less work for the big model.", "The savings go to you."]),
    (20.8, 25.0, ["Cheaper for the people who use it.", "And it pays the people who build it."]),
]
SECONDS = 25.0
REQ = 1.4          # the request leaves you
PAY = 5.8          # 15 sats in
SPLIT = 7.3        # 10 to the author, 5 to OpenAgents
BONUS = 11.6       # 1,000 sats first paid call bonus
SAVE = 16.2        # the meters
STEADY = 20.4      # the hold's steady traffic


# --- lights (as in system1-routing.py) ----------------------------------------------
class Light:
    def __init__(self, pts, t0, dur, color=LIGHT, width=3.0, hold=0.7, glow_r=10.0, head=1.0, ring=False):
        self.pts, self.t0, self.dur, self.color = pts, t0, dur, color
        self.width, self.hold, self.glow_r, self.head, self.ring = width, hold, glow_r, head, ring
        segs = [math.dist(pts[i], pts[i + 1]) for i in range(len(pts) - 1)]
        self.segs, self.total = segs, max(sum(segs), 1e-3)

    def at(self, u):
        left = clamp(u) * self.total
        for i, L in enumerate(self.segs):
            if left <= L or i == len(self.segs) - 1:
                k = 0 if L == 0 else clamp(left / L)
                a, b = self.pts[i], self.pts[i + 1]
                return (a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k), i, k
            left -= L
        return self.pts[-1], len(self.segs) - 1, 1.0

    def draw(self, d, t):
        u = (t - self.t0) / self.dur
        if u <= 0:
            return
        fadek = 1.0 if u <= 1 else clamp(1 - (t - self.t0 - self.dur) / self.hold)
        if fadek <= 0:
            return
        head, seg, k = self.at(out_cubic(u) if u < 1 else 1)
        trail = self.pts[:seg + 1] + [head]
        c = self.color
        for wmul, a in ((4.0, 40), (1.0, 235)):
            d.line([(x * S, y * S) for x, y in trail], fill=(*c, int(a * fadek)),
                   width=max(1, int(self.width * wmul * S * self.head)), joint="curve")
        if u < 1:
            for r, a in ((13, 50), (7, 130), (4, 255)):
                r *= S * self.head
                d.ellipse([head[0] * S - r, head[1] * S - r, head[0] * S + r, head[1] * S + r],
                          fill=(*(CONTENT if a == 255 else c), a))
            if self.ring:
                r = 15 * S * self.head
                d.ellipse([head[0] * S - r, head[1] * S - r, head[0] * S + r, head[1] * S + r],
                          outline=(*c, 220), width=2 * S)
        else:
            land = clamp((t - self.t0 - self.dur) / 0.25)
            x, y = self.pts[-1]
            r = (self.glow_r + 10 * land) * S
            d.ellipse([x * S - r, y * S - r, x * S + r, y * S + r],
                      outline=(*c, int(220 * fadek)), width=2 * S)


class Dot:
    def __init__(self, pts, t0, dur, color, r=3.2, ring=False):
        self.l = Light(pts, t0, dur, color)
        self.color, self.r, self.ring = color, r, ring

    def draw(self, d, t):
        u = (t - self.l.t0) / self.l.dur
        if u <= 0 or u >= 1:
            return
        (x, y), _, _ = self.l.at(ease(u))
        for r, a in ((self.r * 2.6, 60), (self.r, 255)):
            r *= S
            d.ellipse([x * S - r, y * S - r, x * S + r, y * S + r], fill=(*self.color, a))
        if self.ring:
            r = self.r * 3.2 * S
            d.ellipse([x * S - r, y * S - r, x * S + r, y * S + r], outline=(*self.color, 200), width=S)


def via(a, b, bend=0.0, n=14):
    """A gentle curve from a to b, bowed sideways by `bend` points."""
    mx, my = (a[0] + b[0]) / 2, (a[1] + b[1]) / 2
    dx, dy = b[0] - a[0], b[1] - a[1]
    L = math.hypot(dx, dy) or 1
    cx, cy = mx - dy / L * bend, my + dx / L * bend
    pts = []
    for j in range(n + 1):
        u = j / n
        pts.append(((1 - u) ** 2 * a[0] + 2 * (1 - u) * u * cx + u * u * b[0],
                    (1 - u) ** 2 * a[1] + 2 * (1 - u) * u * cy + u * u * b[1]))
    return pts


def s1(a):
    return (HUB[0] + R_S1 * math.cos(a), HUB[1] + R_S1 * math.sin(a))


def ring(a0, a1, n=18):
    """Points along the System 1 ring from angle a0 to a1, the short way."""
    d = math.atan2(math.sin(a1 - a0), math.cos(a1 - a0))
    return [s1(a0 + d * j / n) for j in range(n + 1)]


lights = []
# Beat 1: the request leaves you, System 1 catches it and runs it on the plugin;
# a sliver goes on to the big model.
lights.append(Light([YOU, s1(math.pi)], REQ, 0.7, width=3.0, glow_r=R_S1 + 4, hold=0.6))
lights.append(Light(ring(math.pi, -2.0) + ring(-2.0, -0.42)[1:] + [PLUGIN], REQ + 0.75, 0.9,
                    width=2.8, glow_r=24, hold=1.2))
lights.append(Light([s1(-math.pi / 2), MODEL], REQ + 1.2, 0.5, width=1.3, glow_r=26, hold=0.6, head=0.6))
# Beat 2: the payment in, then the split.
lights.append(Light(via(YOU, HUB, -38), PAY, 0.9, GOLD, width=3.0, glow_r=R_S1 + 4, hold=0.9))
lights.append(Light(via(HUB, PLUGIN, -10) + via(PLUGIN, AUTHOR, 10)[1:], SPLIT, 1.1, GOLD,
                    width=2.8, glow_r=18, hold=1.4))
lights.append(Light(via(HUB, OA, 24), SPLIT + 0.25, 0.9, GOLD, width=2.4, glow_r=18, hold=1.4))
# Beat 3: the first paid call bonus, from OpenAgents' share to the author.
lights.append(Light(via(OA, AUTHOR, 90, 22), BONUS, 1.4, GOLD, width=3.2, glow_r=22, hold=1.6, ring=True))
# Beat 4: the savings stay with you.
lights.append(Light(via(HUB, YOU, -38), SAVE + 1.6, 1.0, LIGHT, width=2.4, glow_r=22, hold=1.8))

# Steady calls from beat 3 on, quieter: white out, gold in, gold on to the author.
rng = random.Random(290)
t = 12.0
while t < SECONDS:
    lights.append(Dot([YOU, s1(math.pi)], t, 0.55, LIGHT, r=2.2))
    lights.append(Dot(ring(math.pi, -2.0) + ring(-2.0, -0.42)[1:] + [PLUGIN], t + 0.55, 0.6, LIGHT, r=2.2))
    lights.append(Dot(via(YOU, HUB, -38), t + 0.9, 0.6, GOLD, r=2.6))
    lights.append(Dot(via(HUB, PLUGIN, -10) + via(PLUGIN, AUTHOR, 10)[1:], t + 1.55, 0.8, GOLD, r=3.0))
    lights.append(Dot(via(HUB, OA, 24), t + 1.65, 0.7, GOLD, r=2.4))
    if rng.random() < 0.18:
        lights.append(Dot([s1(-math.pi / 2), MODEL], t + 0.6, 0.4, LIGHT, r=2.0))
    t += (1.0 if t > STEADY else 1.6) + rng.random() * 0.35


# --- drawing helpers -------------------------------------------------------------------
def bolt(d, x, y, h, fill):
    """A small lightning bolt, top-left at (x, y), h points tall."""
    w = h * 0.62
    pts = [(0.62, 0.0), (0.05, 0.56), (0.45, 0.56), (0.30, 1.0), (0.95, 0.40), (0.55, 0.40), (0.78, 0.0)]
    d.polygon([(x + px * w, y + py * h) for px, py in pts], fill=fill)


def amount(d, x, y, sats, to, a, gold=True, note=None):
    """'⚡ N sats → who': the bolt, the amount in Berkeley Mono, the rest in Geist."""
    if a <= 0:
        return
    col = GOLD if gold else LIGHT
    bolt(d, x, y + 3, 22, (*col, int(255 * a)))
    nx = x + 24
    d.text((nx, y), sats, font=m_amt, fill=(*col, int(255 * a)))
    nx += m_amt.getlength(sats) + 8
    d.text((nx, y + 2), "sats", font=g_lab, fill=(*col, int(235 * a)))
    nx += g_lab.getlength("sats") + 10
    d.text((nx, y + 2), "→ " + to, font=g_lab, fill=(*CONTENT, int(255 * a)))
    if note:
        d.text((x + 24, y + 32), note, font=g_tiny, fill=(*SECONDARY, int(255 * a)))


def node(d, ld, p, r, gray, glow=0.0, color=LIGHT):
    x, y = p[0] * S, p[1] * S
    if glow > 0:
        rr = r * 2.0 * S
        ld.ellipse([x - rr, y - rr, x + rr, y + rr], fill=(*color, int(70 * glow)))
    rr = r * S
    d.ellipse([x - rr, y - rr, x + rr, y + rr], fill=(gray, gray, gray, 255))


def centered(d, p, dy, text, f, fill):
    w = f.getlength(text)
    d.text((p[0] - w / 2, p[1] + dy), text, font=f, fill=fill)


def scene(d, ld, t):
    k = fade(t, 0.2, 1.2)
    if k <= 0:
        return
    edge = (255, 255, 255, int(EDGE_A * k))
    for a, b, bend in ((YOU, HUB, 0), (HUB, PLUGIN, -10), (PLUGIN, AUTHOR, 10), (HUB, OA, 24)):
        d.line([(x * S, y * S) for x, y in via(a, b, bend)], fill=edge, width=S)
    d.line([(HUB[0] * S, (HUB[1] - R_S1) * S), (MODEL[0] * S, MODEL[1] * S)], fill=edge, width=S)
    # money edges in gold, once money has moved on them
    for (a, b, bend), t0 in (((YOU, HUB, -38), PAY), ((PLUGIN, AUTHOR, 10), SPLIT + 0.6),
                             ((HUB, OA, 24), SPLIT + 0.5), ((OA, AUTHOR, 90), BONUS + 0.8)):
        g = fade(t, t0, t0 + 0.8)
        if g > 0:
            d.line([(x * S, y * S) for x, y in via(a, b, bend, 24)], fill=(*GOLD, int(34 * g * k)), width=S)
    calm = fade(t, SAVE, SAVE + 2.0)
    node(d, ld, MODEL, 22 - 6 * calm, int(200 - 70 * calm), glow=0.4 * (1 - calm))
    node(d, ld, YOU, 15, 225, glow=0.3 + 0.7 * fade(t, SAVE + 2.4, SAVE + 3.0))
    node(d, ld, HUB, R_HUB, 225, glow=0.5)
    sweep = out_cubic((t - 0.6) / 1.2)
    if sweep > 0:
        rr = R_S1 * S
        x, y = HUB[0] * S, HUB[1] * S
        ld.arc([x - rr, y - rr, x + rr, y + rr], -90, -90 + 360 * sweep, fill=(*LIGHT, 60), width=6 * S)
        d.arc([x - rr, y - rr, x + rr, y + rr], -90, -90 + 360 * sweep, fill=(*LIGHT, 230), width=2 * S)
    lit = fade(t, REQ + 1.6, REQ + 2.0)
    node(d, ld, PLUGIN, 13, int(196 + 40 * lit), glow=lit)
    au = fade(t, SPLIT + 1.0, SPLIT + 1.6)
    node(d, ld, AUTHOR, 9, 0, glow=0)
    x, y = AUTHOR[0] * S, AUTHOR[1] * S
    rr = 9 * S
    d.ellipse([x - rr, y - rr, x + rr, y + rr], fill=(*GOLD, int(110 + 145 * au)))
    if au > 0:
        rr = 20 * S
        ld.ellipse([x - rr, y - rr, x + rr, y + rr], fill=(*GOLD, int(80 * au)))
    oa = fade(t, SPLIT + 0.9, SPLIT + 1.4)
    node(d, ld, OA, 11, 210, glow=0.4 * oa, color=GOLD)


def scene_words(d, t):
    k = fade(t, 0.4, 1.4)
    if k <= 0:
        return
    centered(d, YOU, 40, "you", g_node, (*CONTENT, int(255 * k)))
    centered(d, HUB, R_S1 + 14, "Coder", g_node, (*CONTENT, int(255 * k)))
    centered(d, HUB, R_S1 + 38, "System 1 in front", g_tiny, (*SECONDARY, int(255 * k)))
    calm = fade(t, SAVE, SAVE + 2.0)
    centered(d, MODEL, -54, "the big model", g_node, (*(SECONDARY if calm else CONTENT), int(255 * k)))
    d.text((PLUGIN[0] + 24, PLUGIN[1] - 12), "Explain this error", font=g_node, fill=(*CONTENT, int(255 * k)))
    d.text((PLUGIN[0] + 24, PLUGIN[1] + 13), "a plugin", font=g_tiny, fill=(*SECONDARY, int(255 * k)))
    d.text((AUTHOR[0] - 30, AUTHOR[1] - 50), "plugin author", font=g_node, fill=(*CONTENT, int(255 * k)))
    d.text((OA[0] + 22, OA[1] - 12), "OpenAgents", font=g_node, fill=(*CONTENT, int(255 * k)))
    d.text((OA[0] + 22, OA[1] + 13), "model, routing, hosting", font=g_tiny, fill=(*SECONDARY, int(255 * k)))


def dim_after(t, t0, t1):
    """Bright when new, then a quieter hold."""
    return fade(t, t0, t0 + 0.5) * (1 - 0.45 * fade(t, t1, t1 + 1.0))


def pulse_labels(d, t):
    # 15 sats in, above the you -> Coder edge
    amount(d, 790, 462, "15", "Coder", dim_after(t, PAY + 0.5, SPLIT + 2.0), note="the whole call")
    # the split
    amount(d, 1240, 304, "10", "plugin author", dim_after(t, SPLIT + 1.0, BONUS),
           note="the plugin's whole fee")
    amount(d, 1490, 870, "5", "OpenAgents", dim_after(t, SPLIT + 1.2, BONUS),
           note="less the Lightning fees")
    # the bonus
    amount(d, 1500, 182, "1,000", "plugin author", dim_after(t, BONUS + 1.3, SAVE + 2.0),
           note="first paid call bonus, from our share")


def caption(d, t):
    for a, b, lines in CAPS:
        k = fade(t, a, a + 0.6) * (1 - fade(t, b - 0.5, b)) if b < SECONDS else fade(t, a, a + 0.6)
        if k <= 0:
            continue
        for j, line in enumerate(lines):
            d.text((LEFT, 150 + j * 46), line, font=g_cap, fill=(*CONTENT, int(255 * k)))


COST_BARE, COST_S1 = 4.00, 1.84  # $ per accepted fix, briefed-agent-ab.md


def meters(d, t):
    a = fade(t, SAVE - 0.3, SAVE + 0.3)
    if a <= 0:
        return
    down = ease((t - SAVE - 0.3) / 1.6)
    cost = COST_BARE + (COST_S1 - COST_BARE) * down
    y = 600
    d.text((LEFT, y), "cost per fix", font=g_small, fill=(*SECONDARY, int(255 * a)))
    d.text((LEFT, y + 28), f"${cost:.2f}", font=m_big, fill=(*CONTENT, int(255 * a)))
    k = fade(t, SAVE + 0.2, SAVE + 0.7) * a
    if k > 0:
        ow = m_old.getlength("$4.00")
        d.text((LEFT, y + 98), "$4.00", font=m_old, fill=(*TERTIARY, int(255 * k)))
        d.line([(LEFT - 2, y + 113), (LEFT + ow + 2, y + 113)], fill=(*TERTIARY, int(255 * k)), width=2)
    k = fade(t, SAVE + 2.2, SAVE + 2.8) * a
    if k > 0:
        x = LEFT + 250
        d.text((x, y), "saved per fix", font=g_small, fill=(*SECONDARY, int(255 * k)))
        d.text((x, y + 28), "$2.16", font=m_big, fill=(*LIGHT, int(255 * k)))
        d.text((x + m_big.getlength("$2.16") + 12, y + 46), "→ you", font=g_lab, fill=(*CONTENT, int(255 * k)))
    k = fade(t, SAVE + 3.0, SAVE + 3.6) * a
    if k > 0:
        d.text((LEFT, y + 160), "Same model, same success rate. 21 real issues, 3 runs each.",
               font=g_small, fill=(*SECONDARY, int(255 * k)))


def footnote(d, t):
    k = fade(t, PAY + 0.6, PAY + 1.4)
    if k <= 0:
        return
    lines = ["Demo payments: the price quoted for Explain this error on Oct 3, 2026",
             "(5 sats + a 10-sat author fee). Split by revenue-share rule v1."]
    for j, line in enumerate(lines):
        d.text((LEFT, H - 92 + j * 22), line, font=g_tiny, fill=(*TERTIARY, int(255 * k)))


# --- frame ------------------------------------------------------------------------------
def glow_bg():
    g = Image.new("L", (W // 4, H // 4), 0)
    cx, cy = 1240, 560
    ImageDraw.Draw(g).ellipse([(cx - 620) / 4, (cy - 480) / 4, (cx + 620) / 4, (cy + 480) / 4], fill=40)
    g = g.filter(ImageFilter.GaussianBlur(60)).resize((W, H), Image.BICUBIC)
    a = np.asarray(g, dtype=np.float32) / 255.0
    base = np.zeros((H, W, 3), np.float32) + np.array(CANVAS, np.float32)
    return base + a[..., None] * np.array([150, 154, 168], np.float32) * 0.45


BG = glow_bg()
vig = Image.new("L", (W // 4, H // 4), 0)
ImageDraw.Draw(vig).ellipse([-W * .2 / 4, -H * .3 / 4, W * 1.2 / 4, H * 1.3 / 4], fill=255)
VIG = np.asarray(vig.filter(ImageFilter.GaussianBlur(50)).resize((W, H), Image.BICUBIC), np.float32)[..., None] / 255.0
VIG = 0.35 + 0.65 * VIG


def over(dst, layer_rgba, k=1.0):
    a = layer_rgba[..., 3:4] * k
    return dst * (1 - a) + layer_rgba[..., :3] * 255.0 * a


def text_layer(draw_fn):
    im = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    draw_fn(ImageDraw.Draw(im))
    return np.asarray(im, np.float32) / 255.0


def render(i):
    t = i / FPS
    frame = BG.copy()
    hi = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    hd = ImageDraw.Draw(hi)
    lit = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    ld = ImageDraw.Draw(lit)
    scene(hd, ld, t)
    for l in lights:
        l.draw(ld, t)
    frame = over(frame, np.asarray(hi.resize((W, H), Image.LANCZOS), np.float32) / 255.0)
    lit_a = np.asarray(lit.resize((W, H), Image.LANCZOS), np.float32) / 255.0
    rgb = lit_a[..., :3] * lit_a[..., 3:4] * 255.0
    small = Image.fromarray(rgb.clip(0, 255).astype(np.uint8)).resize((W // 4, H // 4), Image.BILINEAR)
    bloom = np.asarray(small.filter(ImageFilter.GaussianBlur(5)).resize((W, H), Image.BILINEAR), np.float32)
    frame = frame + rgb + bloom * 1.6
    frame = over(frame, text_layer(lambda d: (scene_words(d, t), pulse_labels(d, t), caption(d, t),
                                              meters(d, t), footnote(d, t))))
    # a slow push in over the whole shot
    img = Image.fromarray(frame.clip(0, 255).astype(np.uint8))
    z = 1.0 + 0.025 * ease(t / SECONDS)
    cw, ch = W / z, H / z
    ox, oy = (W - cw) / 2, (H - ch) / 2
    img = img.transform((W, H), Image.EXTENT, (ox, oy, ox + cw, oy + ch), Image.BICUBIC, fillcolor=CANVAS)
    arr = np.asarray(img, np.float32) * VIG * fade(t, 0.0, 0.6)
    noise = np.random.default_rng(i).normal(0, 3.2, (H, W, 1)).astype(np.float32)
    return Image.fromarray((arr + noise).clip(0, 255).astype(np.uint8))


if args.only:
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    for s in args.only.split(","):
        render(int(float(s) * FPS)).save(out / f"t{float(s):05.2f}.png")
    raise SystemExit
frames = int(FPS * SECONDS)
ff = subprocess.Popen(["ffmpeg", "-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24",
                       "-s", f"{W}x{H}", "-r", str(FPS), "-i", "-", "-c:v", "libx264", "-pix_fmt",
                       "yuv420p", "-crf", "15", "-preset", "slow", "-movflags", "+faststart", args.out],
                      stdin=subprocess.PIPE)
if args.frames_dir:
    Path(args.frames_dir).mkdir(parents=True, exist_ok=True)
for i in range(frames):
    im = render(i)
    ff.stdin.write(im.tobytes())
    if args.frames_dir and i % FPS == 0:
        im.save(Path(args.frames_dir) / f"s{i // FPS:02d}.png")
ff.stdin.close()
ff.wait()
print(f"{frames} frames, {frames / FPS:.2f} s -> {args.out}")
