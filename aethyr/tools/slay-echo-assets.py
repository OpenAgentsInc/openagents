# Firm reason: 6 (Unreal glTF, material and texture asset SDKs have no C ABI).
# Read data-only packages and export privately. Never play/save the source game.
import json
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'slay-assets'
root.mkdir(exist_ok=True)
options = unreal.GLTFExportOptions()
options.default_level_of_detail = 0
options.export_vertex_skin_weights = True
options.export_morph_targets = True
options.bake_material_inputs = unreal.GLTFMaterialBakeMode.DISABLED
options.texture_image_format = unreal.GLTFTextureImageFormat.PNG
report = {'meshes': [], 'materials': []}
pending = []
for path in ['/Game/Slay/Assets/Chr/Echo/Meshes/SK_Echo', '/Game/Slay/Assets/Chr/Echo/Meshes/SK_Echo_Sword']:
    mesh = unreal.load_asset(path)
    if not isinstance(mesh, unreal.SkeletalMesh):
        raise RuntimeError('Missing source mesh ' + path)
    row = {'asset': mesh.get_path_name(), 'slots': []}
    for slot in mesh.materials:
        material = slot.material_interface
        pending.append(material)
        row['slots'].append({'slot': str(slot.material_slot_name), 'material': material.get_path_name() if material else None})
    messages = unreal.GLTFExporter.export_to_gltf(mesh, str(root / (mesh.get_name() + '.gltf')), options, set())
    row['errors'] = [str(v) for v in messages.errors]
    row['warnings'] = [str(v) for v in messages.warnings]
    report['meshes'].append(row)
    if messages.errors:
        raise RuntimeError(str(messages.errors))
pending.append(unreal.load_asset('/Game/Slay/Assets/FX/Orb/M_Simple_Orb'))
seen = set()
while pending:
    obj = pending.pop()
    if not obj or obj.get_path_name() in seen:
        continue
    seen.add(obj.get_path_name())
    task = unreal.AssetExportTask()
    task.object = obj
    task.filename = str(root / (obj.get_name() + '.t3d'))
    task.exporter = unreal.ObjectExporterT3D()
    task.automated = True
    task.prompt = False
    task.replace_identical = True
    unreal.Exporter.run_asset_export_task(task)
    row = {'asset': obj.get_path_name(), 'class': obj.get_class().get_name(), 'textures': []}
    if isinstance(obj, unreal.MaterialInstance):
        pending.append(obj.parent)
    else:
        expressions = unreal.MaterialEditingLibrary.get_material_expressions(obj) if isinstance(obj, unreal.Material) else unreal.MaterialEditingLibrary.get_material_function_expressions(obj)
        for expression in expressions:
            if isinstance(expression, unreal.MaterialExpressionMaterialFunctionCall):
                pending.append(expression.get_editor_property('material_function'))
            if isinstance(expression, unreal.MaterialExpressionTextureBase):
                texture = expression.texture
                if texture and isinstance(texture, unreal.Texture2D):
                    row['textures'].append(texture.get_path_name())
                    if obj.get_name() == 'M_Simple_Orb':
                        task.object = texture
                        task.filename = str(root / (texture.get_name() + '.png'))
                        task.exporter = unreal.TextureExporterPNG()
                        unreal.Exporter.run_asset_export_task(task)
    report['materials'].append(row)
    (root / 'receipt.json').write_text(json.dumps(report, indent=2))
unreal.log('SLAY_ASSET_RECEIPT ' + str(root / 'receipt.json'))
