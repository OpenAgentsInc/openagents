"""Review renders of Alice: a turnaround, a turntable, silhouettes beside
the Ranger, and the deformation poses from the Universal Animation Library.

Run headless:
    Blender -b --factory-startup --python scripts/blender/alice_views.py -- \
        [MODEL.glb] [OUT_DIR]

MODEL defaults to assets/verse/characters/original/alice/alice.lod0.glb and
OUT_DIR to $TMPDIR/alice-views. Writes:

- `turnaround.png`: front, three-quarter, side, and back, standing idle,
  and the bind pose from the front.
- `turntable.png` and `turntable/NN.png`: twelve views around her, idle.
- `silhouettes.png`: Alice and the Universal male Ranger in black at the
  same scale, 64 and 24 pixels tall, enlarged without smoothing.
- `poses.png` and `poses/<clip>.png`: one frame of each clip that stresses
  a joint (walk, jog, sprint, the jump, a cast, a crouch, the two-handed
  swing, folded arms, the sword combo, and the arms raised), from the front
  three-quarter, so elbows, knees, shoulders, and hips can be checked for
  lost volume and candy-wrapper twists.

Every clip comes from the CC0 Universal Animation Library that Verse plays
(`assets/verse/characters/quaternius/gaits.glb` and `animations.glb`); it is
assigned to Alice's armature by bone name, as Verse retargets it.
"""

import math
import os
import sys
import tempfile

import bpy
import numpy as np
from mathutils import Vector

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

CHARS = os.path.join(kit.REPO, "assets", "verse", "characters")
GAITS = os.path.join(CHARS, "quaternius", "gaits.glb")
LIBRARY = os.path.join(CHARS, "quaternius", "animations.glb")
POSES = [
    ("Idle_Loop", GAITS, 0.3),
    ("Walk_Loop", GAITS, 0.25),
    ("Jog_Fwd_Loop", GAITS, 0.25),
    ("Sprint_Loop", GAITS, 0.3),
    ("NinjaJump_Idle_Loop", LIBRARY, 0.5),
    ("Spell_Simple_Shoot", GAITS, 0.35),
    ("Crouch_Idle_Loop", GAITS, 0.3),
    ("TreeChopping_Loop", LIBRARY, 0.35),
    ("Idle_FoldArms_Loop", LIBRARY, 0.4),
    ("Sword_Heavy_Combo", LIBRARY, 0.55),
    ("Jump_Start", GAITS, 0.9),
    ("Dance_Loop", GAITS, 0.4),
]


def args():
    a = kit.args()
    model = a[0] if a else os.path.join(CHARS, "original", "alice", "alice.lod0.glb")
    out = a[1] if len(a) > 1 else os.path.join(tempfile.gettempdir(), "alice-views")
    return model, out


def import_glb(path):
    before = set(bpy.data.objects)
    acts = set(bpy.data.actions)
    bpy.ops.import_scene.gltf(filepath=path)
    new = [o for o in bpy.data.objects if o not in before]
    for o in new:
        if o.type == "MESH" and o.name.startswith("Icosphere"):
            o.hide_render = True
    # Only the hair cards are alpha-masked, as the admitted model has it; the
    # build's opaque material reads the atlas's alpha too.
    for m in bpy.data.materials:
        if m.use_nodes and not m.name.startswith("alice_hair"):
            bsdf = m.node_tree.nodes.get("Principled BSDF")
            if bsdf and bsdf.inputs["Alpha"].links:
                m.node_tree.links.remove(bsdf.inputs["Alpha"].links[0])
    arm = next((o for o in new if o.type == "ARMATURE"), None)
    return new, arm, [a for a in bpy.data.actions if a not in acts]


def play(arm, action, fraction):
    """Pose `arm` `fraction` of the way through `action`."""
    ad = arm.animation_data or arm.animation_data_create()
    for t in ad.nla_tracks:
        t.mute = True
    ad.action = action
    if hasattr(ad, "action_slot") and action.slots:
        ad.action_slot = action.slots[0]
    lo, hi = action.frame_range
    frame = lo + (hi - lo) * fraction
    bpy.context.scene.frame_set(int(frame), subframe=frame - int(frame))


def rest(arm):
    if arm.animation_data:
        arm.animation_data.action = None
    for pb in arm.pose.bones:
        pb.location = (0, 0, 0)
        pb.rotation_quaternion = (1, 0, 0, 0)
        pb.rotation_euler = (0, 0, 0)
        pb.scale = (1, 1, 1)
    bpy.context.view_layer.update()


def stage(size=(700, 1000)):
    s = bpy.context.scene
    s.render.engine = "BLENDER_EEVEE"
    s.render.resolution_x, s.render.resolution_y = size
    s.render.film_transparent = False
    world = bpy.data.worlds.new("w")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.42, 0.5, 0.56, 1)
    world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.9
    s.world = world
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 3.2
    sun.rotation_euler = (0.75, 0.15, -0.55)
    s.collection.objects.link(sun)
    fill = bpy.data.objects.new("fill", bpy.data.lights.new("fill", "SUN"))
    fill.data.energy = 0.8
    fill.rotation_euler = (1.1, 0.0, 2.6)
    s.collection.objects.link(fill)
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    s.collection.objects.link(cam)
    s.camera = cam
    ground = bpy.data.meshes.new("ground")
    ground.from_pydata([(-3, -3, 0), (3, -3, 0), (3, 3, 0), (-3, 3, 0)], [], [(0, 1, 2, 3)])
    g = bpy.data.objects.new("ground", ground)
    m = kit.mat("ground", (0.18, 0.24, 0.14), rough=1.0)
    ground.materials.append(m)
    s.collection.objects.link(g)
    return cam, sun


def aim(cam, angle_deg, target=(0, 0, 0.92), distance=5.4, lift=0.25, lens=70):
    a = math.radians(angle_deg)
    t = Vector(target)
    cam.data.lens = lens
    cam.location = t + Vector((math.sin(a) * distance, -math.cos(a) * distance, lift))
    cam.rotation_euler = (t - cam.location).to_track_quat("-Z", "Y").to_euler()


def render(path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    bpy.context.scene.render.filepath = path
    bpy.ops.render.render(write_still=True)
    return path


def load(path):
    img = bpy.data.images.load(path)
    w, h = img.size
    px = np.array(img.pixels[:], dtype=np.float32).reshape(h, w, 4)
    bpy.data.images.remove(img)
    return px


def sheet(paths, cols, out, scale=1):
    tiles = [load(p) for p in paths]
    h, w = tiles[0].shape[:2]
    rows = (len(tiles) + cols - 1) // cols
    canvas = np.ones((rows * h, cols * w, 4), dtype=np.float32)
    for i, t in enumerate(tiles):
        r, c = divmod(i, cols)
        # Images are stored bottom-up.
        canvas[(rows - 1 - r) * h:(rows - r) * h, c * w:(c + 1) * w] = t
    if scale > 1:
        canvas = canvas.repeat(scale, axis=0).repeat(scale, axis=1)
    img = bpy.data.images.new("sheet", canvas.shape[1], canvas.shape[0], alpha=True)
    img.pixels[:] = canvas.reshape(-1).tolist()
    img.filepath_raw = out
    img.file_format = "PNG"
    img.save()
    bpy.data.images.remove(img)


def label(text, loc):
    curve = bpy.data.curves.new("label", "FONT")
    curve.body = text
    curve.align_x = "CENTER"
    curve.size = 0.11
    o = bpy.data.objects.new("label", curve)
    o.location = loc
    o.rotation_euler = (math.radians(90), 0, 0)
    o.data.materials.append(kit.mat("label", (0.95, 0.95, 0.9)))
    bpy.context.scene.collection.objects.link(o)
    return o


def main():
    model, out = args()
    kit.reset()
    cam, sun = stage()
    objs, arm, _ = import_glb(model)
    _, _, gaits = import_glb(GAITS)
    _, _, library = import_glb(LIBRARY)
    # The libraries bring their own mannequins; keep only their clips.
    for o in list(bpy.data.objects):
        if o not in objs and o.type in ("ARMATURE", "MESH") and o.name != "ground":
            bpy.data.objects.remove(o, do_unlink=True)
    actions = {a.name: a for a in gaits + library}
    idle = actions.get("Idle_Loop")

    # The turnaround and turntable, standing idle.
    play(arm, idle, 0.3)
    shots = []
    for name, ang in (("front", 0), ("three_quarter", 35), ("side", 90), ("back", 180)):
        aim(cam, ang)
        shots.append(render(os.path.join(out, "turnaround", f"{name}.png")))
    rest(arm)
    aim(cam, 0, distance=6.4, target=(0, 0, 0.95))
    shots.append(render(os.path.join(out, "turnaround", "bind.png")))
    sheet(shots, 5, os.path.join(out, "turnaround.png"))
    play(arm, idle, 0.3)
    frames = []
    for k in range(12):
        aim(cam, k * 30)
        frames.append(render(os.path.join(out, "turntable", f"{k:02d}.png")))
    sheet(frames, 6, os.path.join(out, "turntable.png"))

    # Face close-ups: front, three-quarter, and profile, under a soft key.
    s = bpy.context.scene
    s.render.resolution_x, s.render.resolution_y = 800, 800
    sun.data.angle = math.radians(25)
    faces = []
    for name, ang in (("front", 0), ("three_quarter", 35), ("profile", 90)):
        aim(cam, ang, target=(0, -0.01, 1.64), distance=1.0, lift=0.02, lens=85)
        faces.append(render(os.path.join(out, "face", f"{name}.png")))
    sheet(faces, 3, os.path.join(out, "face.png"))
    s.render.resolution_x, s.render.resolution_y = 700, 1000

    # The poses that stress the joints, front three-quarter, labeled.
    tiles = []
    for name, _, fraction in POSES:
        act = actions.get(name)
        if act is None:
            print("MISSING", name)
            continue
        play(arm, act, fraction)
        tag = label(name, (0, 0.6, 2.05))
        aim(cam, 30, target=(0, 0, 0.95), distance=6.0, lift=0.4)
        tiles.append(render(os.path.join(out, "poses", f"{name}.png")))
        bpy.data.objects.remove(tag, do_unlink=True)
    sheet(tiles, 4, os.path.join(out, "poses.png"))

    # Silhouettes beside the Universal male Ranger, at the same scale.
    play(arm, idle, 0.3)
    ranger, rarm, _ = import_glb(os.path.join(CHARS, "quaternius", "outfits", "Male_Ranger.gltf"))
    head, harm, _ = import_glb(os.path.join(CHARS, "quaternius", "base", "Superhero_Male_FullBody.gltf"))
    for o in ranger + head:
        if o.type in ("ARMATURE", "EMPTY") or o.parent is None:
            o.location.x += 0.9
    for a in (rarm, harm):
        if a:
            play(a, idle, 0.3)
    black = kit.mat("black", (0, 0, 0), rough=1.0)
    for o in bpy.data.objects:
        if o.type == "MESH" and o.name != "ground" and not o.hide_render:
            o.data.materials.clear()
            o.data.materials.append(black)
    bpy.data.objects["ground"].hide_render = True
    s = bpy.context.scene
    s.world.node_tree.nodes["Background"].inputs["Color"].default_value = (1, 1, 1, 1)
    s.world.node_tree.nodes["Background"].inputs["Strength"].default_value = 1.0
    s.view_settings.view_transform = "Standard"
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = 2.1
    cam.location = Vector((0.45, -8, 0.92))
    cam.rotation_euler = (math.radians(90), 0, 0)
    sil = []
    for px in (64, 24):
        s.render.resolution_x, s.render.resolution_y = px, px
        s.render.filter_size = 1.5
        path = render(os.path.join(out, "silhouettes", f"{px}.png"))
        sil.append((px, path))
    for px, path in sil:
        tile = load(path)
        scale = 192 // px
        big = tile.repeat(scale, axis=0).repeat(scale, axis=1)
        img = bpy.data.images.new(f"sil{px}", big.shape[1], big.shape[0], alpha=True)
        img.pixels[:] = big.reshape(-1).tolist()
        img.filepath_raw = os.path.join(out, "silhouettes", f"{px}_big.png")
        img.file_format = "PNG"
        img.save()
    sheet([os.path.join(out, "silhouettes", f"{px}_big.png") for px in (64, 24)], 2,
          os.path.join(out, "silhouettes.png"))
    print("VIEWS", out)


main()
