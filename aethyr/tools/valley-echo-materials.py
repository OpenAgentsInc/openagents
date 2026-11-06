# Firm reason: 6 (Unreal's material-expression and asset-export SDK has no C ABI).
# Read source graphs and export SDK text/texture metadata; never save source packages.
import json
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'echo-source'
out = root / 'materials'
out.mkdir(exist_ok=True)
pending = []
slots = {}
for asset_path in ['/Game/AncientContent/Characters/Echo/Materials/MPC_CharacterAlbedo', '/Game/AncientContent/Characters/Echo/Materials/MPC_EyeHighlight', '/Game/AncientContent/Characters/Echo/Materials/SP_Head', '/Game/AncientContent/Characters/Echo/Materials/EyeMaterials/SSProfile/SSP_Eye_Inner']:
    profile = unreal.load_asset(asset_path)
    if profile:
        task = unreal.AssetExportTask()
        task.object = profile
        task.filename = str(out / (profile.get_name() + '.t3d'))
        task.exporter = unreal.ObjectExporterT3D()
        task.automated = True
        task.prompt = False
        task.replace_identical = True
        unreal.Exporter.run_asset_export_task(task)
for name in ['Echo', 'Echo_Hair']:
    mesh = unreal.load_asset('/Game/AncientContent/Characters/Echo/Meshes/' + name)
    pending.extend(slot.material_interface for slot in mesh.materials)
    slots[name] = [{'index': i, 'slot': str(slot.material_slot_name), 'asset': slot.material_interface.get_path_name() if slot.material_interface else None} for i, slot in enumerate(mesh.materials)]
(out / 'slots.json').write_text(json.dumps(slots, indent=2))
seen = set()
rows = []
for path in unreal.EditorAssetLibrary.list_assets('/Game/AncientContent/Characters/Echo/Hair', recursive=True):
    asset = unreal.load_asset(path)
    if isinstance(asset, unreal.GroomAsset):
        task = unreal.AssetExportTask()
        task.object = asset
        task.filename = str(out / (asset.get_name() + '.t3d'))
        task.exporter = unreal.ObjectExporterT3D()
        task.automated = True
        task.prompt = False
        task.replace_identical = True
        unreal.Exporter.run_asset_export_task(task)
        for group in asset.get_editor_property('hair_groups_materials'):
            pending.append(group.get_editor_property('material'))
while pending:
    obj = pending.pop()
    if not obj or obj.get_path_name() in seen:
        continue
    seen.add(obj.get_path_name())
    row = {'asset': obj.get_path_name(), 'class': obj.get_class().get_name()}
    task = unreal.AssetExportTask()
    task.object = obj
    task.filename = str(out / (obj.get_name() + '.t3d'))
    task.exporter = unreal.ObjectExporterT3D()
    task.automated = True
    task.prompt = False
    task.replace_identical = True
    row['exported'] = unreal.Exporter.run_asset_export_task(task)
    if isinstance(obj, unreal.MaterialInstance):
        pending.append(obj.parent)
    else:
        expressions = unreal.MaterialEditingLibrary.get_material_expressions(obj) if isinstance(obj, unreal.Material) else unreal.MaterialEditingLibrary.get_material_function_expressions(obj)
        for e in expressions:
            if isinstance(e, unreal.MaterialExpressionMaterialFunctionCall):
                pending.append(e.get_editor_property('material_function'))
            if isinstance(e, unreal.MaterialExpressionTextureBase):
                t = e.texture
                if t and isinstance(t, unreal.Texture2D):
                    row.setdefault('textures', []).append({'node': e.get_name(), 'asset': t.get_path_name(), 'srgb': t.get_editor_property('srgb'), 'flip_green_channel': t.get_editor_property('flip_green_channel'), 'compression': str(t.get_editor_property('compression_settings'))})
    rows.append(row)
    (out / 'graphs.json').write_text(json.dumps(rows, indent=2))
