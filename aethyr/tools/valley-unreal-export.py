# Firm reason: 6 (Unreal Editor's package, glTF and texture export SDK has no C ABI).
# SDK effects only. Clean commandlet project mounts data assets, never AncientGame modules/plugins.
# Exports the exact Canyon Sandstone pair in AncientWorld_MainGrid_L0_X0_Y0_DL44BCB661.
import json
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'section-source'
root.mkdir(parents=True, exist_ok=True)
mesh_path = '/Game/AncientContent/Megascans/3DAssets/CanyonSandstone/SM_CanyonSandstone'
mesh = unreal.load_asset(mesh_path)
options = unreal.GLTFExportOptions()
options.export_source_model = True
options.bake_material_inputs = unreal.GLTFMaterialBakeMode.DISABLED
options.export_vertex_colors = True
options.texture_image_format = unreal.GLTFTextureImageFormat.PNG
options.default_level_of_detail = 0
messages = unreal.GLTFExporter.export_to_gltf(mesh, str(root / 'canyon.gltf'), options, set())
if messages.errors:
    raise RuntimeError(str(messages.errors))
material = mesh.static_materials[0].material_interface
material_info = {'path': material.get_path_name(), 'textures': [], 'scalars': [], 'vectors': []}
for parameter in material.texture_parameter_values:
    texture = parameter.parameter_value
    name = str(parameter.parameter_info.name)
    row = {'name': name, 'asset': texture.get_path_name() if texture else None}
    if texture:
        row['width'] = texture.blueprint_get_size_x()
        row['height'] = texture.blueprint_get_size_y()
        task = unreal.AssetExportTask()
        task.object = texture
        task.filename = str(root / (name + '.png'))
        task.automated = True
        task.prompt = False
        task.replace_identical = True
        if not unreal.Exporter.run_asset_export_task(task):
            raise RuntimeError('Texture export failed: ' + name)
    material_info['textures'].append(row)
for parameter in material.scalar_parameter_values:
    material_info['scalars'].append({'name': str(parameter.parameter_info.name), 'value': parameter.parameter_value})
for parameter in material.vector_parameter_values:
    c = parameter.parameter_value
    material_info['vectors'].append({'name': str(parameter.parameter_info.name), 'rgba': [c.r, c.g, c.b, c.a]})
# Also retain Unreal's actual fallback as a separately labelled comparison, never as source detail.
p, i, n, uv, tangents = unreal.ProceduralMeshLibrary.get_section_from_static_mesh(mesh, 0, 0)
(root / 'fallback.json').write_text(json.dumps({'positions_cm': [[v.x, v.y, v.z] for v in p], 'indices': list(i), 'normals': [[v.x, v.y, v.z] for v in n], 'uv': [[v.x, v.y] for v in uv]}))
unreal.EditorLoadingAndSavingUtils.load_map('/Game/AncientContent/Maps/AncientWorld')
package = '/Game/__ExternalActors__/AncientContent/Maps/AncientWorld/ER/3T/SGRWHDXJW2KRA0LYL6FI2'
descriptor = next(d for d in unreal.WorldPartitionBlueprintLibrary.get_actor_descs() if str(d.actor_package) == package)
unreal.load_package(package)
actor = unreal.find_object(None, str(descriptor.actor_path))
component = next(c for c in actor.get_components_by_class(unreal.InstancedStaticMeshComponent) if c.static_mesh == mesh)
def vec(v):
    return [v.x, v.y, v.z]
instances = []
for index in range(component.get_instance_count()):
    transform = component.get_instance_transform(index, world_space=True)
    q = transform.rotation
    # Direct SDK matrix samples are an independent check on Coax's coordinate conversion.
    samples = []
    for point in [(0, 0, 0), (100, 0, 0), (0, 100, 0), (0, 0, 100), (-23, 17, 61)]:
        samples.append({'local_cm': list(point), 'world_cm': vec(unreal.MathLibrary.transform_location(transform, unreal.Vector(*point)))})
    instances.append({'index': index, 'translation_cm': vec(transform.translation), 'quaternion_xyzw': [q.x, q.y, q.z, q.w], 'scale': vec(transform.scale3d), 'sdk_samples': samples})
receipt = {'schema': 'valley.unreal.canyon-pair.v1', 'engine': unreal.SystemLibrary.get_engine_version(), 'world': '/Game/AncientContent/Maps/AncientWorld', 'actor_package': package, 'actor_path': str(descriptor.actor_path), 'actor_label': str(descriptor.label), 'component': component.get_name(), 'mesh': mesh.get_path_name(), 'nanite_triangles': mesh.get_num_nanite_triangles(), 'nanite_vertices': mesh.get_num_nanite_vertices(), 'fallback_triangles': len(i)//3, 'material': material_info, 'instances': instances, 'gltf_messages': {'errors': list(messages.errors), 'warnings': list(messages.warnings), 'suggestions': list(messages.suggestions)}}
(root / 'placement.json').write_text(json.dumps(receipt, indent=2))
unreal.log('VALLEY_SECTION_EXPORT_DONE')
