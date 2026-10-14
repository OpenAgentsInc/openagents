"""Export every MetaHuman Creator preset's face as geometry (UE 5.8).

Run headless on a copy of the project:

    DRACONIA_UE_PROJECT=... scripts/unreal/run_ue.sh scripts/unreal/presets_export.py

For each preset character under /MetaHumanCharacter/Optional/Presets (or
$DRACONIA_PRESETS), exports its head as a skeletal mesh into the project
and then as FBX to ~/work/film-out/draconia/metahuman/presets/<name>.fbx,
and logs its body type and the subsystem's preset/blend API, so round 7
can score every preset's face against the reference and start from the
closest. Private MetaHuman data, never used to train AI models.
"""

import os
import traceback

import unreal

SRC = os.environ.get("DRACONIA_PRESETS", "/MetaHumanCharacter/Optional/Presets")
OUT = os.path.expanduser("~/work/film-out/draconia/metahuman/presets")


def log(msg):
    print(f"DRACONIA {msg}", flush=True)
    unreal.log(f"DRACONIA {msg}")


def main():
    os.makedirs(OUT, exist_ok=True)
    sub = unreal.get_editor_subsystem(unreal.MetaHumanCharacterEditorSubsystem)
    log("subsystem API: " + ", ".join(n for n in dir(sub) if any(k in n for k in ("blend", "preset", "parametric", "sculpt", "region", "landmark", "copy", "face_state"))))
    lib = unreal.MetaHumanCharacterExportBlueprintLibrary
    reg = unreal.AssetRegistryHelpers.get_asset_registry()
    assets = [a for a in reg.get_assets_by_path(SRC, recursive=True) if str(a.asset_class_path.asset_name) == "MetaHumanCharacter"]
    log(f"{len(assets)} presets")
    for a in assets:
        name = str(a.asset_name)
        try:
            c = a.get_asset()
            sub.try_add_object_to_edit(c)
            g = unreal.MetaHumanGeometryExportParams()
            g.project_path = f"/Game/Presets/{name}"
            g.head_skeletal_mesh = True
            g.body_skeletal_mesh = False
            g.full_body_skeletal_mesh = False
            g.overwrite_existing_assets = True
            lib.export_geometry(c, g)
            for b in reg.get_assets_by_path(f"/Game/Presets/{name}", recursive=True):
                if str(b.asset_class_path.asset_name) != "SkeletalMesh":
                    continue
                t = unreal.AssetExportTask()
                t.object = b.get_asset()
                t.filename = os.path.join(OUT, f"{name}.fbx")
                t.automated = True
                t.prompt = False
                t.replace_identical = True
                log(f"{name}: {unreal.Exporter.run_asset_export_task(t)}")
            if sub.is_object_added_for_editing(c):
                sub.remove_object_to_edit(c)
        except Exception:
            log(f"{name} failed: " + traceback.format_exc().replace("\n", " | "))
    log("done")
    unreal.SystemLibrary.quit_editor()


main()
