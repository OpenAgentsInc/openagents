# Firm reason: 6 (Blender bpy evaluates the source monster rigs and bone attachments).
"""Bake the two locally licensed Bestiary Standard GLBs as static scene meshes."""
import bpy, struct, sys
from pathlib import Path

root, out, animations = map(Path, sys.argv[sys.argv.index('--') + 1:])
out.mkdir(parents=True, exist_ok=True)
for name in ('Imp', 'Puglin'):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(root / 'Exports/GLB (Godot-Unreal)' / (name + '.glb')))
    arm = next(o for o in bpy.context.scene.objects if o.type == 'ARMATURE')
    meshes = sorted((o for o in bpy.context.scene.objects if o.type == 'MESH' and o.parent == arm), key=lambda o:o.name)
    if not meshes: raise ValueError(f'{name}: no character meshes')
    # The vendor declares these rigs compatible with UAL; bind its idle to avoid
    # placing the monster in its authoring T-pose.
    bpy.ops.import_scene.gltf(filepath=str(animations))
    idle = bpy.data.actions.get('Idle_Loop')
    if idle is None: raise ValueError('UAL idle is missing')
    arm.animation_data_create(); arm.animation_data.action = idle
    if len(idle.slots) != 1: raise ValueError('UAL idle has an unexpected action slot')
    arm.animation_data.action_slot = idle.slots[0]
    bpy.context.scene.frame_set(8)
    deps = bpy.context.evaluated_depsgraph_get(); data = bytearray(); indices = []; seen = {}
    for obj in meshes:
        ev = obj.evaluated_get(deps); mesh = ev.to_mesh(); mesh.calc_loop_triangles()
        if not mesh.uv_layers.active: raise ValueError(f'{obj.name}: missing UVs')
        uv = mesh.uv_layers.active.data; normal_matrix = ev.matrix_world.to_3x3().inverted().transposed()
        for triangle in mesh.loop_triangles:
            for li in triangle.loops:
                loop = mesh.loops[li]; p = ev.matrix_world @ mesh.vertices[loop.vertex_index].co
                normal = (normal_matrix @ loop.normal).normalized(); t = uv[li].uv
                vertex = struct.pack('<8f',p.x,p.z,-p.y,normal.x,normal.z,-normal.y,t.x,1-t.y)
                if vertex not in seen:
                    seen[vertex] = len(seen); data.extend(vertex)
                indices.append(seen[vertex])
        ev.to_mesh_clear()
    if not indices or len(indices) % 3: raise ValueError(f'{name}: invalid topology')
    (out / f'{name}.bin').write_bytes(struct.pack('<II',len(seen),len(indices)) + data + struct.pack('<'+'I'*len(indices),*indices))
    print(f'{name}: {len(seen)} vertices, {len(indices)//3} triangles', flush=True)
