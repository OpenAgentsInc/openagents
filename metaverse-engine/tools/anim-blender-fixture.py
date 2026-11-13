# Firm reason: 6 (original FBX fixture creation through Blender's SDK).
import sys
import bpy
from math import pi

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.object.armature_add()
armature = bpy.context.object
armature.name = "MadeRig"
armature.data.bones[0].name = "Head"
mesh = bpy.data.meshes.new("MadeTriangle")
mesh.from_pydata([(-0.2, 0, 1), (0.2, 0, 1), (0, 0, 1.4)], [], [(0, 1, 2)])
character = bpy.data.objects.new("MadeCharacter", mesh)
bpy.context.collection.objects.link(character)
character.vertex_groups.new(name="Head").add([0, 1, 2], 1, "REPLACE")
modifier = character.modifiers.new("Skin", "ARMATURE")
modifier.object = armature
bone = armature.pose.bones["Head"]
bone.rotation_mode = "XYZ"
for frame, angle in [(1, 0), (31, pi / 2)]:
    bone.rotation_euler = (0, angle, 0)
    bone.keyframe_insert(data_path="rotation_euler", frame=frame)
bpy.context.scene.frame_start = 1
bpy.context.scene.frame_end = 31
bpy.context.scene.render.fps = 30
bpy.ops.export_scene.fbx(
    filepath=sys.argv[sys.argv.index("--") + 1],
    object_types={"ARMATURE", "MESH"},
    add_leaf_bones=False,
    bake_anim=True,
    bake_anim_use_all_actions=True,
)
