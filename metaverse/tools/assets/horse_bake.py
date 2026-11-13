# Firm reason: 6 (Blender bpy evaluates the vendor's rig, bone attachments and animation).
"""Bake the rideable white horse (everglade/docs/mounts-spec.md) into portable frames.

Quaternius Ultimate Animated Animals, Horse_White (CC0 1.0). Its Idle, Walk and Gallop clips,
HB_FRAMES frames each round their loop, as Horse_White-mount-N.bin (N = clip * HB_FRAMES + frame),
the same triangle record as animal_bake.py (<If count, seconds> then <6f4s> a corner: position
x, z, -y, the normal likewise, sRGB colour). The coat is lightened to white (the source's
Horse_White is a dun grey). saddle.txt has a line a frame: the saddle point (the Torso2 bone's
head, where the rider sits) x y z in the same space, then the clip's seconds.

Run Blender in background with --disable-autoexec; the source files are data only:
  Blender --background --factory-startup --disable-autoexec --python-exit-code 1 \
    --python metaverse/tools/assets/horse_bake.py -- ANIMALS_SOURCE_ROOT OUTPUT_DIR
"""
import bpy, json, math, struct, sys
from pathlib import Path

args = sys.argv[sys.argv.index('--') + 1:]
root, output = Path(args[0]), Path(args[1])
output.mkdir(parents=True, exist_ok=True)
FRAMES = 12
CLIPS = ['Idle', 'Walk', 'Gallop']
# the white coat: the body, its lighter patches, the mane and tail; hooves, muzzle and eyes keep theirs
WHITE = {'Main': (0.86, 0.86, 0.83), 'Main_Light': (0.74, 0.74, 0.72), 'Hair': (0.80, 0.78, 0.72)}
SEAT_BONE = 'Torso2'

def srgb(v):
    return max(0, min(255, round(255 * (12.92*v if v <= .0031308 else 1.055*v**(1/2.4)-.055))))

bpy.ops.wm.read_factory_settings(use_empty=True)
path = root / 'glTF' / 'Horse_White.gltf'
doc = json.loads(path.read_text())
colors = {}
for m in doc['materials']:
    rgb = WHITE.get(m['name'], m.get('pbrMetallicRoughness', {}).get('baseColorFactor', [1, 1, 1, 1])[:3])
    colors[m['name']] = bytes([*(srgb(v) for v in rgb), 255])
bpy.ops.import_scene.gltf(filepath=str(path))
arm = next(o for o in bpy.context.scene.objects if o.type == 'ARMATURE')
arm.animation_data_create()
meshes = sorted((o for o in bpy.context.scene.objects if o.type == 'MESH' and o.parent == arm), key=lambda o: o.name)
counts = []; seats = []
for ci, clip in enumerate(CLIPS):
    action = bpy.data.actions.get(clip)
    if action is None: raise ValueError(f'Horse_White has no {clip} clip')
    arm.animation_data.action = action
    span = action.frame_range[1] - action.frame_range[0]
    seconds = span / bpy.context.scene.render.fps
    for frame in range(FRAMES):
        at = action.frame_range[0] + span * frame / FRAMES
        bpy.context.scene.frame_set(math.floor(at), subframe=at - math.floor(at))
        deps = bpy.context.evaluated_depsgraph_get(); data = bytearray(); count = 0
        ea = arm.evaluated_get(deps)
        seat = ea.matrix_world @ ea.pose.bones[SEAT_BONE].head
        seats.append(f'{seat.x:.5f} {seat.z:.5f} {-seat.y:.5f} {seconds:.5f}')
        for obj in meshes:
            ev = obj.evaluated_get(deps); mesh = ev.to_mesh(); mesh.calc_loop_triangles()
            normal_matrix = ev.matrix_world.to_3x3().inverted().transposed()
            for triangle in mesh.loop_triangles:
                mat = mesh.materials[mesh.polygons[triangle.polygon_index].material_index]
                color = colors[mat.name]
                for li in triangle.loops:
                    loop = mesh.loops[li]; p = ev.matrix_world @ mesh.vertices[loop.vertex_index].co
                    normal = (normal_matrix @ loop.normal).normalized()
                    data.extend(struct.pack('<6f4s', p.x, p.z, -p.y, normal.x, normal.z, -normal.y, color)); count += 1
            ev.to_mesh_clear()
        counts.append(count)
        (output / f'Horse_White-mount-{ci * FRAMES + frame}.bin').write_bytes(struct.pack('<If', count, seconds) + data)
    print(f'Horse_White {clip}: {counts[-1]} triangle vertices, {seconds:.3f}s', flush=True)
if len(set(counts)) != 1 or not counts[0]: raise ValueError(f'Horse_White: unstable frame topology {counts}')
(output / 'saddle.txt').write_text('\n'.join(seats) + '\n')
