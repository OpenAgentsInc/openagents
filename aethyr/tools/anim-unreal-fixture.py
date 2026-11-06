# Firm reason: 6 (Unreal Editor FBX import SDK creates an original test fixture).
# The input is the original triangle and single-bone clip made by our Blender fixture.
import os
import unreal

task = unreal.AssetImportTask()
task.filename = os.environ['ANIMLAB_FIXTURE_FBX']
task.destination_path = '/Game/AnimationLabFixture'
task.automated = True
task.save = True
task.replace_existing = True
options = unreal.FbxImportUI()
options.import_as_skeletal = True
options.mesh_type_to_import = unreal.FBXImportType.FBXIT_SKELETAL_MESH
options.import_mesh = True
options.import_animations = True
options.skeletal_mesh_import_data.import_uniform_scale = 0.01
task.options = options
unreal.AssetToolsHelpers.get_asset_tools().import_asset_tasks([task])
if not task.imported_object_paths:
    raise RuntimeError('Original animation fixture did not import')
unreal.log('Animation Lab original fixture import completed')
