# Firm reason: 6 (Unreal Editor asset/glTF SDK has no C ABI).
# SDK effects only: read source data and write private exports; never play or save the source game.
import json
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'echo-source'
root.mkdir(parents=True, exist_ok=True)
base = '/Game/AncientContent/Characters/Echo/'
mesh = unreal.load_asset(base + 'Meshes/Echo')
if not isinstance(mesh, unreal.SkeletalMesh):
    raise RuntimeError('The original Echo skeletal mesh did not load')
options = unreal.GLTFExportOptions()
options.default_level_of_detail = 0
options.export_vertex_skin_weights = True
options.export_animation_sequences = True
options.export_morph_targets = True
options.bake_material_inputs = unreal.GLTFMaterialBakeMode.DISABLED
options.texture_image_format = unreal.GLTFTextureImageFormat.PNG
report = {'mesh': mesh.get_path_name(), 'exports': [], 'materials': [], 'sdk': {}}
for name in ['SkeletalMeshEditorSubsystem', 'AnimationLibrary', 'AnimPoseExtensions']:
    api = getattr(unreal, name, None)
    report['sdk'][name] = [n for n in dir(api) if any(k in n for k in ['bone', 'pose', 'lod', 'curve'])] if api else []
for slot in mesh.materials:
    mat = slot.material_interface
    report['materials'].append({'slot': str(slot.material_slot_name), 'asset': mat.get_path_name() if mat else None})
messages = unreal.GLTFExporter.export_to_gltf(mesh, str(root / 'Echo.gltf'), options, set())
report['exports'].append({'name': 'Echo', 'errors': [str(v) for v in messages.errors], 'warnings': [str(v) for v in messages.warnings]})
if messages.errors:
    raise RuntimeError(str(messages.errors))
options.export_preview_mesh = False
for name in ['Idle', 'Walk_Fwd', 'Jog_Fwd', 'Jump_Idle_Fall', 'Jump_Idle_Land', 'Idle_to_Walk_Fwd', 'Walk_Fwd_to_Idle', 'Idle_To_Jog_Fwd', 'Jog_Fwd_to_Idle', 'VaultOver']:
    asset = unreal.load_asset(base + 'Animations/' + name)
    if not asset:
        report['exports'].append({'name': name, 'missing': True})
        continue
    messages = unreal.GLTFExporter.export_to_gltf(asset, str(root / (name + '.gltf')), options, set())
    report['exports'].append({'name': name, 'seconds': asset.get_play_length(), 'errors': [str(v) for v in messages.errors], 'warnings': [str(v) for v in messages.warnings]})
(root / 'receipt.json').write_text(json.dumps(report, indent=2))
unreal.log('ECHO_EXPORT_RECEIPT ' + str(root / 'receipt.json'))
