"""Render every generated model in one labeled gallery, plus a preview each.

Run headless:
    Blender -b --factory-startup --python scripts/blender/gallery.py -- [MODELS_DIR] [OUT_DIR] [NAME ...]

MODELS_DIR defaults to assets/verse/generated and OUT_DIR to
$TMPDIR/verse-models-gallery. Every `.glb` in MODELS_DIR and its subfolders,
such as `buildings/` (or only the named ones), is imported, posed on its
`idle` clip when it has one, scaled so it reads in a grid cell, and labeled with a Blender text object naming it, its
real size, and the gallery scale. Writes `gallery.png` (the overview),
`previews/<name>.png` (each model alone at its real proportions), and
`contact_sheet.png` (the previews in a grid).
"""

import glob
import math
import os
import sys
import tempfile

import bpy
import numpy as np
from mathutils import Vector

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

CELL = 5.0  # grid spacing across, in gallery units
ROW = 8.0  # grid spacing front to back, leaving room for labels
FIT = 3.0  # each model's largest dimension in the gallery, at most
SKY = (0.53, 0.72, 0.92)
GROUND = (0.2, 0.33, 0.14)


def pose_idle(objs, actions):
    for o in objs:
        if o.type != "ARMATURE" or not o.animation_data:
            continue
        ad = o.animation_data
        for track in ad.nla_tracks:
            track.mute = True
        acts = [a for a in actions if a.name.startswith("idle")]
        if acts:
            ad.action = acts[0]
            # Some importers bind actions to slots; take the first slot.
            if hasattr(ad, "action_slot") and ad.action_slot is None and acts[0].slots:
                ad.action_slot = acts[0].slots[0]


def bounds(objs):
    dg = bpy.context.evaluated_depsgraph_get()
    pts = []
    for o in objs:
        if o.type not in ("MESH", "FONT") or o.hide_render:
            continue
        e = o.evaluated_get(dg)
        me = e.to_mesh()
        pts += [e.matrix_world @ v.co for v in me.vertices]
        e.to_mesh_clear()
    lo = Vector([min(p[i] for p in pts) for i in range(3)])
    hi = Vector([max(p[i] for p in pts) for i in range(3)])
    return lo, hi


def label(text, loc, size, material):
    curve = bpy.data.curves.new("Label_" + text, "FONT")
    curve.body = text
    curve.align_x = "CENTER"
    curve.align_y = "BOTTOM"
    curve.size = size
    curve.extrude = 0.01
    o = bpy.data.objects.new("Label_" + text, curve)
    o.location = loc
    o.rotation_euler = (math.radians(60), 0, 0)
    o.data.materials.append(material)
    o.visible_shadow = False
    bpy.context.scene.collection.objects.link(o)
    return o


def title(name):
    return name.replace("_", " ").title()


def look(cam, target, direction, dist):
    cam.location = target + direction.normalized() * dist
    cam.rotation_euler = (target - cam.location).to_track_quat("-Z", "Y").to_euler()


def render(path, w, h):
    s = bpy.context.scene
    s.render.resolution_x = w
    s.render.resolution_y = h
    s.render.filepath = path
    bpy.ops.render.render(write_still=True)


def contact_sheet(paths, out, cols=4):
    imgs = [bpy.data.images.load(p) for p in paths]
    w, h = imgs[0].size
    rows = math.ceil(len(imgs) / cols)
    sheet = np.ones((rows * h, cols * w, 4), dtype=np.float32)
    sheet[..., :3] = 0.2
    for i, img in enumerate(imgs):
        px = np.array(img.pixels[:], dtype=np.float32).reshape(h, w, 4)
        r, c = divmod(i, cols)
        # Blender's pixel rows run bottom to top.
        y0 = (rows - 1 - r) * h
        sheet[y0 : y0 + h, c * w : (c + 1) * w] = px
    out_img = bpy.data.images.new("contact_sheet", cols * w, rows * h, alpha=True)
    out_img.pixels = sheet.ravel()
    out_img.filepath_raw = out
    out_img.file_format = "PNG"
    out_img.save()


def main():
    a = kit.args()
    models_dir = a[0] if len(a) > 0 else os.path.join(kit.REPO, "assets", "verse", "generated")
    out_dir = a[1] if len(a) > 1 else os.path.join(tempfile.gettempdir(), "verse-models-gallery")
    only = set(a[2:])
    os.makedirs(os.path.join(out_dir, "previews"), exist_ok=True)
    files = sorted(glob.glob(os.path.join(models_dir, "**", "*.glb"), recursive=True))
    names = [os.path.splitext(os.path.basename(f))[0] for f in files]
    if only:
        files = [f for f, n in zip(files, names) if n in only]
        names = [n for n in names if n in only]

    kit.reset()
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE"
    scene.view_settings.view_transform = "Standard"
    scene.render.film_transparent = False
    world = bpy.data.worlds.new("Sky")
    world.use_nodes = True
    bg = world.node_tree.nodes["Background"]
    bg.inputs["Color"].default_value = (*SKY, 1.0)
    bg.inputs["Strength"].default_value = 0.45
    scene.world = world
    sun = bpy.data.objects.new("Sun", bpy.data.lights.new("Sun", "SUN"))
    sun.data.energy = 2.6
    sun.data.angle = math.radians(8)
    sun.rotation_euler = (math.radians(50), math.radians(10), math.radians(-35))
    scene.collection.objects.link(sun)
    ink = kit.mat("Label_Ink", (0.06, 0.06, 0.08), 0.9)
    cam = bpy.data.objects.new("Camera", bpy.data.cameras.new("Camera"))
    scene.collection.objects.link(cam)
    scene.camera = cam

    cols = min(6, max(1, math.ceil(math.sqrt(len(files) * 1.6))))
    rows = math.ceil(len(files) / cols)
    placed = []
    for i, (path, name) in enumerate(zip(files, names)):
        before = set(bpy.data.objects)
        before_actions = set(bpy.data.actions)
        bpy.ops.import_scene.gltf(filepath=path)
        objs = [o for o in bpy.data.objects if o not in before]
        actions = [x for x in bpy.data.actions if x not in before_actions]
        for o in objs:
            if o.type == "MESH" and o.name.startswith("Icosphere"):
                o.hide_render = True
        pose_idle(objs, actions)
        scene.frame_set(2)
        scene.frame_set(1)
        bpy.context.view_layer.update()
        lo, hi = bounds(objs)
        size = hi - lo
        s = min(1.0, FIT / max(size)) if max(size) > FIT else min(FIT / max(size), 8.0)
        s = max(s, 0.1)
        if 0.85 < s < 1.2:
            s = 1.0
        # A skinned mesh may be its rig's sibling, so move one parent of both.
        holder = bpy.data.objects.new("Holder_" + name, None)
        scene.collection.objects.link(holder)
        for o in objs:
            if o.parent is None:
                o.parent = holder
        objs.append(holder)
        r, c = divmod(i, cols)
        cell = Vector(((c - (cols - 1) / 2) * CELL, -(r - (rows - 1) / 2) * ROW, 0))
        base = Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, lo.z))
        holder.scale = (s, s, s)
        # Turn each model three-quarters to the camera; its front is -Y.
        holder.rotation_euler = (0, 0, math.radians(-35))
        holder.location = cell
        bpy.context.view_layer.update()
        holder.location = cell - (holder.matrix_world.to_3x3() @ base)
        dims = "x".join(f"{d:.2g}" for d in (size.x, size.y, size.z))
        note = f"{title(name)}\n{dims} m" + ("" if s == 1.0 else f"  (x{s:.2g})")
        lab = label(note, cell + Vector((0, -2.9, 0.02)), 0.4, ink)
        placed.append((name, objs, lab, cell))

    bpy.ops.mesh.primitive_plane_add(size=1)
    ground = bpy.context.object
    ground.name = "Ground"
    ground.scale = (cols * CELL + 40, rows * ROW + 40, 1)
    ground.data.materials.append(kit.mat("Ground", GROUND, 1.0))

    # Overview: the whole grid from the front, above.
    # An orthographic view from the front, 35 degrees up, so every cell
    # reads at the same size.
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = max(cols * CELL + 2, (rows * ROW * math.sin(math.radians(35)) + 4) * 1.6)
    look(cam, Vector((0, -2.2, 1.0)), Vector((0, -math.cos(math.radians(35)), math.sin(math.radians(35)))), 80)
    cam.data.clip_end = 300
    render(os.path.join(out_dir, "gallery.png"), 2400, 1500)

    # Previews: each model alone, with its label.
    previews = []
    for name, objs, lab, cell in placed:
        for _, others, olab, _ in placed:
            hide = others is not objs
            for o in others:
                o.hide_render = hide or (o.type == "MESH" and o.name.startswith("Icosphere"))
            olab.hide_render = hide
        lo, hi = bounds(objs + [lab])
        center = (lo + hi) / 2
        radius = (hi - lo).length / 2
        look(cam, center, Vector((0.2, -1.0, 0.5)), radius * 2.6 + 0.5)
        cam.data.type = "PERSP"
        cam.data.lens = 40
        p = os.path.join(out_dir, "previews", name + ".png")
        render(p, 800, 600)
        previews.append(p)
    contact_sheet(previews, os.path.join(out_dir, "contact_sheet.png"))
    print("GALLERY", out_dir)


if __name__ == "__main__":
    main()
