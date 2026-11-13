# Firm reason: 6 (Blender bpy evaluates the vendor's rig, bone attachments and animation).
"""Bake the original in-place Gallop clips into portable, triangulated frame data.

Run Blender in background with --disable-autoexec; source files are data only.
With a third argument `e2` it bakes Everglade II's Wolf instead (its monsters, everglade/server/creatures.coax):
its Idle, Gallop, Attack and Death clips, eight frames each, as Wolf-e2-N.bin (N = clip * 8 + frame; Death's
frames run to its last pose, the others round their loop), and the Dungeon kit's Woodfire as one static
frame, Woodfire-e2.bin, when a fourth argument names the Dungeon source; the camp's props from the Fantasy
Props source (a fifth) and the guard's Sword from the Ultimate RPG source (a sixth), each one static frame.
"""
import bpy, json, math, struct, sys
from pathlib import Path
from mathutils import Vector

args = sys.argv[sys.argv.index('--') + 1:]
root, output = Path(args[0]), Path(args[1])
e2 = len(args) > 2 and args[2] == 'e2'
output.mkdir(parents=True, exist_ok=True)
names = ['Wolf'] if e2 else ['Alpaca','Bull','Cow','Deer','Donkey','Fox','Horse','Horse_White','Husky','ShibaInu','Stag','Wolf']
frames = 10
clips = [('Gallop', frames, False)]
if e2: clips = [('Idle', 8, False), ('Gallop', 8, False), ('Attack', 8, True), ('Death', 8, True)]
def srgb(v):
    return max(0, min(255, round(255 * (12.92*v if v <= .0031308 else 1.055*v**(1/2.4)-.055))))

for name in names:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    path = root / 'glTF' / (name + '.gltf')
    doc = json.loads(path.read_text())
    colors = {m['name']: bytes([*(srgb(v) for v in m.get('pbrMetallicRoughness', {}).get('baseColorFactor', [1,1,1,1])[:3]),255]) for m in doc['materials']}
    bpy.ops.import_scene.gltf(filepath=str(path))
    arm = next(o for o in bpy.context.scene.objects if o.type == 'ARMATURE')
    arm.animation_data_create()
    meshes = sorted((o for o in bpy.context.scene.objects if o.type == 'MESH' and o.parent == arm), key=lambda o:o.name)
    if name == 'Stag':
        horns = next(o for o in meshes if o.name == 'Stag_Horns')
        if horns.parent_type != 'BONE' or horns.parent_bone != 'Head':
            raise ValueError('Stag antlers lost their head-bone attachment')
    counts = []
    for index in range(sum(c[1] for c in clips)):
      ci = 0; frame = index
      while frame >= clips[ci][1]: frame -= clips[ci][1]; ci += 1
      clip, count, once = clips[ci]
      action = bpy.data.actions.get(clip)
      if action is None: raise ValueError(f'{name} has no {clip} clip')
      arm.animation_data.action = action
      duration = (action.frame_range[1] - action.frame_range[0]) / bpy.context.scene.render.fps
      if True:
        span = action.frame_range[1]-action.frame_range[0]
        at = action.frame_range[0] + span*frame/((count - 1) if once else count)
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
        (output / (f'{name}-e2-{index}.bin' if e2 else f'{name}-{frame}.bin')).write_bytes(struct.pack('<If', count, duration) + data)
    if len(set(counts)) != 1 or not counts[0]: raise ValueError(f'{name}: unstable frame topology {counts}')
    print(f'{name}: {counts[0]} triangle vertices, {duration:.3f}s', flush=True)

# Everglade II's static meshes (OBJ: data only), their triangles at rest in the same record as an animal frame:
# the Dungeon kit's Woodfire (a fourth argument names the Dungeon source), the Fantasy Props kit's camp pieces
# (a fifth, its source; each face takes its trim texture's colour at the face's middle) and the Ultimate RPG
# Sword the path's guard holds (a sixth). Each as <Name>-e2.bin.
def static(path, name):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.wm.obj_import(filepath=str(path))
    deps = bpy.context.evaluated_depsgraph_get(); data = bytearray(); count = 0; images = {}
    for obj in (o for o in bpy.context.scene.objects if o.type == 'MESH'):
        ev = obj.evaluated_get(deps); mesh = ev.to_mesh(); mesh.calc_loop_triangles()
        normal_matrix = ev.matrix_world.to_3x3().inverted().transposed()
        uvs = mesh.uv_layers.active.data if mesh.uv_layers.active else None
        for triangle in mesh.loop_triangles:
            mat = mesh.materials[mesh.polygons[triangle.polygon_index].material_index]
            node = mat.node_tree.nodes.get('Principled BSDF') if mat.use_nodes else None
            rgb = node.inputs['Base Color'].default_value[:3] if node else mat.diffuse_color[:3]
            color = bytes([*(srgb(v) for v in rgb), 255])
            tex = next((n.image for n in mat.node_tree.nodes if n.type == 'TEX_IMAGE' and n.image), None) if mat.use_nodes else None
            if tex is not None and uvs is not None:
                if tex.name not in images: images[tex.name] = (tex.size[0], tex.size[1], tex.pixels[:])
                w, h, px = images[tex.name]
                u = sum(uvs[li].uv[0] for li in triangle.loops) / 3; v = sum(uvs[li].uv[1] for li in triangle.loops) / 3
                x = min(w - 1, max(0, int((u % 1) * w))); y = min(h - 1, max(0, int((v % 1) * h))); at = (y * w + x) * 4
                color = bytes([*(max(0, min(255, round(px[at + k] * 255))) for k in range(3)), 255])
            for li in triangle.loops:
                loop = mesh.loops[li]; p = ev.matrix_world @ mesh.vertices[loop.vertex_index].co
                normal = (normal_matrix @ loop.normal).normalized()
                data.extend(struct.pack('<6f4s', p.x,p.z,-p.y,normal.x,normal.z,-normal.y,color)); count += 1
        ev.to_mesh_clear()
    (output / f'{name}-e2.bin').write_bytes(struct.pack('<If', count, 0) + data)
    print(f'{name}: {count} triangle vertices', flush=True)
if e2 and len(args) > 3: static(Path(args[3]) / 'OBJ' / 'Woodfire.obj', 'Woodfire')
if e2 and len(args) > 4:
    for name in ['Stall_Empty', 'Crate_Wooden', 'Crate_Metal', 'Barrel', 'Bench']: static(Path(args[4]) / 'Exports' / 'OBJ' / (name + '.obj'), name)
if e2 and len(args) > 5: static(Path(args[5]) / 'OBJ' / 'Sword.obj', 'Sword')
