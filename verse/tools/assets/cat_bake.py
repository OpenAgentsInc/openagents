# Firm reason: 6 (Blender bpy evaluates the vendor's rig, its IK legs and animation).
"""Bake Everglade's cat (everglade/creatures/cat.coax) into portable frames.

No CC0 animated cat with a sit clip exists in the packs we hold, so the cat is made here from
Quaternius Ultimate Animated Animals' Fox (CC0 1.0), the same rig family as the horse and the wolf:
the muzzle is shortened, the ears made smaller and the brush thinned to a cat's tail, and the coat
recoloured grey tabby with a white muzzle, chest and paws. Its Idle and Walk clips are the Fox's; its
Sit is posed here on the rig's own IK: the hips drop and the body pitches up about them, the front
paws stay planted under the chest, the hind legs fold under the hips, the neck levels the head and the
tail curls round on the ground. Sit plays over the Idle clip, so the cat breathes and its ears move.

Each clip is CB_FRAMES frames round its loop, as Cat-e2-N.bin (N = clip * CB_FRAMES + frame, clips
Idle, Walk, Sit), the same triangle record as animal_bake.py (<If count, seconds> then <6f4s> a corner:
position x, z, -y, the normal likewise, sRGB colour), in the source's units.

Run Blender in background with --disable-autoexec; the source files are data only:
  Blender --background --factory-startup --disable-autoexec --python-exit-code 1 \
    --python verse/tools/assets/cat_bake.py -- ANIMALS_SOURCE_ROOT OUTPUT_DIR [PREVIEW_DIR]
ANIMALS_SOURCE_ROOT is the ultimate-animals-source snapshot (it holds Blends/Fox.blend).
It also writes Cat.glb, the shaped cat on its rig with the Fox's Idle and Walk clips (its colours as
materials), the interchange artifact the asset registry records (openagents/beasts/cat).
With PREVIEW_DIR it also renders a still of each clip's first frame there, for a look.
"""
import bpy, math, struct, sys
from pathlib import Path
from mathutils import Matrix, Vector

args = sys.argv[sys.argv.index('--') + 1:]
root, output = Path(args[0]), Path(args[1])
preview = Path(args[2]) if len(args) > 2 else None
output.mkdir(parents=True, exist_ok=True)
FRAMES = 10
CLIPS = [('Idle', False), ('Walk', False), ('Idle', True)]   # the third is Sit, over the Idle clip
# grey tabby: the coat, a white muzzle, chest and paws, dark ear tips and nose
COAT = {'Main': (0.26, 0.25, 0.24), 'Main_Light': (0.80, 0.79, 0.76), 'Black': (0.11, 0.11, 0.12),
        'Grey': (0.62, 0.62, 0.60), 'Eyes': (0.02, 0.02, 0.02)}

def srgb(v):
    return max(0, min(255, round(255 * (12.92*v if v <= .0031308 else 1.055*v**(1/2.4)-.055))))

bpy.ops.wm.open_mainfile(filepath=str(root / 'Blends' / 'Fox.blend'), load_ui=False)
sc = bpy.context.scene
arm = next(o for o in sc.objects if o.type == 'ARMATURE')
mesh_obj = next(o for o in sc.objects if o.type == 'MESH' and o.parent == arm)
arm.animation_data.use_nla = False   # the strips would key the sit back; one clip plays at a time
for o in list(sc.objects):
    if o.type == 'MESH' and o is not mesh_obj: bpy.data.objects.remove(o)
colors = {}
for m in mesh_obj.data.materials:
    if m.name not in COAT: raise ValueError(f'Fox has an unknown material {m.name}')
    colors[m.name] = bytes([*(srgb(v) for v in COAT[m.name]), 255])

# ---- the cat's shape, on the rest mesh and the rig together (the clips are keyed against the rest
# pose, so they play on the new shape): the legs squashed shorter below the shoulder, then the face
LEG_TOP = 1.45; LEG_K = 0.72      # below LEG_TOP the height is LEG_K of the fox's
def squash(p):
    q = p.copy(); q.z = q.z * LEG_K if q.z < LEG_TOP else q.z - LEG_TOP * (1 - LEG_K); return q
bpy.context.view_layer.objects.active = arm
bpy.ops.object.mode_set(mode='EDIT')
ends = {b.name: (squash(b.head), squash(b.tail)) for b in arm.data.edit_bones}
for b in arm.data.edit_bones:
    if b.use_connect: b.tail = ends[b.name][1]
for b in arm.data.edit_bones:
    if not b.use_connect: b.head, b.tail = ends[b.name]
bpy.ops.object.mode_set(mode='OBJECT')
bones = arm.data.bones
groups = {g.index: g.name for g in mesh_obj.vertex_groups}
def weight_of(v, prefix):
    return sum(g.weight for g in v.groups if groups.get(g.group, '').startswith(prefix))
head = bones['Head'].head_local
eye_y = head.y - 0.16            # the muzzle starts in front of the eyes
for v in mesh_obj.data.vertices:
    co = squash(v.co)
    hw = weight_of(v, 'Head') + weight_of(v, 'Neck3')
    if hw > 0.5 and co.y < eye_y:
        # the muzzle: pulled back toward the eyes and a little up, so the face is round and short
        co.y = eye_y + (co.y - eye_y) * 0.22
        co.z = co.z + (head.z - 0.12 - co.z) * 0.22
    if hw > 0.5:
        # a cat's head is broad for its size
        co.x *= 1.22
    ew = weight_of(v, 'Ear')
    if ew > 0.5:
        base = bones['Ear1.' + ('L' if co.x > 0 else 'R')].head_local
        co = base + (co - base) * 0.66
    tw = weight_of(v, 'Tail')
    if tw > 0.5:
        # the brush thinned to a cat's tail round its bones' line
        best = None
        for name in ('Tail1', 'Tail2', 'Tail3', 'Tail4', 'Tail5', 'Tail6', 'Tail7', 'Tail8'):
            a, b = bones[name].head_local, bones[name].tail_local
            ab = b - a; t = max(0.0, min(1.0, (co - a).dot(ab) / ab.length_squared)); p = a + ab * t
            if best is None or (co - p).length < (co - best).length: best = p
        co = best + (co - best) * 0.42
    v.co = co
mesh_obj.data.update()
# the tabby's stripes: bands of darker coat across the back, the flanks, the legs and the tail (the
# face stays plain), each face by where it sits at rest
STRIPE = bytes([*(srgb(v) for v in (0.085, 0.085, 0.09)), 255])
stripe = set()
for poly in mesh_obj.data.polygons:
    if mesh_obj.data.materials[poly.material_index].name != 'Main': continue
    v0 = mesh_obj.data.vertices[poly.vertices[0]]
    if weight_of(v0, 'Head') + weight_of(v0, 'Neck') + weight_of(v0, 'Ear') > 0.3: continue
    c = poly.center
    band = math.sin(c.y * 7.0 + abs(c.x) * 2.5 - c.z * 1.5)
    if band > 0.45: stripe.add(poly.index)

# ---- the sit: a pose laid over whatever the clip has keyed at this frame
PITCH = math.radians(38)   # the body tilts up about the hips
DROP = 0.84                # and the hips come down
def pose_matrix(name):
    return arm.pose.bones[name].matrix.copy()
def set_pose(name, m):
    arm.pose.bones[name].matrix = m
    bpy.context.view_layer.update()
def about(pivot, rot):
    return Matrix.Translation(pivot) @ rot @ Matrix.Translation(-pivot)
def sit():
    hips = arm.pose.bones['BackShoulder.L'].head.copy(); hips.x = 0
    lift = Matrix.Translation(Vector((0, 0, -DROP))) @ about(hips, Matrix.Rotation(-PITCH, 4, 'X'))
    set_pose('Body', lift @ pose_matrix('Body'))
    # the front paws under the chest, on the ground
    for side in ('L', 'R'):
        shoulder = arm.pose.bones['FrontUpperLeg.' + side].head
        ik = pose_matrix('IKFrontLeg.' + side)
        ik.translation = Vector((ik.translation.x, shoulder.y - 0.02, ik.translation.z))
        set_pose('IKFrontLeg.' + side, ik)
    # the hind paws flat on the ground beside the hips, a little forward
    for side in ('L', 'R'):
        hip = arm.pose.bones['BackLeg.' + side].head
        ik = pose_matrix('IKBackLeg.' + side)
        ik.translation = Vector((ik.translation.x * 1.15, hip.y - 0.45, bones['IKBackLeg.' + side].head_local.z))
        set_pose('IKBackLeg.' + side, ik)
    # the neck levels the head again: the cat looks ahead, not up
    neck = arm.pose.bones['Neck1'].head
    set_pose('Neck1', about(neck, Matrix.Rotation(PITCH * 0.55, 4, 'X')) @ pose_matrix('Neck1'))
    nb = arm.pose.bones['Neck3'].head
    set_pose('Neck3', about(nb, Matrix.Rotation(PITCH * 0.45, 4, 'X')) @ pose_matrix('Neck3'))
    # the tail lies along the ground and curls round the side
    for k, name in enumerate(('Tail1', 'Tail2', 'Tail3', 'Tail4', 'Tail5', 'Tail6', 'Tail7', 'Tail8')):
        p = arm.pose.bones[name].head
        bend = Matrix.Rotation(math.radians(24 if k == 0 else 2), 4, 'X') @ Matrix.Rotation(math.radians(0 if k < 2 else 22), 4, 'Z')
        set_pose(name, about(p, bend) @ pose_matrix(name))

counts = []
for ci, (clip, sitting) in enumerate(CLIPS):
    action = bpy.data.actions.get(clip)
    if action is None: raise ValueError(f'Fox has no {clip} clip')
    arm.animation_data.action = action
    span = action.frame_range[1] - action.frame_range[0]
    seconds = span / sc.render.fps
    for frame in range(FRAMES):
        at = action.frame_range[0] + span * frame / FRAMES
        arm.animation_data.action = action
        sc.frame_set(math.floor(at), subframe=at - math.floor(at))
        if sitting:
            # the clip's pose at this frame held, then the sit laid over it (an action would key it back)
            held = {pb.name: (pb.location.copy(), pb.rotation_quaternion.copy(), pb.scale.copy()) for pb in arm.pose.bones}
            arm.animation_data.action = None
            for pb in arm.pose.bones: pb.location, pb.rotation_quaternion, pb.scale = held[pb.name]
            bpy.context.view_layer.update(); sit()
        deps = bpy.context.evaluated_depsgraph_get(); data = bytearray(); count = 0
        ev = mesh_obj.evaluated_get(deps); mesh = ev.to_mesh(); mesh.calc_loop_triangles()
        normal_matrix = ev.matrix_world.to_3x3().inverted().transposed()
        for triangle in mesh.loop_triangles:
            color = STRIPE if triangle.polygon_index in stripe else colors[mesh.materials[mesh.polygons[triangle.polygon_index].material_index].name]
            for li in triangle.loops:
                loop = mesh.loops[li]; p = ev.matrix_world @ mesh.vertices[loop.vertex_index].co
                normal = (normal_matrix @ loop.normal).normalized()
                data.extend(struct.pack('<6f4s', p.x, p.z, -p.y, normal.x, normal.z, -normal.y, color)); count += 1
        ev.to_mesh_clear()
        counts.append(count)
        (output / f'Cat-e2-{ci * FRAMES + frame}.bin').write_bytes(struct.pack('<If', count, seconds) + data)
        if preview and frame == 0:
            for view, yaw in (('side', 1.5708), ('front', 0.35)):
                cam = bpy.data.objects.get('cb_cam') or bpy.data.objects.new('cb_cam', bpy.data.cameras.new('cb_cam'))
                if cam.name not in sc.collection.objects: sc.collection.objects.link(cam)
                sc.camera = cam; c = Vector((0, -0.4, 1.4))
                cam.location = c + Vector((math.sin(yaw) * 7, -math.cos(yaw) * 7, 0.8))
                cam.rotation_euler = (c - cam.location).to_track_quat('-Z', 'Y').to_euler()
                sc.render.engine = 'BLENDER_WORKBENCH'; sc.display.shading.light = 'STUDIO'; sc.display.shading.color_type = 'MATERIAL'
                for m in mesh_obj.data.materials: m.diffuse_color = (*COAT[m.name], 1)
                sc.render.resolution_x = 640; sc.render.resolution_y = 480
                sc.render.filepath = str(preview / f'cat-{["idle", "walk", "sit"][ci]}-{view}.png')
                bpy.ops.render.render(write_still=True)
    print(f'Cat {["Idle", "Walk", "Sit"][ci]}: {counts[-1]} triangle vertices, {seconds:.3f}s', flush=True)
if len(set(counts)) != 1 or not counts[0]: raise ValueError(f'Cat: unstable frame topology {counts}')
# the registry's interchange artifact: the shaped cat at rest, its rig and the clips it plays
for m in mesh_obj.data.materials:
    m.use_nodes = True; bsdf = next((n for n in m.node_tree.nodes if n.type == 'BSDF_PRINCIPLED'), None)
    if bsdf: bsdf.inputs['Base Color'].default_value = (*COAT[m.name], 1)
keep = {'Idle', 'Walk'}
for a in list(bpy.data.actions):
    if a.name not in keep: bpy.data.actions.remove(a)
for o in list(sc.objects):
    if o not in (arm, mesh_obj): bpy.data.objects.remove(o)
arm.animation_data.action = None
bpy.ops.export_scene.gltf(filepath=str(output / 'Cat.glb'), export_format='GLB', export_animations=True, export_animation_mode='ACTIONS', export_apply=False)
