"""Export the MetaHuman plugin's own textures for Draconia's private import.

Run with scripts/unreal/run_ue.sh. Writes every texture under the
MetaHuman plugin folders listed below to
~/work/film-out/draconia/metahuman/textures/<asset name>.<ext> (or
$DRACONIA_MH_TEXTURES): the archetype head, eye and teeth maps, skin micro
normal and cavity tiles, the pre-integrated skin LUT, wrinkle masks,
freckle and accent masks, eyelash coverage and eye presets. These are
MetaHuman assets under the Unreal Engine EULA: private, never committed,
never used to train AI models.
"""

import os

import unreal

OUT = os.environ.get("DRACONIA_MH_TEXTURES", os.path.expanduser("~/work/film-out/draconia/metahuman/textures"))
FOLDERS = [
    "/MetaHuman/IdentityTemplate/DefaultArchetypeTexture",
    "/MetaHumanCharacter/Lookdev_UHM",
    "/MetaHumanCharacter/Textures",
    "/MetaHumanCharacter/Tools/EyePresets",
]


def main():
    os.makedirs(OUT, exist_ok=True)
    registry = unreal.AssetRegistryHelpers.get_asset_registry()
    n = 0
    for folder in FOLDERS:
        assets = registry.get_assets_by_path(folder, recursive=True)
        for a in assets:
            if str(a.asset_class_path.asset_name) != "Texture2D":
                continue
            tex = a.get_asset()
            task = unreal.AssetExportTask()
            task.object = tex
            task.filename = os.path.join(OUT, f"{a.asset_name}.tga")
            task.automated = True
            task.prompt = False
            task.replace_identical = True
            ok = unreal.Exporter.run_asset_export_task(task)
            n += int(bool(ok))
            print(f"DRACONIA export {a.asset_name}: {ok}", flush=True)
    print(f"DRACONIA exported {n} textures to {OUT}", flush=True)
    unreal.SystemLibrary.quit_editor()


main()
