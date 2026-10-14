"""Export Draconia's rigged MetaHuman, changing nothing (UE 5.8).

Run headless on a copy of the project, never on the one open in the
editor:

    DRACONIA_UE_PROJECT=.../DraconiaMH-export/DraconiaMH.uproject \
        scripts/unreal/run_ue.sh scripts/unreal/export_draconia.py

Unlike draconia_metahuman.py, this sets no look and asks no cloud service:
it only exports what the owner's session made (the full face rig and the
downloaded texture sources) to ~/work/film-out/draconia/metahuman/export
($DRACONIA_MH_EXPORT): the head and body DNA (dna/), and the DCC bundle
with its textures (the head's albedo, normal, cavity, roughness and the
animated wrinkle maps). Private MetaHuman assets: never committed, never
used to train AI models.
"""

import os
import traceback

import unreal

EXPORT = os.environ.get("DRACONIA_MH_EXPORT", os.path.expanduser("~/work/film-out/draconia/metahuman/export"))


def log(msg):
    print(f"DRACONIA {msg}", flush=True)
    unreal.log(f"DRACONIA {msg}")


def main():
    c = unreal.load_asset("/Game/Draconia/Draconia.Draconia")
    sub = unreal.get_editor_subsystem(unreal.MetaHumanCharacterEditorSubsystem)
    lib = unreal.MetaHumanCharacterExportBlueprintLibrary
    opened = sub.try_add_object_to_edit(c)
    log(f"opened for edit: {opened}")
    try:
        d = unreal.MetaHumanDNAExportParams()
        d.external_path = os.path.join(EXPORT, "dna")
        d.dna_head = True
        d.dna_body = True
        log(f"DNA: {lib.export_dna(c, d)}")
    except Exception:
        log("DNA failed: " + traceback.format_exc().replace("\n", " | "))
    try:
        p = unreal.MetaHumanDCCExportParams()
        p.external_path = os.path.join(EXPORT, "dcc")
        p.bake_make_up = True
        p.compress_in_zip_file = False
        p.archive_name = "Draconia"
        log(f"DCC: {lib.export_dcc(c, p)}")
    except Exception:
        log("DCC failed: " + traceback.format_exc().replace("\n", " | "))
    if opened and sub.is_object_added_for_editing(c):
        sub.remove_object_to_edit(c)
    log("done")
    unreal.SystemLibrary.quit_editor()


main()
