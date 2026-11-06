# Firm reason: 6 (Unreal static-mesh/glTF export SDK has no C ABI).
# Read the literal orb mesh; do not play or save the source Blueprint.
import json
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'slay-assets'
mesh = unreal.load_asset('/Game/Slay/Assets/Chr/Orb/SM_Sphere_WithoutDistanceFields')
if not isinstance(mesh, unreal.StaticMesh):
    raise RuntimeError('Missing source orb mesh')
options = unreal.GLTFExportOptions()
options.default_level_of_detail = 0
options.bake_material_inputs = unreal.GLTFMaterialBakeMode.DISABLED
if not (root / 'SlayOrb.gltf').exists():
    messages = unreal.GLTFExporter.export_to_gltf(mesh, str(root / 'SlayOrb.gltf'), options, set())
    if messages.errors:
        raise RuntimeError(str(messages.errors))
rows = []
for name in ['T_Distortion_HQ', 'T_OC_EnergyTile_Greyscale']:
    texture = unreal.load_asset('/Game/Slay/Assets/FX/Orb/' + name)
    rows.append({'name': name, 'srgb': texture.get_editor_property('srgb'), 'compression': str(texture.get_editor_property('compression_settings'))})
(root / 'orb-texture-receipt.json').write_text(json.dumps(rows, indent=2))
unreal.log('SLAY_ORB_MESH_RECEIPT')
