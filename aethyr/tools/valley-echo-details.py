# Firm reason: 6 (Unreal's material, Blueprint export and animation SDK has no C ABI).
# Data inspection/export only: no actor spawning, construction scripts, BeginPlay or source saves.
import json
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'echo-source'
result = {'materials': [], 'blueprints': [], 'animations': []}
for name in ['Echo', 'Echo_Hair']:
    mesh = unreal.load_asset('/Game/AncientContent/Characters/Echo/Meshes/' + name)
    for slot in mesh.materials:
        mat = slot.material_interface
        if not mat or mat.get_path_name() in [m['asset'] for m in result['materials']]:
            continue
        row = {'asset': mat.get_path_name(), 'textures': []}
        for texture in unreal.MaterialEditingLibrary.get_material_used_textures(mat):
            if not isinstance(texture, unreal.Texture2D):
                continue
            item = {'asset': texture.get_path_name(), 'file': texture.get_name() + '.png', 'width': texture.blueprint_get_size_x(), 'height': texture.blueprint_get_size_y()}
            if not (root / item['file']).exists():
                task = unreal.AssetExportTask()
                task.object = texture
                task.filename = str(root / item['file'])
                task.automated = True
                task.prompt = False
                task.replace_identical = True
                item['exported'] = unreal.Exporter.run_asset_export_task(task)
            row['textures'].append(item)
        result['materials'].append(row)
        (root / 'details.json').write_text(json.dumps(result, indent=2))
for path in ['/Game/AncientContent/Blueprints/Character/BP_EchoCharacter', '/Game/AncientContent/Characters/Echo/Echo_AnimBP', '/Game/AncientContent/Blueprints/Abilities/GA_Vault', '/Game/AncientContent/Blueprints/Camera/BP_ThirdPersonCameraMode']:
    asset = unreal.load_asset(path)
    if asset:
        task = unreal.AssetExportTask()
        task.object = asset
        task.filename = str(root / (asset.get_name() + '.t3d'))
        task.exporter = unreal.ObjectExporterT3D()
        task.automated = True
        task.prompt = False
        task.replace_identical = True
        result['blueprints'].append({'asset': path, 'exported': unreal.Exporter.run_asset_export_task(task)})
options = unreal.GLTFExportOptions()
options.export_vertex_skin_weights = True
options.export_animation_sequences = True
options.export_preview_mesh = False
options.bake_material_inputs = unreal.GLTFMaterialBakeMode.DISABLED
for path in unreal.EditorAssetLibrary.list_assets('/Game/AncientContent/Characters/Echo/Animations', recursive=False):
    asset = unreal.load_asset(path)
    if not isinstance(asset, unreal.AnimSequence) or (root / (asset.get_name() + '.gltf')).exists():
        continue
    messages = unreal.GLTFExporter.export_to_gltf(asset, str(root / (asset.get_name() + '.gltf')), options, set())
    result['animations'].append({'asset': path, 'errors': [str(v) for v in messages.errors], 'seconds': asset.get_play_length()})
(root / 'details.json').write_text(json.dumps(result, indent=2))
