# Firm reason: 6 (Unreal's Blueprint subobject-data SDK has no C ABI).
# Read serialized component templates; no actors, construction scripts, events or source saves.
import json
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'echo-source'
bp = unreal.load_asset('/Game/AncientContent/Blueprints/Character/BP_EchoCharacter')
subs = unreal.get_engine_subsystem(unreal.SubobjectDataSubsystem)
rows = []
for handle in subs.k2_gather_subobject_data_for_blueprint(bp):
    data = unreal.SubobjectDataBlueprintFunctionLibrary.get_data(handle)
    obj = unreal.SubobjectDataBlueprintFunctionLibrary.get_object_for_blueprint(data, bp)
    if not obj:
        continue
    row = {'name': obj.get_name(), 'class': obj.get_class().get_path_name(), 'properties': {}}
    for key in ['relative_location', 'relative_rotation', 'relative_scale3d', 'attach_socket_name', 'groom_asset', 'binding_asset', 'override_materials', 'hair_groups_desc', 'skeletal_mesh_asset', 'visibility', 'hidden_in_game']:
        try:
            row['properties'][key] = str(obj.get_editor_property(key))
        except Exception:
            pass
    rows.append(row)
    if isinstance(obj, unreal.GroomComponent):
        task = unreal.AssetExportTask()
        task.object = obj
        task.filename = str(root / (obj.get_name() + '.t3d'))
        task.exporter = unreal.ObjectExporterT3D()
        task.automated = True
        task.prompt = False
        task.replace_identical = True
        unreal.Exporter.run_asset_export_task(task)
(root / 'components.json').write_text(json.dumps(rows, indent=2))
