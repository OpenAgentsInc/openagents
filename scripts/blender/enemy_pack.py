"""Convert creatures from Quaternius's Easy Animated Enemy Pack to glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/enemy_pack.py -- PACK.zip OUT_DIR [NAME ...]

PACK.zip is "Easy Animated Enemy Pack - Jan 2019.zip" (CC0, Quaternius;
SHA-256 a97f38b9...2b21004). Each creature goes through `convert_fbx`'s
opaque import, is scaled to Verse's meters with its base center at the
origin, has its actions renamed to Verse's lowercase clip names, and, when
it carries more triangles than its budget, is decimated. The giant spider is
for Wild Shape; the rest are ambient wildlife.
"""

import os
import sys
import tempfile
import zipfile

import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(__file__))
import convert_fbx  # noqa: E402
import kit  # noqa: E402

PACK_DIR = "Easy Animated Enemy Pack - Jan 2019/FBX/"

# name: (FBX stem, how to measure, meters, triangle budget, {source clip: Verse clip})
CREATURES = {
    "giant_spider": (
        "Spider",
        "span",
        2.0,
        3000,
        {"Spider_Idle": "idle", "Spider_Walk": "walk", "Spider_Attack": "attack", "Spider_Jump": "jump", "Spider_Death": "death"},
    ),
    "rat": (
        "Rat",
        "span",
        0.45,
        2400,
        {"Rat_Idle": "idle", "Rat_Walk": "walk", "Rat_Run": "run", "Rat_Attack": "attack", "Rat_Jump": "jump", "Rat_Death": "death"},
    ),
    "frog": (
        "Frog",
        "span",
        0.22,
        2400,
        {"Frog_Idle": "idle", "Frog_Jump": "jump", "Frog_Attack": "attack", "Frog_Death": "death"},
    ),
    "snake": (
        "Snake",
        "span",
        0.6,
        2400,
        {"Snake_Idle": "idle", "Snake_Walk": "walk", "Snake_Attack": "attack", "Snake_Jump": "jump"},
    ),
    "wasp": (
        "Wasp",
        "span",
        0.3,
        2400,
        {"Wasp_Flying": "fly", "Wasp_Attack": "attack", "Wasp_Death": "death"},
    ),
}


def rest_points(mesh_objs):
    dg = bpy.context.evaluated_depsgraph_get()
    pts = []
    for o in mesh_objs:
        e = o.evaluated_get(dg)
        me = e.to_mesh()
        pts += [e.matrix_world @ v.co for v in me.vertices]
        e.to_mesh_clear()
    return pts


def drop_curve(act, fc):
    if hasattr(act, "layers") and act.layers:
        for layer in act.layers:
            for strip in layer.strips:
                for bag in strip.channelbags:
                    if fc in list(bag.fcurves):
                        bag.fcurves.remove(fc)
                        return
    else:
        act.fcurves.remove(fc)


def bake_scale(arm, body, s, base):
    """Scale the rig by `s` about `base` and bake it into rig, mesh, and keys.

    Bone translation keys are in bone space, so they scale by the armature's
    full world scale.
    """
    arm.location = (arm.location - base) * s
    arm.scale = arm.scale * s
    world_scale = arm.scale.x
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    for o in body:
        o.select_set(True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    for o in body:
        # Fold the parent inverse into the mesh, so its vertices sit in the
        # rig's space as glTF skinning expects.
        world = o.matrix_world.copy()
        o.matrix_parent_inverse.identity()
        o.matrix_world = world
        bpy.ops.object.select_all(action="DESELECT")
        o.select_set(True)
        bpy.context.view_layer.objects.active = o
        bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    for act in bpy.data.actions:
        for fc in kit.fcurves(act):
            if fc.data_path.endswith(".location"):
                for kp in fc.keyframe_points:
                    kp.co.y *= world_scale
                    kp.handle_left.y *= world_scale
                    kp.handle_right.y *= world_scale


def convert(fbx, out, measure, meters, budget, clips):
    convert_fbx.import_opaque(fbx)
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    body = [o for o in bpy.data.objects if o.type == "MESH"]
    # Drop the pack's helper objects that aren't skinned to the rig.
    for o in list(bpy.data.objects):
        if o.type not in ("ARMATURE", "MESH"):
            bpy.data.objects.remove(o)

    # Some clips key the rig object's own transform (the Rat's hold its
    # import scale); that would undo the baked scale, so drop those curves.
    for act in bpy.data.actions:
        for fc in kit.fcurves(act):
            if not fc.data_path.startswith("pose."):
                drop_curve(act, fc)

    # Measure the first idle frame (some rest poses are far from how the
    # creature stands), then scale and center through the armature. A mesh
    # with its own scale under the rig deforms differently once that scale is
    # baked, so measure again and repeat until the size holds.
    idle = next(a for a in bpy.data.actions if a.name.split("|")[-1] == next(iter(clips)))
    arm.animation_data_create()
    arm.animation_data.action = idle
    for _ in range(4):
        bpy.context.scene.frame_set(2)
        bpy.context.scene.frame_set(1)
        pts = rest_points(body)
        lo = Vector([min(p[i] for p in pts) for i in range(3)])
        hi = Vector([max(p[i] for p in pts) for i in range(3)])
        extent = max(hi.x - lo.x, hi.y - lo.y) if measure == "span" else (hi - lo).length
        s = meters / extent
        base = Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, lo.z))
        if abs(s - 1) < 0.005 and base.length < 0.002:
            break
        bake_scale(arm, body, s, base)

    # Decimate over budget, before the armature deforms.
    before = kit.triangles(body)
    if before > budget:
        for o in body:
            dec = o.modifiers.new("Decimate", "DECIMATE")
            dec.ratio = budget / before * 0.98
            dec.use_collapse_triangulate = True
            bpy.context.view_layer.objects.active = o
            bpy.ops.object.modifier_move_to_index(modifier=dec.name, index=0)
            bpy.ops.object.modifier_apply(modifier=dec.name)

    # Rename the clips; keep only the ones Verse names.
    for act in list(bpy.data.actions):
        src = act.name.split("|")[-1]
        if src in clips:
            act.name = clips[src]
            act.use_fake_user = True
        else:
            bpy.data.actions.remove(act)
    arm.name = "Armature"
    arm.data.pose_position = "POSE"
    if arm.animation_data:
        arm.animation_data.action = None
    for track in list(arm.animation_data.nla_tracks if arm.animation_data else []):
        arm.animation_data.nla_tracks.remove(track)
    for act in bpy.data.actions:
        track = arm.animation_data.nla_tracks.new()
        track.name = act.name
        track.strips.new(act.name, int(act.frame_range[0]), act)
    return kit.export(out, animations=True, extra={"source": fbx.split("/")[-1], "triangles_source": before})


def main():
    a = kit.args()
    pack, out_dir, names = a[0], a[1], a[2:] or list(CREATURES)
    tmp = tempfile.mkdtemp(prefix="enemy-pack-")
    with zipfile.ZipFile(pack) as z:
        for name in names:
            z.extract(PACK_DIR + CREATURES[name][0] + ".fbx", tmp)
    for name in names:
        stem, measure, meters, budget, clips = CREATURES[name]
        convert(os.path.join(tmp, PACK_DIR, stem + ".fbx"), os.path.join(out_dir, name + ".glb"), measure, meters, budget, clips)


if __name__ == "__main__":
    main()
