#!/usr/bin/env python3
"""Four short shots for the Coder v1 launch film (episode 290, sections 4-6),
in the look of system1-routing.py: the noir canvas, the request light, a
soft bloom, Geist for words and Berkeley Mono for numbers.

  scripts/film/coder-segments.py SEGMENT OUT.mp4 [--only SECONDS,...]

SEGMENT is one of:

  harnesses   (17.5 s) One Coder, every agent. Coder at the center with the
              harnesses it can drive around it (crates/coder-new/src/
              harness_pick.rs: Claude Code, Codex, OpenCode, Grok, Devin,
              Cursor). Auto picks; Shift+Tab steps a highlight around them in
              the order the episode shows; then one ring through all of them:
              same conversation, same tools, same safety.
  safety      (20.0 s) Safety you don't have to think about. Agents on the
              left, what they do on the right, a gate between. Edits and
              tests pass; a deploy, merging a pull request, a secret, a
              payment, a DNS change stop at the gate (the floors in
              crates/coder-new/src/floors.rs and risk_policy.rs) until a
              person answers: one is allowed, one is declined, on every
              agent. Then each chat in its own copy of the repo.
  everywhere  (17.5 s) Everywhere you are. One conversation as one thread
              between a terminal, a phone and the web; every message shows
              up on all three. Mac, Windows and Linux, the web, iPhone and
              Android.
  metaharness (21.5 s) What a metaharness is (episode 290 take "Metaharness",
              section 3). Opens on the routing shot's close-up of the big
              model in its tools, pulls back to three harnesses (Claude Code,
              Codex, OpenCode), each a model in its tools with every step
              going into the model; then Coder, a System 1 ring, comes in
              front: small steps go to a small model or Microcoder, only the
              hard parts reach a harness.
  savings     (20.5 s) What it saves (section 5). Opens on the routing shot's
              beat 2 counters, then the two runs side by side: cost per fix
              $4.00 vs $1.84, time per fix 284 s vs 121 s, 19 of 63 fixes
              accepted both ways (docs/inference/briefed-agent-ab.md, #11211).
  devday      (10.5 s) The promise and the payout (section 8's close): a
              timeline from OpenAI's DevDay (2023-11-06, revenue sharing
              promised) and Episode 1 the next day to October 2026, "Shared
              with developers: zero", then gold running out to plugin authors.
  delegation  (18.5 s) Delegation across agents. Claude Code hands a piece to
              Codex, Codex hands one to OpenCode (nesting is bounded at 2),
              each in its own worktree; short reports come back (the Report
              fields from #11511); the parent's prompt cache stays warm.

The only numbers on screen are in `delegation`: the last turn of the same
3-turn Claude Code chat read 0.9965 of its prompt from the cache with a
delegation in turn 2 and 0.9977 without (#11511 closing comment, from Claude
Code's usage records), shown as 99.7% and 99.8%.

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
ap.add_argument("segment", choices=["harnesses", "safety", "everywhere", "delegation", "metaharness", "savings", "devday"])
ap.add_argument("out")
ap.add_argument("--only", help="render only these seconds to PNGs (comma list) in OUT as a dir")
ap.add_argument("--beats", action="store_true",
                help="only write OUT's .beats.json (the animation beats cut.py check reads)")
args = ap.parse_args()

# Animation beats, in the segment's own seconds: a cut that ends inside one
# chops the animation off (cut.py check fails it). Written beside the video as
# <out>.beats.json.
BEATS = {
    "delegation": [
        {"from": 9.6, "to": 12.75, "what": "the report card"},
        {"from": 13.4, "to": 15.9, "what": "the prompt cache numbers"},
    ],
}


def write_beats(out):
    import json
    side = Path(out).with_suffix(".beats.json")
    side.write_text(json.dumps(BEATS.get(args.segment, []), indent=1) + "\n")
    return side


if args.beats:
    print(write_beats(args.out))
    raise SystemExit

GEIST = Path(os.path.expanduser(os.environ.get("FILM_FONTS", "~/work/film-out/fonts")))
BERKELEY = Path(os.path.expanduser(os.environ.get("BERKELEY_FONTS", "~/Library/Fonts")))

W, H, FPS = 1920, 1080, 30
S = 2  # live layers are drawn at twice the size, then scaled down

# --- palette (system1-routing.py) -------------------------------------------
CANVAS = (10, 10, 10)
CONTENT = (237, 237, 237)
SECONDARY = (129, 129, 129)
TERTIARY = (101, 101, 101)
LIGHT = (236, 240, 255)
GOLD = (255, 206, 84)
EDGE_A = 38
LEFT = 118
CX, CY = 1238.0, 540.0


def font(name, size):
    path = GEIST / name if name.startswith("Geist") else BERKELEY / name
    return ImageFont.truetype(str(path), size)


g_cap = font("Geist-Regular.ttf", 34)
g_sub = font("Geist-Regular.ttf", 22)
g_msg = font("Geist-Regular.ttf", 22)
g_node = font("Geist-Regular.ttf", 19)
g_core = font("Geist-Regular.ttf", 21)
g_small = font("Geist-Regular.ttf", 17)
g_tiny = font("Geist-Regular.ttf", 15)
g_bubble = font("Geist-Regular.ttf", 12)
m_big = font("BerkeleyMono-Regular.ttf", 50)
m_key = font("BerkeleyMono-Regular.ttf", 20)


# --- easing -----------------------------------------------------------------
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


def window(t, a, b, fin=0.5, fout=0.35):
    return fade(t, a, a + fin) * (1 - fade(t, b - fout, b))


def lerp(a, b, k):
    return (a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k)


def mix(c1, c2, k):
    return tuple(int(c1[i] + (c2[i] - c1[i]) * k) for i in range(3))


def polar(r, a, c=(CX, CY)):
    return (c[0] + r * math.cos(a), c[1] + r * math.sin(a))


def arc_pts(c, r, a0, a1, n=24):
    d = math.atan2(math.sin(a1 - a0), math.cos(a1 - a0))
    return [polar(r, a0 + d * j / n, c) for j in range(n + 1)]


def curve(a, b, bend=0.0, n=24):
    """A gentle quadratic curve from a to b, bowed sideways by `bend` points."""
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


# --- lights (system1-routing.py) --------------------------------------------
class Light:
    """A light along screen points: the trail behind the head, the head, and
    a ring at the target once it lands."""

    def __init__(self, pts, t0, dur, color=LIGHT, width=3.0, hold=0.7, glow_r=10.0, head=1.0,
                 land=True):
        self.pts, self.t0, self.dur, self.color = pts, t0, dur, color
        self.width, self.hold, self.glow_r, self.head, self.land = width, hold, glow_r, head, land
        segs = [math.dist(pts[i], pts[i + 1]) for i in range(len(pts) - 1)]
        self.segs, self.total = segs, max(sum(segs), 1e-3)

    def at(self, u):
        left = clamp(u) * self.total
        for i, L in enumerate(self.segs):
            if left <= L or i == len(self.segs) - 1:
                k = 0 if L == 0 else clamp(left / L)
                return lerp(self.pts[i], self.pts[i + 1], k), i, k
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
        elif self.land:
            land = clamp((t - self.t0 - self.dur) / 0.25)
            x, y = self.pts[-1]
            r = (self.glow_r + 10 * land) * S
            d.ellipse([x * S - r, y * S - r, x * S + r, y * S + r],
                      outline=(*c, int(220 * fadek)), width=2 * S)


class Dot:
    """A small pulse along a path."""

    def __init__(self, pts, t0, dur, color=LIGHT, r=3.0):
        self.l = Light(pts, t0, dur, color)
        self.color, self.r = color, r

    def draw(self, d, t):
        u = (t - self.l.t0) / self.l.dur
        if u <= 0 or u >= 1:
            return
        (x, y), _, _ = self.l.at(ease(u))
        for r, a in ((self.r * 2.6, 60), (self.r, 255)):
            r *= S
            d.ellipse([x * S - r, y * S - r, x * S + r, y * S + r], fill=(*self.color, a))


def disc(d, p, r, gray, a=255):
    x, y = p
    d.ellipse([(x - r) * S, (y - r) * S, (x + r) * S, (y + r) * S], fill=(gray, gray, gray, int(a)))


def ring(d, p, r, color, a, width=2.0):
    x, y = p
    d.ellipse([(x - r) * S, (y - r) * S, (x + r) * S, (y + r) * S], outline=(*color, int(a)),
              width=max(1, int(width * S)))


def glow(ld, p, r, a):
    x, y = p
    for k, aa in ((1.6, 0.35), (1.15, 0.7)):
        rr = r * k
        ld.ellipse([(x - rr) * S, (y - rr) * S, (x + rr) * S, (y + rr) * S], fill=(*LIGHT, int(a * aa)))


def line(d, pts, color, a, width=1.0):
    d.line([(x * S, y * S) for x, y in pts], fill=(*color, int(a)), width=max(1, int(width * S)),
           joint="curve")


def rrect(d, box, r, outline=None, fill=None, width=1.5, k=1.0):
    x0, y0, x1, y1 = box
    d.rounded_rectangle([x0 * S, y0 * S, x1 * S, y1 * S], radius=r * S,
                        outline=None if outline is None else (*outline[:3], int(outline[3] * k)),
                        fill=None if fill is None else (*fill[:3], int(fill[3] * k)),
                        width=max(1, int(width * S)))


def label_side(d, p, r, ang, text, f, fill):
    """A label outside its node, away from the center (route_map.rs)."""
    x, y = p
    c, s = math.cos(ang), math.sin(ang)
    tw = d.textlength(text, font=f)
    asc, desc = f.getmetrics()
    th = asc + desc
    gap = r + 12
    if c > 0.4:
        lx, ly = x + c * gap, y + s * gap - th / 2
    elif c < -0.4:
        lx, ly = x + c * gap - tw, y + s * gap - th / 2
    elif s >= 0:
        lx, ly = x - tw / 2, y + gap - 2
    else:
        lx, ly = x - tw / 2, y - gap - th + 2
    d.text((lx, ly), text, font=f, fill=fill)


def centered(d, p, text, f, fill):
    tw = d.textlength(text, font=f)
    asc, desc = f.getmetrics()
    d.text((p[0] - tw / 2, p[1] - (asc + desc) / 2 + 1), text, font=f, fill=fill)


def captions(d, t, caps, y0=150):
    for a, b, lines in caps:
        k = window(t, a, b)
        if k <= 0:
            continue
        for j, ln in enumerate(lines):
            d.text((LEFT, y0 + j * 46), ln, font=g_cap, fill=(*CONTENT, int(255 * k)))


# --- the frame (system1-routing.py) -----------------------------------------
def glow_bg(cx, cy, rx=560, ry=520):
    g = Image.new("L", (W // 4, H // 4), 0)
    ImageDraw.Draw(g).ellipse([(cx - rx) / 4, (cy - ry) / 4, (cx + rx) / 4, (cy + ry) / 4], fill=40)
    g = g.filter(ImageFilter.GaussianBlur(60)).resize((W, H), Image.BICUBIC)
    a = np.asarray(g, dtype=np.float32) / 255.0
    base = np.zeros((H, W, 3), np.float32) + np.array(CANVAS, np.float32)
    return base + a[..., None] * np.array([150, 154, 168], np.float32) * 0.45


vig = Image.new("L", (W // 4, H // 4), 0)
ImageDraw.Draw(vig).ellipse([-W * .2 / 4, -H * .3 / 4, W * 1.2 / 4, H * 1.3 / 4], fill=255)
VIG = np.asarray(vig.filter(ImageFilter.GaussianBlur(50)).resize((W, H), Image.BICUBIC), np.float32)[..., None] / 255.0
VIG = 0.35 + 0.65 * VIG


def over(dst, layer, k=1.0):
    a = layer[..., 3:4] * k
    return dst * (1 - a) + layer[..., :3] * 255.0 * a


# Segments that open on the last frame of the shot before them (a match cut),
# so they don't fade up from black.
MATCHED = {"metaharness", "savings"}


def compose(i, t, bg, draw_hi, draw_lit, draw_words, seconds, focus=(CX, CY)):
    hi = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    lit = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    draw_hi(ImageDraw.Draw(hi), t)
    draw_lit(ImageDraw.Draw(lit), t)
    frame = bg.copy()
    frame = over(frame, np.asarray(hi.resize((W, H), Image.LANCZOS), np.float32) / 255.0)
    lit_a = np.asarray(lit.resize((W, H), Image.LANCZOS), np.float32) / 255.0
    rgb = lit_a[..., :3] * lit_a[..., 3:4] * 255.0
    small = Image.fromarray(rgb.clip(0, 255).astype(np.uint8)).resize((W // 4, H // 4), Image.BILINEAR)
    bloom = np.asarray(small.filter(ImageFilter.GaussianBlur(5)).resize((W, H), Image.BILINEAR), np.float32)
    frame = frame + rgb + bloom * 1.6
    words = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    draw_words(ImageDraw.Draw(words), t)
    frame = over(frame, np.asarray(words, np.float32) / 255.0)
    img = Image.fromarray(frame.clip(0, 255).astype(np.uint8))
    # a slow push-in toward the stage over the whole shot
    z = 1.0 + 0.03 * ease(t / seconds)
    fx, fy = focus
    ox, oy = fx - fx / z, fy - fy / z
    img = img.transform((W, H), Image.EXTENT, (ox, oy, ox + W / z, oy + H / z), Image.BICUBIC,
                        fillcolor=CANVAS)
    k = 1.0 if args.segment in MATCHED else fade(t, 0.0, 0.5)
    arr = np.asarray(img, np.float32) * VIG * k
    noise = np.random.default_rng(i).normal(0, 3.2, (H, W, 1)).astype(np.float32)
    return Image.fromarray((arr + noise).clip(0, 255).astype(np.uint8))


# =============================================================================
# harnesses: One Coder, every agent
# =============================================================================
def seg_harnesses():
    SECONDS = 17.5
    R_CORE, R_H = 66.0, 300.0
    # The Shift+Tab order the episode shows (section 4), clockwise from the top.
    NAMES = ["Claude Code", "Codex", "OpenCode", "Grok", "Devin", "Cursor"]
    ang = [-math.pi / 2 + i * 2 * math.pi / 6 for i in range(6)]
    pos = [polar(R_H, a) for a in ang]
    born = [0.9 + 0.32 * i for i in range(6)]
    CAPS = [
        (0.5, 4.4, ["Coder doesn't replace your agents.", "It sits in front of all of them."]),
        (4.7, 8.7, ["Leave it on Auto,", "and Coder picks for you."]),
        (8.95, 12.85, ["Or press Shift+Tab", "and choose."]),
        (13.1, 99, ["Same conversation, same tools,", "same safety, whichever one answers."]),
    ]
    # Auto: each message goes into Coder, and on to the agent it picks.
    MSGS = [(5.0, "Fix the flaky parser test", 0), (5.75, "Write the migration", 1),
            (6.5, "Explain this crate", 2), (7.25, "Review my diff", 5)]
    MSG_Y0, MSG_DY = 352, 52
    # Shift+Tab: Auto -> each agent -> Auto, one press every half second.
    STEPS = [None, 0, 1, 2, 3, 4, 5, None]
    T_TAB0, T_TAB = 9.15, 0.5

    def anchor(k, text):
        return (LEFT + g_msg.getlength(text) + 22, MSG_Y0 + k * MSG_DY + 14)

    lights = []
    for k, (t0, text, h) in enumerate(MSGS):
        a = anchor(k, text)
        lights.append(Light([a, (CX, CY)], t0 + 0.1, 0.42, width=3.0, glow_r=R_CORE + 4, hold=0.35))
        lights.append(Light([polar(R_CORE, ang[h]), pos[h]], t0 + 0.55, 0.32, width=2.4,
                            glow_r=22, hold=0.6))
    # Shift+Tab: a light steps along the ring from one choice to the next.
    for j in range(1, len(STEPS)):
        a0 = ang[STEPS[j - 1]] if STEPS[j - 1] is not None else None
        a1 = ang[STEPS[j]] if STEPS[j] is not None else None
        t0 = T_TAB0 + j * T_TAB
        if a0 is not None and a1 is not None:
            lights.append(Light(arc_pts((CX, CY), R_H, a0, a1, 18), t0, 0.3, width=2.0, glow_r=22,
                                hold=0.3, head=0.7))
        elif a1 is not None:  # out of Auto, from the center
            lights.append(Light([polar(R_CORE, a1), pos[STEPS[j]]], t0, 0.3, width=2.0, glow_r=22,
                                hold=0.3, head=0.7))
        else:  # back to Auto
            lights.append(Light([pos[STEPS[j - 1]], polar(R_CORE, ang[STEPS[j - 1]])], t0, 0.3,
                                width=2.0, glow_r=R_CORE + 4, hold=0.5, head=0.7))
    # The last beat: the same conversation goes round all of them.
    rng = random.Random(290)
    for j in range(46):
        t = 13.6 + rng.random() * 3.6
        h = rng.randrange(6)
        lights.append(Dot([polar(R_CORE, ang[h]), pos[h]], t, 0.5, r=2.3))
        lights.append(Dot(arc_pts((CX, CY), R_H, ang[h], ang[h] + 2 * math.pi / 6, 10), t + 0.5, 0.6, r=2.0))

    def selected(t):
        if t < T_TAB0 + T_TAB:
            return None
        j = min(int((t - T_TAB0) / T_TAB), len(STEPS) - 1)
        return STEPS[j], T_TAB0 + j * T_TAB

    def auto_pick(t):
        """The agent Auto last picked, and how lit it still is."""
        best = None
        for t0, _, h in MSGS:
            if t >= t0 + 0.8:
                best = (h, 1 - fade(t, t0 + 1.0, t0 + 1.9))
        return best

    def hi(d, t):
        appear = fade(t, 0.0, 1.0)
        for i in range(6):
            k = out_cubic((t - born[i]) / 0.5)
            if k <= 0:
                continue
            e = lerp(polar(R_CORE, ang[i]), pos[i], k)
            line(d, [polar(R_CORE, ang[i]), e], (255, 255, 255), EDGE_A + 20)
        # the outer ring, faint, then lit in the last beat
        ring_k = fade(t, 2.8, 3.8)
        if ring_k > 0:
            ring(d, (CX, CY), R_H, (255, 255, 255), EDGE_A * ring_k, 1.0)
        sel = selected(t)
        for i in range(6):
            k = out_cubic((t - born[i]) / 0.5)
            if k <= 0:
                continue
            g = 205
            if sel and sel[0] == i:
                g = 245
            disc(d, pos[i], 17 * k, g)
            flash = 1 - fade(t, born[i] + 0.2, born[i] + 1.0)
            if flash > 0:
                ring(d, pos[i], 17 * (1.3 + 0.9 * (1 - flash)), LIGHT, 160 * flash, 1.0)
        rc = R_CORE * appear
        if rc > 0:
            disc(d, (CX, CY), rc, 232)

    def lit(ld, t):
        appear = fade(t, 0.0, 1.0)
        if appear > 0:
            glow(ld, (CX, CY), R_CORE * 1.05, 120 * appear)
        sel = selected(t)
        if sel:
            i, t0 = sel
            k = out_cubic((t - t0) / 0.25)
            if i is not None:
                glow(ld, pos[i], 17 + 6 * k, 150)
                ring(ld, pos[i], 30 + 4 * (1 - k), LIGHT, 230, 2.0)
            else:
                ring(ld, (CX, CY), R_CORE + 14, LIGHT, 200 * k * (1 - fade(t, 12.9, 13.4)), 2.0)
        ap_ = auto_pick(t)
        if ap_ and ap_[1] > 0:
            glow(ld, pos[ap_[0]], 20, 140 * ap_[1])
        # same everything: the ring through all of them lights up
        sweep = out_cubic((t - 13.2) / 1.4)
        if sweep > 0:
            box = [(CX - R_H) * S, (CY - R_H) * S, (CX + R_H) * S, (CY + R_H) * S]
            ld.arc(box, -90, -90 + 360 * sweep, fill=(*LIGHT, 70), width=7 * S)
            ld.arc(box, -90, -90 + 360 * sweep, fill=(*LIGHT, 220), width=2 * S)
            for i in range(6):
                kk = fade(t, 13.2 + 1.4 * i / 6, 13.5 + 1.4 * i / 6)
                if kk > 0:
                    glow(ld, pos[i], 19, 110 * kk)
        for l in lights:
            l.draw(ld, t)

    def words(d, t):
        captions(d, t, CAPS)
        for i in range(6):
            k = fade(t, born[i] + 0.15, born[i] + 0.6)
            if k > 0:
                sel = selected(t)
                col = CONTENT if (sel and sel[0] == i) or t > 13.4 else (200, 200, 200)
                label_side(d, pos[i], 17, ang[i], NAMES[i], g_node, (*col, int(255 * k)))
        a = fade(t, 0.5, 1.2)
        if a > 0:
            sel = selected(t)
            mode = "Auto" if sel is None or sel[0] is None else NAMES[sel[0]]
            if t > 12.95:
                mode = "Auto"
            d.text((CX - g_core.getlength("Coder") / 2, CY - 22), "Coder", font=g_core,
                   fill=(14, 14, 14, int(255 * a)))
            f = g_tiny if len(mode) < 10 else font("Geist-Regular.ttf", 13)
            d.text((CX - f.getlength(mode) / 2, CY + 6), mode, font=f, fill=(48, 48, 48, int(255 * a)))
        # Auto's messages under the caption
        for k, (t0, text, h) in enumerate(MSGS):
            a = fade(t, t0 - 0.15, t0 + 0.15) * (1 - fade(t, 8.45, 8.85))
            if a <= 0:
                continue
            y = MSG_Y0 + k * MSG_DY + 8 * (1 - fade(t, t0 - 0.15, t0 + 0.2))
            done = fade(t, t0 + 0.6, t0 + 1.4)
            d.text((LEFT, y), text, font=g_msg, fill=(*mix(CONTENT, SECONDARY, done), int(255 * a)))
            ax, ay = anchor(k, text)
            d.ellipse([ax - 3.2, ay - 3.2, ax + 3.2, ay + 3.2], fill=(*LIGHT, int(255 * a)))
            # where it went, once it lands
            ka = fade(t, t0 + 0.9, t0 + 1.3) * a
            if ka > 0:
                d.text((ax + 14, y + 3), "→ " + NAMES[h], font=g_small, fill=(*SECONDARY, int(255 * ka)))
        # the key, pressed once per step
        ka = window(t, 9.0, 12.85)
        if ka > 0:
            key = "Shift+Tab"
            kw = m_key.getlength(key)
            x0, y0 = LEFT, 360
            sel = selected(t)
            press = 0.0
            if sel:
                press = 1 - fade(t, sel[1], sel[1] + 0.35)
            col = mix(SECONDARY, CONTENT, press)
            d.rounded_rectangle([x0, y0, x0 + kw + 36, y0 + 46], radius=8,
                                outline=(*col, int(255 * ka)), width=2)
            d.text((x0 + 18, y0 + 10), key, font=m_key, fill=(*col, int(255 * ka)))
            # the cycle, with the current choice lit
            order = ["Auto"] + NAMES
            x, y = LEFT, 440
            cur = "Auto" if sel is None or sel[0] is None else NAMES[sel[0]]
            for j, nm in enumerate(order):
                c = CONTENT if nm == cur else TERTIARY
                d.text((x, y + j * 30), nm, font=g_small, fill=(*c, int(255 * ka)))
                if nm == cur:
                    d.ellipse([x - 16, y + j * 30 + 9, x - 9, y + j * 30 + 16], fill=(*LIGHT, int(255 * ka)))

    return SECONDS, glow_bg(CX, CY), hi, lit, words, (CX, CY)


# =============================================================================
# safety: Some things always wait for you
# =============================================================================
def seg_safety():
    SECONDS = 20.0
    AX, GX, TX = 760.0, 1190.0, 1560.0
    AGENTS = ["Claude Code", "Codex", "OpenCode", "Grok", "Devin", "Cursor"]
    a_pos = [(AX, 290 + i * 100) for i in range(6)]
    # What the agents do; the floored ones (floors.rs) wait at the gate.
    ACTS = [("Edit files", False), ("Run tests", False), ("Deploy to production", True),
            ("Merge a pull request", True), ("Secrets", True), ("Payments", True), ("DNS", True)]
    t_pos = [(TX, 270 + i * 90) for i in range(len(ACTS))]
    g_pos = [(GX, y) for _, y in t_pos]
    CAPS = [
        (0.5, 4.5, ["Some things always", "wait for you."]),
        (4.75, 11.0, ["Deploys, merges, secrets,", "payments, DNS: a person answers."]),
        (11.25, 14.35, ["On every agent,", "before it runs, not after."]),
        (14.6, 99, ["Each chat works in", "its own copy of your repo."]),
    ]
    SUB = (15.4, 99, "Nothing touches your checkout until you say so.")
    rng = random.Random(2901)
    lights = []
    holds = []  # (agent, act, t_arrive, t_release, outcome) outcome: pass / decline / clear

    def path_to_gate(a, k):
        return curve(a_pos[a], g_pos[k], bend=(k - a) * 4, n=18)

    def send(a, k, t, outcome="pass", t_release=None):
        floored = ACTS[k][1]
        p = path_to_gate(a, k)
        if not floored:
            lights.append(Dot(p + [t_pos[k]], t, 0.9, r=2.4))
            return
        lights.append(Light(p, t, 0.6, width=1.8, glow_r=8, hold=0.25, head=0.6, land=False))
        holds.append((a, k, t + 0.6, t_release, outcome))
        if outcome == "pass" and t_release is not None:
            lights.append(Light([g_pos[k], t_pos[k]], t_release, 0.45, width=2.4, glow_r=14, hold=0.8))

    # beat 1: edits and tests flow; floored steps stop at the gate
    for j in range(26):
        t = 0.9 + rng.random() * 3.4
        a = rng.randrange(6)
        send(a, rng.randrange(2), t)
    for a, k, t in ((0, 2, 1.4), (2, 4, 2.0), (4, 6, 2.6), (1, 3, 3.1), (5, 5, 3.5)):
        send(a, k, t, "clear", 4.45)
    # beat 2: two asks, answered on the card
    CARD1 = (5.0, "Let Claude Code deploy to production?", 0, 2, 6.7, "Allow")
    CARD2 = (7.9, "Let Codex merge a pull request?", 1, 3, 9.6, "Deny")
    send(0, 2, CARD1[0], "pass", CARD1[4] + 0.15)
    send(1, 3, CARD2[0], "decline", CARD2[4] + 0.15)
    for j in range(18):  # the agents carry on with the rest
        t = 5.0 + rng.random() * 6.0
        send(rng.randrange(6), rng.randrange(2), t)
    for t in (10.0, 10.4, 10.8):  # Codex carries on after the no
        send(1, rng.randrange(2), t)
    # beat 3: every agent, a floored step, held
    for a in range(6):
        send(a, 2 + (a % 5), 11.5 + 0.22 * a, "clear", 14.2)
    for j in range(10):
        send(rng.randrange(6), rng.randrange(2), 11.4 + rng.random() * 2.6)

    # beat 4: each chat in its own copy of the repo
    TRUNK_Y = 560.0
    TX0, TX1 = 760.0, 1760.0
    LANES = [("OpenCode", -300.0, 15.3), ("Claude Code", -150.0, 15.7), ("Codex", 150.0, 16.1)]
    FORK_X = [880.0, 1000.0, 1120.0]

    def lane_pts(j):
        name, dy, _ = LANES[j]
        fx = FORK_X[j]
        return curve((fx, TRUNK_Y), (fx + 150, TRUNK_Y + dy), bend=dy * 0.25, n=16) + [(TX1 - 40, TRUNK_Y + dy)]

    lane_dots = []
    for j in range(3):
        for c in range(5):
            u = 0.42 + 0.12 * c
            lane_dots.append((j, u, LANES[j][2] + 1.0 + 0.55 * c + 0.13 * j))
    for j in range(3):
        pts = lane_pts(j)
        lights.append(Light(pts, LANES[j][2], 1.0, width=2.0, glow_r=6, hold=0.6, head=0.7))
    for j in range(24):
        jj = rng.randrange(3)
        t = 16.4 + rng.random() * 3.6
        L = Light(lane_pts(jj), 0, 1)
        u0 = 0.35 + rng.random() * 0.2
        lights.append(Dot([L.at(u0)[0], L.at(min(1, u0 + 0.35))[0]], t, 0.7, r=2.0))

    def hold_state(h, t):
        a, k, ta, tr, outcome = h
        if t < ta:
            return None
        if tr is None or t < tr:
            return ("held", 1.0)
        if outcome == "pass":
            return None
        k2 = 1 - fade(t, tr, tr + 0.6)
        return ("gone", k2) if k2 > 0 else None

    gate_k = lambda t: fade(t, 0.3, 1.2) * (1 - fade(t, 14.4, 15.1))
    trunk_k = lambda t: fade(t, 14.7, 15.4)

    def hi(d, t):
        k = gate_k(t)
        if k > 0:
            for a in range(6):
                for j in range(len(ACTS)):
                    line(d, path_to_gate(a, j), (255, 255, 255), 24 * k)
            for j in range(len(ACTS)):
                line(d, [g_pos[j], t_pos[j]], (255, 255, 255), (EDGE_A + 10) * k)
            # the gate: open where edits and tests pass, closed at the floors
            line(d, [(GX, 236), (GX, 246)], (255, 255, 255), 60 * k, 2)
            for j, (name, floored) in enumerate(ACTS):
                y = t_pos[j][1]
                if floored:
                    line(d, [(GX, y - 34), (GX, y + 34)], CONTENT, 200 * k, 3)
                    for dy in (-34, 34):
                        line(d, [(GX - 7, y + dy), (GX + 7, y + dy)], CONTENT, 200 * k, 2)
                else:
                    line(d, [(GX, y - 34), (GX, y - 22)], (255, 255, 255), 60 * k, 2)
                    line(d, [(GX, y + 22), (GX, y + 34)], (255, 255, 255), 60 * k, 2)
            for a in range(6):
                disc(d, a_pos[a], 15, 205, 255 * k)
            for j, (name, floored) in enumerate(ACTS):
                disc(d, t_pos[j], 9 if floored else 7, 205 if floored else 120, 255 * k)
        k = trunk_k(t)
        if k > 0:
            line(d, [(TX0, TRUNK_Y), (TX1, TRUNK_Y)], CONTENT, 200 * k, 3)
            for x in (TX0 + 30, TX0 + 330, TX0 + 600, TX1 - 70):
                disc(d, (x, TRUNK_Y), 6, 220, 255 * k)
            for j in range(3):
                kk = fade(t, LANES[j][2] + 0.9, LANES[j][2] + 1.4)
                if kk > 0:
                    line(d, lane_pts(j), (255, 255, 255), 70 * kk, 1.5)
            for j, u, tb in lane_dots:
                kk = out_cubic((t - tb) / 0.35)
                if kk > 0:
                    p = Light(lane_pts(j), 0, 1).at(u)[0]
                    disc(d, p, 5 * kk, 210)

    def lit(ld, t):
        for l in lights:
            l.draw(ld, t)
        for h in holds:
            st = hold_state(h, t)
            if st is None:
                continue
            a, k, ta, tr, outcome = h
            p = g_pos[k]
            x = p[0] - 12
            if st[0] == "held":
                pulse = 0.5 + 0.5 * math.sin((t - ta) * 7)
                glow(ld, (x, p[1]), 7, 200)
                disc(ld, (x, p[1]), 4.5, 245)
                ring(ld, (x, p[1]), 11 + 6 * pulse, LIGHT, 90 + 100 * (1 - pulse), 1.5)
            else:  # declined or cleared: it dims and is gone
                disc(ld, (x - 18 * (1 - st[1]), p[1]), 4.5, 160, 255 * st[1])
        k = trunk_k(t)
        if k > 0:
            for j in range(3):
                kk = fade(t, LANES[j][2] + 0.9, LANES[j][2] + 1.4)
                if kk > 0:
                    end = lane_pts(j)[-1]
                    glow(ld, end, 8, 90 * kk)

    def card(d, t, t0, text, ans, ta):
        a = window(t, t0 + 0.55, ta + 1.5, 0.35, 0.35)
        if a <= 0:
            return
        x0, y0 = LEFT, 300
        w = max(g_msg.getlength(text) + 56, 470)
        d.rounded_rectangle([x0, y0, x0 + w, y0 + 132], radius=12, fill=(18, 18, 18, int(235 * a)),
                            outline=(*SECONDARY, int(150 * a)), width=1)
        d.text((x0 + 28, y0 + 24), text, font=g_msg, fill=(*CONTENT, int(255 * a)))
        chosen = fade(t, ta, ta + 0.2)
        bx = x0 + 28
        for name in ("Allow", "Deny"):
            bw = g_small.getlength(name) + 40
            on = chosen if name == ans else 0.0
            fill = (*mix((30, 30, 30), CONTENT, on), int(255 * a))
            d.rounded_rectangle([bx, y0 + 74, bx + bw, y0 + 112], radius=8, fill=fill,
                                outline=(*SECONDARY, int(160 * a)), width=1)
            d.text((bx + 20, y0 + 82), name, font=g_small, fill=(*mix(CONTENT, (14, 14, 14), on), int(255 * a)))
            bx += bw + 14
        k = fade(t, ta + 0.5, ta + 0.9) * a
        if k > 0:
            note = "It runs." if ans == "Allow" else "Declined. The agent carries on."
            d.text((x0 + 4, y0 + 152), note, font=g_small, fill=(*SECONDARY, int(255 * k)))

    def words(d, t):
        captions(d, t, CAPS)
        k = window(t, SUB[0], SUB[1])
        if k > 0:
            d.text((LEFT, 268), SUB[2], font=g_sub, fill=(*SECONDARY, int(255 * k)))
        k = gate_k(t)
        if k > 0:
            for a in range(6):
                d.text((AX - 26 - g_node.getlength(AGENTS[a]), a_pos[a][1] - 12), AGENTS[a], font=g_node,
                       fill=(*CONTENT, int(225 * k)))
            for j, (name, floored) in enumerate(ACTS):
                d.text((TX + 22, t_pos[j][1] - 12), name, font=g_node,
                       fill=(*(CONTENT if floored else SECONDARY), int(235 * k)))
            d.text((GX - g_small.getlength("waits for you") / 2, 880), "waits for you", font=g_small,
                   fill=(*SECONDARY, int(255 * k * fade(t, 1.5, 2.2))))
        for t0, text, a, kk_, ta, ans in (CARD1, CARD2):
            card(d, t, t0, text, ans, ta)
        k = trunk_k(t)
        if k > 0:
            d.text((TX0, TRUNK_Y + 22), "your checkout", font=g_small, fill=(*CONTENT, int(235 * k)))
            for j in range(3):
                kk = fade(t, LANES[j][2] + 1.0, LANES[j][2] + 1.5)
                if kk > 0:
                    name, dy, _ = LANES[j]
                    y = TRUNK_Y + dy
                    d.text((FORK_X[j] + 170, y - 34 if dy < 0 else y + 12), f"a chat on {name}",
                           font=g_small, fill=(*SECONDARY, int(255 * kk)))

    return SECONDS, glow_bg(CX, CY, 620, 520), hi, lit, words, (CX, CY)


# =============================================================================
# everywhere: one conversation, every device
# =============================================================================
def seg_everywhere():
    SECONDS = 17.5
    # devices: name, center, (w, h) of the screen, platforms
    DEV = {
        "term": dict(name="Terminal", c=(850.0, 640.0), wh=(300, 200), plat="Mac · Windows · Linux"),
        "phone": dict(name="Phone", c=(1240.0, 380.0), wh=(176, 300), plat="iPhone · Android"),
        "web": dict(name="Web", c=(1630.0, 640.0), wh=(300, 200), plat="openagents.com"),
    }
    CAPS = [
        (0.5, 4.6, ["Start in the terminal."]),
        (4.85, 8.85, ["Keep going on your phone."]),
        (9.1, 12.6, ["Review on the web."]),
        (12.85, 99, ["One conversation,", "on every device."]),
    ]
    # The conversation (the episode 289 ask), each message written on one
    # device and then shown on all three.
    MSGS = [  # (t, from device, mine?, text)
        (1.2, "term", True, "Fix the flaky parser test"),
        (2.6, "term", False, "Found it: a race in setup."),
        (5.4, "phone", True, "Run it a hundred times"),
        (6.8, "phone", False, "All green. Ready to commit."),
        (9.6, "web", True, "Commit and open a PR"),
        (11.0, "web", False, "Committed. PR is open."),
    ]
    ACTIVE = [(0.0, 4.75, "term"), (4.75, 9.0, "phone"), (9.0, 12.75, "web")]
    lights = []

    def scr_box(k):
        (x, y), (w, h) = DEV[k]["c"], DEV[k]["wh"]
        return (x - w / 2, y - h / 2, x + w / 2, y + h / 2)

    def edge(k, toward):
        """Where the line from device k's center toward `toward` leaves its frame."""
        x0, y0, x1, y1 = scr_box(k)
        cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
        dx, dy = toward[0] - cx, toward[1] - cy
        sx = (x1 - x0) / 2 + 22
        sy = (y1 - y0) / 2 + 22
        f = min(sx / abs(dx) if dx else 1e9, sy / abs(dy) if dy else 1e9)
        return (cx + dx * f, cy + dy * f)

    def link(a, b, bend=-40):
        ca, cb = DEV[a]["c"], DEV[b]["c"]
        return curve(edge(a, cb), edge(b, ca), bend=bend, n=24)

    for t0, src, mine, text in MSGS:
        for k in DEV:
            if k != src:
                lights.append(Light(link(src, k), t0 + 0.15, 0.6, width=2.4, glow_r=10, hold=0.5))
    # the closing loop: one thread through all three
    LOOP = link("term", "phone") + link("phone", "web") + link("web", "term", -40)

    def active(t):
        for a, b, k in ACTIVE:
            if a <= t < b:
                return k, fade(t, a, a + 0.4) * (1 - fade(t, b - 0.3, b))
        return None, 0.0

    def device(d, k, alpha, lit_k):
        x0, y0, x1, y1 = scr_box(k)
        col = mix((150, 150, 150), CONTENT, lit_k)
        if k == "phone":
            rrect(d, (x0 - 8, y0 - 14, x1 + 8, y1 + 14), 20, outline=(*col, 220), width=2, k=alpha)
            rrect(d, ((x0 + x1) / 2 - 16, y0 - 6, (x0 + x1) / 2 + 16, y0 - 2), 2, fill=(*col, 160), k=alpha)
        elif k == "term":
            rrect(d, (x0 - 10, y0 - 10, x1 + 10, y1 + 10), 10, outline=(*col, 220), width=2, k=alpha)
            d.polygon([((x0 - 40) * S, (y1 + 14) * S), ((x1 + 40) * S, (y1 + 14) * S),
                       ((x1 + 56) * S, (y1 + 30) * S), ((x0 - 56) * S, (y1 + 30) * S)],
                      outline=(*col, int(220 * alpha)), width=2 * S)
        else:
            rrect(d, (x0 - 10, y0 - 30, x1 + 10, y1 + 10), 10, outline=(*col, 220), width=2, k=alpha)
            line(d, [(x0 - 10, y0 - 6), (x1 + 10, y0 - 6)], col, 140 * alpha, 1.5)
            for j in range(3):
                disc(d, (x0 + 6 + 14 * j, y0 - 18), 4, col[0], 200 * alpha)
            rrect(d, (x0 + 60, y0 - 25, x1 - 20, y0 - 12), 6, outline=(*col, 120), width=1, k=alpha)

    def bubbles(d, k, t, alpha):
        """The conversation inside device k: newest at the bottom."""
        x0, y0, x1, y1 = scr_box(k)
        shown = []
        for t0, src, mine, text in MSGS:
            ta = t0 if src == k else t0 + 0.75
            if t >= ta - 0.05:
                shown.append((ta, mine, text))
        pad = 12
        bh = 26 if k != "phone" else 24
        yb = y1 - pad
        for ta, mine, text in reversed(shown):
            kk = out_cubic((t - ta) / 0.35)
            f = g_bubble
            maxw = (x1 - x0) - 2 * pad - 20
            words_ = text
            while f.getlength(words_) > maxw - 16 and len(words_) > 4:
                words_ = words_[:-2]
            if words_ != text:
                words_ = words_.rstrip() + "…"
            bw = f.getlength(words_) + 20
            top = yb - bh
            if top < y0 + pad:
                break
            if mine:
                bx0, bx1 = x1 - pad - bw, x1 - pad
                fill = (60, 62, 68, int(255 * alpha * kk))
            else:
                bx0, bx1 = x0 + pad, x0 + pad + bw
                fill = (28, 28, 30, int(255 * alpha * kk))
            dy = 8 * (1 - kk)
            d.rounded_rectangle([bx0, top + dy, bx1, yb + dy], radius=9, fill=fill)
            d.text((bx0 + 10, top + 6 + dy), words_, font=f, fill=(*CONTENT, int(240 * alpha * kk)))
            yb = top - 7

    def hi(d, t):
        appear = fade(t, 0.0, 1.0)
        act, ak = active(t)
        for k in DEV:
            lk = ak if k == act else 0.0
            if t > 12.9:
                lk = fade(t, 13.0, 14.0)
            device(d, k, appear, lk)

    def lit(ld, t):
        for l in lights:
            l.draw(ld, t)
        act, ak = active(t)
        if act:
            x0, y0, x1, y1 = scr_box(act)
            for e in (24, 12):
                ld.rounded_rectangle([(x0 - e) * S, (y0 - e) * S, (x1 + e) * S, (y1 + e) * S],
                                     radius=(14 + e) * S, outline=(*LIGHT, int((40 if e == 24 else 90) * ak)),
                                     width=(6 if e == 24 else 2) * S)
        sweep = out_cubic((t - 13.0) / 1.8)
        if sweep > 0:
            L = Light(LOOP, 0, 1)
            head, seg, kk = L.at(sweep)
            trail = LOOP[:seg + 1] + [head]
            line(ld, trail, LIGHT, 60, 9)
            line(ld, trail, LIGHT, 230, 2.4)
            if sweep < 1:
                glow(ld, head, 6, 255)
            else:  # then a slow pulse rides the loop
                for j in range(3):
                    u = ((t - 14.8) * 0.22 + j / 3) % 1.0
                    p = L.at(u)[0]
                    glow(ld, p, 5, 200)
                    disc(ld, p, 3, 250)

    def words(d, t):
        captions(d, t, CAPS)
        appear = fade(t, 0.3, 1.1)
        act, ak = active(t)
        for k, dv in DEV.items():
            x0, y0, x1, y1 = scr_box(k)
            bubbles(d, k, t, appear)
            yb = y1 + (46 if k == "term" else 30)
            col = mix(SECONDARY, CONTENT, ak if k == act else (fade(t, 13.0, 14.0) if t > 12.9 else 0))
            d.text(((x0 + x1) / 2 - g_node.getlength(dv["name"]) / 2, yb), dv["name"], font=g_node,
                   fill=(*col, int(255 * appear)))
            kp = fade(t, 13.6 + 0.4 * list(DEV).index(k), 14.2 + 0.4 * list(DEV).index(k))
            if kp > 0:
                d.text(((x0 + x1) / 2 - g_small.getlength(dv["plat"]) / 2, yb + 30), dv["plat"],
                       font=g_small, fill=(*SECONDARY, int(255 * kp)))

    return SECONDS, glow_bg(CX, CY + 40, 640, 480), hi, lit, words, (CX, CY)


# =============================================================================
# delegation: one agent hands a piece to another
# =============================================================================
def seg_delegation():
    SECONDS = 18.5
    P = (930.0, 560.0)     # the parent, Claude Code
    C1 = (1400.0, 400.0)   # Codex
    C2 = (1700.0, 690.0)   # OpenCode
    RP, RC = 70.0, 52.0
    CAPS = [
        (0.5, 4.6, ["One agent can hand", "a piece to another."]),
        (4.85, 8.95, ["Each one works in", "its own copy of the repo."]),
        (9.2, 12.75, ["A short report comes back."]),
        (13.0, 99, ["The parent's prompt cache", "stays warm."]),
    ]
    REPORT = ["outcome", "summary", "changed files", "checks", "worktree and branch"]
    lights = []
    lights.append(Light(curve(P, C1, -40), 1.5, 0.7, width=3.0, glow_r=RC + 4, hold=0.6))
    lights.append(Light(curve(C1, C2, -40), 5.4, 0.7, width=3.0, glow_r=RC + 4, hold=0.6))
    # reports back: short and small
    lights.append(Light(curve(C2, C1, -40), 9.6, 0.7, width=1.8, glow_r=RC + 4, hold=0.6, head=0.7))
    lights.append(Light(curve(C1, P, -40), 10.6, 0.8, width=1.8, glow_r=RP + 4, hold=0.8, head=0.7))
    # each child's worktree: a lane under it with work landing on it
    WT = {1: (C1, 2.3), 2: (C2, 6.2)}
    rng = random.Random(2902)
    commits = {1: [], 2: []}
    for c, (p, tb) in WT.items():
        for j in range(6):
            t = tb + 0.7 + 0.45 * j + 0.2 * rng.random()
            x = p[0] - 80 + 32 * j
            lights.append(Dot([p, (x, p[1] + RC + 70)], t, 0.55, r=2.2))
            commits[c].append((x, t + 0.55))
    # the parent keeps working on its own chat
    for j in range(16):
        t = 1.0 + rng.random() * 16.5
        a = rng.random() * 2 * math.pi
        lights.append(Dot([polar(RP + 8, a, P), polar(RP + 70, a, P)], t, 0.6, r=1.8))

    def node(d, p, r, k):
        disc(d, p, r * k, 232)

    def lane(d, p, k, tb, t, c):
        y = p[1] + RC + 70
        x0, x1 = p[0] - 110, p[0] + 110
        e = lerp((x0, y), (x1, y), out_cubic((t - tb) / 0.6))
        line(d, [p, (p[0], y)], (255, 255, 255), EDGE_A + 20, 1)
        line(d, [(x0, y), e], CONTENT, 160 * k, 2)
        for x, tl in commits[c]:
            kk = out_cubic((t - tl) / 0.3)
            if kk > 0:
                disc(d, (x, y), 5 * kk, 215)

    def hi(d, t):
        appear = fade(t, 0.0, 1.0)
        node(d, P, RP, appear)
        k1 = out_cubic((t - 1.9) / 0.5)
        if k1 > 0:
            line(d, curve(P, C1, -40), (255, 255, 255), EDGE_A + 15)
            node(d, C1, RC, k1)
            lane(d, C1, k1, 2.3, t, 1)
        k2 = out_cubic((t - 5.8) / 0.5)
        if k2 > 0:
            line(d, curve(C1, C2, -40), (255, 255, 255), EDGE_A + 15)
            node(d, C2, RC, k2)
            lane(d, C2, k2, 6.2, t, 2)

    def lit(ld, t):
        appear = fade(t, 0.0, 1.0)
        warm = fade(t, 13.0, 14.0)
        glow(ld, P, RP * 1.05, (110 + 60 * warm) * appear)
        # the cache: a steady ring round the parent, all the way through
        ring(ld, P, RP + 16, LIGHT, (70 + 120 * warm) * appear, 2 + 1.5 * warm)
        for c, p, tb in ((1, C1, 1.9), (2, C2, 5.8)):
            k = out_cubic((t - tb) / 0.5)
            if k > 0:
                glow(ld, p, RC * 1.05 * k, 90)
        for l in lights:
            l.draw(ld, t)

    def words(d, t):
        captions(d, t, CAPS)
        a = fade(t, 0.5, 1.2)
        centered(d, P, "Claude Code", g_core, (14, 14, 14, int(255 * a)))
        for p, tb, nm in ((C1, 2.0, "Codex"), (C2, 5.9, "OpenCode")):
            k = fade(t, tb, tb + 0.5)
            if k > 0:
                centered(d, p, nm, g_node, (14, 14, 14, int(255 * k)))
        for p, tb in ((C1, 2.6), (C2, 6.5)):
            k = fade(t, tb, tb + 0.5)
            if k > 0:
                txt = "its own worktree"
                d.text((p[0] - g_small.getlength(txt) / 2, p[1] + RC + 84), txt, font=g_small,
                       fill=(*SECONDARY, int(255 * k)))
        k = fade(t, 13.4, 14.0)
        if k > 0:
            d.text((P[0] - g_small.getlength("prompt cache") / 2, P[1] - RP - 66), "prompt cache", font=g_small,
                   fill=(*SECONDARY, int(255 * k)))
        # the report, field by field
        ka = window(t, 9.6, 12.75)
        if ka > 0:
            x0, y0 = LEFT, 280
            d.rounded_rectangle([x0, y0, x0 + 330, y0 + 70 + 34 * len(REPORT)], radius=12,
                                fill=(18, 18, 18, int(235 * ka)), outline=(*SECONDARY, int(150 * ka)), width=1)
            d.text((x0 + 24, y0 + 20), "Report", font=g_msg, fill=(*CONTENT, int(255 * ka)))
            for j, fld in enumerate(REPORT):
                kk = fade(t, 10.0 + 0.18 * j, 10.3 + 0.18 * j) * ka
                d.ellipse([x0 + 26, y0 + 72 + 34 * j, x0 + 32, y0 + 78 + 34 * j], fill=(*LIGHT, int(255 * kk)))
                d.text((x0 + 44, y0 + 64 + 34 * j), fld, font=g_small, fill=(*SECONDARY, int(255 * kk)))
            kk = fade(t, 11.2, 11.6) * ka
            d.text((x0 + 4, y0 + 90 + 34 * len(REPORT)), "The full transcript stays on disk.", font=g_tiny,
                   fill=(*TERTIARY, int(255 * kk)))
        # the measured cache read (#11511)
        km = fade(t, 13.4, 13.9)
        if km > 0:
            y = 330
            for x, label, v in ((LEFT, "with a delegation", 99.7), (LEFT + 290, "without", 99.8)):
                d.text((x, y), label, font=g_small, fill=(*SECONDARY, int(255 * km)))
                d.text((x, y + 28), f"{v:.1f}%", font=m_big, fill=(*CONTENT, int(255 * km)))
            k2 = fade(t, 14.8, 15.3)
            d.text((LEFT, y + 112), "of the prompt read from the cache, last turn", font=g_small,
                   fill=(*SECONDARY, int(255 * k2)))
            k3 = fade(t, 15.4, 15.9)
            d.text((LEFT, y + 150), "Same 3-turn Claude Code chat, from its usage records.", font=g_tiny,
                   fill=(*TERTIARY, int(255 * k3)))

    return SECONDS, glow_bg(1280, 500, 620, 480), hi, lit, words, (1280, 500)


# =============================================================================
# metaharness: a harness wraps a model in tools; Coder sits in front of them
# =============================================================================
# It opens on the frame system1-routing.py's beat 1 ends on (the big model in a
# thin ring of its tools, the counters at $4.00 and 284 s), so a short
# crossfade from that shot reads as one camera pulling back.
def routing_beat1_words(d, t, k):
    """The counters system1-routing.py still shows at 8.5 s (its caption and
    messages have faded by then), faded by k."""
    if k <= 0:
        return
    for x, label, value in ((LEFT, "cost per fix", "$4.00"), (LEFT + 250, "time per fix", "284 s")):
        d.text((x, 690), label, font=g_small, fill=(*SECONDARY, int(255 * k)))
        d.text((x, 718), value, font=m_big, fill=(*CONTENT, int(255 * k)))


def seg_metaharness():
    SECONDS = 21.5
    TOOLS = ["Bash", "Read", "Edit", "Write", "Grep", "Web"]
    t_ang = [-math.pi / 2 + math.pi / 6 + i * 2 * math.pi / 6 for i in range(6)]
    NAMES = ["Claude Code", "Codex", "OpenCode"]
    C0, R0, RT0 = (1238.0, 540.0), 78.0, 154.0       # the routing shot's close-up
    HOME = [(1010.0, 560.0), (1340.0, 560.0), (1670.0, 560.0)]
    SC = 0.42                                         # each harness after the pull-back
    PULL = (0.5, 2.9)
    BORN = [None, 1.9, 2.3]
    CODER, R_C, R_S1 = (680.0, 600.0), 44.0, 98.0
    HOOKS = ["route", "brief", "squeeze", "gate", "watch", "verify"]
    h_ang = [-math.pi / 2 + i * 2 * math.pi / 6 for i in range(6)]
    T_CODER = 11.0
    SMALL = [((550.0, 850.0), "small model"), ((810.0, 850.0), "Microcoder")]
    CAPS = [
        (1.0, 10.7, ["A harness wraps a model in its tools.", "Every step goes through the big model."]),
        (11.0, 99, ["Coder is a metaharness: it sits in front", "and decides what each step needs."]),
    ]
    STEPS = [("Read the failing test", 0), ("Which file next?", 1), ("Is this command safe?", 0),
             ("Fix the parser bug", None), ("Rename the helper", 1), ("Refactor the module", None)]
    T_STEP0, T_STEP = 12.9, 1.15
    LIST_Y0, LIST_DY = 352, 52

    def place(i, t):
        """A harness's center and scale at time t."""
        if i == 0:
            k = ease((t - PULL[0]) / (PULL[1] - PULL[0]))
            return lerp(C0, HOME[0], k), 1 + (SC - 1) * k
        k = out_cubic((t - BORN[i]) / 0.9)
        return lerp((HOME[i][0] + 260, HOME[i][1]), HOME[i], k), SC * (0.6 + 0.4 * k)

    def alpha(i, t):
        return 1.0 if i == 0 else fade(t, BORN[i], BORN[i] + 0.6)

    rng = random.Random(2903)
    chatter = []  # (t, harness, tool): a step out to a tool and back into the model
    for j in range(300):
        t = rng.random() * SECONDS
        h = rng.randrange(3)
        if h and t < BORN[h] + 0.6:
            continue
        if t > T_CODER + 1.5 and rng.random() < 0.7:  # Coder keeps small steps away from them
            continue
        chatter.append((t, h, rng.randrange(6)))
    feed = []  # every step straight into a model, before Coder
    for j in range(20):
        t = 1.4 + j * 0.47
        feed.append((t, j % 3, rng.uniform(300, 760)))
    ticks = []  # after Coder: steady small work, now and then a hard part to a harness
    for j in range(48):
        t = T_CODER + 1.6 + rng.random() * (SECONDS - T_CODER - 1.6)
        ticks.append((t, rng.random() < 0.18, rng.randrange(6), rng.randrange(3), rng.randrange(2)))

    def tool_pos(i, k, t):
        c, s = place(i, t)
        return polar(RT0 * s, t_ang[k], c)

    def core(i, t):
        return place(i, t)

    def hi(d, t):
        for i in range(3):
            a = alpha(i, t)
            if a <= 0:
                continue
            c, s = place(i, t)
            ring(d, c, RT0 * s, (255, 255, 255), (EDGE_A + 30) * a, 1.0)
            for k in range(6):
                p = polar(RT0 * s, t_ang[k], c)
                line(d, [polar(R0 * s, t_ang[k], c), p], (255, 255, 255), (EDGE_A + 10) * a)
                disc(d, p, 7.5 * max(s, 0.55), 215, 255 * a)
            disc(d, c, R0 * s, 236, 255 * a)
        ka = fade(t, T_CODER, T_CODER + 0.8)
        if ka > 0:
            for i in range(3):
                c, s = place(i, t)
                k = out_cubic((t - T_CODER - 0.4 - 0.15 * i) / 0.6)
                if k > 0:
                    line(d, [CODER, lerp(CODER, c, k)], (255, 255, 255), EDGE_A * ka)
            for (p, _) in SMALL:
                line(d, [CODER, p], (255, 255, 255), EDGE_A * ka)
                disc(d, p, 12 * ka, 190)
            disc(d, CODER, R_C * ka, 236)
            sweep = out_cubic((t - T_CODER - 0.2) / 1.2)
            if sweep > 0:
                box = [(CODER[0] - R_S1) * S, (CODER[1] - R_S1) * S, (CODER[0] + R_S1) * S, (CODER[1] + R_S1) * S]
                d.arc(box, -90, -90 + 360 * sweep, fill=(255, 255, 255, 120), width=2 * S)
            for j, a_ in enumerate(h_ang):
                kk = fade(t, T_CODER + 0.4 + 0.12 * j, T_CODER + 0.8 + 0.12 * j)
                if kk > 0:
                    disc(d, polar(R_S1, a_, CODER), 7 * kk, 230)

    lights = []
    for t0, h, y in feed:
        lights.append(("feed", t0, h, y))

    def lit(ld, t):
        for i in range(3):
            a = alpha(i, t)
            if a > 0:
                c, s = place(i, t)
                glow(ld, c, R0 * s * 1.05, 110 * a)
        for t0, h, k in chatter:
            if not (t0 <= t <= t0 + 1.2):
                continue
            c, s = place(h, t)
            p = polar(RT0 * s, t_ang[k], c)
            Light([polar(R0 * s, t_ang[k], c), p], t0, 0.22, width=1.4, glow_r=6, hold=0.2, head=0.5,
                  land=False).draw(ld, t)
            Light([p, polar(R0 * s, t_ang[k], c)], t0 + 0.26, 0.22, width=1.4, glow_r=R0 * s, hold=0.2,
                  head=0.5, land=False).draw(ld, t)
        for t0, h, y in feed:
            if t0 <= t <= t0 + 1.4:
                c, s = place(h, t)
                Light([(-40, y), c], t0, 0.75, width=2.2, glow_r=R0 * s + 4, hold=0.5, head=0.8).draw(ld, t)
        if t >= T_CODER:
            glow(ld, CODER, R_C * 1.05, 120 * fade(t, T_CODER, T_CODER + 0.8))
        for j, (text, small) in enumerate(STEPS):
            t0 = T_STEP0 + j * T_STEP
            a = (LEFT + g_msg.getlength(text) + 22, LIST_Y0 + j * LIST_DY + 14)
            entry = math.atan2(a[1] - CODER[1], a[0] - CODER[0])
            Light([a, polar(R_S1, entry, CODER)], t0 + 0.1, 0.45, width=2.6, glow_r=12, hold=0.4).draw(ld, t)
            if small is not None:
                p = SMALL[small][0]
                Light([polar(R_S1, entry, CODER), p], t0 + 0.6, 0.35, width=2.2, glow_r=18, hold=0.5,
                      head=0.8).draw(ld, t)
            else:
                h = 0 if j == 3 else 1
                c, s = place(h, t)
                Light([polar(R_S1, entry, CODER), c], t0 + 0.6, 0.5, width=2.6, glow_r=R0 * s + 5, hold=0.6).draw(ld, t)
        for t0, hard, hook, h, sm in ticks:
            if not (t0 <= t <= t0 + 1.0):
                continue
            src = polar(R_S1, h_ang[hook], CODER)
            if hard:
                Dot([src, place(h, t)[0]], t0, 0.7, r=2.6).draw(ld, t)
            else:
                Dot([src, SMALL[sm][0]], t0, 0.5, r=2.2).draw(ld, t)

    def words(d, t):
        routing_beat1_words(d, t, 1 - fade(t, 0.15, 0.75))
        captions(d, t, CAPS)
        for i in range(3):
            a = alpha(i, t)
            c, s = place(i, t)
            k_in = (1 - fade(t, PULL[0], PULL[0] + 0.6)) if i == 0 else 0
            if k_in > 0:
                centered(d, (c[0], c[1] - 10), "System 2", g_core, (14, 14, 14, int(255 * k_in)))
                centered(d, (c[0], c[1] + 16), "the big model", g_tiny, (40, 40, 40, int(255 * k_in)))
                for k in range(6):
                    label_side(d, polar(RT0 * s, t_ang[k], c), 8, t_ang[k], TOOLS[k], g_small,
                               (*SECONDARY, int(255 * k_in)))
            kl = a * fade(t, (BORN[i] or PULL[1] - 0.8) + 0.3, (BORN[i] or PULL[1] - 0.8) + 0.9)
            if kl > 0:
                y = c[1] + RT0 * s + 26
                centered(d, (c[0], y), NAMES[i], g_node, (*CONTENT, int(255 * kl)))
                centered(d, (c[0], y + 26), "a model in its tools", g_tiny, (*SECONDARY, int(255 * kl)))
        ka = fade(t, T_CODER + 0.3, T_CODER + 0.9)
        if ka > 0:
            centered(d, CODER, "Coder", g_core, (14, 14, 14, int(255 * ka)))
            centered(d, (CODER[0], CODER[1] - R_S1 - 66), "System 1", g_node, (*CONTENT, int(255 * ka)))
            centered(d, (CODER[0], CODER[1] - R_S1 - 44), "small, fast models", g_tiny, (*SECONDARY, int(255 * ka)))
            for j, a_ in enumerate(h_ang):
                kk = fade(t, T_CODER + 0.5 + 0.12 * j, T_CODER + 0.9 + 0.12 * j)
                if kk > 0:
                    label_side(d, polar(R_S1, a_, CODER), 7, a_, HOOKS[j], g_tiny, (*SECONDARY, int(255 * kk)))
            for p, name in SMALL:
                centered(d, (p[0], p[1] + 30), name, g_small, (*SECONDARY, int(255 * ka)))
        for j, (text, small) in enumerate(STEPS):
            t0 = T_STEP0 + j * T_STEP
            a = fade(t, t0 - 0.2, t0 + 0.2)
            if a <= 0:
                continue
            y = LIST_Y0 + j * LIST_DY + 8 * (1 - fade(t, t0 - 0.2, t0 + 0.25))
            done = fade(t, t0 + 1.0, t0 + 2.0)
            d.text((LEFT, y), text, font=g_msg, fill=(*mix(CONTENT, SECONDARY, done), int(255 * a)))
            ax, ay = LEFT + g_msg.getlength(text) + 22, LIST_Y0 + j * LIST_DY + 14
            d.ellipse([ax - 3.2, ay - 3.2, ax + 3.2, ay + 3.2], fill=(*LIGHT, int(255 * a)))
            if done > 0.5:
                tag = "small model" if small == 0 else "Microcoder" if small == 1 else NAMES[0 if j == 3 else 1]
                d.text((ax + 14, ay - 9), tag, font=g_tiny, fill=(*TERTIARY, int(255 * a * (done - 0.5) * 2)))

    return SECONDS, glow_bg(1240, 560, 760, 520), hi, lit, words, (1100, 560)


# =============================================================================
# savings: the measured numbers, side by side
# =============================================================================
# Opens on the counters system1-routing.py's beat 2 ends on ($1.84 and 121 s,
# the struck-out $4.00 and 284 s, "Same model. 21 real issues, 3 runs each."),
# then lays the two runs side by side. docs/inference/briefed-agent-ab.md
# (#11211): per accepted fix $4.00 (bare Claude Code) vs $1.84 (briefed),
# median 284 s vs 121 s, 19 of 63 accepted each way.
def seg_savings():
    SECONDS = 20.5
    m_mid = font("BerkeleyMono-Regular.ttf", 30)
    m_old = font("BerkeleyMono-Regular.ttf", 24)
    CAPS = [
        (0.9, 11.3, ["We measured it: the same big model,", "21 real issues, three runs each."]),
        (11.6, 99, ["Half the cost, more than twice as fast,", "and nothing worse."]),
    ]
    X0, BAR = 520.0, 960.0
    # (label, on its own, with System 1, their labels, full scale, when each bar grows)
    ROWS = [("cost per fix", 4.00, 1.84, "$4.00", "$1.84", 4.00, (0.6, 5.2)),
            ("time per fix", 284, 121, "284 s", "121 s", 284, (1.6, 6.2)),
            ("fixes accepted", 19, 19, "19 of 63", "19 of 63", 63, (8.6, 9.0))]
    TAGS = 10.6
    Y0, DY = 360, 150

    def bar_w(v, full, t, t0):
        return BAR * v / full * out_cubic((t - t0) / 1.1)

    rng = random.Random(2904)
    pulses = [(0.6 + rng.random() * (SECONDS - 0.6), rng.randrange(3), rng.randrange(2)) for _ in range(110)]

    def hi(d, t):
        for r, (_, a_, b_, _, _, full, ts) in enumerate(ROWS):
            y = Y0 + r * DY
            k = fade(t, ts[0] - 0.4, ts[0])
            if k <= 0:
                continue
            for j, (v, g) in enumerate(((a_, 110), (b_, 225))):
                w = bar_w(v, full, t, ts[j])
                yy = y + 34 + j * 40
                rrect(d, (X0, yy, X0 + BAR, yy + 22), 5, outline=(255, 255, 255, 26), k=k)
                if w > 1:
                    rrect(d, (X0, yy, X0 + w, yy + 22), 5, fill=(g, g, g, 255), k=k)

    def lit(ld, t):
        for t0, r, j in pulses:
            name, a_, b_, _, _, full, ts = ROWS[r]
            if t0 < ts[j] + 1.2 or not (t0 <= t <= t0 + 1.6):
                continue
            v = (a_, b_)[j]
            y = Y0 + r * DY + 34 + j * 40 + 11
            Dot([(X0 + 4, y), (X0 + BAR * v / full - 4, y)], t0, 1.4 * v / full + 0.3, r=2.6).draw(ld, t)
        for r, (_, a_, b_, _, _, full, ts) in enumerate(ROWS):
            for j, v in enumerate((a_, b_)):
                k = clamp((t - ts[j]) / 1.1)
                if 0 < k < 1:
                    x = X0 + BAR * v / full * out_cubic(k)
                    glow(ld, (x, Y0 + r * DY + 34 + j * 40 + 11), 9, 160)

    def words(d, t):
        # beat 2's counters, as the routing shot leaves them
        k0 = 1 - fade(t, 0.2, 0.8)
        if k0 > 0:
            for j, ln in enumerate(["OpenAgents puts System 1 in front: small, fast models.",
                                    "Only the hard parts reach the big model."]):
                d.text((LEFT, 150 + j * 46), ln, font=g_cap, fill=(*CONTENT, int(255 * k0)))
            for x, label, value, old, word in ((LEFT, "cost per fix", "$1.84", "$4.00", "cheaper"),
                                               (LEFT + 250, "time per fix", "121 s", "284 s", "faster")):
                d.text((x, 690), label, font=g_small, fill=(*SECONDARY, int(255 * k0)))
                d.text((x, 718), value, font=m_big, fill=(*CONTENT, int(255 * k0)))
                d.text((x, 788), old, font=m_old, fill=(*TERTIARY, int(255 * k0)))
                ow = m_old.getlength(old)
                d.line([(x - 2, 803), (x + ow + 2, 803)], fill=(*TERTIARY, int(255 * k0)), width=2)
                nw = m_mid.getlength("2×")
                d.text((x + 100, 786), "2×", font=m_mid, fill=(*CONTENT, int(255 * k0)))
                d.text((x + 100 + nw + 9, 792), word, font=g_core, fill=(*CONTENT, int(255 * k0)))
        captions(d, t, CAPS)
        for j, (name, g, tl) in enumerate((("Claude Code on its own", 110, 0.4), ("With System 1 in front", 225, 5.0))):
            kl = fade(t, tl, tl + 0.6)
            if kl > 0:
                x = X0 + j * 360
                d.rounded_rectangle([x, 292, x + 26, 306], radius=3, fill=(g, g, g, int(255 * kl)))
                d.text((x + 38, 288), name, font=g_small, fill=(*SECONDARY, int(255 * kl)))
        for r, (name, a_, b_, la, lb, full, ts) in enumerate(ROWS):
            y = Y0 + r * DY
            k = fade(t, ts[0] - 0.4, ts[0] + 0.2)
            if k <= 0:
                continue
            d.text((LEFT, y + 52), name, font=g_msg, fill=(*CONTENT, int(255 * k)))
            for j, (v, lab) in enumerate(((a_, la), (b_, lb))):
                kk = fade(t, ts[j] + 0.8, ts[j] + 1.2)
                if kk > 0:
                    x = X0 + BAR * v / full + 16
                    d.text((x, y + 30 + j * 40), lab, font=m_old,
                           fill=(*(CONTENT if j else SECONDARY), int(255 * kk)))
        for r, tag in ((0, "half the cost"), (1, "2.3× faster"), (2, "the same")):
            k = fade(t, TAGS + 0.35 * r, TAGS + 0.35 * r + 0.6)
            if k > 0:
                d.text((X0 + BAR + 150, Y0 + r * DY + 70), tag, font=g_core, fill=(*CONTENT, int(255 * k)))
        # the small print stays from the routing shot to the end
        d.text((LEFT, 850), "Same model. 21 real issues, 3 runs each.", font=g_small, fill=(*SECONDARY, 255))

    return SECONDS, glow_bg(1100, 540, 900, 520), hi, lit, words, (1000, 560)


# =============================================================================
# devday: they promised revenue sharing; we pay the people
# =============================================================================
# The series began on 2023-11-07, the day after OpenAI's DevDay, where revenue
# sharing for builders was announced (docs/transcripts/230.md, 288.md). A
# timeline: the promise, the first episode, three years of nothing shared,
# then gold running out to plugin authors.
def seg_devday():
    SECONDS = 10.5
    CAPS = [
        (0.5, 5.3, ["They promised developers", "revenue sharing."]),
        (5.6, 99, ["We're paying the people."]),
    ]
    Y, XA, XB = 560.0, 360.0, 1560.0
    MARKS = [(XA, "Nov 6, 2023", "OpenAI DevDay: revenue sharing promised", 0.8),
             (XA + 120, "Nov 7, 2023", "Episode 1", 1.6),
             (XB, "Oct 2026", "Shared with developers: zero", 3.6)]
    AUTHORS = [(1300.0 + 170 * math.cos(a), 330.0 + 120 * math.sin(a)) for a in
               [-2.9, -2.3, -1.7, -1.1, -0.5, 0.1]]
    rng = random.Random(2905)
    gold = [(5.8 + rng.random() * (SECONDS - 5.8), rng.randrange(len(AUTHORS))) for _ in range(46)]

    def hi(d, t):
        k = out_cubic((t - 0.5) / 3.2)
        if k > 0:
            line(d, [(XA, Y), (XA + (XB - XA) * k, Y)], (255, 255, 255), 90, 1.5)
        for x, _, _, tm in MARKS:
            kk = fade(t, tm, tm + 0.4)
            if kk > 0:
                disc(d, (x, Y), 8 * kk, 225)
        ka = fade(t, 5.6, 6.3)
        if ka > 0:
            hub = (1300.0, 330.0)
            line(d, [(XB, Y), hub], (255, 255, 255), EDGE_A * 2 * ka)
            disc(d, hub, 26 * ka, 236)
            for p in AUTHORS:
                line(d, [hub, p], (255, 255, 255), EDGE_A * ka)
                disc(d, p, 9 * ka, 205)

    def lit(ld, t):
        k = out_cubic((t - 0.5) / 3.2)
        if 0 < k < 1:
            glow(ld, (XA + (XB - XA) * k, Y), 10, 170)
        for x, _, _, tm in MARKS:
            kk = 1 - fade(t, tm, tm + 1.0)
            if 0 < kk < 1:
                ring(ld, (x, Y), 8 + 18 * (1 - kk), LIGHT, 200 * kk, 1.5)
        if t > 5.6:
            glow(ld, (1300.0, 330.0), 28, 110 * fade(t, 5.6, 6.3))
        for t0, a in gold:
            if t0 <= t <= t0 + 0.9:
                Dot([(1300.0, 330.0), AUTHORS[a]], t0, 0.8, GOLD, r=3.4).draw(ld, t)

    def words(d, t):
        captions(d, t, CAPS)
        for j, (x, date, what, tm) in enumerate(MARKS):
            kk = fade(t, tm + 0.1, tm + 0.6)
            if kk <= 0:
                continue
            dy = 34 if j != 1 else -74
            d.text((x - 8, Y + dy), date, font=g_small, fill=(*SECONDARY, int(255 * kk)))
            d.text((x - 8, Y + dy + 24), what, font=g_msg if j != 1 else g_small,
                   fill=(*CONTENT, int(255 * kk)))
        ka = fade(t, 5.9, 6.5)
        if ka > 0:
            centered(d, (1300.0, 395.0), "plugin authors, paid in Bitcoin", g_msg, (*SECONDARY, int(255 * ka)))

    return SECONDS, glow_bg(1000, 540, 900, 520), hi, lit, words, (1000, 540)


SEGMENTS = {"harnesses": seg_harnesses, "safety": seg_safety, "everywhere": seg_everywhere,
            "delegation": seg_delegation, "metaharness": seg_metaharness, "savings": seg_savings,
            "devday": seg_devday}
SECONDS, BG, HI, LIT, WORDS, FOCUS = SEGMENTS[args.segment]()


def render(i):
    return compose(i, i / FPS, BG, HI, LIT, WORDS, SECONDS, FOCUS)


if args.only:
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    for s in args.only.split(","):
        render(int(float(s) * FPS)).save(out / f"{args.segment}-t{float(s):05.2f}.png")
    raise SystemExit
frames = int(FPS * SECONDS)
ff = subprocess.Popen(["ffmpeg", "-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24",
                       "-s", f"{W}x{H}", "-r", str(FPS), "-i", "-", "-c:v", "libx264", "-pix_fmt",
                       "yuv420p", "-crf", "15", "-preset", "slow", "-movflags", "+faststart", args.out],
                      stdin=subprocess.PIPE)
for i in range(frames):
    ff.stdin.write(render(i).tobytes())
ff.stdin.close()
ff.wait()
write_beats(args.out)
print(f"{frames} frames, {frames / FPS:.2f} s -> {args.out}")
