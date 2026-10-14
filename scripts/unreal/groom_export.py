"""Export MetaHuman grooms' hair cards for Verse (UE 5.8).

Run headless on a copy of the project:

    DRACONIA_GROOMS="Hair/Hair_S_Updo Eyebrows/Eyebrows_M_Natural" \
        scripts/unreal/run_ue.sh scripts/unreal/groom_export.py

For each groom under /MetaHumanCharacter/Optional/Grooms/GroomAssets it
exports the cards meshes (every group, LOD0) as FBX and the cards atlases
(attribute and tangent) as TGA to
~/work/film-out/draconia/metahuman/grooms/<groom>/. The cards sit on the
archetype head; `metahuman_import.py` (MH_GROOMS) carries them onto
Draconia's head and shades them as Verse hair. Private MetaHuman assets:
never committed, never used to train AI models.
"""

import os
import traceback

import unreal

ROOT = "/MetaHumanCharacter/Optional/Grooms/GroomAssets"
GROOMS = os.environ.get("DRACONIA_GROOMS", "Hair/Hair_S_Updo Eyebrows/Eyebrows_M_Natural").split()
OUT = os.path.expanduser("~/work/film-out/draconia/metahuman/grooms")


def log(msg):
    print(f"DRACONIA {msg}", flush=True)
    unreal.log(f"DRACONIA {msg}")


def export(obj, path):
    t = unreal.AssetExportTask()
    t.object = obj
    t.filename = path
    t.automated = True
    t.prompt = False
    t.replace_identical = True
    return unreal.Exporter.run_asset_export_task(t)


def main():
    reg = unreal.AssetRegistryHelpers.get_asset_registry()
    for g in GROOMS:
        name = g.split("/")[-1]
        out = os.path.join(OUT, name)
        os.makedirs(out, exist_ok=True)
        for a in reg.get_assets_by_path(f"{ROOT}/{g}", recursive=True):
            n = str(a.asset_name)
            cls = str(a.asset_class_path.asset_name)
            try:
                if cls == "StaticMesh" and n.endswith("_LOD0"):
                    log(f"{n}: {export(a.get_asset(), os.path.join(out, n + '.fbx'))}")
                elif cls == "Texture2D" and "Atlas" in n:
                    log(f"{n}: {export(a.get_asset(), os.path.join(out, n + '.tga'))}")
            except Exception:
                log(f"{n} failed: " + traceback.format_exc().replace("\n", " | "))
    # The archetype face the cards were groomed on, in the same export
    # frame, so the cards can be carried onto Draconia's head exactly.
    face = unreal.load_asset("/MetaHumanCharacter/Face/SKM_Face")
    if face is not None:
        log(f"SKM_Face: {export(face, os.path.join(OUT, 'SKM_Face.fbx'))}")
    log("done")
    unreal.SystemLibrary.quit_editor()


main()
