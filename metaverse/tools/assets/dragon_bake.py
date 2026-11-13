# Firm reason: 6 (Blender's bpy evaluates the dragon's rig and its glTF animations; it has no C API).
"""Bake Shapechange's dragon (assets/verse/beasts/dragon.gltf, CC0) into portable frame records.

    Blender -b --factory-startup --disable-autoexec --python metaverse/tools/assets/dragon_bake.py -- \
        assets/verse/beasts/dragon.gltf OUT_DIR

The clips and their frame counts are everglade/dragon-clips.coax's (DR_CLIP_NAME, DR_FRAMES,
DR_LOOPS), read from that file so the two never disagree. A loop's frame i is i/n of the way round
it, a clip played once i/(n-1) of the way to its last pose. Frame k of all of them, in that order,
is OUT_DIR/dragon-k.bin, the record animal_bake.py writes: the corner count (u32) and the clip's
seconds (f32), then per triangle corner x, y, z, the flat face normal (6 x f32, glTF space: +y up,
the dragon facing +z, its feet at y 0) and an sRGB colour from the corner's material
(baseColorFactor) whose fourth byte is 255, or 128 on the glowing eye material (Dragon_Eye).
OUT_DIR/saddle.txt holds the `saddle` bone's head a line a frame, in the same order and space: where
a rider sits as the clip moves the chest (everglade/dragon-clips.coax DR_SEAT).
"""
import bpy, math, re, struct, sys
from pathlib import Path

args = sys.argv[sys.argv.index('--') + 1:]
source, output = Path(args[0]), Path(args[1])
output.mkdir(parents=True, exist_ok=True)
clips_src = (Path(__file__).resolve().parents[3] / 'everglade' / 'dragon-clips.coax').read_text()
def table(name, kind):
    body = re.search(name + r' :: \[\d+\] ' + kind + r'\.\[(.*?)\];', clips_src).group(1)
    return [x.strip().strip('"') for x in body.split(',')]
names = table('DR_CLIP_NAME', 'string')
counts = [int(x) for x in table('DR_FRAMES', 'int')]
loops = [x == 'true' for x in table('DR_LOOPS', 'bool')]

def srgb(v):
    return max(0, min(255, round(255 * (12.92*v if v <= .0031308 else 1.055*v**(1/2.4)-.055))))

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(source))
arm = next(o for o in bpy.context.scene.objects if o.type == 'ARMATURE')
arm.animation_data_create()
meshes = [o for o in bpy.context.scene.objects if o.type == 'MESH' and o.parent == arm]   # not the importer's bone shapes
def colour(mat):
    node = mat.node_tree.nodes.get('Principled BSDF') if mat.node_tree else None
    rgb = node.inputs['Base Color'].default_value[:3] if node else mat.diffuse_color[:3]
    return bytes([*(srgb(v) for v in rgb), 128 if mat.name.startswith('Dragon_Eye') else 255])

def action(name):
    for a in bpy.data.actions:
        if a.name == name or a.name.startswith(name + '_') or a.name.startswith(name + '.'):
            return a
    raise ValueError(f'the dragon has no {name} clip: {[a.name for a in bpy.data.actions]}')

index = 0; corners = None; colours = {}; seats = []
for c, name in enumerate(names):
    act = action(name); arm.animation_data.action = act
    start, end = act.frame_range
    seconds = (end - start) / bpy.context.scene.render.fps
    n = counts[c]
    for i in range(n):
        at = start + (end - start) * i / (n if loops[c] else max(n - 1, 1))
        bpy.context.scene.frame_set(math.floor(at), subframe=at - math.floor(at))
        deps = bpy.context.evaluated_depsgraph_get(); data = bytearray(); count = 0
        seat = arm.matrix_world @ arm.pose.bones['saddle'].head
        seats.append(f'{seat.x:.4f} {seat.z:.4f} {-seat.y:.4f}')
        for obj in meshes:
            ev = obj.evaluated_get(deps); mesh = ev.to_mesh(); mesh.calc_loop_triangles()
            m = ev.matrix_world
            for tri in mesh.loop_triangles:
                mat = obj.material_slots[mesh.polygons[tri.polygon_index].material_index].material
                col = colours.setdefault(mat.name, colour(mat))
                ps = [m @ mesh.vertices[mesh.loops[li].vertex_index].co for li in tri.loops]
                nrm = (ps[1] - ps[0]).cross(ps[2] - ps[0])
                nrm = nrm.normalized() if nrm.length > 1e-12 else nrm
                for p in ps:
                    data.extend(struct.pack('<6f4s', p.x, p.z, -p.y, nrm.x, nrm.z, -nrm.y, col)); count += 1
            ev.to_mesh_clear()
        if corners is None: corners = count
        if count != corners or count == 0: raise ValueError(f'unstable dragon topology: {count} != {corners}')
        (output / f'dragon-{index}.bin').write_bytes(struct.pack('<If', count, seconds) + data)
        index += 1
    print(f'{name}: {n} frames, {seconds:.4f} s', flush=True)
(output / 'saddle.txt').write_text('\n'.join(seats) + '\n')
print(f'dragon: {index} frames of {corners} corners ({corners // 3} triangles)', flush=True)
