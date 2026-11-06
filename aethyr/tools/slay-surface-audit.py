# Firm reason: 6 (Unreal groom, animation and material reflection SDKs have no C ABI).
"""Read source surface/animation metadata without spawning or playing actors."""
import json
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'slay-assets'
rows = []
pending = []
for path in unreal.EditorAssetLibrary.list_assets('/Game/Slay/Assets/Chr/Echo', recursive=True):
    if '/Hair/' not in path:
        continue
    obj = unreal.load_asset(path)
    if isinstance(obj, unreal.GroomAsset):
        row = {'asset': path, 'materials': []}
        for slot in obj.get_editor_property('hair_groups_materials'):
            material = slot.get_editor_property('material')
            row['materials'].append(material.get_path_name() if material else None)
            pending.append(material)
        rows.append(row)
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
    if not unreal.Exporter.run_asset_export_task(task):
        raise RuntimeError('Cannot export ' + obj.get_path_name())
    if isinstance(obj, unreal.MaterialInstance):
        pending.append(obj.parent)
    elif isinstance(obj, unreal.Material):
        for expression in unreal.MaterialEditingLibrary.get_material_expressions(obj):
            if isinstance(expression, unreal.MaterialExpressionMaterialFunctionCall):
                pending.append(expression.get_editor_property('material_function'))
(root / 'surface-audit.json').write_text(json.dumps(rows, indent=2))
subs = unreal.get_engine_subsystem(unreal.SubobjectDataSubsystem)
bp = unreal.load_asset('/Game/Slay/Tools/BlueprintLightTarget/BP_LightTarget_Solo')
lights = []
for handle in subs.k2_gather_subobject_data_for_blueprint(bp):
    data = unreal.SubobjectDataBlueprintFunctionLibrary.get_data(handle)
    obj = unreal.SubobjectDataBlueprintFunctionLibrary.get_object_for_blueprint(data, bp)
    if not obj:
        continue
    row = {'name': obj.get_name(), 'class': obj.get_class().get_name(), 'properties': {}}
    for key in ['relative_location', 'relative_rotation', 'relative_scale3d',
                'source_width', 'source_height', 'intensity', 'intensity_units', 'specular_scale']:
        try:
            value = obj.get_editor_property(key)
            row['properties'][key] = [value.x, value.y, value.z] if isinstance(value, unreal.Vector) else str(value)
        except Exception:
            pass
    lights.append(row)
(root / 'light-template-audit.json').write_text(json.dumps(lights, indent=2))
anim = unreal.load_asset('/Game/Slay/Sequences/TF/Shots/TF0290/Animations/TF0290_Echo_Face')
if anim:
    row = {'asset': anim.get_path_name()}
    for key in ['additive_anim_type', 'ref_pose_type', 'data_model']:
        try:
            row[key] = str(anim.get_editor_property(key))
        except Exception as error:
            row[key] = str(error)
    try:
        model = anim.get_editor_property('data_model')
        row['bone_tracks'] = [str(track.get_editor_property('name')) for track in model.get_editor_property('bone_animation_tracks')]
    except Exception as error:
        row['bone_tracks_error'] = str(error)
    (root / 'face-animation-audit.json').write_text(json.dumps(row, indent=2))
