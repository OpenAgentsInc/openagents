# Firm reason: 6 (Unreal Editor's static-mesh, glTF and texture export SDK has no C ABI).
# Data-only SDK adapter. The Coax packer owns coordinate conversion, material selection and budgets.
import json
import re
from pathlib import Path
import unreal

command = unreal.SystemLibrary.get_command_line()
source = Path(unreal.Paths.project_saved_dir()) / 'world-source'
world = json.loads((source / 'world.json').read_text())
part = re.search(r'-worldpart=(\d+)', command)
root = source / ('part%03d' % int(part[1])) if part else source
root.mkdir(parents=True, exist_ok=True)
if part and (root / 'world.json').exists():
    world = json.loads((root / 'world.json').read_text())
start = re.search(r'-worldstart=(\d+)', command)
end = re.search(r'-worldend=(\d+)', command)
start = int(start[1]) if start else 0
end = int(end[1]) if end else len(world['meshes'])
materials = {}
textures = {}
material_file = root / 'materials.json'
if material_file.exists():
    previous = json.loads(material_file.read_text())
    materials = {m['path']: m for m in previous['materials']}
    textures = {t['path']: t for t in previous['textures']}

def export_texture(texture):
    path = texture.get_path_name()
    if path in textures:
        return
    name = 'texture%03d.png' % len(textures)
    task = unreal.AssetExportTask()
    task.object = texture
    task.filename = str(root / name)
    task.automated = True
    task.prompt = False
    task.replace_identical = True
    if not unreal.Exporter.run_asset_export_task(task):
        raise RuntimeError('Texture export failed: ' + path)
    textures[path] = {'path': path, 'file': name, 'width': texture.blueprint_get_size_x(), 'height': texture.blueprint_get_size_y()}

def export_material(material):
    if not material or material.get_path_name() in materials:
        return
    row = {'path': material.get_path_name(), 'textures': [], 'scalars': [], 'vectors': []}
    for name in unreal.MaterialEditingLibrary.get_texture_parameter_names(material):
        texture = unreal.MaterialEditingLibrary.get_material_instance_texture_parameter_value(material, name)
        row['textures'].append({'name': str(name), 'path': texture.get_path_name() if texture else None})
        if texture:
            export_texture(texture)
    for name in unreal.MaterialEditingLibrary.get_scalar_parameter_names(material):
        row['scalars'].append({'name': str(name), 'value': unreal.MaterialEditingLibrary.get_material_instance_scalar_parameter_value(material, name)})
    for name in unreal.MaterialEditingLibrary.get_vector_parameter_names(material):
        c = unreal.MaterialEditingLibrary.get_material_instance_vector_parameter_value(material, name)
        row['vectors'].append({'name': str(name), 'value': [c.r, c.g, c.b, c.a]})
    materials[row['path']] = row

if '-worldoverrides' in command:
    flags = json.loads((source / 'component-flags.json').read_text())
    for component in flags['components']:
        for path in component['overrides']:
            if path:
                export_material(unreal.load_asset(path))
    material_file.write_text(json.dumps({'materials': list(materials.values()), 'textures': list(textures.values())}, indent=2))
    switch_rows = []
    for path in materials:
        material = unreal.load_asset(path)
        switches = []
        for name in unreal.MaterialEditingLibrary.get_static_switch_parameter_names(material):
            value = (unreal.MaterialEditingLibrary.get_material_instance_static_switch_parameter_value(material, name)
                     if isinstance(material, unreal.MaterialInstance) else
                     unreal.MaterialEditingLibrary.get_material_default_static_switch_parameter_value(material, name))
            switches.append({'name': str(name), 'value': value})
        switch_rows.append({'path': path, 'switches': switches})
    (source / 'material-flags.json').write_text(json.dumps({'materials': switch_rows}, indent=2))
    graph_dir = source / 'material-graphs'
    graph_dir.mkdir(exist_ok=True)
    pending = [unreal.load_asset(path) for path in materials]
    seen = set()
    while pending:
        obj = pending.pop()
        if not obj or obj.get_path_name() in seen:
            continue
        seen.add(obj.get_path_name())
        task = unreal.AssetExportTask()
        task.object = obj
        task.filename = str(graph_dir / (obj.get_name() + '.t3d'))
        task.exporter = unreal.ObjectExporterT3D()
        task.automated = True
        task.prompt = False
        task.replace_identical = True
        if not unreal.Exporter.run_asset_export_task(task):
            raise RuntimeError('Material graph export failed: ' + obj.get_path_name())
        if isinstance(obj, unreal.MaterialInstance):
            pending.append(obj.parent)
        else:
            expressions = (unreal.MaterialEditingLibrary.get_material_expressions(obj) if isinstance(obj, unreal.Material)
                           else unreal.MaterialEditingLibrary.get_material_function_expressions(obj))
            for expression in expressions:
                if isinstance(expression, unreal.MaterialExpressionMaterialFunctionCall):
                    pending.append(expression.get_editor_property('material_function'))

for index in range(start, min(end, len(world['meshes']))):
    row = world['meshes'][index]
    mesh = unreal.load_asset(row['path'])
    options = unreal.GLTFExportOptions()
    options.export_source_model = False
    options.bake_material_inputs = unreal.GLTFMaterialBakeMode.DISABLED
    options.export_vertex_colors = True
    options.texture_image_format = unreal.GLTFTextureImageFormat.PNG
    options.default_level_of_detail = 0
    filename = 'mesh%03d.gltf' % index
    if not (root / filename).exists():
        result = unreal.GLTFExporter.export_to_gltf(mesh, str(root / filename), options, set())
        if result.errors:
            raise RuntimeError(str(result.errors))
    for slot in mesh.static_materials:
        export_material(slot.material_interface)
    row['materials'] = [slot.material_interface.get_path_name() if slot.material_interface else None for slot in mesh.static_materials]
    (root / 'world.json').write_text(json.dumps(world, indent=2))
    material_file.write_text(json.dumps({'materials': list(materials.values()), 'textures': list(textures.values())}, indent=2))
    unreal.log('VALLEY_WORLD_ASSET_DONE %d/%d %s' % (index + 1, len(world['meshes']), row['path']))
