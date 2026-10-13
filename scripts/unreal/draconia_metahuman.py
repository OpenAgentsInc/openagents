"""Draconia as a MetaHuman, authored and exported headless in Unreal Editor.

Run with scripts/unreal/run_ue.sh (it picks the newest finished UE 5.x and a
private project outside the repository):

    scripts/unreal/run_ue.sh scripts/unreal/draconia_metahuman.py

Steps, each logged with a `DRACONIA` prefix so the run can be checked from
the editor log:

1. Create (or load) the MetaHuman Character asset /Game/Draconia/Draconia.
2. Set her look in MetaHuman Creator's own parameters, after the photoreal
   reference (docs/verse/draconia/parity.md): fair, cool skin with light
   freckles and a little redness on the nose and cheeks, grey-green eyes
   with a soft limbal ring, natural lips, no makeup; platinum grooms for
   hair, brows, lashes and peach fuzz when the optional groom content is
   installed.
3. Ask Epic's cloud for the full face rig (joints and blendshapes) and the
   high-resolution textures. These need a signed-in Epic account; the
   result of each is logged, and the export continues without them.
4. Build the DCC export (DNA, meshes, textures, grooms) into
   ~/work/film-out/draconia/metahuman/export (or $DRACONIA_MH_EXPORT).

Everything this makes is a MetaHuman asset under the Unreal Engine EULA:
kept in the private asset pipeline, never committed, and never used to
train AI models.
"""

import os
import traceback

import unreal

EXPORT = os.environ.get("DRACONIA_MH_EXPORT", os.path.expanduser("~/work/film-out/draconia/metahuman/export"))
PACKAGE = "/Game/Draconia"
NAME = "Draconia"


def log(msg):
    unreal.log(f"DRACONIA {msg}")
    print(f"DRACONIA {msg}", flush=True)


def character():
    path = f"{PACKAGE}/{NAME}.{NAME}"
    if unreal.EditorAssetLibrary.does_asset_exist(path):
        log("loading existing character")
        return unreal.load_asset(path)
    tools = unreal.AssetToolsHelpers.get_asset_tools()
    c = tools.create_asset(asset_name=NAME, package_path=PACKAGE, asset_class=unreal.MetaHumanCharacter, factory=unreal.new_object(type=unreal.MetaHumanCharacterFactoryNew))
    log(f"created {c}")
    return c


def look(c, sub):
    # Skin: fair and cool (MetaHuman's skin-tone chart), slightly rough,
    # light freckles, gentle redness on the nose and cheeks.
    skin = unreal.MetaHumanCharacterSkinProperties()
    skin.u = 0.12
    skin.v = 0.35
    skin.roughness = 0.9
    freckles = unreal.MetaHumanCharacterFrecklesProperties()
    freckles.density = 0.35
    freckles.strength = 0.35
    freckles.saturation = 0.6
    freckles.tone_shift = 0.3
    freckles.mask = unreal.MetaHumanCharacterFrecklesMask.TYPE1
    soft = unreal.MetaHumanCharacterAccentRegionProperties()
    soft.lightness = 0.5
    soft.redness = 0.25
    soft.saturation = 0.5
    rosy = unreal.MetaHumanCharacterAccentRegionProperties()
    rosy.lightness = 0.45
    rosy.redness = 0.45
    rosy.saturation = 0.55
    accents = unreal.MetaHumanCharacterAccentRegions()
    accents.cheeks = rosy
    accents.nose = rosy
    accents.chin = soft
    accents.ears = rosy
    accents.forehead = soft
    accents.lips = soft
    accents.under_eye = soft
    accents.scalp = soft
    settings = unreal.MetaHumanCharacterSkinSettings()
    settings.skin = skin
    settings.freckles = freckles
    settings.accents = accents
    settings.enable_texture_overrides = False
    c.preview_material_type = unreal.MetaHumanCharacterSkinPreviewMaterial.EDITABLE
    if os.environ.get("DRACONIA_ARGS", "").find("--skin") >= 0:
        # Committing skin crashes UE 5.7 in GetBodyTexture when the optional
        # content (body texture sets) is missing; opt in once it's present.
        skin.body_texture_index = 2
        skin.face_texture_index = 3
        sub.commit_skin_settings(character=c, skin_settings=settings)
        log("skin committed")
    else:
        log("skin skipped (needs optional content; pass --skin)")

    # Eyes: grey-green with a soft limbal ring.
    iris = unreal.MetaHumanCharacterEyeIrisProperties()
    iris.pattern = unreal.MetaHumanCharacterEyesIrisPattern.IRIS002
    iris.primary_color_u = 0.55
    iris.primary_color_v = 0.35
    iris.secondary_color_u = 0.45
    iris.secondary_color_v = 0.25
    iris.color_blend = 0.4
    iris.limbal_ring_size = 0.6
    iris.limbal_ring_softness = 0.15
    iris.global_saturation = 0.8
    eye = unreal.MetaHumanCharacterEyeProperties()
    eye.iris = iris
    eyes = c.eyes_settings
    eyes.eye_left = eye
    eyes.eye_right = eye
    sub.commit_eyes_settings(character=c, eyes_settings=eyes)
    log("eyes committed")


def grooms(c):
    """Platinum grooms from the optional content, if it is installed."""
    registry = unreal.AssetRegistryHelpers.get_asset_registry()
    for slot, folder in (("Hair", "Hair"), ("Eyebrows", "Eyebrows"), ("Eyelashes", "Eyelashes"), ("Peachfuzz", "Peachfuzz")):
        f = unreal.ARFilter(package_paths=[f"/MetaHumanCharacter/Optional/Grooms/Bindings/{folder}"], class_paths=[unreal.MetaHumanWardrobeItem.static_class().get_class_path_name()])
        items = registry.get_assets(filter=f)
        log(f"groom slot {slot}: {len(items)} items")
        if not items:
            continue
        names = sorted(str(a.asset_name) for a in items)
        log(f"groom slot {slot} names: {names[:40]}")
        # Prefer an updo or bun for hair.
        pick = items[0]
        if slot == "Hair":
            for a in items:
                n = str(a.asset_name).lower()
                if "bun" in n or "updo" in n:
                    pick = a
                    break
        key = c.internal_collection.try_add_item_from_wardrobe_item(slot_name=slot, wardrobe_item=pick.get_asset())
        if key is not None:
            c.internal_collection.default_instance.set_single_slot_selection(slot_name=slot, item_key=key)
            log(f"groom {slot}: {pick.asset_name}")


def cloud(c, sub):
    try:
        rig = unreal.MetaHumanCharacterAutoRiggingRequestParams()
        rig.blocking = True
        rig.report_progress = False
        rig.rig_type = unreal.MetaHumanRigType.JOINTS_AND_BLENDSHAPES
        sub.request_auto_rigging(c, rig)
        log("auto-rig requested")
    except Exception as e:
        log(f"auto-rig failed: {e}")
    try:
        tex = unreal.MetaHumanCharacterTextureRequestParams()
        tex.blocking = True
        tex.report_progress = False
        sub.request_texture_sources(c, tex)
        log("texture download requested")
    except Exception as e:
        log(f"texture download failed: {e}")


def export(c, sub):
    os.makedirs(EXPORT, exist_ok=True)
    p = unreal.MetaHumanCharacterEditorBuildParameters()
    p.pipeline_type = unreal.MetaHumanDefaultPipelineType.DCC
    p.absolute_build_path = EXPORT
    p.bake_makeup = True
    p.export_zip_file = False
    sub.build_meta_human(character=c, params=p)
    log(f"DCC export to {EXPORT}")


def main():
    sub = unreal.get_editor_subsystem(unreal.MetaHumanCharacterEditorSubsystem)
    c = character()
    if not sub.try_add_object_to_edit(c):
        log("could not open the character for edit")
    try:
        for step in (look, ):
            try:
                step(c, sub)
            except Exception:
                log("step failed: " + traceback.format_exc().replace("\n", " | "))
        try:
            grooms(c)
        except Exception:
            log("grooms failed: " + traceback.format_exc().replace("\n", " | "))
        if os.environ.get("DRACONIA_ARGS", "").find("--no-cloud") < 0:
            cloud(c, sub)
        unreal.EditorAssetLibrary.save_loaded_asset(c)
        try:
            export(c, sub)
        except Exception:
            log("export failed: " + traceback.format_exc().replace("\n", " | "))
    finally:
        if sub.is_object_added_for_editing(c):
            sub.remove_object_to_edit(c)
        log("done")
        unreal.SystemLibrary.quit_editor()


main()
