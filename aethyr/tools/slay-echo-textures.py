# Firm reason: 6 (Unreal texture source export SDK has no C ABI).
# Explicit source texture effects only; Coax decides package bindings.
import json
from pathlib import Path
import unreal

root = Path(unreal.Paths.project_saved_dir()) / 'slay-assets'
names = [
    'T_echo_Costume1_D', 'T_echo_Costume2_D', 'T_echo_Costume3_D',
    'T_echo_Costume4_D', 'T_echo_Costume5_D', 'T_echo_Head_noBrows_D',
    'T_eyes_sclera_D', 'T_EyeIrisBaseColor_D', 'T_Hair_M',
    'T_echo_Costume1_ORM', 'T_echo_Costume2_ORM', 'T_echo_Costume3_ORM',
    'T_echo_Costume3_Gold_ORM', 'T_echo_Costume4_ORM', 'T_echo_Costume5_ORM',
    'T_echo_Head_ORM', 'T_echo_Costume1_N', 'T_echo_Costume2_N',
    'T_echo_Costume3_N', 'T_echo_Costume4_N', 'T_echo_Costume5_N',
    'T_echo_Head_N', 'T_Eye_Wet_N', 'T_Head_thickness_M', 'T_toksvig_macro',
    'T_Head_additional_R', 'T_echo_Head_micro_N', 'T_echo_HeadBrows_N',
    'T_echo_Head_Brows_M', 'T_echo_Head_MatID', 'T_EchoSword_BC',
    'T_EchoSword_N', 'T_EchoSword_ORM', 'T_Head_highpass_S',
]
paths = {}
for folder in ['/Game/Slay/Assets/Chr/Echo/Textures', '/Game/Slay/Assets/EchoSword']:
    for path in unreal.EditorAssetLibrary.list_assets(folder, recursive=True):
        paths[path.rsplit('.', 1)[-1].casefold()] = path
rows = []
for name in names:
    path = paths.get(name.casefold())
    if not path:
        raise RuntimeError('Missing source texture ' + name)
    texture = unreal.load_asset(path)
    if not isinstance(texture, unreal.Texture2D):
        raise RuntimeError('Not a source Texture2D ' + path)
    task = unreal.AssetExportTask()
    task.object = texture
    task.filename = str(root / (name + '.png'))
    task.exporter = unreal.TextureExporterPNG()
    task.automated = True
    task.prompt = False
    task.replace_identical = True
    if not unreal.Exporter.run_asset_export_task(task):
        raise RuntimeError('Texture export failed ' + path)
    rows.append({'asset': texture.get_path_name(), 'file': name + '.png',
                 'srgb': texture.get_editor_property('srgb'),
                 'flip_green_channel': texture.get_editor_property('flip_green_channel')})
    (root / 'texture-receipt.json').write_text(json.dumps(rows, indent=2))
unreal.log('SLAY_TEXTURE_RECEIPT')
