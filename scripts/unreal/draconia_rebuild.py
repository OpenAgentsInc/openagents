"""Rebuild Draconia's MetaHuman in UE 5.8: her own face, her look, the rig.

Run headless on a copy of the project (never the one open in the editor):

    DRACONIA_UE_PROJECT=.../DraconiaMH-r6/DraconiaMH.uproject \
        scripts/unreal/run_ue.sh scripts/unreal/draconia_rebuild.py

Steps, in the order MetaHuman Creator requires (shape and preset edits
need the rig removed first), each logged with a `DRACONIA` prefix:

1. Remove the face rig.
2. Fit her own face into MetaHuman's identity space: the shaped head from
   `metahuman_import.py` (MH_SHAPE_OUT; $DRACONIA_FIT_HEAD) through
   `fit_state_to_target_vertices`, then commit it.
3. Her look in Creator's own controls: pale skin with freckles and a
   little redness, grey-green eyes, a blonde updo groom, blonde brows,
   lashes and peach fuzz.
4. Create the full rig (joints and blend shapes; Epic's cloud, with the
   owner's stored sign-in).
5. Download the texture sources (the highest resolution offered).
6. Save, then export the head and body DNA, the DCC bundle, and the head's
   geometry and materials into the project (for Unreal's own render).

Private MetaHuman assets: never committed, never used to train AI models.
"""

import os
import traceback

import unreal

MH = os.path.expanduser("~/work/film-out/draconia/metahuman")
HEAD = os.environ.get("DRACONIA_FIT_HEAD", os.path.join(MH, "draconia_shape_head_r6.txt"))
PARTS = os.path.join(MH, "archetype")
EXPORT = os.environ.get("DRACONIA_MH_EXPORT", os.path.join(MH, "export"))
GROOMS = "/MetaHumanCharacter/Optional/Grooms/Bindings"


def log(msg):
    print(f"DRACONIA {msg}", flush=True)
    unreal.log(f"DRACONIA {msg}")


def step(name, fn):
    try:
        r = fn()
        log(f"{name}: {r}")
        return r
    except Exception:
        log(f"{name} failed: " + traceback.format_exc().replace("\n", " | "))
        return None


def ue(x, y, z):
    return unreal.Vector(x, z, y)


def fit(c, sub):
    p = unreal.MetaHumanCharacterFitToVerticesParams()
    with open(HEAD) as f:
        p.head_vertices = [ue(*map(float, line.split()[:3])) for line in f if line.strip()]
    for name, field in (("teeth_lod0_mesh.obj", "teeth_vertices"), ("eyeLeft_lod0_mesh.obj", "left_eye_vertices"), ("eyeRight_lod0_mesh.obj", "right_eye_vertices")):
        vs = []
        with open(os.path.join(PARTS, name)) as f:
            for line in f:
                if line.startswith("v "):
                    vs.append(ue(*map(float, line.split()[1:4])))
        setattr(p, field, vs)
    o = unreal.FitToTargetOptions()
    o.alignment_options = unreal.MetaHumanAlignmentOptions.SCALING_ROTATION_TRANSLATION
    o.adapt_neck = True
    o.disable_high_frequency_delta = True
    p.options = o
    ok = sub.fit_state_to_target_vertices(character=c, params=p)
    sub.commit_face_state(c)
    return ok


def skin(c, sub):
    s = unreal.MetaHumanCharacterSkinProperties()
    # The palest end of Creator's tone chart (u is depth of tone: 0.05
    # gives a cheek albedo of about 220,157,137 sRGB, 0.35 about
    # 183,120,93; v is warmth, higher a touch cooler), from
    # skin_tone_probe.py; the reference's skin is pale and cool.
    s.u = float(os.environ.get("DRACONIA_SKIN_U", "0.0"))
    s.v = float(os.environ.get("DRACONIA_SKIN_V", "0.85"))
    s.roughness = 0.9
    s.body_texture_index = 2
    s.face_texture_index = 3
    fr = unreal.MetaHumanCharacterFrecklesProperties()
    fr.density = 0.45
    fr.strength = 0.4
    fr.saturation = 0.6
    fr.tone_shift = 0.3
    fr.mask = unreal.MetaHumanCharacterFrecklesMask.TYPE1
    soft = unreal.MetaHumanCharacterAccentRegionProperties()
    soft.lightness = 0.5
    soft.redness = 0.25
    soft.saturation = 0.5
    rosy = unreal.MetaHumanCharacterAccentRegionProperties()
    rosy.lightness = 0.45
    rosy.redness = 0.35
    rosy.saturation = 0.55
    a = unreal.MetaHumanCharacterAccentRegions()
    a.cheeks = rosy
    a.nose = rosy
    a.ears = rosy
    a.chin = soft
    a.forehead = soft
    a.lips = soft
    a.under_eye = soft
    a.scalp = soft
    settings = unreal.MetaHumanCharacterSkinSettings()
    settings.skin = s
    settings.freckles = fr
    settings.accents = a
    sub.commit_skin_settings(character=c, skin_settings=settings)
    return True


def eyes(c, sub):
    iris = unreal.MetaHumanCharacterEyeIrisProperties()
    iris.pattern = unreal.MetaHumanCharacterEyesIrisPattern.IRIS002
    iris.primary_color_u = 0.55
    iris.primary_color_v = 0.35
    iris.secondary_color_u = 0.45
    iris.secondary_color_v = 0.25
    iris.color_blend = 0.4
    iris.limbal_ring_size = 0.6
    iris.limbal_ring_softness = 0.15
    iris.global_saturation = 0.6
    e = unreal.MetaHumanCharacterEyeProperties()
    e.iris = iris
    es = c.eyes_settings
    es.eye_left = e
    es.eye_right = e
    sub.commit_eyes_settings(character=c, eyes_settings=es)
    return True


def grooms(c, sub):
    reg = unreal.AssetRegistryHelpers.get_asset_registry()
    picks = {
        "Hair": ("Hair", "WI_Hair_S_Updo", 0.12),
        "Eyebrows": ("Eyebrows", "WI_Eyebrows_M_Natural", 0.3),
        "Eyelashes": ("Eyelashes", None, None),
        "Peachfuzz": ("Peachfuzz", None, 0.1),
    }
    preview = sub.get_preview_collection(c)
    chosen = {}
    for slot, (folder, want, melanin) in picks.items():
        f = unreal.ARFilter(package_paths=[f"{GROOMS}/{folder}"], class_paths=[unreal.MetaHumanWardrobeItem.static_class().get_class_path_name()])
        items = sorted(reg.get_assets(filter=f), key=lambda a: str(a.asset_name))
        if not items:
            log(f"no {slot} grooms")
            continue
        pick = next((a for a in items if str(a.asset_name) == want), items[0])
        key = preview.try_add_item_from_wardrobe_item(slot_name=slot, wardrobe_item=pick.get_asset())
        if key is None:
            log(f"could not add {pick.asset_name}")
            continue
        preview.default_instance.try_add_slot_selection(selection=unreal.MetaHumanPipelineSlotSelection(slot_name=slot, selected_item=key))
        chosen[slot] = (key, melanin)
        log(f"groom {slot}: {pick.asset_name}")
    sub.on_edit_preview_collection(c)
    sub.assemble_for_preview(character=c)
    # Blonde: low melanin on hair, brows and fuzz.
    for slot, (key, melanin) in chosen.items():
        if melanin is None:
            continue
        params = preview.default_instance.get_instance_parameters(item_path=unreal.MetaHumanPaletteItemPath(item_key=key))
        for prm in params:
            if prm.name == "Melanin":
                prm.set_float(value=melanin)
            elif prm.name == "Redness":
                prm.set_float(value=0.08)
    sub.on_edit_preview_collection(c)
    return list(chosen)


def main():
    c = unreal.load_asset("/Game/Draconia/Draconia.Draconia")
    sub = unreal.get_editor_subsystem(unreal.MetaHumanCharacterEditorSubsystem)
    lib = unreal.MetaHumanCharacterExportBlueprintLibrary
    log(f"opened: {sub.try_add_object_to_edit(c)}")
    # DRACONIA_STEPS picks steps (default all): rig,fit,skin,eyes,grooms,
    # autorig,textures,export. A skin-only pass keeps the rig.
    steps = os.environ.get("DRACONIA_STEPS", "rig,fit,skin,eyes,grooms,autorig,textures,export").split(",")
    try:
        if "rig" in steps:
            step("remove rig", lambda: sub.remove_face_rig(c))
        if "fit" in steps:
            step("fit her face", lambda: fit(c, sub))
        if "skin" in steps:
            step("skin", lambda: skin(c, sub))
        if "eyes" in steps:
            step("eyes", lambda: eyes(c, sub))
        if "grooms" in steps:
            step("grooms", lambda: grooms(c, sub))
        unreal.EditorAssetLibrary.save_loaded_asset(c)

        def rig():
            r = unreal.MetaHumanCharacterAutoRiggingRequestParams()
            r.blocking = True
            r.report_progress = False
            r.rig_type = unreal.MetaHumanRigType.JOINTS_AND_BLEND_SHAPES
            sub.request_auto_rigging(c, r)
            return c.has_face_dna() if hasattr(c, "has_face_dna") else "requested"

        if "autorig" in steps:
            step("full rig", rig)

        def textures():
            t = unreal.MetaHumanCharacterTextureRequestParams()
            t.blocking = True
            t.report_progress = False
            for field in [n for n in dir(t) if "resolution" in n.lower()]:
                try:
                    enum = type(getattr(t, field))
                    best = sorted([k for k in dir(enum) if k.isupper()])[-1]
                    setattr(t, field, getattr(enum, best))
                    log(f"textures {field} = {best}")
                except Exception:
                    pass
            sub.request_texture_sources(c, t)
            return c.has_high_resolution_textures

        if "textures" in steps:
            step("textures", textures)
        unreal.EditorAssetLibrary.save_loaded_asset(c)

        def dna():
            d = unreal.MetaHumanDNAExportParams()
            d.external_path = os.path.join(EXPORT, "dna")
            d.dna_head = True
            d.dna_body = True
            return lib.export_dna(c, d)

        if "export" in steps:
            step("DNA", dna)

        def dcc():
            p = unreal.MetaHumanDCCExportParams()
            p.external_path = os.path.join(EXPORT, "dcc")
            p.bake_make_up = True
            p.compress_in_zip_file = False
            p.archive_name = "Draconia"
            return lib.export_dcc(c, p)

        if "export" in steps:
            step("DCC", dcc)

        def geometry():
            g = unreal.MetaHumanGeometryExportParams()
            g.project_path = "/Game/Draconia/Rigged"
            g.head_skeletal_mesh = True
            g.body_skeletal_mesh = True
            g.full_body_skeletal_mesh = False
            g.overwrite_existing_assets = True
            r = lib.export_geometry(c, g)
            m = unreal.MetaHumanMaterialsExportParams()
            m.project_path = "/Game/Draconia/Rigged"
            m.apply_as_overrides = True
            r2 = lib.export_materials(c, m)
            unreal.EditorAssetLibrary.save_directory("/Game/Draconia/Rigged")
            return (r, r2)

        if "export" in steps:
            step("geometry and materials", geometry)
    finally:
        if sub.is_object_added_for_editing(c):
            sub.remove_object_to_edit(c)
        unreal.EditorAssetLibrary.save_loaded_asset(c)
        log("done")
        unreal.SystemLibrary.quit_editor()


main()
