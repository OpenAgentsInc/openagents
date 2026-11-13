# Firm reason: 6 (Blender bpy is required to author, bake and export skeletal animation on the UAL rig).
"""Two-handed weapon clips for the Universal Animation Library rig, authored here (owned, CC0).

The Universal Animation Library 1 and 2 have no two-handed weapon clips: in every sword, chop and
melee clip the hands stay 0.9 m or more apart. This makes them: for a weapon with both hands on its
haft, the right hand low and the left SPACING metres up it,

  TwoHand_<Kind>_Idle_Loop   the ready stance over UAL1 Idle_Loop's legs and breathing
  TwoHand_<Kind>_Run_Loop    the same hold over UAL1 Jog_Fwd_Loop's run
  TwoHand_<Kind>_Cleave      an overhead cleave over Idle_Loop's legs: anticipation, impact at
                             IMPACT of its length, follow-through and recovery

for Kind Axe (0.40 m between the hands) and Sword (0.16 m, a greatsword's grip). The haft is keyed
in spine_03's space; each hand's wrist is IK-pinned (lowerarm, chain 2) to its place on the haft and
its rotation copies the grip (thumb toward the head, knuckles along the edge), and the fingers close
as in UAL1 Driving_Loop. Then everything is baked to plain keys and exported with the rig.

  Blender --background --factory-startup --disable-autoexec --python-exit-code 1 \
    --python metaverse/tools/assets/two_hand_clips.py -- UAL1_Standard.glb OUT_DIR

writes OUT_DIR/two-hand-clips.blend (the source), OUT_DIR/two-hand-clips.glb (the rig, its mesh and
the six clips) and OUT_DIR/report.json (each clip's frames and how far a fist strays from its haft).
Never run source-pack scripts.
"""
import bpy, json, math, sys
from pathlib import Path
from mathutils import Vector, Matrix

FPS = 60
IMPACT = 0.46                 # share of the cleave at its impact (0.55 s of 1.2 s)
FIST_ALONG = 0.08             # metres from the wrist to the fist's middle, toward the knuckles
FIST_PALM = 0.035             # and toward the palm
FINGERS = ("thumb_", "index_", "middle_", "ring_", "pinky_")
# Each weapon as the game holds it (everglade/scene/charactercreator.coax CC_KIT_*): the metres
# between the fists, its scale from the source, where the right fist closes (source units up the
# weapon), and its mesh's bounds as boxes (source y0, y1, x0, x1, z0, z1; x is the edge, y up the haft).
WEAPONS = {
    "Axe": (0.40, 0.6, -0.36, ((-0.664, 0.462, -0.06, 0.152, -0.064, 0.064), (0.462, 0.62, -0.30, 0.325, -0.066, 0.066),
                               (0.62, 1.30, -0.423, 0.448, -0.068, 0.068), (1.30, 1.487, -0.30, 0.325, -0.068, 0.068))),
    "Sword": (0.16, 0.55, 0.05, ((-0.151, 0.475, -0.073, 0.073, -0.051, 0.051), (0.475, 0.80, -0.217, 0.217, -0.051, 0.051),
                                 (0.80, 2.30, -0.153, 0.153, -0.014, 0.014), (2.30, 2.584, -0.093, 0.093, -0.014, 0.014))),
}
# The body the weapon must keep clear of: (joint, its end: a joint or metres up the joint, radius, clearance).
# The head and neck get 8 cm of room; the weapon never enters the torso.
HEAD = ("Head", 0.08, 0.18, 0.12, 0.08)       # from 8 to 18 cm up the head joint: the skull, 12 cm round
NECK = ("neck_01", "Head", 0.07, 0.08)
TORSO = ("spine_01", "spine_03", 0.15, 0.0)  # spine_01 to 8 cm above spine_03
CLEAR_HEAD = 0.08


def lerp(a, b, t): return a + (b - a) * t
def smooth(t): t = min(max(t, 0.0), 1.0); return t * t * (3 - 2 * t)
def V(*x): return Vector(x)


# The haft in spine_03's space (+X the character's left, +Y up, +Z ahead): the right fist's place, the
# direction up the haft, the way the edge faces (squared to the haft), and the spine's lean forward and
# twist (radians). The hold carries the weapon low across the body, its head ahead of the left hip.
N = V(0.707, 0.707, 0.0)     # the cleave's plane faces up and to the left: it runs from high right to low left
def cut(d): return N.cross(d.normalized())   # the edge in that plane, square to the haft


HOLD = (V(-0.20, -0.22, 0.26), V(0.88, 0.25, 0.40), V(0, -1, 0), 0.0, 0.0)
RUN = (V(-0.18, -0.20, 0.30), V(0.88, 0.36, 0.30), V(0, -1, 0), 0.0, 0.0)
# The cleave: (seconds, fist, direction, edge, lean, twist). From the hold the weapon rises on the right,
# then goes up and back over the right shoulder, its head behind and beside the head and the hands below
# the chin (the wind-up, held); then over the shoulder and down across the front of the body from high
# right to low left (impact at 0.55 s), the follow-through low to the left, and back to the hold. Each
# fist stays within reach of its shoulder (about 55 cm in spine_03's space).
def key(t, g, d, lean, twist): return (t, g, d, cut(d), lean, twist)
CLEAVE = {
    "Axe": [
        (0.00,) + HOLD,
        key(0.14, V(-0.05, -0.20, 0.33), V(-0.66, 0.72, 0.20), 0.02, -0.18),
        key(0.30, V(-0.20, -0.12, 0.30), V(-0.45, 0.62, -0.64), -0.08, -0.25),
        key(0.40, V(-0.20, -0.125, 0.32), V(-0.43, 0.58, -0.69), -0.10, -0.28),
        key(0.48, V(-0.15, -0.15, 0.32), V(-0.62, 0.78, 0.10), 0.00, -0.12),
        key(0.515, V(-0.10, -0.04, 0.36), V(0.80, 0.36, 0.48), 0.14, 0.05),
        key(0.55, V(-0.06, 0.04, 0.325), V(0.86, -0.45, 0.235), 0.20, 0.10),
        key(0.75, V(-0.03, -0.01, 0.33), V(0.756, -0.605, 0.252), 0.20, 0.25),
        (1.20,) + HOLD,
    ],
    "Sword": [
        (0.00,) + HOLD,
        key(0.14, V(-0.07, -0.20, 0.33), V(-0.62, 0.68, 0.38), 0.02, -0.18),
        key(0.30, V(-0.24, -0.12, 0.33), V(-0.45, 0.62, -0.64), -0.08, -0.25),
        key(0.40, V(-0.24, -0.11, 0.34), V(-0.43, 0.58, -0.69), -0.10, -0.28),
        key(0.48, V(-0.15, -0.15, 0.32), V(-0.62, 0.78, 0.10), 0.00, -0.12),
        key(0.515, V(-0.10, -0.04, 0.36), V(0.80, 0.36, 0.48), 0.14, 0.05),
        key(0.55, V(-0.085, -0.01, 0.275), V(0.80, -0.45, 0.40), 0.20, 0.10),
        key(0.75, V(-0.005, -0.01, 0.305), V(0.756, -0.605, 0.252), 0.20, 0.25),
        (1.20,) + HOLD,
    ],
}
CLEAVE_SECONDS = 1.20
# On the downswing the left hand slides down the axe's haft toward the right, as an axe is swung, and back
# up as it returns to the hold: (seconds, metres between the fists). The greatsword's grip is too short to slide.
SLIDE = {"Axe": [(0.0, 0.40), (0.14, 0.22), (0.40, 0.24), (0.55, 0.15), (0.75, 0.12), (1.20, 0.40)], "Sword": [(0.0, 0.16), (1.20, 0.16)]}


def slide(kind, t):
    keys = SLIDE[kind]
    for (ta, a), (tb, b) in zip(keys, keys[1:]):
        if ta <= t <= tb:
            return lerp(a, b, smooth((t - ta) / (tb - ta)))
    return keys[-1][1]


def keyed(keys, t):
    for k in range(len(keys) - 1):
        a, b = keys[k], keys[k + 1]
        if a[0] <= t <= b[0]:
            u = smooth((t - a[0]) / (b[0] - a[0]))
            return (a[1].lerp(b[1], u), a[2].normalized().lerp(b[2].normalized(), u).normalized(),
                    a[3].normalized().lerp(b[3].normalized(), u).normalized(), lerp(a[4], b[4], u), lerp(a[5], b[5], u))
    last = keys[-1]
    return last[1], last[2].normalized(), last[3].normalized(), last[4], last[5]


def still(pose):
    return lambda t: (pose[0], pose[1].normalized(), pose[2].normalized(), pose[3], pose[4])


def armature():
    return next(o for o in bpy.data.objects if o.type == 'ARMATURE')


def assign(arm, action):
    arm.animation_data_create()
    arm.animation_data.action = action
    if hasattr(arm.animation_data, "action_slot") and action.slots:
        arm.animation_data.action_slot = action.slots[0]


def empty(name):
    o = bpy.data.objects.new(name, None)
    bpy.context.scene.collection.objects.link(o)
    o.rotation_mode = 'QUATERNION'
    return o


def fingers_closed(arm):
    """Each finger joint's rotation in UAL1 Driving_Loop (both hands round a wheel), at its first frame."""
    assign(arm, bpy.data.actions["Driving_Loop"])
    bpy.context.scene.frame_set(int(bpy.data.actions["Driving_Loop"].frame_range[0]))
    return {pb.name: pb.rotation_quaternion.copy() for pb in arm.pose.bones if pb.name.startswith(FINGERS)}


def author(arm, name, base, frames, kind, haft, closed, spacing=None):
    spacing = spacing or (lambda t: WEAPONS[kind][0])
    scene = bpy.context.scene
    work = bpy.data.actions[base].copy(); work.name = name + "_work"
    assign(arm, work)
    start = int(bpy.data.actions[base].frame_range[0])
    spines = ("spine_01", "spine_02", "spine_03")
    # 1. the spine's lean and twist on top of the base, and the closed fingers
    poses = []
    for f in range(frames + 1):
        scene.frame_set(start + f)
        lean, twist = haft(f / FPS)[3:5]
        row = {}
        for s in spines:
            pb = arm.pose.bones[s]
            # a third of the lean and twist at each spine joint: about the bone's own X (lean) and Y (twist)
            delta = Matrix.Rotation(lean / 3, 4, 'X') @ Matrix.Rotation(twist / 3, 4, 'Y')
            row[s] = (pb.rotation_quaternion @ delta.to_quaternion())
        poses.append(row)
    for f, row in enumerate(poses):
        for s, q in row.items():
            pb = arm.pose.bones[s]; pb.rotation_quaternion = q; pb.keyframe_insert("rotation_quaternion", frame=start + f)
        for b, q in closed.items():
            pb = arm.pose.bones[b]; pb.rotation_quaternion = q; pb.keyframe_insert("rotation_quaternion", frame=start + f)
    # 2. the haft each frame, and the hands' targets on it
    tr, tl, rr, rl, pr, pl = (empty(n) for n in ("wrist_r", "wrist_l", "grip_r", "grip_l", "pole_r", "pole_l"))
    hafts = []
    for f in range(frames + 1):
        scene.frame_set(start + f)
        g, d, edge = haft(f / FPS)[0:3]
        m = arm.matrix_world @ arm.pose.bones["spine_03"].matrix
        r3 = m.to_3x3().normalized()
        G = m @ g; D = (r3 @ d).normalized()
        E = (r3 @ edge).normalized()
        E = (E - D * E.dot(D)).normalized()
        X = E.cross(D)
        rot = Matrix((X, E, D)).transposed().to_quaternion()   # bone X, Y (knuckles) and Z (thumb)
        GL = G + D * spacing(f / FPS)
        for o, loc in ((tr, G - E * FIST_ALONG + X * FIST_PALM), (tl, GL - E * FIST_ALONG - X * FIST_PALM),
                       (rr, G), (rl, GL), (pr, m @ Vector((-0.65, -0.35, -0.15))), (pl, m @ Vector((0.65, -0.35, -0.15)))):
            o.location = loc; o.keyframe_insert("location", frame=start + f)
        for o in (rr, rl):
            o.rotation_quaternion = rot; o.keyframe_insert("rotation_quaternion", frame=start + f)
        hafts.append((G.copy(), D.copy()))
    for side, wrist, grip, pole in (("r", tr, rr, pr), ("l", tl, rl, pl)):
        ik = arm.pose.bones["lowerarm_" + side].constraints.new('IK')
        ik.target = wrist; ik.pole_target = pole; ik.chain_count = 2; ik.pole_angle = math.radians(-90)
        cr = arm.pose.bones["hand_" + side].constraints.new('COPY_ROTATION')
        cr.target = grip; cr.target_space = 'WORLD'; cr.owner_space = 'WORLD'
    # 3. bake to plain keys
    bpy.context.view_layer.objects.active = arm
    arm.select_set(True)
    bpy.ops.object.mode_set(mode='POSE')
    bpy.ops.pose.select_all(action='SELECT')
    bpy.ops.nla.bake(frame_start=start, frame_end=start + frames, only_selected=False, visual_keying=True,
                     clear_constraints=True, use_current_action=False, bake_types={'POSE'})
    bpy.ops.object.mode_set(mode='OBJECT')
    baked = arm.animation_data.action; baked.name = name; baked.use_fake_user = True
    for o in (tr, tl, rr, rl, pr, pl):
        bpy.data.objects.remove(o)
    bpy.data.actions.remove(work)
    # 4. how far each fist strays from its place on the haft, and how close the weapon comes to the head, neck
    # and torso, every half frame (60 a second), with the weapon placed from the fists as the game does
    worst = 0.0; worst_at = 0.0; worst_side = ""
    near = {"head": 9.0, "neck": 9.0, "torso": 9.0}; near_at = {}
    for f in range(frames + 1):
        G, D = hafts[f]
        for sub in ((0.0, 0.5) if f < frames else (0.0,)):
            scene.frame_set(start + f, subframe=sub)
            r = fist(arm, "r"); l = fist(arm, "l")
            if sub == 0.0:
                for side, x, want in (("right", r, G), ("left", l, G + D * spacing(f / FPS))):
                    o = (x - want).length
                    if o > worst: worst = o; worst_at = f / FPS; worst_side = side
            for part, c in clearance(arm, kind, r, l).items():
                if c < near[part]: near[part] = c; near_at[part] = round((f + sub) / FPS, 3)
    return {"name": name, "base": base, "frames": frames + 1, "seconds": frames / FPS, "spacing_m": [round(min(spacing(f / FPS) for f in range(frames + 1)), 3), round(max(spacing(f / FPS) for f in range(frames + 1)), 3)],
            "fist_off_haft_m": round(worst, 4), "fist_off_haft_at_s": round(worst_at, 3), "fist_off_haft_side": worst_side,
            "clearance_m": {k: round(v, 4) for k, v in near.items()}, "clearance_at_s": near_at}


def fist(arm, side):
    """The fist's middle: FIST_ALONG toward the knuckles and FIST_PALM under the palm from the wrist."""
    mw = arm.matrix_world @ arm.pose.bones["hand_" + side].matrix
    return mw.translation + mw.to_3x3().normalized() @ Vector(((-1 if side == "r" else 1) * FIST_PALM, FIST_ALONG, 0))


def up(arm, bone):
    """A bone's up (the rest pose's world up) as posed."""
    pb = arm.pose.bones[bone]
    rot = (arm.matrix_world @ pb.matrix @ pb.bone.matrix_local.inverted() @ arm.matrix_world.inverted()).to_3x3().normalized()
    return rot @ Vector((0, 0, 1))


def capsules(arm):
    at = lambda b: (arm.matrix_world @ arm.pose.bones[b].matrix).translation
    h = at(HEAD[0]); hu = up(arm, HEAD[0])
    s3 = at(TORSO[1])
    return {"head": (h + hu * HEAD[1], h + hu * HEAD[2], HEAD[3], HEAD[4]),
            "neck": (at(NECK[0]), at(NECK[1]), NECK[2], NECK[3]),
            "torso": (at(TORSO[0]), s3 + up(arm, TORSO[1]) * 0.08, TORSO[2], TORSO[3])}


def clearance(arm, kind, r, l):
    """Each body part's room (metres past its radius) to the weapon's boxes, held from fist r to fist l."""
    spacing, sc, grip, boxes = WEAPONS[kind]
    D = (l - r).normalized()
    hr = (arm.matrix_world @ arm.pose.bones["hand_r"].matrix).to_3x3().normalized()
    E = hr @ Vector((0, 1, 0)); E = (E - D * E.dot(D)).normalized(); Z = E.cross(D)
    O = r - D * (sc * grip)
    out = {}
    for part, (a, b, rad, _) in capsules(arm).items():
        best = 9.0
        n = max(2, int((b - a).length / 0.01) + 1)
        for i in range(n):
            p = a.lerp(b, i / (n - 1)) - O
            q = (p.dot(E) / sc, p.dot(D) / sc, p.dot(Z) / sc)
            for x0x1 in boxes:
                y0, y1, x0, x1, z0, z1 = x0x1
                dx = max(x0 - q[0], 0, q[0] - x1); dy = max(y0 - q[1], 0, q[1] - y1); dz = max(z0 - q[2], 0, q[2] - z1)
                best = min(best, sc * math.sqrt(dx * dx + dy * dy + dz * dz) - rad)
        out[part] = best
    return out


def main():
    a = sys.argv[sys.argv.index("--") + 1:]
    ual1, out = Path(a[0]), Path(a[1]); out.mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.context.scene.render.fps = FPS
    bpy.ops.import_scene.gltf(filepath=str(ual1))
    arm = armature()
    sources = {x.name for x in bpy.data.actions}
    closed = fingers_closed(arm)
    idle_frames = int(round((bpy.data.actions["Idle_Loop"].frame_range[1] - bpy.data.actions["Idle_Loop"].frame_range[0])))
    run_frames = int(round((bpy.data.actions["Jog_Fwd_Loop"].frame_range[1] - bpy.data.actions["Jog_Fwd_Loop"].frame_range[0])))
    report = []
    for kind in WEAPONS:
        report.append(author(arm, f"TwoHand_{kind}_Idle_Loop", "Idle_Loop", idle_frames, kind, still(HOLD), closed))
        report.append(author(arm, f"TwoHand_{kind}_Run_Loop", "Jog_Fwd_Loop", run_frames, kind, still(RUN), closed))
        report.append(author(arm, f"TwoHand_{kind}_Cleave", "Idle_Loop", int(CLEAVE_SECONDS * FPS), kind, lambda t, k=kind: keyed(CLEAVE[k], t), closed,
                             lambda t, k=kind: slide(k, t)))
    print(json.dumps(report))
    bad = [c["name"] for c in report if c["fist_off_haft_m"] > 0.01 or c["clearance_m"]["head"] < CLEAR_HEAD
           or c["clearance_m"]["neck"] < CLEAR_HEAD or c["clearance_m"]["torso"] < 0.0]
    if bad:
        raise SystemExit("two_hand_clips: a fist leaves its haft, or the weapon comes within 8 cm of the head or neck "
                         "or enters the torso, in " + ", ".join(bad))
    for x in list(bpy.data.actions):
        if x.name in sources:
            bpy.data.actions.remove(x)
    for o in list(bpy.data.objects):
        if o.type == 'MESH' and o.parent is None:
            bpy.data.objects.remove(o)   # the library's preview sphere
    arm.animation_data.action = None
    bpy.ops.wm.save_as_mainfile(filepath=str(out / "two-hand-clips.blend"), compress=True)
    bpy.ops.object.select_all(action='SELECT')
    bpy.ops.export_scene.gltf(filepath=str(out / "two-hand-clips.glb"), export_format='GLB', use_selection=True,
                              export_animations=True, export_animation_mode='ACTIONS', export_force_sampling=True,
                              export_frame_step=1, export_anim_slide_to_zero=True, export_optimize_animation_size=False,
                              export_yup=True, export_materials='NONE', export_def_bones=False)
    (out / "report.json").write_text(json.dumps({"blender_version": bpy.app.version_string, "fps": FPS, "impact": IMPACT,
                                                 "clips": report}, indent=1) + "\n")


main()
