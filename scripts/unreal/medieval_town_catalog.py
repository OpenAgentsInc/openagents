#!/usr/bin/env python3
"""Catalogs a Modular Medieval Town export.

Reads what `medieval_town_ue.py` wrote (`meshes.json`, `materials.json`,
`textures.json`, `maps/*.json`) and writes, beside them:

- `catalog.json`: per mesh, its category, triangles per level, bounds in
  meters, how it sits on the kit's 2 m and 4 m grid, material slots, and
  level count; per-category totals; texture and material totals.
- `layout.json`: each demo map's layout: placements per category, the
  buildings (the map's Blueprint house actors) with their footprints in
  their own frames, wall levels, and piece mix, the blocks they form, and
  the open ground between blocks.
- `catalog.md`: the same as a readable summary.

All of it stays in the private output directory. Names and counts may be
quoted in the plan; geometry may not.

Usage: medieval_town_catalog.py EXPORT_DIR
"""

import json
import math
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

# The pack's folders name most categories; names settle the rest.
FOLDERS = [
    ("Demo/", "demo"),
    ("VFX/", "vfx"),
    ("Architecture/MergedMeshes", "merged-building"),
    ("Modules/Basement", "basement"),
    ("Modules/Castle", "castle"),
    ("Modules/Doors", "door"),
    ("Modules/Doorways", "door"),
    ("Modules/Floor_Ceiling", "floor"),
    ("Modules/Interior", "interior-wall"),
    ("Modules/Roof", "roof"),
    ("Modules/Stairs", "stairs"),
    ("Modules/Tower", "tower"),
    ("Modules/Walls_elements", "wall-trim"),
    ("Modules/Walls_", "wall"),
    ("Modules/Windows", "window"),
    ("Modules", "street-paving"),
    ("Props/Organic", "nature"),
]
ARCH_NAMES = [
    ("arch", r"^sm_arch"),
    ("chimney", r"chimney"),
    ("fence", r"fence"),
    ("porch", r"porch"),
    ("roof", r"roof"),
    ("window", r"shutter|window"),
    ("stairs", r"stair"),
    ("column", r"column"),
    ("ground", r"ground|backdrop"),
]
PROP_NAMES = [
    ("prop/market", r"stall|tent|crate|box|bag|sack|basket|barrel|cart|carriage|wagon|bucket"),
    ("prop/fountain", r"fountain"),
    ("prop/lighting", r"lamp|lantern|torch|candle|wax"),
    ("prop/fence", r"fence|curb"),
    ("prop/furniture", r"chair|table|bench|coffin|pot"),
    ("prop/smithy", r"anvil"),
    ("prop/street", r"sign|poster|flag|rope|pipe|board"),
    ("prop/building-part", r"balcony|window|plane"),
    ("prop/water-wheel", r"water_wheel"),
]


def category(rel):
    name = rel.split("/")[-1].lower()
    for folder, cat in FOLDERS:
        if folder in rel:
            return cat
    if "/Props" in rel:
        for cat, pattern in PROP_NAMES:
            if re.search(pattern, name):
                return cat
        return "prop/other"
    if "/Architecture" in rel:
        for cat, pattern in ARCH_NAMES:
            if re.search(pattern, name):
                return cat
        return "architecture-other"
    return "other"


def grid_fit(extent_m):
    """The largest of 4, 2, 1, and 0.5 m that the extent is a multiple of."""
    for step in (4.0, 2.0, 1.0, 0.5):
        if extent_m >= step * 0.95:
            ratio = extent_m / step
            if abs(ratio - round(ratio)) < 0.04:
                return step
    return None


def mesh_entries(meshes):
    entries = []
    for m in meshes:
        lo = [v / 100.0 for v in m["bounds_cm"]["min"]]
        hi = [v / 100.0 for v in m["bounds_cm"]["max"]]
        size = [round(hi[i] - lo[i], 3) for i in range(3)]
        # Unreal: X forward, Y right, Z up. A wall's run is its longer
        # horizontal side.
        run = max(size[0], size[1])
        entries.append(
            {
                "path": m["path"],
                "name": m["path"].split("/")[-1],
                "category": category(m["path"]),
                "triangles": m.get("triangles", []),
                "source_triangles": m.get("glb_triangles"),
                "reduced_glb": m.get("render_glb"),
                "vertices": m.get("vertices", []),
                "lods": m.get("lods"),
                "size_m": size,
                "min_m": [round(v, 3) for v in lo],
                "run_m": round(run, 3),
                "depth_m": round(min(size[0], size[1]), 3),
                "height_m": round(size[2], 3),
                "grid_run": grid_fit(run),
                "grid_height": grid_fit(size[2]),
                "material_slots": [s["slot"] for s in m.get("material_slots", [])],
                "materials": [(s["material"] or "").split(".")[-1] for s in m.get("material_slots", [])],
                "nanite": m.get("nanite"),
                "simple_collision": m.get("simple_collision"),
                "glb": m.get("glb"),
                "export_errors": m.get("export_errors", []),
            }
        )
    return entries


def totals(entries, uses):
    by = defaultdict(
        lambda: {"meshes": 0, "triangles": 0, "max_triangles": 0, "lods": Counter(), "grid": Counter(), "placed": 0}
    )
    for e in entries:
        t = by[e["category"]]
        t["meshes"] += 1
        tri = e["triangles"][0] if e["triangles"] else 0
        t["triangles"] += tri
        t["max_triangles"] = max(t["max_triangles"], tri)
        t["lods"][e["lods"]] += 1
        t["grid"][str(e["grid_run"])] += 1
        t["placed"] += uses.get(e["name"], 0)
    return {k: {**v, "lods": dict(v["lods"]), "grid": dict(v["grid"])} for k, v in sorted(by.items())}


# --- Layout -----------------------------------------------------------------

# Blueprint actors that are whole buildings in the demo town.
BUILDING_CLASS = re.compile(r"^BP_(residential_house|tavern|restaurant|market|blacksmith)")
# Anything wider than this is ground, backdrop, or sky, not town.
TOWN_PIECE_LIMIT_M = 60.0


def rotate(q, v):
    """`v` turned by the unit quaternion `q` = (x, y, z, w)."""
    x, y, z, w = q
    tx = 2 * (y * v[2] - z * v[1])
    ty = 2 * (z * v[0] - x * v[2])
    tz = 2 * (x * v[1] - y * v[0])
    return [
        v[0] + w * tx + (y * tz - z * ty),
        v[1] + w * ty + (z * tx - x * tz),
        v[2] + w * tz + (x * ty - y * tx),
    ]


def yaw(q):
    x, y, z, w = q
    return math.degrees(math.atan2(2 * (w * z + x * y), 1 - 2 * (y * y + z * z)))


def placement_boxes(placed, mesh_index):
    """World-space axis-aligned boxes, meters, one per placed instance."""
    boxes = []
    for p in placed:
        mesh = mesh_index.get(p["mesh"].split(".")[0].split("/Medieval_Town/")[-1])
        if mesh is None:
            continue
        for t in p.get("instances") or [p]:
            loc = [v / 100.0 for v in t["location_cm"]]
            q, s = t["rotation_quat"], t["scale"]
            lo = mesh["min_m"]
            hi = [lo[i] + mesh["size_m"][i] for i in range(3)]
            corners = [
                rotate(q, [x * s[0], y * s[1], z * s[2]])
                for x in (lo[0], hi[0])
                for y in (lo[1], hi[1])
                for z in (lo[2], hi[2])
            ]
            mn = [min(c[i] for c in corners) + loc[i] for i in range(3)]
            mx = [max(c[i] for c in corners) + loc[i] for i in range(3)]
            boxes.append(
                {
                    "mesh": mesh["name"],
                    "category": mesh["category"],
                    "actor": p["actor"],
                    "local_min": lo,
                    "local_size": mesh["size_m"],
                    "location": loc,
                    "quat": q,
                    "scale": s,
                    "min": mn,
                    "max": mx,
                    "size": max(mx[0] - mn[0], mx[1] - mn[1]),
                    "height": mx[2] - mn[2],
                    "yaw": yaw(q),
                }
            )
    return boxes


def local_box(boxes, origin, yaw_deg):
    """The pieces' extent in a frame at `origin` turned by `yaw_deg`."""
    c, s = math.cos(math.radians(-yaw_deg)), math.sin(math.radians(-yaw_deg))
    xs, ys, zs = [], [], []
    for b in boxes:
        lo, size = b["local_min"], b["local_size"]
        for x in (lo[0], lo[0] + size[0]):
            for y in (lo[1], lo[1] + size[1]):
                for z in (lo[2], lo[2] + size[2]):
                    w = rotate(b["quat"], [x * b["scale"][0], y * b["scale"][1], z * b["scale"][2]])
                    dx, dy = w[0] + b["location"][0] - origin[0], w[1] + b["location"][1] - origin[1]
                    xs.append(dx * c - dy * s)
                    ys.append(dx * s + dy * c)
                    zs.append(w[2] + b["location"][2])
    return [min(xs), min(ys), min(zs)], [max(xs), max(ys), max(zs)]


def obb_corners(origin, yaw_deg, lo, hi):
    c, s = math.cos(math.radians(yaw_deg)), math.sin(math.radians(yaw_deg))
    return [
        (origin[0] + x * c - y * s, origin[1] + x * s + y * c)
        for x, y in ((lo[0], lo[1]), (hi[0], lo[1]), (hi[0], hi[1]), (lo[0], hi[1]))
    ]


def point_in(poly, p):
    inside = False
    for i in range(len(poly)):
        a, b = poly[i], poly[(i + 1) % len(poly)]
        if (a[1] > p[1]) != (b[1] > p[1]):
            x = a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1])
            if p[0] < x:
                inside = not inside
    return inside


def segment_distance(p, a, b):
    ax, ay = b[0] - a[0], b[1] - a[1]
    t = max(0.0, min(1.0, ((p[0] - a[0]) * ax + (p[1] - a[1]) * ay) / (ax * ax + ay * ay or 1)))
    return math.hypot(p[0] - a[0] - t * ax, p[1] - a[1] - t * ay)


def polygon_distance(p, q):
    if any(point_in(q, v) for v in p) or any(point_in(p, v) for v in q):
        return 0.0
    best = float("inf")
    for poly, other in ((p, q), (q, p)):
        for v in poly:
            for i in range(len(other)):
                best = min(best, segment_distance(v, other[i], other[(i + 1) % len(other)]))
    return best


def buildings_of(data, boxes_by_actor):
    """One record per Blueprint building: its type, pieces, and footprint
    in its own frame."""
    out = []
    for actor in data.get("actors", []):
        if not BUILDING_CLASS.match(actor["class"]):
            continue
        boxes = [b for b in boxes_by_actor.get(actor["actor"], []) if b["size"] < TOWN_PIECE_LIMIT_M]
        if not boxes:
            continue
        origin = [v / 100.0 for v in actor["location_cm"]]
        yaw_deg = actor["rotation_rpy_deg"][2]
        shell = [b for b in boxes if not b["category"].startswith("prop") and b["category"] != "nature"]
        lo, hi = local_box(shell or boxes, origin, yaw_deg)
        walls = [b for b in boxes if b["category"] in ("wall", "window", "door") and b["height"] >= 3.5]
        levels = sorted({round(b["min"][2] - lo[2], 1) for b in walls})
        out.append(
            {
                "actor": actor["actor"],
                "type": actor["class"][3:-2] if actor["class"].endswith("_C") else actor["class"],
                "origin_m": [round(v, 2) for v in origin],
                "yaw_deg": round(yaw_deg, 1),
                "pieces": len(boxes),
                "footprint_m": [round(hi[0] - lo[0], 2), round(hi[1] - lo[1], 2)],
                "height_m": round(hi[2] - lo[2], 2),
                "wall_levels_m": levels,
                "categories": dict(Counter(b["category"] for b in boxes).most_common()),
                "polygon": obb_corners(origin, yaw_deg, lo, hi),
            }
        )
    return out


def blocks_of(buildings, touch=1.0):
    """Groups buildings whose footprints come within `touch` m of each
    other: a terrace or a block."""
    parent = list(range(len(buildings)))

    def find(i):
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i

    for i in range(len(buildings)):
        for j in range(i + 1, len(buildings)):
            if polygon_distance(buildings[i]["polygon"], buildings[j]["polygon"]) <= touch:
                parent[find(i)] = find(j)
    groups = defaultdict(list)
    for i in range(len(buildings)):
        groups[find(i)].append(buildings[i])
    blocks = []
    for members in groups.values():
        pts = [p for b in members for p in b["polygon"]]
        xs, ys = [p[0] for p in pts], [p[1] for p in pts]
        blocks.append(
            {
                "buildings": len(members),
                "extent_m": [round(max(xs) - min(xs), 1), round(max(ys) - min(ys), 1)],
                "polygons": [b["polygon"] for b in members],
            }
        )
    blocks.sort(key=lambda b: -b["buildings"])
    return blocks


def street_widths(blocks):
    """The open distance from each block to the nearest other block, m: the
    street or lane between them."""
    widths = []
    for i, a in enumerate(blocks):
        best = float("inf")
        for j, b in enumerate(blocks):
            if i != j:
                for p in a["polygons"]:
                    for q in b["polygons"]:
                        best = min(best, polygon_distance(p, q))
        if best < float("inf"):
            widths.append(round(best, 1))
    return sorted(widths)


def percentiles(values, points=(0, 10, 25, 50, 75, 90, 100)):
    if not values:
        return {}
    v = sorted(values)
    return {p: round(v[min(len(v) - 1, int(round(p / 100 * (len(v) - 1))))], 1) for p in points}


def layout(maps_dir, entries):
    index = {e["path"]: e for e in entries}
    result = {}
    for path in sorted(maps_dir.glob("*.json")):
        data = json.loads(path.read_text())
        boxes = placement_boxes(data["placed"], index)
        if not boxes:
            result[path.stem] = {"placements": 0}
            continue
        town = [b for b in boxes if b["size"] < TOWN_PIECE_LIMIT_M]
        by_actor = defaultdict(list)
        for b in boxes:
            by_actor[b["actor"]].append(b)
        buildings = buildings_of(data, by_actor)
        blocks = blocks_of(buildings)
        if buildings:
            centers = [b["origin_m"] for b in buildings]
        else:
            centers = [[(b["min"][0] + b["max"][0]) / 2, (b["min"][1] + b["max"][1]) / 2] for b in town]
        types = defaultdict(list)
        for b in buildings:
            types[b["type"]].append(b)
        in_buildings = sum(b["pieces"] for b in buildings)
        result[path.stem] = {
            "placements": len(boxes),
            "town_placements": len(town),
            "placements_in_buildings": in_buildings,
            "outsized_placements": dict(Counter(b["mesh"] for b in boxes if b["size"] >= TOWN_PIECE_LIMIT_M)),
            "actor_classes": data.get("actor_classes", {}),
            "town_extent_m": [
                round(max(c[0] for c in centers) - min(c[0] for c in centers), 1),
                round(max(c[1] for c in centers) - min(c[1] for c in centers), 1),
            ],
            "by_category": dict(Counter(b["category"] for b in town).most_common()),
            "top_meshes": dict(Counter(b["mesh"] for b in town).most_common(40)),
            "buildings": len(buildings),
            "building_types": {
                t: {
                    "count": len(v),
                    "pieces": v[0]["pieces"],
                    "footprint_m": v[0]["footprint_m"],
                    "height_m": v[0]["height_m"],
                    "wall_levels_m": v[0]["wall_levels_m"],
                    "categories": v[0]["categories"],
                }
                for t, v in sorted(types.items())
            },
            "building_yaw_mod_90": dict(Counter(round(b["yaw_deg"] % 90.0) for b in buildings).most_common(12)),
            "footprint_percentiles_m2": percentiles([b["footprint_m"][0] * b["footprint_m"][1] for b in buildings]),
            "height_percentiles_m": percentiles([b["height_m"] for b in buildings]),
            "pieces_percentiles": percentiles([b["pieces"] for b in buildings]),
            "blocks": [{"buildings": b["buildings"], "extent_m": b["extent_m"]} for b in blocks],
            "street_width_percentiles_m": percentiles(street_widths(blocks)),
        }
    return result


# --- Report -----------------------------------------------------------------


def markdown(catalog, lay):
    s = catalog["summary"]
    lines = [
        "# Modular Medieval Town catalog (private)",
        "",
        f"- Static meshes: {s['meshes']} ({s['exported']} exported to glb, {s['failed']} failed)",
        f"- Textures: {s['textures']} ({s['texture_pngs']} PNG, {s['texture_exrs']} EXR); sizes {s['texture_sizes']}",
        f"- Materials: {s['materials']} ({s['master_materials']} masters, {s['instances']} instances)",
        f"- Triangles at LOD0, all meshes: {s['triangles']} as Unreal renders them, "
        f"{s['source_triangles']} in the source models; {s['reduced_lod0']} meshes render a reduced LOD0",
        "",
        "| Category | Meshes | Triangles | Largest | Placed in town | LOD counts | Run on grid |",
        "| --- | ---: | ---: | ---: | ---: | --- | --- |",
    ]
    for cat, t in catalog["categories"].items():
        lines.append(
            f"| {cat} | {t['meshes']} | {t['triangles']} | {t['max_triangles']} | {t['placed']} | {t['lods']} | {t['grid']} |"
        )
    lines += ["", "## Demo maps", ""]
    for name, m in lay.items():
        lines.append(f"### {name}")
        for key in (
            "placements", "town_placements", "placements_in_buildings", "outsized_placements",
            "town_extent_m", "buildings", "building_yaw_mod_90", "footprint_percentiles_m2",
            "height_percentiles_m", "pieces_percentiles", "street_width_percentiles_m", "by_category",
        ):
            if key in m:
                lines.append(f"- {key}: {m[key]}")
        if m.get("blocks"):
            lines.append(f"- blocks (buildings, extent m): {[(b['buildings'], b['extent_m']) for b in m['blocks']]}")
        for t, v in m.get("building_types", {}).items():
            lines.append(f"- type {t}: {v}")
        lines.append("")
    lines += [
        "## Meshes",
        "",
        "| Mesh | Category | Tris LOD0 | Tris source | LODs | Size (m) | Grid | Slots |",
        "| --- | --- | ---: | ---: | ---: | --- | --- | ---: |",
    ]
    for e in catalog["meshes"]:
        tri = e["triangles"][0] if e["triangles"] else 0
        lines.append(
            f"| {e['name']} | {e['category']} | {tri} | {e['source_triangles']} | {e['lods']} | {e['size_m']} "
            f"| {e['grid_run']} | {len(e['material_slots'])} |"
        )
    return "\n".join(lines) + "\n"


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    out = Path(sys.argv[1]).expanduser()
    meshes = json.loads((out / "meshes.json").read_text())
    textures = json.loads((out / "textures.json").read_text()) if (out / "textures.json").exists() else []
    materials = json.loads((out / "materials.json").read_text()) if (out / "materials.json").exists() else []
    entries = mesh_entries(meshes)
    lay = layout(out / "maps", entries) if (out / "maps").is_dir() else {}
    uses = Counter()
    town = next((json.loads(p.read_text()) for p in (out / "maps").glob("Maps__medieval_town.json")), None)
    if town:
        for p in town["placed"]:
            uses[p["mesh"].split(".")[-1]] += len(p.get("instances") or [p])
    catalog = {
        "summary": {
            "meshes": len(entries),
            "exported": sum(1 for e in entries if e["glb"]),
            "failed": sum(1 for e in entries if not e["glb"]),
            "triangles": sum(e["triangles"][0] for e in entries if e["triangles"]),
            "source_triangles": sum(e["source_triangles"] or 0 for e in entries),
            "reduced_lod0": sum(1 for e in entries if e["reduced_glb"]),
            "textures": len(textures),
            "texture_pngs": sum(1 for t in textures if t.get("png")),
            "texture_exrs": sum(1 for t in textures if t.get("exr")),
            "texture_sizes": dict(Counter(f"{t.get('width')}x{t.get('height')}" for t in textures).most_common()),
            "materials": len(materials),
            "master_materials": sum(1 for m in materials if m["class"] == "Material"),
            "instances": sum(1 for m in materials if m["class"] != "Material"),
        },
        "categories": totals(entries, uses),
        "meshes": entries,
    }
    (out / "catalog.json").write_text(json.dumps(catalog, indent=1))
    (out / "layout.json").write_text(json.dumps(lay, indent=1))
    (out / "catalog.md").write_text(markdown(catalog, lay))
    print(f"catalog: {out / 'catalog.md'}")


if __name__ == "__main__":
    main()
