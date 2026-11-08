# Firm reason: 6 (Blender's Python-only bpy SDK imports, applies scene transforms, and exports glTF).
import bpy
import os
import sys

source, output = sys.argv[sys.argv.index('--') + 1:]
os.makedirs(output, exist_ok=True)
for filename in sorted(os.listdir(source)):
    if not filename.endswith('.glb'):
        continue
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=os.path.join(source, filename))
    meshes = [o for o in bpy.context.scene.objects if o.type == 'MESH']
    bpy.ops.object.select_all(action='DESELECT')
    for obj in meshes:
        matrix = obj.matrix_world.copy()
        obj.parent = None
        obj.matrix_world = matrix
        obj.select_set(True)
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    if not meshes:
        raise RuntimeError('No meshes: ' + filename)
    bpy.context.view_layer.objects.active = meshes[0]
    bpy.ops.object.join()
    dest = os.path.join(output, filename[:-4])
    os.makedirs(dest, exist_ok=True)
    bpy.ops.export_scene.gltf(filepath=os.path.join(dest, 'model.gltf'), export_format='GLTF_SEPARATE', use_selection=True, export_animations=False)
