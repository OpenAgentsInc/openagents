# Firm reason: 6 (Blender bpy evaluates the vendor's rig, bone attachments and animation).
"""Bake the original in-place Gallop clips into portable, triangulated frame data.

Run Blender in background with --disable-autoexec; source files are data only.
"""
import bpy, json, math, struct, sys
from pathlib import Path
from mathutils import Vector

root, output = map(Path, sys.argv[sys.argv.index('--') + 1:])
output.mkdir(parents=True, exist_ok=True)
names = ['Alpaca','Bull','Cow','Deer','Donkey','Fox','Horse','Horse_White','Husky','ShibaInu','Stag','Wolf']
frames = 10
def srgb(v):
    return max(0, min(255, round(255 * (12.92*v if v <= .0031308 else 1.055*v**(1/2.4)-.055))))

for name in names:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    path = root / 'glTF' / (name + '.gltf')
    doc = json.loads(path.read_text())
    colors = {m['name']: bytes([*(srgb(v) for v in m.get('pbrMetallicRoughness', {}).get('baseColorFactor', [1,1,1,1])[:3]),255]) for m in doc['materials']}
    bpy.ops.import_scene.gltf(filepath=str(path))
    arm = next(o for o in bpy.context.scene.objects if o.type == 'ARMATURE')
    action = bpy.data.actions.get('Gallop')
    if action is None: raise ValueError(f'{name} has no Gallop clip')
    arm.animation_data_create(); arm.animation_data.action = action
    meshes = sorted((o for o in bpy.context.scene.objects if o.type == 'MESH' and o.parent == arm), key=lambda o:o.name)
    if name == 'Stag':
        horns = next(o for o in meshes if o.name == 'Stag_Horns')
        if horns.parent_type != 'BONE' or horns.parent_bone != 'Head':
            raise ValueError('Stag antlers lost their head-bone attachment')
    duration = (action.frame_range[1] - action.frame_range[0]) / bpy.context.scene.render.fps
    counts = []
    for frame in range(frames):
        at = action.frame_range[0] + (action.frame_range[1]-action.frame_range[0])*frame/frames
        bpy.context.scene.frame_set(math.floor(at), subframe=at-math.floor(at))
        deps = bpy.context.evaluated_depsgraph_get(); data = bytearray(); count = 0
        if name == 'Stag':
            head = arm.evaluated_get(deps).matrix_world @ arm.evaluated_get(deps).pose.bones['Head'].head
            if (horns.evaluated_get(deps).matrix_world.translation - head).length > 1.0:
                raise ValueError('Stag antlers detached from the animated head')
        for obj in meshes:
            ev = obj.evaluated_get(deps); mesh = ev.to_mesh(); mesh.calc_loop_triangles()
            normal_matrix = ev.matrix_world.to_3x3().inverted().transposed()
            for triangle in mesh.loop_triangles:
                mat = mesh.materials[mesh.polygons[triangle.polygon_index].material_index]
                color = colors[mat.name]
                for li in triangle.loops:
                    loop = mesh.loops[li]; p = ev.matrix_world @ mesh.vertices[loop.vertex_index].co
                    normal = (normal_matrix @ loop.normal).normalized()
                    data.extend(struct.pack('<6f4s', p.x,p.z,-p.y,normal.x,normal.z,-normal.y,color));count += 1
            ev.to_mesh_clear()
        counts.append(count)
        (output / f'{name}-{frame}.bin').write_bytes(struct.pack('<If', count, duration) + data)
    if len(set(counts)) != 1 or not counts[0]: raise ValueError(f'{name}: unstable frame topology {counts}')
    print(f'{name}: {counts[0]} triangle vertices, {duration:.3f}s', flush=True)
