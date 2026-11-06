# Firm reason: 6 (Unreal's texture and material-expression asset SDK has no C ABI).
# Export the literal texture nodes as well as parameter overrides; NullRHI has no compiled used-texture list.
import json
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'echo-source'
result = []
seen = set()

def texture_export(texture):
    if not isinstance(texture, unreal.Texture2D) or texture.get_path_name() in seen:
        return
    seen.add(texture.get_path_name())
    row = {'asset': texture.get_path_name(), 'file': texture.get_name() + '.png', 'width': texture.blueprint_get_size_x(), 'height': texture.blueprint_get_size_y()}
    if not (root / row['file']).exists():
        task = unreal.AssetExportTask()
        task.object = texture
        task.filename = str(root / row['file'])
        task.automated = True
        task.prompt = False
        task.replace_identical = True
        row['exported'] = unreal.Exporter.run_asset_export_task(task)
    result.append(row)
    (root / 'literal-textures.json').write_text(json.dumps(result, indent=2))

for path in ['Hair/hair_alpha', 'Head/T_echo_Head_ORM', 'Head/T_echoGirl_Head_N']:
    texture_export(unreal.load_asset('/Game/AncientContent/Characters/Echo/Textures/' + path))
visited = set()
for mesh_name in ['Echo', 'Echo_Hair']:
    mesh = unreal.load_asset('/Game/AncientContent/Characters/Echo/Meshes/' + mesh_name)
    for slot in mesh.materials:
        material = slot.material_interface
        while isinstance(material, unreal.MaterialInstance):
            material = material.parent
        pending = [material] if material else []
        while pending:
            obj = pending.pop()
            if obj.get_path_name() in visited:
                continue
            visited.add(obj.get_path_name())
            try:
                expressions = unreal.MaterialEditingLibrary.get_material_expressions(obj) if isinstance(obj, unreal.Material) else unreal.MaterialEditingLibrary.get_material_function_expressions(obj)
                for expression in expressions:
                    if isinstance(expression, unreal.MaterialExpressionTextureBase):
                        texture_export(expression.texture)
                    if isinstance(expression, unreal.MaterialExpressionMaterialFunctionCall) and expression.get_editor_property("material_function"):
                        pending.append(expression.get_editor_property("material_function"))
            except Exception as error:
                unreal.log_warning(str(error))
(root / 'literal-textures.json').write_text(json.dumps(result, indent=2))
