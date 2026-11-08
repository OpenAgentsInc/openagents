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
    --python verse/tools/assets/two_hand_clips.py -- UAL1_Standard.glb OUT_DIR

writes OUT_DIR/two-hand-clips.blend (the source), OUT_DIR/two-hand-clips.glb (the rig, its mesh and
the six clips) and OUT_DIR/report.json (each clip's frames and how far a fist strays from its haft).
Never run source-pack scripts.
"""
import bpy, json, math, sys
from pathlib import Path
from mathutils import Vector, Matrix

FPS = 30
IMPACT = 0.46                 # share of the cleave at its impact
FIST_ALONG = 0.08             # metres from the wrist to the fist's middle, toward the knuckles
FIST_PALM = 0.035             # and toward the palm
KINDS = (("Axe", 0.40), ("Sword", 0.16))
FINGERS = ("thumb_", "index_", "middle_", "ring_", "pinky_")


def lerp(a, b, t): return a + (b - a) * t
def smooth(t): t = min(max(t, 0.0), 1.0); return t * t * (3 - 2 * t)


# The haft in spine_03's space (+X the character's left, +Y up, +Z ahead): the right hand's grip point
# and the direction up the haft.
READY = (Vector((-0.10, -0.30, 0.24)), Vector((0.55, 0.62, 0.22)))
RUN = (Vector((-0.10, -0.26, 0.22)), Vector((0.52, 0.70, 0.18)))
# The cleave: (seconds, grip, direction, spine lean forward (radians), spine twist to the left)
CLEAVE = [
    (0.00, READY[0], READY[1], 0.00, 0.00),
    (0.30, Vector((-0.12, 0.26, 0.02)), Vector((0.20, 0.55, -0.80)), -0.10, -0.22),   # wound up over the right shoulder
    (0.40, Vector((-0.10, 0.30, 0.00)), Vector((0.18, 0.62, -0.76)), -0.12, -0.25),   # held, the anticipation
    (0.55, Vector((-0.06, -0.06, 0.38)), Vector((0.06, -0.10, 1.00)), 0.22, 0.02),    # impact: straight ahead, chest high
    (0.75, Vector((0.02, -0.40, 0.26)), Vector((0.10, -0.80, 0.55)), 0.38, 0.14),     # follow-through, down to the left
    (1.20, READY[0], READY[1], 0.00, 0.00),
]
CLEAVE_SECONDS = 1.20


def cleave_at(t):
    for k in range(len(CLEAVE) - 1):
        a, b = CLEAVE[k], CLEAVE[k + 1]
        if a[0] <= t <= b[0]:
            u = smooth((t - a[0]) / (b[0] - a[0]))
            return a[1].lerp(b[1], u), a[2].lerp(b[2], u).normalized(), lerp(a[3], b[3], u), lerp(a[4], b[4], u)
    last = CLEAVE[-1]
    return last[1], last[2].normalized(), last[3], last[4]


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


def author(arm, name, base, frames, spacing, haft, closed):
    scene = bpy.context.scene
    work = bpy.data.actions[base].copy(); work.name = name + "_work"
    assign(arm, work)
    start = int(bpy.data.actions[base].frame_range[0])
    spines = ("spine_01", "spine_02", "spine_03")
    # 1. the spine's lean and twist on top of the base, and the closed fingers
    poses = []
    for f in range(frames + 1):
        scene.frame_set(start + f)
        _, _, lean, twist = haft(f / FPS)
        row = {}
        for s in spines:
            pb = arm.pose.bones[s]
            # a third of the lean and twist at each spine joint: about the bone's own X (lean) and Y (twist)
            delta = Matrix.Rotation(-lean / 3, 4, 'X') @ Matrix.Rotation(twist / 3, 4, 'Y')
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
        g, d, _, _ = haft(f / FPS)
        m = arm.matrix_world @ arm.pose.bones["spine_03"].matrix
        r3 = m.to_3x3().normalized()
        G = m @ g; D = (r3 @ d).normalized()
        E = (r3 @ Vector((1, 0, 0)).cross(d)).normalized()   # the edge: across the swing, ahead at impact
        E = (E - D * E.dot(D)).normalized()
        X = E.cross(D)
        rot = Matrix((X, E, D)).transposed().to_quaternion()   # bone X, Y (knuckles) and Z (thumb)
        GL = G + D * spacing
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
    # 4. how far each fist strays from the haft's line
    worst = 0.0
    for f in range(frames + 1):
        scene.frame_set(start + f)
        G, D = hafts[f]
        for side, sign in (("r", -1), ("l", 1)):
            pb = arm.pose.bones["hand_" + side]
            mw = arm.matrix_world @ pb.matrix
            fist = mw.translation + mw.to_3x3().normalized() @ Vector((sign * FIST_PALM, FIST_ALONG, 0))
            off = fist - G
            worst = max(worst, (off - D * off.dot(D)).length)
    return {"name": name, "base": base, "frames": frames + 1, "seconds": frames / FPS, "spacing_m": spacing,
            "fist_off_haft_m": round(worst, 4)}


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
    for kind, spacing in KINDS:
        report.append(author(arm, f"TwoHand_{kind}_Idle_Loop", "Idle_Loop", idle_frames, spacing, lambda t: (READY[0], READY[1].normalized(), 0.0, 0.0), closed))
        report.append(author(arm, f"TwoHand_{kind}_Run_Loop", "Jog_Fwd_Loop", run_frames, spacing, lambda t: (RUN[0], RUN[1].normalized(), 0.0, 0.0), closed))
        report.append(author(arm, f"TwoHand_{kind}_Cleave", "Idle_Loop", int(CLEAVE_SECONDS * FPS), spacing, cleave_at, closed))
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
    print(json.dumps(report))


main()
