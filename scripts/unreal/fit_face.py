"""Fit Draconia's own head into MetaHuman's identity space (UE 5.8).

Run with scripts/unreal/run_ue.sh:

    scripts/unreal/run_ue.sh scripts/unreal/fit_face.py

Reads a head in MetaHuman's face topology (one `x y z` line per vertex of
head_lod0_mesh, DNA centimetres, y up, z forward), written by
`scripts/blender/metahuman_import.py` with MH_SHAPE_OUT
(~/work/film-out/draconia/metahuman/draconia_shape_head.txt, or
$DRACONIA_FIT_HEAD), plus the archetype's eyes and teeth from `dna_dump`
($DRACONIA_FIT_PARTS, default .../metahuman/archetype). MetaHuman
Creator's `fit_state_to_target_vertices` (local; no Epic sign-in) solves
the closest face its identity model can make, with rigid alignment and
scale, without the high-frequency residual, so what comes out is a
plausible MetaHuman face of Draconia's proportions. The fitted state is
committed to /Game/Draconia/Draconia and exported as a posed DNA to
.../metahuman/export/fit/ for `dna_dump`.

Private MetaHuman data: never committed, never used to train AI models.
"""

import os
import traceback

import unreal

MH = os.path.expanduser("~/work/film-out/draconia/metahuman")
HEAD = os.environ.get("DRACONIA_FIT_HEAD", os.path.join(MH, "draconia_shape_head.txt"))
PARTS = os.environ.get("DRACONIA_FIT_PARTS", os.path.join(MH, "archetype"))
OUT = os.environ.get("DRACONIA_FIT_OUT", os.path.join(MH, "export", "fit"))


def log(msg):
    print(f"DRACONIA {msg}", flush=True)
    unreal.log(f"DRACONIA {msg}")


def ue(x, y, z):
    # DNA (x, y up, z forward) to Unreal (Z up, the face toward +Y), as
    # Unreal imports a DNA: its rigid alignment only refines this, and a
    # head given facing another way comes back pitched over on its neck.
    return unreal.Vector(x, z, y)


def read_txt(path):
    with open(path) as f:
        return [ue(*map(float, line.split()[:3])) for line in f if line.strip()]


def read_obj(path):
    out = []
    with open(path) as f:
        for line in f:
            if line.startswith("v "):
                out.append(ue(*map(float, line.split()[1:4])))
    return out


def main():
    sub = unreal.get_editor_subsystem(unreal.MetaHumanCharacterEditorSubsystem)
    c = unreal.load_asset("/Game/Draconia/Draconia.Draconia")
    if not sub.try_add_object_to_edit(c):
        log("could not open the character for edit")
    try:
        p = unreal.MetaHumanCharacterFitToVerticesParams()
        p.head_vertices = read_txt(HEAD)
        for name, field in (("teeth_lod0_mesh.obj", "teeth_vertices"), ("eyeLeft_lod0_mesh.obj", "left_eye_vertices"), ("eyeRight_lod0_mesh.obj", "right_eye_vertices")):
            path = os.path.join(PARTS, name)
            if os.path.exists(path):
                setattr(p, field, read_obj(path))
        o = unreal.FitToTargetOptions()
        o.alignment_options = unreal.MetaHumanAlignmentOptions.SCALING_ROTATION_TRANSLATION
        o.adapt_neck = True
        o.disable_high_frequency_delta = True
        p.options = o
        log(f"fitting {len(p.head_vertices)} head vertices")
        ok = sub.fit_state_to_target_vertices(character=c, params=p)
        log(f"fit: {ok}")
        sub.commit_face_state(c)
        coeffs = sub.get_face_model_coefficients(c)
        log(f"identity coefficients: {len(coeffs)}; first {[round(v, 3) for v in list(coeffs)[:8]]}")
        unreal.EditorAssetLibrary.save_loaded_asset(c)
        os.makedirs(OUT, exist_ok=True)
        lib = unreal.MetaHumanCharacterExportBlueprintLibrary
        for fn in ("export_posed_dna", "export_dna"):
            try:
                if fn == "export_posed_dna":
                    pp = unreal.MetaHumanPosedDNAExportParams()
                    fields = [n for n in dir(pp) if not n.startswith("_") and n not in ("assign", "cast", "copy", "export_text", "get_editor_property", "import_text", "set_editor_properties", "set_editor_property", "static_struct", "to_dict", "to_tuple")]
                    log(f"posed DNA params: {fields} {pp.to_dict()}")
                    for k in fields:
                        if "path" in k:
                            setattr(pp, k, OUT)
                        elif "name" in k:
                            setattr(pp, k, "Draconia_posed")
                    r = lib.export_posed_dna(c, pp)
                else:
                    d = unreal.MetaHumanDNAExportParams()
                    d.external_path = OUT
                    d.dna_head = True
                    d.dna_body = True
                    r = lib.export_dna(c, d)
                log(f"{fn}: {r}")
            except Exception:
                log(f"{fn} failed: " + traceback.format_exc().replace("\n", " | "))
        # Without a rig there is no head DNA; the fitted face still comes
        # out as geometry: a skeletal mesh in the project, then FBX.
        try:
            g = unreal.MetaHumanGeometryExportParams()
            g.project_path = "/Game/Draconia/Fit"
            g.head_skeletal_mesh = True
            g.body_skeletal_mesh = False
            g.full_body_skeletal_mesh = False
            g.overwrite_existing_assets = True
            log(f"export_geometry: {lib.export_geometry(c, g)}")
            # Keep the fitted head in the project (render_reference.py
            # shows it in Unreal's column).
            unreal.EditorAssetLibrary.save_directory("/Game/Draconia/Fit")
            reg = unreal.AssetRegistryHelpers.get_asset_registry()
            for a in reg.get_assets_by_path("/Game/Draconia/Fit", recursive=True):
                log(f"fit asset {a.asset_name} {a.asset_class_path.asset_name}")
                if str(a.asset_class_path.asset_name) != "SkeletalMesh":
                    continue
                t = unreal.AssetExportTask()
                t.object = a.get_asset()
                t.filename = os.path.join(OUT, f"{a.asset_name}.fbx")
                t.automated = True
                t.prompt = False
                t.replace_identical = True
                log(f"FBX {t.filename}: {unreal.Exporter.run_asset_export_task(t)}")
        except Exception:
            log("geometry export failed: " + traceback.format_exc().replace("\n", " | "))
    except Exception:
        log("fit failed: " + traceback.format_exc().replace("\n", " | "))
    finally:
        if sub.is_object_added_for_editing(c):
            sub.remove_object_to_edit(c)
        log("done")
        unreal.SystemLibrary.quit_editor()


main()
