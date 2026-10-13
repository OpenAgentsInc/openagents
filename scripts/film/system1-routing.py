#!/usr/bin/env python3
"""System 1 in front of System 2, on the route map: a 28 s shot for the
Coder v1 launch film.

  scripts/film/system1-routing.py OUT.mp4 [--frames-dir DIR] [--only SECONDS,...]

Beats (the timeline constants below hold the exact times):
  0-9       Some other harness: the big model (System 2) at the center in a
            thin ring that only carries its tools (Bash, Read, Edit, Write,
            Grep, Web). Every message and every tool step goes straight into
            the model and back. The cost and time per fix climb to bare
            Claude Code's and hold.
  9-17.5    OpenAgents: a System 1 ring sweeps in (the decisions from
            docs/coder/design/system-one-hooks.md: route, brief, squeeze,
            gate, watch, verify) and the route map fades in around it (the
            real routes, engines, plugins, decks and knowledge from
            crates/openagents-chat-app/src/route_map/sources.json, at the
            angles route_map/layout.rs gives them). Light now stops at the
            ring and hops out to what serves it; only the hard part reaches
            the center. The counters fall to the briefed agent's and hold.
  17.5-23.5 The map grows: plugins other people add appear on the outer
            ring, gold runs out past them to their authors, the ring takes
            more.
  23.5-28   A calm hold on the whole map, panned to the middle.

Numbers on screen: docs/inference/briefed-agent-ab.md (#11211): per accepted
change $4.00 (bare Claude Code) vs $1.84 (briefed), median 284 s vs 121 s,
21 closed issues x 3 runs, same model, 19/63 accepted each.

Colors are the map's own on the noir canvas: crates/oa-tokens/src/noir.rs
(canvas 0a0a0a, content ededed, secondary 818181, tertiary 656565), the map's
edge (white at alpha 38, visual.rs), the request light (236,240,255) and the
payment gold (255,206,84) from crates/openagents-desktop/src/route_future.rs.

Fonts (licensed, not in the repo): FILM_FONTS (Geist, default
~/work/film-out/fonts) and BERKELEY_FONTS (default ~/Library/Fonts).
Needs Pillow, numpy and ffmpeg.
"""

import argparse
import json
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

REPO = Path(__file__).resolve().parents[2]
SOURCES = REPO / "crates/openagents-chat-app/src/route_map/sources.json"
GEIST = Path(os.path.expanduser(os.environ.get("FILM_FONTS", "~/work/film-out/fonts")))
BERKELEY = Path(os.path.expanduser(os.environ.get("BERKELEY_FONTS", "~/Library/Fonts")))

W, H, FPS = 1920, 1080, 30
S = 2  # static layers are drawn at twice the size, then scaled down

# --- palette ---------------------------------------------------------------
CANVAS = (10, 10, 10)
CONTENT = (237, 237, 237)
SECONDARY = (129, 129, 129)
TERTIARY = (101, 101, 101)
LIGHT = (236, 240, 255)
GOLD = (255, 206, 84)
EDGE_A = 38

# Node grays by kind: the map's kinds, in the noir black and white.
KIND_GRAY = {
    "front": 235, "family": 150, "route": 205, "answer": 92, "knowledge": 80,
    "model": 150, "coder": 220, "engine": 175, "plugin": 196, "screen": 120,
}


def font(name, size):
    path = GEIST / name if name.startswith("Geist") else BERKELEY / name
    return ImageFont.truetype(str(path), size)


# --- the map: route_map.rs Map::build, from the real sources ---------------
src = json.loads(SOURCES.read_text())
FAMILIES = ["answers", "work", "gym", "screens", "boundaries"]
FAMILY_LABEL = {"answers": "Answers", "work": "Work", "gym": "The Gym",
                "screens": "Screens and actions", "boundaries": "Boundaries"}
# Route ids in plain words for the screen (the map itself shows the ids).
ROUTE_WORDS = {
    "meta": "About us", "smalltalk": "Small talk", "general": "General",
    "product.kb": "Knowledge", "codebase.kb": "Our code", "work.dispatch": "Tasks",
    "cli": "Commands", "wallet": "Wallet", "account": "Account",
    "clarify": "Ask back", "end": "End", "refuse": "Refuse", "gym.news": "Gym news",
    "eval.run": "Run tests", "eval.author": "New plugin", "eval.check": "Checks",
    "eval.result": "Results", "eval.credit": "Credit",
    "capability.missing": "Not yet", "presentation.open": "Decks",
    "standing.rule": "Standing rules",
}
SCREEN_LABEL = {"wallet": "Wallet", "account.computers": "Computers",
                "account.keys": "Identity keys", "account.playtest": "Playtest",
                "account.report_problem": "Report a problem"}

nodes = []


def add(nid, kind, label, parent):
    depth = 0 if parent is None else nodes[parent]["depth"] + 1
    nodes.append(dict(id=nid, kind=kind, label=label, parent=parent, depth=depth, weight=0.3))
    return len(nodes) - 1


def find(nid):
    for i, n in enumerate(nodes):
        if n["id"] == nid:
            return i
    return None


front = add("front", "front", "OpenAgents", None)
nodes[front]["weight"] = 1.0
fam = {}
for word in FAMILIES:
    if any(r["family"] == word for r in src["routes"]):
        fam[word] = add("family:" + word, "family", FAMILY_LABEL[word], front)
        nodes[fam[word]]["weight"] = 0.8
max_rows = max([v["rows"] for v in src["labeled"]["routes"].values()] + [1])
routes = {}
for r in src["routes"]:
    if r["family"] in fam:
        i = add("route:" + r["id"], "route", ROUTE_WORDS.get(r["id"], r["id"]), fam[r["family"]])
        rows = src["labeled"]["routes"].get(r["id"], {}).get("rows", 0)
        nodes[i]["weight"] = 0.35 + 0.65 * math.sqrt(rows / max_rows)
        routes[r["id"]] = i
if "general" in routes:
    add("model", "model", "Chat model", routes["general"])
for a in src["answers"]:
    r = next((routes[x] for x in a["routes"] if x in routes), None)
    if r is not None:
        add("answer:" + a["id"], "answer", a.get("chip") or a["id"], r)
for a in src["answers"]:
    screen = (a.get("offer") or {}).get("screen")
    frm = find("answer:" + a["id"])
    if screen and frm is not None and find("screen:" + screen) is None:
        add("screen:" + screen, "screen", SCREEN_LABEL.get(screen, screen), nodes[frm]["parent"])
if "presentation.open" in routes:
    for d in src["decks"]:
        add("deck:" + d["id"], "screen", d["title"], routes["presentation.open"])
for c in src["capabilities"]:
    if c["id"] == "chat.cli" and c.get("route") in routes:
        add("screen:" + c["id"], "screen", c["name"], routes[c["route"]])
if "product.kb" in routes:
    base = add("knowledge:product", "knowledge", "Product knowledge", routes["product.kb"])
    for e in src["knowledge"]["product"]:
        add("knowledge:" + e["id"], "knowledge", e["title"], base)
if "codebase.kb" in routes:
    add("knowledge:codebase", "knowledge", "The OpenAgents code", routes["codebase.kb"])
coder = None
if "work.dispatch" in routes:
    coder = add("coder", "coder", "Coder", routes["work.dispatch"])
    nodes[coder]["weight"] = 0.9
    for e in src["engines"]:
        add("engine:" + e["id"], "engine", e["name"], coder)
    add("knowledge:coding", "knowledge", "Coding knowledge", coder)
    for p in src["plugins"]:
        i = add("plugin:" + p["dir"], "plugin", p["name"], coder)
        tests = sum(t.get("should_fire", 0) + t.get("should_not_fire", 0) for t in p.get("tests", []))
        nodes[i]["weight"] = 0.35 + 0.05 * min(tests, 12)

N = len(nodes)
children = [[] for _ in range(N)]
for i, n in enumerate(nodes):
    if n["parent"] is not None:
        children[n["parent"]].append(i)

# --- layout: route_map/layout.rs Layout::of --------------------------------
RINGS = [0.0, 240.0, 500.0, 790.0, 1080.0, 1320.0]
FAMILY_PAD = 2.0
BASE_R = {"front": 34, "family": 22, "route": 16, "coder": 20, "model": 14, "plugin": 12,
          "engine": 10, "answer": 7, "knowledge": 7, "screen": 7}


def radius(kind, weight):
    return BASE_R[kind] * (0.7 + 0.6 * max(0.0, min(1.0, weight)))


def min_leaves(kind):
    return 2.5 if kind in ("route", "coder", "family") else 1.0


leaves = [0.0] * N
for i in reversed(range(N)):
    s = sum(leaves[c] for c in children[i])
    if nodes[i]["kind"] == "front":
        s += FAMILY_PAD * len(children[i])
    leaves[i] = max(s, min_leaves(nodes[i]["kind"]))
spans = [(0.0, 0.0)] * N
pos = [(0.0, 0.0)] * N
spans[front] = (-math.pi / 2, -math.pi / 2 + 2 * math.pi)
for i in range(N):
    a0, a1 = spans[i]
    pad = FAMILY_PAD if nodes[i]["kind"] == "front" else 0.0
    room = max(leaves[i], 1.0)
    at = a0 + (a1 - a0) * (pad / 2) / room
    for c in children[i]:
        wdt = (a1 - a0) * leaves[c] / room
        spans[c] = (at, at + wdt)
        at += wdt + (a1 - a0) * pad / room
    d = min(nodes[i]["depth"], len(RINGS) - 1)
    ang = (a0 + a1) / 2
    pos[i] = (0.0, 0.0) if d == 0 else (RINGS[d] * math.cos(ang), RINGS[d] * math.sin(ang))
scale = 1.0
P = np.array(pos)
maxr = np.array([radius(n["kind"], 1.0) for n in nodes])
for a in range(N):
    dist = np.hypot(*(P[a + 1:] - P[a]).T)
    need = maxr[a] + maxr[a + 1:] + 2.0
    ok = dist > 0
    if ok.any():
        scale = max(scale, float((need[ok] / dist[ok]).max()))
P *= scale
RAD = np.array([radius(n["kind"], n["weight"]) for n in nodes])
ANG = np.array([(spans[i][0] + spans[i][1]) / 2 for i in range(N)])
RING_W = [r * scale for r in RINGS]

# --- the film's own layer: System 2 at the center, System 1 in front -----
# The angles are the layout's; the rings are redrawn in screen points so the
# center has room for System 2 and the System 1 ring in front of it.
CX, CY = 1238.0, 540.0
RS = [0.0, 212.0, 292.0, 368.0, 428.0]          # screen radius by depth
R_CORE = 60.0                                   # System 2's disc
R_S1 = 128.0                                    # the System 1 ring
R_FUT, R_AUTH = 462.0, 496.0                    # new plugins, their authors
NODE_PX = 0.40                                  # world radius to screen points
HOOKS = ["route", "brief", "squeeze", "gate", "watch", "verify"]  # system-one-hooks.md
MORE_HOOKS = ["context", "recall", "escalate"]                        # grown in beat 3
# Plugins other people add, around the outer ring (illustrative names; the
# first is the episode 289 example, "meeting notes into Linear tickets").
FUTURE = ["Linear tickets", "Release notes", "Flaky tests", "Changelog", "Code review",
          "Migrations", "Docs sync", "Bug triage", "Benchmarks", "Translations",
          "Security scan", "Dependency bumps", "Invoices", "Data cleanup", "Test fixtures",
          "Onboarding", "Incident notes", "API docs"]
fut_ang = [-math.pi / 2 + 0.17 + i * (2 * math.pi / len(FUTURE)) for i in range(len(FUTURE))]


def polar(r, a):
    return (CX + r * math.cos(a), CY + r * math.sin(a))


fut_pos = [polar(R_FUT, a) for a in fut_ang]
auth_pos = [polar(R_AUTH, a) for a in fut_ang]
fut_born = [8.9 + 0.13 * i for i in range(len(FUTURE))]
# Each future plugin hangs off the route nearest it in angle.
route_ids = [i for i, n in enumerate(nodes) if n["kind"] == "route"]


def nearest_route(a):
    return min(route_ids, key=lambda i: abs(math.atan2(math.sin(ANG[i] - a), math.cos(ANG[i] - a))))


SCR = [polar(RS[min(n["depth"], 4)], ANG[i]) for i, n in enumerate(nodes)]
CORE = (CX, CY)
ZOOM = 1.0


def sp(p):
    return p


def s1_point(a):
    return polar(R_S1, a)


outer = [i for i, n in enumerate(nodes) if n["depth"] >= 3]
fut_pos = [polar(R_FUT + (14 if i % 2 else -10), a) for i, a in enumerate(fut_ang)]
auth_pos = [polar(R_FUT + (14 if i % 2 else -10) + 36, a) for i, a in enumerate(fut_ang)]
fut_parent = [min(outer, key=lambda j: math.dist(SCR[j], p)) for p in fut_pos]


hook_ang = [-math.pi / 2 + math.pi / 6 + i * 2 * math.pi / len(HOOKS) for i in range(len(HOOKS))]
more_ang = [hook_ang[i] + math.pi / len(HOOKS) for i in (0, 2, 4)]

# --- easing ---------------------------------------------------------------


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


# --- the static map (drawn once at 2x) ------------------------------------
f_family = font("Geist-Regular.ttf", 15 * S)
f_route = font("Geist-Regular.ttf", 12 * S)
f_member = font("Geist-Regular.ttf", 11 * S)


def draw_labels(d, items, placed, discs):
    """items: (x, y, r, angle, text, font, fill). Labels sit outside their
    node, away from the center; one that overlaps a placed label is skipped
    (route_map.rs paint)."""
    for x, y, r, ang, text, f, fill, major in items:
        c, s = math.cos(ang), math.sin(ang)
        gap = r + 5 * S
        tw = d.textlength(text, font=f)
        asc, desc = f.getmetrics()
        th = asc + desc
        if c > 0.4:
            first = (x + c * gap, y + s * gap - th / 2)
        elif c < -0.4:
            first = (x + c * gap - tw, y + s * gap - th / 2)
        elif s >= 0:
            first = (x - tw / 2, y + gap)
        else:
            first = (x - tw / 2, y - gap - th)
        # the side the layout picks, then under and over the node
        tries = [first, (x - tw / 2, y + gap), (x - tw / 2, y - gap - th),
                 (x - gap - tw, y - th / 2), (x + gap, y - th / 2)] if major else [first]
        for lx, ly in tries:
            box = (lx - 2 * S, ly + 2 * S, lx + tw + 2 * S, ly + th - 2 * S)
            if box[0] < 600 * S or box[2] > W * S - 8 or box[1] < 8 or box[3] > H * S - 8:
                continue
            # the System 1 ring and its words keep their room
            nx = min(max(CX * S, box[0]), box[2])
            ny = min(max(CY * S, box[1]), box[3])
            if math.hypot(nx - CX * S, ny - CY * S) < (R_S1 + 58) * S:
                continue
            if any(not (box[2] < b[0] or box[0] > b[2] or box[3] < b[1] or box[1] > b[3])
                   for b in placed + discs):
                continue
            placed.append(box)
            d.text((lx, ly), text, font=f, fill=fill)
            break


def static_map():
    im = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    for i, n in enumerate(nodes):
        if n["parent"] is None or n["parent"] == front:
            continue  # the center's spokes are drawn live, to System 2
        a, b = SCR[n["parent"]], SCR[i]
        d.line([(a[0] * S, a[1] * S), (b[0] * S, b[1] * S)], fill=(255, 255, 255, EDGE_A), width=S)
    for i, n in enumerate(nodes):
        if i == front:
            continue
        x, y = SCR[i]
        r = max(RAD[i] * NODE_PX, 1.8) * S
        g = KIND_GRAY[n["kind"]]
        d.ellipse([x * S - r, y * S - r, x * S + r, y * S + r], fill=(g, g, g, 255))
    # the System 1 ring and its words keep their room
    discs = [(x * S - r, y * S - r, x * S + r, y * S + r) for i, (x, y) in enumerate(SCR)
              if i != front for r in [max(RAD[i] * NODE_PX, 1.8) * S + 2 * S]
              if nodes[i]["kind"] in ("family", "route", "coder")]
    items = []
    order = sorted(range(N), key=lambda i: (nodes[i]["depth"], i))
    for i in order:
        n = nodes[i]
        if i == front or n["kind"] in ("answer", "knowledge") and n["id"] not in (
                "knowledge:product", "knowledge:coding", "knowledge:codebase"):
            continue
        if "." in n["label"] or n["label"].islower():
            continue
        x, y = SCR[i]
        r = max(RAD[i] * NODE_PX, 1.8) * S
        f = f_family if n["kind"] == "family" else f_route if n["kind"] in ("route", "coder") else f_member
        fill = (*CONTENT, 225) if n["kind"] in ("family", "coder") else (*SECONDARY, 235) \
            if n["kind"] in ("route", "engine", "plugin") else (*TERTIARY, 235)
        items.append((x * S, y * S, r, ANG[i], n["label"], f, fill, n["kind"] in ("family", "route", "coder")))
    placed = []
    draw_labels(d, items, placed, discs)
    return im.resize((W, H), Image.LANCZOS), placed


MAP, PLACED = static_map()

# --- text -----------------------------------------------------------------
g_cap = font("Geist-Regular.ttf", 34)
g_msg = font("Geist-Regular.ttf", 22)
g_small = font("Geist-Regular.ttf", 17)
g_tiny = font("Geist-Regular.ttf", 15)
g_core = font("Geist-Regular.ttf", 21)
g_hook = font("Geist-Regular.ttf", 15)
g_fut = font("Geist-Regular.ttf", 13)
m_big = font("BerkeleyMono-Regular.ttf", 50)
m_mid = font("BerkeleyMono-Regular.ttf", 30)
m_old = font("BerkeleyMono-Regular.ttf", 24)

LEFT = 118

# --- the timeline (seconds) ------------------------------------------------
# Beat 1, some other harness: the big model in a thin ring of its own tools.
B1_CAPTION = (0.8, 8.6)
B1_MSGS = [1.8, 3.0, 4.2, 5.4]
B1_CLIMB = (1.8, 6.8)        # the counters climb, then hold
# Beat 2, OpenAgents: System 1 in front, the route map around it.
B2 = 9.0
HARNESS_OUT = (9.0, 10.0)
S1_SWEEP = 9.2               # the ring sweeps on over 1.2 s
MAP_IN = (10.2, 11.8)
CAM_OUT = (9.2, 11.4)        # the camera eases back from the close-up
B2_CAPTION = (9.3, 17.2)
B2_MSGS = [11.6, 12.4, 13.2, 14.0, 14.8]
B2_FALL = (11.8, 14.4)       # the counters fall, then hold
B2_RATIO = 14.4
METERS_OUT = 22.6
# Beat 3: the map grows.
B3 = 17.5
B3_CAPTION = (17.6, 23.2)
FUT_START, FUT_STEP = 17.8, 0.21
MORE_HOOKS_AT = 18.6
# Beat 4: the hold.
B4_CAPTION = (23.5, 26.6)
PAN = (26.0, 27.6)
SECONDS = 28.0
Z_CLOSE = 1.6                # beat 1's close-up on the big model

TOOLS = ["Bash", "Read", "Edit", "Write", "Grep", "Web"]
R_H = 96.0                   # the harness ring, in scene points
tool_ang = [-math.pi / 2 + math.pi / 6 + i * 2 * math.pi / len(TOOLS) for i in range(len(TOOLS))]
tool_pos = [polar(R_H, a) for a in tool_ang]
fut_born = [FUT_START + FUT_STEP * i for i in range(len(FUTURE))]


def zcam(t):
    """The camera's zoom about the map's center."""
    back = ease((t - CAM_OUT[0]) / (CAM_OUT[1] - CAM_OUT[0]))
    z = Z_CLOSE + (1.0 - Z_CLOSE) * back
    return z * (1 + 0.02 * ease((t - CAM_OUT[1]) / 14.0)) * (1 - 0.05 * fade(t, *PAN))


def to_scene(p, z):
    return (CX + (p[0] - CX) / z, CY + (p[1] - CY) / z)


LEFT = 118

MSG1 = [  # (text, tool steps through the big model)
    ("Fix the flaky parser test", [0, 4, 1, 2, 0]),
    ("Find where the config loads", [4, 1, 4, 1]),
    ("Add a test for the fix", [1, 3, 0, 2, 0]),
    ("Is it done?", [0, 1, 0]),
]
MSG2 = [  # (text, target, hard part reaches System 2)
    ("Why is the build failing?", "engine:claude_code", True),
    ("Send a thousand sats", "screen:wallet", False),
    ("Outline this crate", "plugin:crates/plugin-outline", False),
    ("Turn my notes into tickets", "plugin:crates/plugin-action-items", False),
    ("Is the fix done?", "route:eval.check", False),
]
MSG_Y0, MSG_DY = 352, 52


def idx(nid):
    i = find(nid)
    assert i is not None, nid
    return i


# --- lights -----------------------------------------------------------------
class Light:
    """A light along screen points: the trail behind the head, the head, and
    a glow at the target once it lands (route_map.rs paint_light)."""

    def __init__(self, pts, t0, dur, color=LIGHT, width=3.0, hold=0.7, glow_r=10.0, head=1.0):
        self.pts, self.t0, self.dur, self.color = pts, t0, dur, color
        self.width, self.hold, self.glow_r, self.head = width, hold, glow_r, head
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
        else:
            land = clamp((t - self.t0 - self.dur) / 0.25)
            x, y = self.pts[-1]
            r = (self.glow_r + 10 * land) * S
            d.ellipse([x * S - r, y * S - r, x * S + r, y * S + r],
                      outline=(*c, int(220 * fadek)), width=2 * S)


class Dot:
    """A small pulse along a path (route_future.rs traffic)."""

    def __init__(self, pts, t0, dur, color, r=3.2):
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


def msg_anchor(k, text):
    w = g_msg.getlength(text)
    return (LEFT + w + 22, MSG_Y0 + k * MSG_DY + 14)


lights = []
rng = random.Random(289)
# Beat 1: every message goes straight into the big model, and every tool step
# goes out through the thin ring and back into it.
for k, ((text, steps), t0) in enumerate(zip(MSG1, B1_MSGS)):
    a = to_scene(msg_anchor(k, text), Z_CLOSE)
    lights.append(Light([a, CORE], t0 + 0.15, 0.5, width=3.4, glow_r=R_CORE + 4, hold=0.5))
    t = t0 + 0.75
    for s in steps:
        p = tool_pos[s]
        lights.append(Light([CORE, p], t, 0.2, width=2.0, glow_r=10, hold=0.3, head=0.7))
        lights.append(Light([p, CORE], t + 0.24, 0.2, width=2.0, glow_r=R_CORE + 3, hold=0.3, head=0.7))
        t += 0.24
# Chatter: the big model calling its tools all the time.
for j in range(120):
    t = 2.0 + rng.random() * 6.4
    p = tool_pos[rng.randrange(len(TOOLS))]
    lights.append(Light([CORE, p], t, 0.22, width=1.0, glow_r=7, hold=0.2, head=0.45))
    lights.append(Light([p, CORE], t + 0.26, 0.22, width=1.0, glow_r=R_CORE + 2, hold=0.2, head=0.45))


# Beat 2: the ring catches each message and runs it around to the decision
# nearest what serves it; it hops out; only the hard part goes on inward.
def arc(a0, a1, n=16):
    d = math.atan2(math.sin(a1 - a0), math.cos(a1 - a0))
    return [s1_point(a0 + d * j / n) for j in range(n + 1)]


for k, ((text, target, hard), t0) in enumerate(zip(MSG2, B2_MSGS)):
    a = msg_anchor(k, text)
    ti = idx(target)
    tp = SCR[ti]
    entry = math.atan2(a[1] - CY, a[0] - CX)
    exit_ = math.atan2(tp[1] - CY, tp[0] - CX)
    lights.append(Light([a] + arc(entry, exit_) + [tp], t0 + 0.1, 0.8, width=2.8,
                        glow_r=RAD[ti] * NODE_PX + 7, hold=0.8))
    if hard:
        lights.append(Light(arc(entry, entry + 0.5, 4) + [CORE], t0 + 0.55, 0.4, width=1.4,
                            glow_r=R_CORE + 4, hold=0.6, head=0.6))
# Steady traffic from beat 2 on: requests from the ring out to what serves
# them; in beat 3 also to the new plugins, with gold out past them to their
# authors (route_live.rs `share`).
members = [i for i, n in enumerate(nodes) if n["kind"] in ("plugin", "engine", "knowledge", "answer", "screen", "coder", "model")]
for j in range(330):
    t = 11.8 + rng.random() * (SECONDS - 11.8)
    hook = rng.randrange(len(HOOKS))
    if t > B3 + 0.6 and rng.random() < 0.55:
        f = rng.randrange(len(FUTURE))
        if t < fut_born[f] + 0.5:
            continue
        hp = s1_point(hook_ang[hook])
        pp = fut_pos[f]
        lights.append(Dot([hp, SCR[fut_parent[f]], pp], t, 0.9, LIGHT, r=2.4))
        lights.append(Dot([pp, auth_pos[f]], t + 0.92, 0.55, GOLD, r=3.4))
    else:
        p = SCR[rng.choice(members)]
        lights.append(Dot([s1_point(hook_ang[hook]), p], t, 0.65, LIGHT, r=2.2))
    if rng.random() < 0.12:  # now and then a hard part goes to System 2
        lights.append(Dot([s1_point(hook_ang[hook]), CORE], t, 0.45, LIGHT, r=2.6))
# Gold to the authors of today's plugins too.
for j in range(26):
    t = B3 + 0.6 + rng.random() * (SECONDS - B3 - 0.6)
    p = SCR[idx(rng.choice(["plugin:crates/plugin-outline", "plugin:crates/plugin-action-items", "plugin:packages/jev-probe"]))]
    lights.append(Dot([CORE, p], t, 0.7, GOLD, r=2.6))


# --- counters -------------------------------------------------------------
COST_BARE, COST_S1 = 4.00, 1.84  # $ per accepted change, briefed-agent-ab.md
TIME_BARE, TIME_S1 = 284, 121    # median seconds


def counters(t):
    up = ease((t - B1_CLIMB[0]) / (B1_CLIMB[1] - B1_CLIMB[0]))
    down = ease((t - B2_FALL[0]) / (B2_FALL[1] - B2_FALL[0]))
    cost = COST_BARE * up + (COST_S1 - COST_BARE) * down
    secs = TIME_BARE * up + (TIME_S1 - TIME_BARE) * down
    return cost, secs, down


# --- frame ------------------------------------------------------------------
def glow_bg():
    g = Image.new("L", (W // 4, H // 4), 0)
    ImageDraw.Draw(g).ellipse([(CX - 560) / 4, (CY - 520) / 4, (CX + 560) / 4, (CY + 520) / 4], fill=40)
    g = g.filter(ImageFilter.GaussianBlur(60)).resize((W, H), Image.BICUBIC)
    a = np.asarray(g, dtype=np.float32) / 255.0
    base = np.zeros((H, W, 3), np.float32) + np.array(CANVAS, np.float32)
    return base + a[..., None] * np.array([150, 154, 168], np.float32) * 0.45


BG = glow_bg()
vig = Image.new("L", (W // 4, H // 4), 0)
ImageDraw.Draw(vig).ellipse([-W * .2 / 4, -H * .3 / 4, W * 1.2 / 4, H * 1.3 / 4], fill=255)
VIG = np.asarray(vig.filter(ImageFilter.GaussianBlur(50)).resize((W, H), Image.BICUBIC), np.float32)[..., None] / 255.0
VIG = 0.35 + 0.65 * VIG
MAP_A = np.asarray(MAP, np.float32) / 255.0


def over(dst, layer_rgba, k=1.0):
    a = layer_rgba[..., 3:4] * k
    return dst * (1 - a) + layer_rgba[..., :3] * 255.0 * a


def text_layer(draw_fn):
    im = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    draw_fn(ImageDraw.Draw(im))
    return np.asarray(im, np.float32) / 255.0


def caption(d, t):
    caps = [
        (*B1_CAPTION, ["Today, every step goes", "through the big model."]),
        (*B2_CAPTION, ["Put System 1 in front.", "Send only the hard parts on."]),
        (*B3_CAPTION, ["Anyone can add to the map,", "and gets paid when it's used."]),
        (*B4_CAPTION, ["System 1 in front.", "System 2 for the hard parts."]),
    ]
    for a, b, lines in caps:
        k = fade(t, a, a + 0.6) * (1 - fade(t, b - 0.5, b))
        if k <= 0:
            continue
        for j, line in enumerate(lines):
            d.text((LEFT, 150 + j * 46), line, font=g_cap, fill=(*CONTENT, int(255 * k)))


def messages(d, t):
    groups = (([m[0] for m in MSG1], B1_MSGS, B1_CAPTION[1]), ([m[0] for m in MSG2], B2_MSGS, B2_CAPTION[1]))
    for texts, times, end in groups:
        for k, (text, t0) in enumerate(zip(texts, times)):
            a = fade(t, t0 - 0.2, t0 + 0.2) * (1 - fade(t, end - 0.5, end))
            if a <= 0:
                continue
            y = MSG_Y0 + k * MSG_DY + 8 * (1 - fade(t, t0 - 0.2, t0 + 0.25))
            done = fade(t, t0 + 1.0, t0 + 2.0)
            col = tuple(int(CONTENT[i] + (SECONDARY[i] - CONTENT[i]) * done) for i in range(3))
            d.text((LEFT, y), text, font=g_msg, fill=(*col, int(255 * a)))
            ax, ay = msg_anchor(k, text)
            r = 3.2
            d.ellipse([ax - r, ay - r, ax + r, ay + r], fill=(*LIGHT, int(255 * a)))


def meters(d, t):
    a = fade(t, B1_CLIMB[0] - 0.4, B1_CLIMB[0]) * (1 - fade(t, METERS_OUT - 0.6, METERS_OUT))
    if a <= 0:
        return
    cost, secs, down = counters(t)
    y = 690
    for x, label, value, old, ratio in (
        (LEFT, "cost per fix", f"${cost:.2f}", "$4.00", "2×"),
        (LEFT + 250, "time per fix", f"{secs:.0f} s", "284 s", "2×"),
    ):
        d.text((x, y), label, font=g_small, fill=(*SECONDARY, int(255 * a)))
        d.text((x, y + 28), value, font=m_big, fill=(*CONTENT, int(255 * a)))
        k = fade(t, B2_FALL[0] - 0.3, B2_FALL[0] + 0.3) * a
        if k > 0:  # the old number, struck through
            ow = m_old.getlength(old)
            d.text((x, y + 98), old, font=m_old, fill=(*TERTIARY, int(255 * k)))
            d.line([(x - 2, y + 113), (x + ow + 2, y + 113)], fill=(*TERTIARY, int(255 * k)), width=2)
        k = fade(t, B2_RATIO, B2_RATIO + 0.6) * a
        if k > 0:
            word = "cheaper" if "cost" in label else "faster"
            nw = m_mid.getlength(ratio)
            d.text((x + 100, y + 96), ratio, font=m_mid, fill=(*CONTENT, int(255 * k)))
            d.text((x + 100 + nw + 9, y + 102), word, font=g_core, fill=(*CONTENT, int(255 * k)))
    k = fade(t, B2_RATIO + 0.5, B2_RATIO + 1.1) * a
    if k > 0:
        d.text((LEFT, y + 160), "Same model. 21 real issues, 3 runs each.", font=g_small,
               fill=(*SECONDARY, int(255 * k)))


def hook_list():
    hooks = [(ang, name, S1_SWEEP + 0.2 + 0.16 * i) for i, (ang, name) in enumerate(zip(hook_ang, HOOKS))]
    hooks += [(ang, name, MORE_HOOKS_AT + 0.5 * i) for i, (ang, name) in enumerate(zip(more_ang, MORE_HOOKS))]
    return hooks


def core_and_ring(d, ld, t):
    """System 2 at the center, beat 1's harness ring, and the System 1 ring,
    drawn at 2x; glows go on the light layer `ld`, which blooms."""
    busy = fade(t, 1.8, 2.6) * (1 - fade(t, 8.8, 10.0))
    calm = fade(t, B2, B2 + 1.4)
    mapk = fade(t, *MAP_IN)
    cx, cy = CORE[0] * S, CORE[1] * S
    rc = R_CORE * S * (1 + 0.05 * busy * math.sin(t * 19))
    # spokes from System 2 to the families, once the map is in
    if mapk > 0:
        for i in children[front]:
            x, y = SCR[i]
            d.line([(cx, cy), (x * S, y * S)], fill=(255, 255, 255, int(EDGE_A * mapk)), width=S)
    # beat 1: the thin harness ring and its tools
    hk = fade(t, 0.3, 1.2) * (1 - fade(t, *HARNESS_OUT))
    if hk > 0:
        rr = R_H * S
        d.ellipse([cx - rr, cy - rr, cx + rr, cy + rr], outline=(*LIGHT, int(110 * hk)), width=S)
        for (x, y) in tool_pos:
            d.line([(cx, cy), (x * S, y * S)], fill=(255, 255, 255, int((EDGE_A + 40 * busy) * hk)), width=S)
            r = 6 * S
            d.ellipse([x * S - r, y * S - r, x * S + r, y * S + r], fill=(205, 205, 205, int(255 * hk)))
    heat = 0.35 + 0.65 * busy - 0.15 * calm
    for r, a in ((1.22, 70), (1.08, 120)):
        rr = rc * r
        ld.ellipse([cx - rr, cy - rr, cx + rr, cy + rr], fill=(*LIGHT, int(a * heat)))
    # load: ripples off the big model while everything runs through it
    for j in range(3):
        ph = ((t * 1.7) + j / 3) % 1.0
        k = busy * (1 - ph) ** 1.5
        if k > 0.01:
            rr = rc * (1.1 + 0.9 * ph)
            ld.ellipse([cx - rr, cy - rr, cx + rr, cy + rr], outline=(*LIGHT, int(140 * k)), width=2 * S)
    g = int(232 - 40 * calm)
    d.ellipse([cx - rc, cy - rc, cx + rc, cy + rc], fill=(g, g, g, 255))
    # the System 1 ring
    sweep = out_cubic((t - S1_SWEEP) / 1.2)
    if sweep > 0:
        rr = R_S1 * S
        grow = fade(t, B3 + 0.5, B3 + 2.5)
        width = int((2 + 1.2 * grow) * S)
        a0 = -90
        ld.arc([cx - rr, cy - rr, cx + rr, cy + rr], a0, a0 + 360 * sweep, fill=(*LIGHT, 60 + int(30 * grow)), width=width * 3)
        d.arc([cx - rr, cy - rr, cx + rr, cy + rr], a0, a0 + 360 * sweep, fill=(*LIGHT, 230), width=width)
        for ang, name, born in hook_list():
            k = out_cubic((t - born) / 0.45)
            if k <= 0:
                continue
            x, y = s1_point(ang)
            r = 8 * S * k
            ld.ellipse([x * S - r * 1.6, y * S - r * 1.6, x * S + r * 1.6, y * S + r * 1.6], fill=(*LIGHT, 90))
            d.ellipse([x * S - r, y * S - r, x * S + r, y * S + r], fill=(*CONTENT, 255))


def side_label(d, x, y, ang, name, f, fill, gap=16):
    c, s_ = math.cos(ang), math.sin(ang)
    tw = f.getlength(name)
    ix, iy = x + c * gap, y + s_ * gap
    lx = ix if c > 0.3 else ix - tw if c < -0.3 else ix - tw / 2
    ly = iy - 9 + (9 if s_ > 0.5 else -9 if s_ < -0.5 else 0)
    d.text((lx, ly), name, font=f, fill=fill)


def core_labels(d, t):
    a = fade(t, 0.5, 1.2)
    cx, cy = CORE
    if a > 0:
        line1, line2 = "System 2", "the big model"
        w1, w2 = g_core.getlength(line1), g_tiny.getlength(line2)
        dark = 1 - fade(t, B2, B2 + 1.4) * 0.2
        d.text((cx - w1 / 2, cy - 15), line1, font=g_core, fill=(14, 14, 14, int(255 * a * dark)))
        d.text((cx - w2 / 2, cy + 10), line2, font=g_tiny, fill=(40, 40, 40, int(255 * a * dark)))
    hk = fade(t, 0.6, 1.4) * (1 - fade(t, HARNESS_OUT[0], HARNESS_OUT[0] + 0.6))
    if hk > 0:
        for (x, y), ang, name in zip(tool_pos, tool_ang, TOOLS):
            side_label(d, x, y, ang, name, g_hook, (*SECONDARY, int(255 * hk)), gap=12)
    for ang, name, born in hook_list():
        kk = fade(t, born + 0.25, born + 0.7)
        if kk <= 0:
            continue
        x, y = s1_point(ang)
        side_label(d, x, y, ang, name, g_hook, (*CONTENT, int(235 * kk)))
    kk = fade(t, S1_SWEEP + 1.0, S1_SWEEP + 1.6)
    if kk > 0:
        tag = "System 1"
        tw = g_core.getlength(tag)
        d.text((CORE[0] - tw / 2, CORE[1] - R_S1 - 40), tag, font=g_core, fill=(*CONTENT, int(255 * kk)))


def future_layer(d, t):
    for f, name in enumerate(FUTURE):
        k = out_cubic((t - fut_born[f]) / 0.6)
        if k <= 0:
            continue
        p, q, au = fut_pos[f], SCR[fut_parent[f]], auth_pos[f]
        e = (q[0] + (p[0] - q[0]) * k, q[1] + (p[1] - q[1]) * k)
        d.line([(q[0] * S, q[1] * S), (e[0] * S, e[1] * S)], fill=(255, 255, 255, int(EDGE_A * 1.3)), width=S)
        flash = 1 - fade(t, fut_born[f] + 0.2, fut_born[f] + 1.2)
        r = 7.5 * S * k
        d.ellipse([e[0] * S - r, e[1] * S - r, e[0] * S + r, e[1] * S + r], fill=(210, 210, 210, 255))
        if flash > 0:
            rr = r * (1.3 + 0.9 * (1 - flash))
            d.ellipse([e[0] * S - rr, e[1] * S - rr, e[0] * S + rr, e[1] * S + rr], outline=(*LIGHT, int(160 * flash)), width=S)
        ka = fade(t, fut_born[f] + 0.4, fut_born[f] + 1.0)
        if ka > 0:  # the author, out past the plugin
            d.line([(p[0] * S, p[1] * S), (au[0] * S, au[1] * S)], fill=(*GOLD, int(40 * ka)), width=S)
            r = 3.6 * S
            d.ellipse([au[0] * S - r, au[1] * S - r, au[0] * S + r, au[1] * S + r], fill=(*GOLD, int(200 * ka)))


def future_labels(d, t, placed):
    for f, name in enumerate(FUTURE):
        k = fade(t, fut_born[f] + 0.3, fut_born[f] + 0.8)
        if k <= 0:
            continue
        x, y = fut_pos[f]
        s = math.sin(fut_ang[f])
        tw = g_fut.getlength(name)
        ly = y - 27 if (s < -0.6 or s > 0.3) else y + 10
        lx = x - tw / 2
        box = (lx, ly, lx + tw, ly + 16)
        if box[0] < (600 if t < B4_CAPTION[1] - 0.6 else 0) or box[2] > W - 6:
            continue
        if any(not (box[2] < b[0] or box[0] > b[2] or box[3] < b[1] or box[1] > b[3]) for b in placed):
            continue
        placed.append(box)
        d.text((lx, ly), name, font=g_fut, fill=(*SECONDARY, int(255 * k)))


def render(i):
    t = i / FPS
    frame = BG.copy()
    mapk = fade(t, *MAP_IN)
    if mapk > 0:
        frame = over(frame, MAP_A, mapk)
    # live drawing at 2x: the center, the rings, new plugins, lights
    hi = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    hd = ImageDraw.Draw(hi)
    lit = Image.new("RGBA", (W * S, H * S), (0, 0, 0, 0))
    ld = ImageDraw.Draw(lit)
    core_and_ring(hd, ld, t)
    future_layer(hd, t)
    for l in lights:
        l.draw(ld, t)
    frame = over(frame, np.asarray(hi.resize((W, H), Image.LANCZOS), np.float32) / 255.0)
    lit_a = np.asarray(lit.resize((W, H), Image.LANCZOS), np.float32) / 255.0
    rgb = lit_a[..., :3] * lit_a[..., 3:4] * 255.0
    small = Image.fromarray(rgb.clip(0, 255).astype(np.uint8)).resize((W // 4, H // 4), Image.BILINEAR)
    bloom = np.asarray(small.filter(ImageFilter.GaussianBlur(5)).resize((W, H), Image.BILINEAR), np.float32)
    frame = frame + rgb + bloom * 1.6
    frame = over(frame, text_layer(lambda d: (core_labels(d, t), future_labels(d, t, list(PLACED_1X)))))
    # the camera: zoom about the map's center, and pan it to the middle in the hold
    img = Image.fromarray(frame.clip(0, 255).astype(np.uint8))
    z = zcam(t)
    pan = (CX - W / 2) * fade(t, *PAN)
    cw, ch = W / z, H / z
    ox, oy = CX - (CX - pan) / z, CY - CY / z
    img = img.transform((W, H), Image.EXTENT, (ox, oy, ox + cw, oy + ch), Image.BICUBIC, fillcolor=CANVAS)
    # the words on screen, over the camera
    arr = over(np.asarray(img, np.float32), text_layer(lambda d: (caption(d, t), messages(d, t), meters(d, t))))
    arr = arr * VIG * fade(t, 0.0, 0.6)
    noise = np.random.default_rng(i).normal(0, 3.2, (H, W, 1)).astype(np.float32)
    return Image.fromarray((arr + noise).clip(0, 255).astype(np.uint8))



PLACED_1X = [(b[0] / S, b[1] / S, b[2] / S, b[3] / S) for b in PLACED]

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
