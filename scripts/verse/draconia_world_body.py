"""Draconia's lower body for the walkable castle world (`docs/verse/draconia/world.md`).

The parity builds (`build-rig-*/mh.<expr>.glb`) stop at the robe's hem,
about 0.7 m up: they are framed chest-up. Walking round her needs the rest,
so this script builds it beside them, never touching them:

- her MetaHuman body's legs, hips and hands (the newest
  `rig-body-down/body_lod0_mesh.obj`, arms lowered), the legs in dark
  tights and the hands in MetaHuman's body skin;
- black leather shoes, a shell over her feet;
- a pleated charcoal skirt from under the sweater to the knee;
- the robe carried down from its hem to the shin, following the hem's own
  outline, open where the robe is open.

Run headless (the world's launcher runs it when its inputs are newer):

    Blender -b --factory-startup --python scripts/verse/draconia_world_body.py -- \
        [--model mh.neutral.glb] [--body body_lod0_mesh.obj] [--out draconia_lower.glb]

Writes `~/work/film-out/draconia/world/draconia_lower.glb`. Private
MetaHuman asset: never commit it and never use it or its renders for AI
training.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Vector

HOME = os.path.expanduser("~/work/film-out/draconia")
MH = os.path.join(HOME, "metahuman")


def arg(name, default):
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    if name in argv:
        return argv[argv.index(name) + 1]
    return default


def newest(paths):
    paths = [p for p in paths if os.path.exists(p)]
    return max(paths, key=os.path.getmtime) if paths else None


MODEL = arg(
    "--model",
    newest(
        [
            os.path.join(MH, "latest/mh.neutral.glb"),
            os.path.join(MH, "build-rig-neutral/mh.neutral.glb"),
        ]
    ),
)
BODY = arg(
    "--body",
    newest(
        [
            os.path.join(MH, d, "body_lod0_mesh.obj")
            for d in ("rig-body-down", "archetype-body-down")
        ]
    ),
)
SKIN = os.path.join(MH, "export/dcc/Draconia/Maps/Body_Basecolor.png")
OUT = arg("--out", os.path.join(HOME, "world/draconia_lower.glb"))

# Heights in Blender's frame (Z up, metres; she faces -Y).
BODY_TOP = 0.97  # the body is kept below this; the sweater ends at 0.89
SKIRT_TOP = 0.95
SKIRT_HEM = 0.47
ROBE_TOP = 0.82  # inside the parity robe, above its ragged hem (0.70)
ROBE_HEM = 0.17
BINS = 96


def material(name, color, rough, shading, params, image=None):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = color
    b.inputs["Roughness"].default_value = rough
    if image:
        t = m.node_tree.nodes.new("ShaderNodeTexImage")
        t.image = bpy.data.images.load(image)
        m.node_tree.links.new(t.outputs["Color"], b.inputs["Base Color"])
    m["verse_shading"] = shading
    m["verse_shading_params"] = params
    return m


def clear():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def hem_outline():
    """The parity robe's radius at its hem, per angle bin about the body's
    axis, or None where the robe is open; and that axis."""
    bpy.ops.import_scene.gltf(filepath=MODEL)
    robe = [o for o in bpy.context.scene.objects if o.type == "MESH" and o.name.startswith("robe")]
    if not robe:
        raise SystemExit(f"{MODEL}: no robe mesh")
    pts = []
    for o in robe:
        for v in o.data.vertices:
            p = o.matrix_world @ v.co
            if 0.69 < p.z < 0.80:
                pts.append(p)
    for o in list(bpy.context.scene.objects):
        bpy.data.objects.remove(o)
    cx = sum(p.x for p in pts) / len(pts)
    cy = sum(p.y for p in pts) / len(pts)
    radius = [None] * BINS
    for p in pts:
        a = math.atan2(p.y - cy, p.x - cx)
        k = int((a + math.pi) / math.tau * BINS) % BINS
        r = math.hypot(p.x - cx, p.y - cy)
        radius[k] = max(radius[k] or 0.0, r)
    # Fill single-bin holes; the front opening stays open.
    for k in range(BINS):
        if radius[k] is None and radius[k - 1] and radius[(k + 1) % BINS]:
            radius[k] = 0.5 * (radius[k - 1] + radius[(k + 1) % BINS])
    return radius, (cx, cy)


def body_part():
    bpy.ops.wm.obj_import(filepath=BODY, forward_axis="NEGATIVE_Z", up_axis="Y", global_scale=0.01)
    body = bpy.context.selected_objects[0]
    body.name = "lower_body"
    bpy.context.view_layer.objects.active = body
    # The importer's axis turn and centimetre scale go into the vertices.
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    bm = bmesh.new()
    bm.from_mesh(body.data)
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if v.co.z > BODY_TOP], context="VERTS")
    body.data.materials.clear()
    tights = material("draconia_tights", (0.018, 0.017, 0.02, 1), 0.55, "cloth", [0.45, 0.4, 0, 0])
    skin = material(
        "draconia_body_skin",
        (0.6, 0.5, 0.45, 1),
        0.5,
        "skin",
        [0.85, 0.85, 0.5, 0],
        SKIN if os.path.exists(SKIN) else None,
    )
    body.data.materials.append(tights)
    body.data.materials.append(skin)
    for f in bm.faces:
        c = f.calc_center_median()
        f.material_index = 1 if abs(c.x) > 0.185 and c.z > 0.5 else 0
        f.smooth = True
    bm.to_mesh(body.data)
    bm.free()
    return body


def shoes(body):
    """A plain shoe on each foot: the convex hull of the foot below the
    ankle, lifted a little off the skin, its sole flat on the floor."""
    bm = bmesh.new()
    for side in (-1, 1):
        pts = [v.co.copy() for v in body.data.vertices if v.co.z < 0.1 and v.co.x * side > 0]
        verts = [bm.verts.new(p) for p in pts]
        bmesh.ops.convex_hull(bm, input=verts)
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    bmesh.ops.subdivide_edges(bm, edges=bm.edges, cuts=1, use_grid_fill=True)
    for _ in range(4):
        bmesh.ops.smooth_vert(bm, verts=bm.verts, factor=0.5, use_axis_x=True, use_axis_y=True, use_axis_z=True)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.normal_update()
    for v in bm.verts:
        v.co += v.normal * 0.006
        v.co.z = max(v.co.z, 0.0)
    uv = bm.loops.layers.uv.new()
    for f in bm.faces:
        for loop in f.loops:
            loop[uv].uv = (loop.vert.co.x * 4.0, loop.vert.co.y * 4.0)
    me = bpy.data.meshes.new("shoes")
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new("shoes", me)
    bpy.context.scene.collection.objects.link(o)
    me.materials.append(material("draconia_shoes", (0.01, 0.009, 0.009, 1), 0.28, "standard", [0, 0, 0, 0]))
    for p in me.polygons:
        p.use_smooth = True
    return o


def band(name, radius_at, top, hem, mat, segments=BINS, rows=24, open_bins=None, thick=0.003, uv_scale=3.0):
    """A hanging cloth band round the body's axis: `radius_at(k, t)` gives
    bin k's radius at t (0 at the top, 1 at the hem); bins in `open_bins`
    are left out (the robe's front opening)."""
    bm = bmesh.new()
    grid = {}
    for k in range(segments + 1):
        kk = k % segments
        a = -math.pi + (k / segments) * math.tau
        for j in range(rows + 1):
            t = j / rows
            r = radius_at(kk, t)
            z = top + (hem - top) * t
            v = bm.verts.new((math.cos(a) * r, math.sin(a) * r, z))
            grid[(k, j)] = v
    uv = bm.loops.layers.uv.new()
    for k in range(segments):
        if open_bins and (k in open_bins):
            continue
        for j in range(rows):
            f = bm.faces.new([grid[(k, j)], grid[(k + 1, j)], grid[(k + 1, j + 1)], grid[(k, j + 1)]])
            f.smooth = True
            for loop in f.loops:
                co = loop.vert.co
                loop[uv].uv = (math.atan2(co.y, co.x) * 0.25 * uv_scale, co.z * uv_scale)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-5)
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    bm.normal_update()
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    me.materials.append(mat)
    # Outward normals (the band was built counter-clockwise from above).
    bpy.context.view_layer.objects.active = o
    o.select_set(True)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.normals_make_consistent(inside=False)
    bpy.ops.object.mode_set(mode="OBJECT")
    s = o.modifiers.new("solid", "SOLIDIFY")
    s.thickness = thick
    s.offset = -1.0
    bpy.ops.object.modifier_apply(modifier=s.name)
    o.select_set(False)
    return o


def ellipse(body, lo, hi):
    """The centre, half-width and half-depth of her hips between heights
    lo and hi. The hanging arms are left out: the width is measured near
    the body's mid-plane and the depth near its centre line."""
    pts = [v.co for v in body.data.vertices if lo < v.co.z < hi and abs(v.co.x) < 0.07]
    cy = 0.5 * (min(p.y for p in pts) + max(p.y for p in pts))
    b = max(abs(p.y - cy) for p in pts)
    a = max(abs(v.co.x) for v in body.data.vertices if lo < v.co.z < hi and abs(v.co.y - cy) < 0.05 and abs(v.co.x) < 0.2)
    return (0.0, cy), a, b


def radius(a, b, k):
    """The radius of an ellipse of half-axes a (x) and b (y) at bin k."""
    t = -math.pi + (k / BINS) * math.tau
    return a * b / math.hypot(b * math.cos(t), a * math.sin(t))


def main():
    clear()
    hem, center = hem_outline()
    body = body_part()
    shoes(body)
    # The feet inside the shoes go (they would show through at the toes).
    bm = bmesh.new()
    bm.from_mesh(body.data)
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if v.co.z < 0.075], context="VERTS")
    bm.to_mesh(body.data)
    bm.free()
    center, ha, hb = ellipse(body, 0.78, SKIRT_TOP)
    cx, cy = center
    print(f"hips: centre {center}, half-width {ha:.3f}, half-depth {hb:.3f}")
    skirt = material("draconia_skirt", (0.03, 0.03, 0.034, 1), 0.7, "cloth", [0.5, 0.5, 0, 0])

    def skirt_r(k, t):
        a = -math.pi + (k / BINS) * math.tau
        pleat = 0.006 * abs(math.sin(a * 14))
        return radius(ha + 0.015, hb + 0.02, k) + 0.06 * t * t + pleat

    s = band("skirt", skirt_r, SKIRT_TOP, SKIRT_HEM, skirt, rows=16, thick=0.003)
    s.location = (cx, cy, 0)
    robe_mat = material("draconia_robe_lower", (0.012, 0.012, 0.014, 1), 0.75, "cloth", [0.55, 0.5, 0, 0])
    open_bins = {k for k in range(BINS) if hem[k] is None}

    def robe_r(k, t):
        a = -math.pi + (k / BINS) * math.tau
        folds = (0.004 + 0.016 * t) * math.sin(a * 9 + 0.7 * math.sin(a * 4))
        return radius(ha + 0.07, hb + 0.07, k) + 0.07 * t**1.5 + folds

    r = band("robe_lower", robe_r, ROBE_TOP, ROBE_HEM, robe_mat, rows=28, open_bins=open_bins, thick=0.004)
    r.location = (cx, cy, 0)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.export_scene.gltf(
        filepath=OUT,
        export_format="GLB",
        use_selection=True,
        export_extras=True,
        export_apply=True,
        export_yup=True,
    )
    print(f"wrote {OUT} (body {BODY}, robe hem from {MODEL}, open bins {len(open_bins)})")


main()
