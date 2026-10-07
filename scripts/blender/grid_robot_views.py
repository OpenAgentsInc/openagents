"""Review renders and motion checks for the Grid robot.

Run headless:

    Blender -b --factory-startup --python scripts/blender/grid_robot_views.py -- \
        [MODEL.glb] [OUT_DIR]

MODEL defaults to the built `grid-robot.lod0.glb` and OUT_DIR to
/private/tmp/claude-501/grid-robot. Writes:

- `turnaround.png`: front, three-quarter, side, and back in the bind pose,
  on the Grid's near-black field.
- `poses.png`: one frame of each clip the Grid plays, from the front
  three-quarter.
- `motion.json`: for every sampled frame of those clips, the lowest point of
  each foot (planting), and every pair of rigid parts on different bones
  that intersect in that frame but not at rest.

Every clip comes from the CC0 Universal Animation Library
(`assets/verse/characters/quaternius/gaits.glb`), assigned to the robot's
armature by bone name, as Verse retargets it.
"""

import json
import math
import os
import sys

import bpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

GAITS = os.path.join(kit.REPO, "assets", "verse", "characters", "quaternius", "gaits.glb")
DEFAULT = os.path.join(kit.REPO, "assets", "verse", "characters", "original", "grid-robot", "build", "grid-robot.lod0.glb")
# The clips the Grid plays.
CLIPS = [
    ("Idle_Loop", 0.5),
    ("Walk_Loop", 0.25),
    ("Walk_Loop", 0.75),
    ("Jog_Fwd_Loop", 0.25),
    ("Jump_Loop", 0.5),
    ("Dance_Loop", 0.25),
    ("Dance_Loop", 0.6),
]


def setup(model):
    kit.reset()
    bpy.ops.import_scene.gltf(filepath=model)
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    body = next(o for o in bpy.data.objects if o.type == "MESH")
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=GAITS)
    for o in set(bpy.data.objects) - before:
        bpy.data.objects.remove(o)
    s = bpy.context.scene
    s.render.engine = "BLENDER_EEVEE"
    s.world = bpy.data.worlds.new("field")
    s.world.use_nodes = True
    bg = s.world.node_tree.nodes["Background"]
    bg.inputs["Color"].default_value = (0.004, 0.004, 0.005, 1)
    bg.inputs["Strength"].default_value = 1.0
    s.view_settings.view_transform = "Standard"
    for name, energy, rot in (("key", 3.0, (0.9, 0.15, 0.7)), ("rim", 2.0, (1.2, 0.0, 3.6)), ("fill", 0.6, (1.3, 0.0, -1.2))):
        light = bpy.data.objects.new(name, bpy.data.lights.new(name, "SUN"))
        light.data.energy = energy
        light.rotation_euler = rot
        s.collection.objects.link(light)
    # The Grid's floor: a black plane with a white 1 m lattice.
    bpy.ops.mesh.primitive_grid_add(x_subdivisions=13, y_subdivisions=13, size=12)
    grid = bpy.context.object
    wire = grid.modifiers.new("wire", "WIREFRAME")
    wire.thickness = 0.008
    grid.data.materials.append(kit.mat("GridLine", (0.5, 0.5, 0.5), emit=(0.55, 0.55, 0.55), strength=1.0))
    return arm, body


def assign(arm, clip):
    act = bpy.data.actions[clip]
    arm.animation_data_create()
    arm.animation_data.action = act
    slots = getattr(arm.animation_data, "action_suitable_slots", None)
    if slots:
        arm.animation_data.action_slot = slots[0]
    return act.frame_range


def camera(target, yaw, dist=3.4, height=1.0, lens=50):
    cam = bpy.context.scene.camera
    if cam is None:
        cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
        bpy.context.scene.collection.objects.link(cam)
        bpy.context.scene.camera = cam
    cam.data.lens = lens
    t = Vector(target)
    cam.location = t + Vector((math.sin(yaw) * dist, -math.cos(yaw) * dist, height))
    cam.rotation_euler = (t - cam.location).to_track_quat("-Z", "Y").to_euler()


def render(path, w=520, h=900):
    s = bpy.context.scene
    s.render.resolution_x, s.render.resolution_y = w, h
    s.render.filepath = path
    bpy.ops.render.render(write_still=True)


def sheet(paths, out):
    imgs = [bpy.data.images.load(p) for p in paths]
    w, h = imgs[0].size
    sheet = bpy.data.images.new("sheet", w * len(imgs), h)
    px = [0.0] * (w * len(imgs) * h * 4)
    for k, img in enumerate(imgs):
        src = list(img.pixels)
        for row in range(h):
            a = (row * w * len(imgs) + k * w) * 4
            px[a:a + w * 4] = src[row * w * 4:(row + 1) * w * 4]
    sheet.pixels = px
    sheet.filepath_raw = out
    sheet.file_format = "PNG"
    sheet.save()


def parts(body):
    """Vertex indices of each bone's rigid part."""
    groups = {g.index: g.name for g in body.vertex_groups}
    out = {}
    for v in body.data.vertices:
        out.setdefault(groups[v.groups[0].group], set()).add(v.index)
    return out


def trees(body, by_bone):
    dg = bpy.context.evaluated_depsgraph_get()
    e = body.evaluated_get(dg)
    me = e.to_mesh()
    pts = [e.matrix_world @ v.co for v in me.vertices]
    out = {}
    for bone, verts in by_bone.items():
        polys = [list(p.vertices) for p in me.polygons if p.vertices[0] in verts]
        out[bone] = BVHTree.FromPolygons(pts, polys)
    feet = {side: min(pts[i].z for b in (f"foot_{side}", f"ball_{side}") for i in by_bone.get(b, ())) for side in "lr"}
    e.to_mesh_clear()
    return out, feet


def overlaps(t):
    names = sorted(t)
    hits = {}
    for i, a in enumerate(names):
        for b in names[i + 1:]:
            n = len(t[a].overlap(t[b]))
            if n:
                hits[(a, b)] = n
    return hits


def main():
    a = kit.args()
    model = a[0] if a else DEFAULT
    out = a[1] if len(a) > 1 else "/private/tmp/claude-501/grid-robot"
    os.makedirs(out, exist_ok=True)
    arm, body = setup(model)
    target = (0, 0, 0.95)
    views = []
    for k, (label, yaw) in enumerate((("front", 0.0), ("three-quarter", 0.7), ("side", math.pi / 2), ("back", math.pi))):
        camera(target, yaw)
        p = os.path.join(out, f"turn_{k}_{label}.png")
        render(p)
        views.append(p)
    sheet(views, os.path.join(out, "turnaround.png"))
    camera((0, 0, 1.7), 0.35, dist=1.0, height=0.05, lens=50)
    render(os.path.join(out, "face.png"), 700, 700)

    by_bone = parts(body)
    rest, _ = trees(body, by_bone)
    at_rest = set(overlaps(rest))
    report = {"rest_overlaps": sorted("/".join(p) for p in at_rest), "clips": {}}
    poses = []
    for clip, phase in CLIPS:
        f0, f1 = assign(arm, clip)
        if (clip, phase) == (clip, CLIPS[[c for c, _ in CLIPS].index(clip)][1]):
            lows = {"l": [], "r": []}
            new = {}
            steps = 24
            for i in range(steps + 1):
                bpy.context.scene.frame_set(int(round(f0 + (f1 - f0) * i / steps)))
                t, feet = trees(body, by_bone)
                for side in "lr":
                    lows[side].append(round(feet[side], 4))
                for pair, n in overlaps(t).items():
                    if pair not in at_rest:
                        key = "/".join(pair)
                        new[key] = max(new.get(key, 0), n)
            report["clips"][clip] = {
                "foot_low_m": {s: [min(v), max(v)] for s, v in lows.items()},
                "new_overlaps": new,
            }
        bpy.context.scene.frame_set(int(round(f0 + (f1 - f0) * phase)))
        camera(target, 0.6)
        p = os.path.join(out, f"pose_{clip}_{int(phase * 100)}.png")
        render(p)
        poses.append(p)
    sheet(poses, os.path.join(out, "poses.png"))
    with open(os.path.join(out, "motion.json"), "w") as f:
        json.dump(report, f, indent=1)
    print("MOTION", json.dumps(report))


main()
