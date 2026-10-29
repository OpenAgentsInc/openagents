#!/usr/bin/env python3
"""Everglade's map and walking times, drawn from the blockout's numbers.

Reads the arrays in verse/everglade/plan.coax (the one source of the city's
layout) and writes docs/verse/everglade-map.svg, or with --times prints the
walking times the plan doc tables (verse/player.coax walks at 3.5 m/s).

  python3 verse/everglade/map.py docs/verse/everglade-map.svg
  python3 verse/everglade/map.py --times
"""
import heapq
import math
import random
import re
import sys
from pathlib import Path

PLAN = Path(__file__).with_name("plan.coax")
WALK = 3.5


def arrays(text):
    out = {}
    for m in re.finditer(r"^(EG_[A-Z_]+) :: \[\d+\] (f32|int|string|bool)\.\[(.*?)\];", text, re.S | re.M):
        name, kind, body = m.groups()
        if kind == "string":
            out[name] = re.findall(r'"([^"]*)"', body)
        elif kind == "bool":
            out[name] = [w == "true" for w in re.findall(r"true|false", body)]
        else:
            out[name] = [float(w) for w in re.findall(r"-?\d+(?:\.\d+)?", re.sub(r"//.*", "", body))]
    for m in re.finditer(r"^(EG_[A-Z_]+) :: (-?\d+(?:\.\d+)?);", text, re.M):
        out[m.group(1)] = float(m.group(2))
    return out


def groups(xs, n):
    return [xs[i:i + n] for i in range(0, len(xs), n)]


def walk_graph(a):
    nodes = groups(a["EG_NODE_XZ"], 2)
    links = [(int(p), int(q)) for p, q in groups(a["EG_LINK_AB"], 2)]
    near = {k: [] for k in range(len(nodes))}
    for p, q in links:
        d = math.dist(nodes[p], nodes[q])
        near[p].append((q, d))
        near[q].append((p, d))
    return nodes, links, near


def distances(near, start):
    best = {start: 0.0}
    todo = [(0.0, start)]
    while todo:
        d, k = heapq.heappop(todo)
        if d > best[k]:
            continue
        for q, w in near[k]:
            if d + w < best.get(q, 1e18):
                best[q] = d + w
                heapq.heappush(todo, (d + w, q))
    return best


def walk_to(nodes, best, x, z):
    """The shortest walk to (x, z): by the streets to a node, then straight over."""
    return min(best[k] + math.dist(nodes[k], (x, z)) for k in best)


def times(a):
    nodes, links, near = walk_graph(a)
    gate = int(a["EG_SOUTH_GATE_NODE"])
    fountain = 0
    from_gate = distances(near, gate)
    from_commons = distances(near, fountain)
    rows = []
    doors = groups(a["EG_DOOR"], 2)
    for name, (x, z) in zip(a["EG_BUILDING_NAME"], doors):
        if name == "HOUSE":
            continue
        rows.append((name.title(), walk_to(nodes, from_gate, x, z), walk_to(nodes, from_commons, x, z)))
    for name, (x, z) in zip(a["EG_PLACE_NAME"], groups(a["EG_PLACE"], 2)):
        if name in ("THE DEEP WOOD", "THE RUINS", "THE CRYPT", "THE RIVER CROSSING", "THE ROAD IN", "NORTH GATE"):
            rows.append((name.title(), walk_to(nodes, from_gate, x, z), walk_to(nodes, from_commons, x, z)))
    print("| Place | From the South Gate | From the fountain |")
    print("| --- | --- | --- |")
    for name, g, c in rows:
        print(f"| {name} | {g:.0f} m, {fmt(g / WALK)} | {c:.0f} m, {fmt(c / WALK)} |")


def fmt(seconds):
    s = int(round(seconds / 5) * 5)
    return f"{s} s" if s < 60 else f"{s // 60} min {s % 60:02d} s"


def svg(a):
    S = 4.0                     # pixels a metre
    W = H = int(208 * S)
    pad_top = 70
    def X(x): return (104 - x) * S          # +x is west: the map's left
    def Y(z): return (104 - z) * S + pad_top
    o = []
    o.append(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H + pad_top + 60}" width="{W}" height="{H + pad_top + 60}" font-family="Georgia, serif">')
    o.append("<title>Everglade: the forest city's plan</title>")
    o.append(f'<rect width="{W}" height="{H + pad_top + 60}" fill="#eadfc4"/>')
    o.append(f'<rect x="{X(100)}" y="{Y(100)}" width="{200 * S}" height="{200 * S}" fill="#cfd8a6" stroke="#8a7a52" stroke-width="2"/>')
    # The forest ring: trees everywhere past the palisade but the trails, the river and the landmarks.
    nodes, links, near = walk_graph(a)
    rnd = random.Random(7)
    props = groups(a["EG_PROP"], 5)
    buildings = groups(a["EG_BUILDING"], 5)
    def seg_d(p, q, x, z):
        (ax, az), (bx, bz) = p, q
        dx, dz = bx - ax, bz - az
        t = max(0.0, min(1.0, ((x - ax) * dx + (z - az) * dz) / (dx * dx + dz * dz)))
        return math.dist((ax + dx * t, az + dz * t), (x, z))
    def box_d(b, x, z):
        return math.hypot(max(b[0] - x, x - b[2], 0), max(b[1] - z, z - b[3], 0))
    trees = []
    for _ in range(2600):
        x, z = rnd.uniform(-99, 99), rnd.uniform(-99, 99)
        if -70 < x < 70 and -62 < z < 66:
            continue
        if min(seg_d(nodes[p], nodes[q], x, z) for p, q in links) < 5:
            continue
        if any(box_d(b, x, z) < 4 for b in props + buildings):
            continue
        if any(math.dist(t[:2], (x, z)) < 3.2 for t in trees):
            continue
        trees.append((x, z, rnd.uniform(1.6, 2.6)))
    for x, z, r in trees:
        o.append(f'<circle cx="{X(x):.1f}" cy="{Y(z):.1f}" r="{r * S:.1f}" fill="#6f8f4e" stroke="#4f6b36" stroke-width="1"/>')
    # Inside the walls: the city's ground.
    o.append(f'<rect x="{X(63.6)}" y="{Y(59.6)}" width="{127.2 * S}" height="{115.2 * S}" fill="#e2d6b0"/>')
    for x, z, r, _ in groups(a["EG_SQUARE"], 4):
        o.append(f'<circle cx="{X(x)}" cy="{Y(z)}" r="{r * S}" fill="#d6c7a0" stroke="#b9a77c" stroke-width="1.5"/>')
    # The streets and trails.
    half = a["EG_PATH_HALF"]
    for p, q in links:
        (ax, az), (bx, bz) = nodes[p], nodes[q]
        o.append(f'<line x1="{X(ax):.1f}" y1="{Y(az):.1f}" x2="{X(bx):.1f}" y2="{Y(bz):.1f}" stroke="#b08a5a" stroke-width="{2 * half * S}" stroke-linecap="round"/>')
    # The river and the bridge.
    for b, name in zip(props, a["EG_PROP_NAME"]):
        if name == "RIVER":
            o.append(f'<rect x="{X(b[2])}" y="{Y(b[3])}" width="{(b[2] - b[0]) * S}" height="{(b[3] - b[1]) * S}" fill="#7fa7c0"/>')
    br = a["EG_BRIDGE"]
    o.append(f'<rect x="{X(br[2])}" y="{Y(br[3])}" width="{(br[2] - br[0]) * S}" height="{(br[3] - br[1]) * S}" fill="#8a6a44" stroke="#5a4428"/>')
    # The palisade.
    for b in groups(a["EG_PALISADE"], 5):
        o.append(f'<rect x="{X(b[2]):.1f}" y="{Y(b[3]):.1f}" width="{(b[2] - b[0]) * S:.1f}" height="{(b[3] - b[1]) * S:.1f}" fill="#5a3f24"/>')
    # The buildings and the props.
    for b, name in zip(buildings, a["EG_BUILDING_NAME"]):
        fill = "#d9c2a0" if name == "HOUSE" else "#c79a6a"
        o.append(f'<rect x="{X(b[2])}" y="{Y(b[3])}" width="{(b[2] - b[0]) * S}" height="{(b[3] - b[1]) * S}" fill="{fill}" stroke="#5a4428" stroke-width="2"/>')
        if name != "HOUSE":
            cx, cz = (b[0] + b[2]) / 2, (b[1] + b[3]) / 2
            o.append(label(X(cx), Y(cz) + 4, name.title(), 12, "#3a2a18"))
    for (x, z) in groups(a["EG_DOOR"], 2):
        o.append(f'<rect x="{X(x) - 3}" y="{Y(z) - 3}" width="6" height="6" fill="#3a2a18"/>')
    for b, name in zip(props, a["EG_PROP_NAME"]):
        if name == "RIVER":
            continue
        if name == "FOUNTAIN":
            o.append(f'<circle cx="{X(0)}" cy="{Y(0)}" r="{3 * S}" fill="#9fc0d0" stroke="#5a6a70" stroke-width="2"/>')
            continue
        o.append(f'<rect x="{X(b[2])}" y="{Y(b[3])}" width="{(b[2] - b[0]) * S}" height="{(b[3] - b[1]) * S}" fill="#a08a6a" stroke="#5a4428"/>')
    # Where the characters stand.
    for (x, z, _), cat in zip(groups(a["EG_SPOT"], 3), a["EG_SPOT_CAT"]):
        fill = "#e0a030" if cat else "#a02828"
        o.append(f'<circle cx="{X(x):.1f}" cy="{Y(z):.1f}" r="4" fill="{fill}" stroke="#fff6e0" stroke-width="1.2"/>')
    # The districts and the trails' ends.
    for (x, z), name in zip(groups(a["EG_PLACE"], 2), a["EG_PLACE_NAME"]):
        o.append(label(X(x), Y(z) - 14, name, 15, "#2a1e10", bold=True))
    for g, name in zip(groups(a["EG_GATE"], 4), a["EG_GATE_NAME"]):
        if name in ("SOUTH GATE", "NORTH GATE"):
            continue
        dx = 16 if g[0] < 0 else -16
        o.append(label(X(g[0]) + dx * 3, Y(g[1]) - 14, name, 13, "#2a1e10", bold=True))
    # The title, the scale and the key.
    o.append(label(W / 2, 44, "EVERGLADE · the forest city", 30, "#2a1e10", bold=True))
    y = H + pad_top + 30
    x0 = 30
    o.append(f'<rect x="{x0}" y="{y - 6}" width="{35 * S}" height="8" fill="#2a1e10"/>')
    o.append(f'<rect x="{x0 + 35 * S}" y="{y - 6}" width="{35 * S}" height="8" fill="#fff6e0" stroke="#2a1e10"/>')
    o.append(f'<text x="{x0}" y="{y + 22}" font-size="13" fill="#2a1e10">0</text>')
    o.append(f'<text x="{x0 + 70 * S}" y="{y + 22}" font-size="13" fill="#2a1e10" text-anchor="end">70 m: twenty seconds on foot</text>')
    kx = x0 + 70 * S + 40
    o.append(f'<circle cx="{kx}" cy="{y - 2}" r="5" fill="#a02828"/><text x="{kx + 10}" y="{y + 3}" font-size="13" fill="#2a1e10">a character</text>')
    o.append(f'<circle cx="{kx + 110}" cy="{y - 2}" r="5" fill="#e0a030"/><text x="{kx + 120}" y="{y + 3}" font-size="13" fill="#2a1e10">a cat</text>')
    o.append(f'<rect x="{kx + 175}" y="{y - 5}" width="7" height="7" fill="#3a2a18"/><text x="{kx + 188}" y="{y + 3}" font-size="13" fill="#2a1e10">a door</text>')
    nx, ny = W - 50, H + pad_top + 22
    o.append(f'<path d="M{nx},{ny - 26} L{nx + 9},{ny} L{nx},{ny - 6} L{nx - 9},{ny} Z" fill="#2a1e10"/><text x="{nx}" y="{ny + 18}" font-size="14" fill="#2a1e10" text-anchor="middle">N</text>')
    o.append("</svg>")
    return "\n".join(o) + "\n"


def label(x, y, text, size, fill, bold=False):
    half = len(text) * size * 0.36
    x = min(max(x, half + 8), 208 * 4.0 - half - 8)
    text = text.replace("&", "&amp;").replace("'", "&#39;")
    weight = ' font-weight="bold"' if bold else ""
    return (f'<text x="{x:.1f}" y="{y:.1f}" font-size="{size}" fill="{fill}" text-anchor="middle"{weight} '
            f'stroke="#eadfc4" stroke-width="3" paint-order="stroke" letter-spacing="0.5">{text}</text>')


def main():
    a = arrays(PLAN.read_text())
    if sys.argv[1:] == ["--times"]:
        times(a)
        return
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    Path(sys.argv[1]).write_text(svg(a))


if __name__ == "__main__":
    main()
