# Firm reason: 6 (Blender bpy evaluates the source monster rigs and bone attachments).
"""Bake compatible UAL animation frames for the two Bestiary Standard GLBs.

With a fourth argument `e2` it bakes Everglade II's goblin (the Puglin, everglade/server/creatures.coax)
instead: Idle_Loop, Jog_Fwd_Loop, Sword_Attack, Spell_Simple_Shoot and Death01, eight frames each, as
Puglin-e2-N.bin (N = clip * 8 + frame; the attack, shot and death run to their last pose), at a third of its
triangles (a Decimate ahead of the rig, so every frame keeps one topology).
"""
import bpy, math, struct, sys
from pathlib import Path

args = sys.argv[sys.argv.index('--') + 1:]
root, out, animations = map(Path, args[:3])
e2 = len(args) > 3 and args[3] == 'e2'
out.mkdir(parents=True, exist_ok=True)
for name in (('Puglin',) if e2 else ('Imp', 'Puglin')):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(root / 'Exports/GLB (Godot-Unreal)' / (name + '.glb')))
    arm = next(o for o in bpy.context.scene.objects if o.type == 'ARMATURE')
    meshes = sorted((o for o in bpy.context.scene.objects if o.type == 'MESH' and o.parent == arm), key=lambda o:o.name)
    if not meshes: raise ValueError(f'{name}: no character meshes')
    # The vendor declares these rigs compatible with UAL; bake a varied in-place cycle.
    bpy.ops.import_scene.gltf(filepath=str(animations))
    actions = ['Idle_Loop', 'Walk_Loop', 'Jog_Fwd_Loop', 'Dance_Loop', 'Sword_Attack', 'Spell_Simple_Shoot']
    if e2:
        actions = ['Idle_Loop', 'Jog_Fwd_Loop', 'Sword_Attack', 'Spell_Simple_Shoot', 'Death01']
        for obj in meshes:
            d = obj.modifiers.new('e2-decimate', 'DECIMATE'); d.ratio = 0.35
            while obj.modifiers[0] != d: bpy.context.view_layer.objects.active = obj; bpy.ops.object.modifier_move_up(modifier=d.name)
    arm.animation_data_create()
    for index in range(len(actions) * 8):
        action = bpy.data.actions.get(actions[index // 8])
        if action is None or len(action.slots) != 1: raise ValueError('missing compatible UAL action')
        arm.animation_data.action = action; arm.animation_data.action_slot = action.slots[0]
        once = e2 and index // 8 >= 2
        at = action.frame_range[0] + (action.frame_range[1] - action.frame_range[0]) * (index % 8) / (7 if once else 8)
        bpy.context.scene.frame_set(math.floor(at), subframe=at-math.floor(at))
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
        (out / (f'{name}-e2-{index}.bin' if e2 else f'{name}-{index}.bin')).write_bytes(struct.pack('<II',len(seen),len(indices)) + data + struct.pack('<'+'I'*len(indices),*indices))
        print(f'{name}: {len(seen)} vertices, {len(indices)//3} triangles', flush=True)
