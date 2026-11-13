# Firm reason: 6 (Unreal's reflected asset/default-object and texture SDK has no C ABI).
# Read serialized defaults only; do not spawn actors, run Blueprint events or save source packages.
import json
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'echo-source'
result = {'controller': {}, 'materials': [], 'errors': []}
path = '/Game/AncientContent/Blueprints/Character/BP_EchoCharacter.BP_EchoCharacter_C'
try:
    cls = unreal.load_class(None, path)
    cdo = unreal.get_default_object(cls) if cls else None
    if cdo:
        result['controller']['class'] = cls.get_path_name()
        for part, obj, properties in [
            ('movement', cdo.character_movement, ['max_walk_speed', 'max_acceleration', 'braking_deceleration_walking', 'ground_friction', 'gravity_scale', 'jump_z_velocity', 'air_control', 'rotation_rate', 'max_step_height', 'walkable_floor_angle', 'orient_rotation_to_movement', 'use_controller_desired_rotation']),
            ('capsule', cdo.capsule_component, ['capsule_radius', 'capsule_half_height']),
            ('mesh', cdo.mesh, ['relative_location', 'relative_rotation', 'relative_scale3d', 'anim_class'])]:
            row = {}
            for name in properties:
                try:
                    value = obj.get_editor_property(name)
                    row[name] = value if isinstance(value, (float, int, bool, str)) else str(value)
                except Exception as error:
                    row[name] = {'error': str(error)}
            result['controller'][part] = row
except Exception as error:
    result['errors'].append(str(error))
(root / 'settings.json').write_text(json.dumps(result, indent=2))
mesh = unreal.load_asset('/Game/AncientContent/Characters/Echo/Meshes/Echo')
seen = set()
for slot in mesh.materials:
    mat = slot.material_interface
    if not mat or mat.get_path_name() in seen:
        continue
    seen.add(mat.get_path_name())
    row = {'asset': mat.get_path_name(), 'textures': [], 'scalars': [], 'vectors': []}
    for name in unreal.MaterialEditingLibrary.get_texture_parameter_names(mat):
        texture = (unreal.MaterialEditingLibrary.get_material_instance_texture_parameter_value(mat, name) if isinstance(mat, unreal.MaterialInstanceConstant) else unreal.MaterialEditingLibrary.get_material_default_texture_parameter_value(mat, name))
        item = {'name': str(name), 'asset': texture.get_path_name() if texture else None}
        if texture and isinstance(texture, unreal.Texture2D):
            item['file'] = texture.get_name() + '.png'
            item['width'] = texture.blueprint_get_size_x()
            item['height'] = texture.blueprint_get_size_y()
            output = root / item['file']
            if not output.exists():
                task = unreal.AssetExportTask()
                task.object = texture
                task.filename = str(output)
                task.automated = True
                task.prompt = False
                task.replace_identical = True
                item['exported'] = unreal.Exporter.run_asset_export_task(task)
        row['textures'].append(item)
    for name in unreal.MaterialEditingLibrary.get_scalar_parameter_names(mat):
        row['scalars'].append({'name': str(name), 'value': (unreal.MaterialEditingLibrary.get_material_instance_scalar_parameter_value(mat, name) if isinstance(mat, unreal.MaterialInstanceConstant) else unreal.MaterialEditingLibrary.get_material_default_scalar_parameter_value(mat, name))})
    for name in unreal.MaterialEditingLibrary.get_vector_parameter_names(mat):
        value = (unreal.MaterialEditingLibrary.get_material_instance_vector_parameter_value(mat, name) if isinstance(mat, unreal.MaterialInstanceConstant) else unreal.MaterialEditingLibrary.get_material_default_vector_parameter_value(mat, name))
        row['vectors'].append({'name': str(name), 'rgba': [value.r, value.g, value.b, value.a]})
    result['materials'].append(row)
    (root / 'settings.json').write_text(json.dumps(result, indent=2))
(root / 'settings.json').write_text(json.dumps(result, indent=2))
unreal.log('ECHO_SETTINGS ' + json.dumps(result['controller']))

sub = unreal.get_editor_subsystem(unreal.SkeletalMeshEditorSubsystem)
result['sections'] = [sub.get_lod_material_slot(mesh, 0, i) for i in range(sub.get_num_sections(mesh, 0))]
result['components'] = []
try:
    bp = unreal.load_asset('/Game/AncientContent/Blueprints/Character/BP_EchoCharacter')
    subs = unreal.get_engine_subsystem(unreal.SubobjectDataSubsystem)
    for handle in subs.k2_gather_subobject_data_for_blueprint(bp):
        data = unreal.SubobjectDataBlueprintFunctionLibrary.get_data(handle)
        obj = unreal.SubobjectDataBlueprintFunctionLibrary.get_object_for_blueprint(data, bp)
        row = {'name': obj.get_name(), 'class': obj.get_class().get_path_name(), 'properties': {}}
        for key in ['skeletal_mesh_asset', 'groom_asset', 'relative_location', 'relative_rotation', 'relative_scale3d', 'target_arm_length', 'field_of_view']:
            try:
                value = obj.get_editor_property(key)
                row['properties'][key] = value.get_path_name() if isinstance(value, unreal.Object) else str(value)
            except Exception:
                pass
        result['components'].append(row)
except Exception as error:
    result['errors'].append(str(error))
(root / 'settings.json').write_text(json.dumps(result, indent=2))

try:
    table = unreal.load_asset('/Game/AncientContent/Characters/Echo/Data/DT_EchoAttributeDefaults')
    result['attributes'] = unreal.DataTableFunctionLibrary.export_data_table_to_json_string(table)
except Exception as error:
    result['errors'].append(str(error))
(root / 'settings.json').write_text(json.dumps(result, indent=2))
options = unreal.GLTFExportOptions()
options.export_vertex_skin_weights = True
options.export_morph_targets = False
options.bake_material_inputs = unreal.GLTFMaterialBakeMode.DISABLED
for name in ['Echo_Hair']:
    asset = unreal.load_asset('/Game/AncientContent/Characters/Echo/Meshes/' + name)
    if asset:
        messages = unreal.GLTFExporter.export_to_gltf(asset, str(root / (name + '.gltf')), options, set())
        result[name] = {'errors': [str(v) for v in messages.errors], 'warnings': [str(v) for v in messages.warnings]}
(root / 'settings.json').write_text(json.dumps(result, indent=2))
