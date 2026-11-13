# Firm reason: 1 (Unreal Engine's Python API: Creator's character, rig service and exporters run only inside Unreal Editor)
"""Makes a private human character in Unreal's character creator with no clicks: from a preset, a
look recipe (characters/NAME.json), the cloud face rig, the texture download and the exports
that pack_human.coax reads (docs/metaverse/2026-11-04-speaking-characters.md, "Making the mayor").

Runs inside Unreal Editor 5.8, headless, on a private project outside the repository (never one
open in the editor):

    UnrealEditor-Cmd PROJECT.uproject -ExecutePythonScript=metaverse/tools/humans/make_character.py \
        -unattended -nocrashreports -nosplash -RenderOffscreen -stdout

or, for the render step (it keeps ticking until its shots are taken), as the project's startup
script (PythonScriptPluginSettings +StartupScripts=...) for one launch. Settings come from the
environment:

  CHARACTER_RECIPE   the look recipe (JSON; default characters/everglade-mayor.json beside this)
  CHARACTER_STEPS    comma-separated, in this order (default look,save):
                       look      duplicate the base preset (first run), blend the face toward the
                                 recipe's other presets, set skin, eyes and grooms
                       save      save the character asset
                       render    front, three-quarter and side shots of the preview, to
                                 CHARACTER_RENDER, then quit
                       autorig   the cloud face rig, joints and blend shapes (blocking)
                       textures  the cloud texture sources, highest resolution (blocking)
                       export    head and body DNA, the DCC bundle (maps), to CHARACTER_EXPORT
                       cards     each chosen groom's hair cards (LOD 0, OBJ) and cards atlas (TGA),
                                 to CHARACTER_EXPORT/grooms/NAME (they sit on the archetype
                                 head; pack_human.coax carries them onto the DNA head)
  CHARACTER_RENDER   where shots go (PNG)
  CHARACTER_EXPORT   where exports go

The cloud steps use the editor's stored sign-in (Epic Online Services' persistent login, kept
per user by the engine). Every log line starts with CHARACTER.

Everything this makes is a private asset under the Unreal Engine EULA: never committed, never
in a golden, dataset, eval or public trace, never used to train AI models.
"""
import json
import math
import os
import time
import traceback

import unreal

HERE = os.path.dirname(os.path.abspath(__file__))
RECIPE = os.environ.get("CHARACTER_RECIPE", os.path.join(HERE, "characters", "everglade-mayor.json"))
STEPS = os.environ.get("CHARACTER_STEPS", "look,save").split(",")
RENDER = os.path.expanduser(os.environ.get("CHARACTER_RENDER", "~/work/film-out/everglade-mayor/renders"))
EXPORT = os.path.expanduser(os.environ.get("CHARACTER_EXPORT", "~/work/film-out/everglade-mayor/metahuman/export"))
PRESETS = "/MetaHumanCharacter/Optional/Presets"
GROOMS = "/MetaHumanCharacter/Optional/Grooms/Bindings"
GROOM_DIRS = {"Beard": "Beards", "Mustache": "Mustaches"}
SHOTS = [("front", 0.0), ("three-quarter", 35.0), ("side", 90.0)]


def log(msg):
    print(f"CHARACTER {msg}", flush=True)


def step(name, fn):
    try:
        r = fn()
        log(f"{name}: {r}")
        return r
    except Exception:
        log(f"{name} failed: " + traceback.format_exc().replace("\n", " | "))
        return None


def character(recipe):
    """The character asset /Game/<name>/<name>, duplicated from the base preset on the first run."""
    name = recipe["name"]
    path = f"/Game/{name}/{name}"
    lib = unreal.EditorAssetLibrary
    if not lib.does_asset_exist(path):
        src = f"{PRESETS}/{recipe['base']}"
        log(f"duplicating {src} to {path}: {lib.duplicate_asset(src, path) is not None}")
    return unreal.load_asset(f"{path}.{name}")


def face(c, sub, recipe):
    """The face's identity coefficients blended toward other presets (weights sum with the base's
    to 1). The coefficient arrays carry integer headers that match between presets, so a blend
    leaves them as they are."""
    blend = recipe.get("blend") or {}
    if not blend:
        return "base face"
    sub.remove_face_rig(c)

    def coefficients(preset):
        p = unreal.load_asset(f"{PRESETS}/{preset}.{preset}")
        added = sub.try_add_object_to_edit(p)
        got = list(sub.get_face_model_coefficients(p))
        if added:
            sub.remove_object_to_edit(p)
        return got

    # from the base preset's face every run, so a rerun doesn't blend twice
    mine = coefficients(recipe["base"])
    out = [x * (1.0 - sum(blend.values())) for x in mine]
    for preset, w in blend.items():
        theirs = coefficients(preset)
        if len(theirs) != len(mine):
            raise RuntimeError(f"{preset}: {len(theirs)} coefficients, the base has {len(mine)}")
        out = [a + w * b for a, b in zip(out, theirs)]
    # the headers (counts) exactly as the base's
    for i, (a, b) in enumerate(zip(mine, out)):
        if a == int(a) and abs(a - b) < 1e-3:
            out[i] = a
    sub.set_face_model_coefficients(c, out)
    sub.commit_face_state(c)
    return f"{len(out)} coefficients, blend {blend}"


def skin(c, sub, recipe):
    r = recipe.get("skin") or {}
    settings = c.skin_settings
    s = settings.skin
    for k, v in r.items():
        setattr(s, k, v)
    settings.skin = s
    sub.commit_skin_settings(character=c, skin_settings=settings)
    return dict(r)


def eyes(c, sub, recipe):
    r = dict(recipe.get("eyes") or {})
    es = c.eyes_settings
    for side in ("eye_left", "eye_right"):
        e = getattr(es, side)
        iris = e.iris
        for k, v in r.items():
            setattr(iris, k, getattr(unreal.MetaHumanCharacterEyesIrisPattern, v) if k == "pattern" else v)
        e.iris = iris
        setattr(es, side, e)
    sub.commit_eyes_settings(character=c, eyes_settings=es)
    return r


def grooms(c, sub, recipe):
    reg = unreal.AssetRegistryHelpers.get_asset_registry()
    preview = sub.get_preview_collection(c)
    chosen = {}
    for slot, (item, params) in (recipe.get("grooms") or {}).items():
        if item is None:
            preview.default_instance.set_single_slot_selection(slot_name=slot, item_key=unreal.MetaHumanPaletteItemKey())
            continue
        found = reg.get_assets_by_package_name(f"{GROOMS}/{GROOM_DIRS.get(slot, slot)}/{item}")
        if not found:
            log(f"no groom {item}")
            continue
        key = preview.try_add_item_from_wardrobe_item(slot_name=slot, wardrobe_item=found[0].get_asset())
        if key is None:
            log(f"could not add {item}")
            continue
        preview.default_instance.set_single_slot_selection(slot_name=slot, item_key=key)
        chosen[slot] = (key, params)
    sub.on_edit_preview_collection(c)
    sub.assemble_for_preview(character=c)
    for slot, (key, params) in chosen.items():
        ps = preview.default_instance.get_instance_parameters(item_path=unreal.MetaHumanPaletteItemPath(item_key=key))
        names = []
        for p in ps:
            names.append(str(p.name))
            if str(p.name) in params:
                v = params[str(p.name)]
                if p.type == unreal.MetaHumanCharacterInstanceParameterType.FLOAT:
                    p.set_float(value=float(v))
                elif p.type == unreal.MetaHumanCharacterInstanceParameterType.BOOL:
                    p.set_bool(value=bool(v))
                elif p.type == unreal.MetaHumanCharacterInstanceParameterType.COLOR:
                    p.set_color(value=unreal.LinearColor(*v))
        log(f"groom {slot} parameters: {names}")
    sub.on_edit_preview_collection(c)
    sub.assemble_for_preview(character=c)
    return list(chosen)


def autorig(c, sub):
    r = unreal.MetaHumanCharacterAutoRiggingRequestParams()
    r.blocking = True
    r.report_progress = False
    r.rig_type = unreal.MetaHumanRigType.JOINTS_AND_BLEND_SHAPES
    sub.request_auto_rigging(c, r)
    return "requested"


def textures(c, sub):
    t = unreal.MetaHumanCharacterTextureRequestParams()
    t.blocking = True
    t.report_progress = False
    sub.request_texture_sources(c, t)
    return f"high resolution: {c.has_high_resolution_textures}"


def export(c, name):
    lib = unreal.MetaHumanCharacterExportBlueprintLibrary
    d = unreal.MetaHumanDNAExportParams()
    d.external_path = os.path.join(EXPORT, "dna")
    d.dna_head = True
    d.dna_body = True
    a = lib.export_dna(c, d)
    p = unreal.MetaHumanDCCExportParams()
    p.external_path = os.path.join(EXPORT, "dcc")
    p.bake_make_up = True
    p.compress_in_zip_file = False
    p.archive_name = name
    b = lib.export_dcc(c, p)
    return (a, b)


CARDS_ROOT = "/MetaHumanCharacter/Optional/Grooms/GroomAssets"
CARDS_SLOTS = ("Hair", "Eyebrows", "Beard", "Mustache")   # lashes and peach fuzz stay the DNA's or none


def save_as(obj, path):
    t = unreal.AssetExportTask()
    t.object, t.filename, t.automated, t.prompt, t.replace_identical = obj, path, True, False, True
    return unreal.Exporter.run_asset_export_task(t)


def cards(recipe):
    """Per chosen groom its cards and atlas, as the engine ships them (they sit on the archetype head)."""
    reg = unreal.AssetRegistryHelpers.get_asset_registry()
    done = []
    for slot, (item, _) in (recipe.get("grooms") or {}).items():
        if item is None or slot not in CARDS_SLOTS:
            continue
        groom = item[len("WI_"):] if item.startswith("WI_") else item
        out = os.path.join(EXPORT, "grooms", groom)
        os.makedirs(out, exist_ok=True)
        for a in reg.get_assets_by_path(f"{CARDS_ROOT}/{GROOM_DIRS.get(slot, slot)}/{groom}", recursive=True):
            n, cls = str(a.asset_name), str(a.asset_class_path.asset_name)
            if cls == "StaticMesh" and n.endswith("_LOD0"):
                log(f"cards {n}: {save_as(a.get_asset(), os.path.join(out, n + '.obj'))}")
            elif cls == "Texture2D" and "CardsAtlas_Attribute" in n:
                log(f"atlas {n}: {save_as(a.get_asset(), os.path.join(out, n + '.tga'))}")
        done.append(groom)
    return done


def render(c, sub, name):
    """Spawns the preview, lights it as a portrait and shoots it once shaders and textures settle."""
    os.makedirs(RENDER, exist_ok=True)
    unreal.SystemLibrary.execute_console_command(None, "t.IdleWhenNotForeground 0")
    unreal.get_editor_subsystem(unreal.LevelEditorSubsystem).new_level(f"/Game/{name}/RenderLevel")
    actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
    for a in actors.get_all_level_actors():
        if a.get_class().get_name() in ("SkyAtmosphere", "SkyLight", "DirectionalLight", "ExponentialHeightFog", "VolumetricCloud", "PostProcessVolume"):
            actors.destroy_actor(a)
    # the preview assembled from the saved character (grooms, clothes, the downloaded textures);
    # CHARACTER_CARDS=1 draws the grooms as cards (strands bound to a rigged face can lag the shot)
    if os.environ.get("CHARACTER_CARDS"):
        vs = c.viewport_settings
        vs.always_use_hair_cards = True
        c.viewport_settings = vs
    sub.assemble_for_preview(character=c)
    figure = sub.spawn_meta_human_actor(c, True)
    origin, extent = figure.get_actor_bounds(False)
    # MetaHumans stand facing +Y, Z up, centimetres; the eyes sit about 12 cm under the crown
    eyes = unreal.Vector(0.0, 2.0, float(os.environ.get("CHARACTER_EYE_Z", "0")) or origin.z + extent.z - 12.0)

    def place(cls, off):
        a = actors.spawn_actor_from_class(cls, eyes + off, unreal.Rotator(0, 0, 0))
        a.set_actor_rotation(unreal.MathLibrary.find_look_at_rotation(a.get_actor_location(), eyes), False)
        return a

    for off, inten, col, size in ((unreal.Vector(-80, 90, 35), float(os.environ.get("CHARACTER_KEY", "4")), (1.0, 0.95, 0.88), 90.0),
                                  (unreal.Vector(90, 100, 0), 2.5, (0.9, 0.93, 1.0), 110.0)):
        lc = place(unreal.RectLight, off).light_component
        lc.set_intensity(inten)
        lc.set_light_color(unreal.LinearColor(*col, 1.0))
        lc.set_editor_property("source_width", size)
        lc.set_editor_property("source_height", size)
    rim = place(unreal.PointLight, unreal.Vector(40, -60, 30)).light_component
    rim.set_intensity(float(os.environ.get("CHARACTER_RIM", "20")))
    rim.set_attenuation_radius(150.0)
    world = unreal.EditorLevelLibrary.get_editor_world()
    rt = unreal.RenderingLibrary.create_render_target2d(world, 1600, 1600, unreal.TextureRenderTargetFormat.RTF_RGBA8_SRGB)
    cap = actors.spawn_actor_from_class(unreal.SceneCapture2D, eyes, unreal.Rotator(0, 0, 0))
    sc = cap.capture_component2d
    sc.texture_target = rt
    sc.capture_source = unreal.SceneCaptureSource.SCS_FINAL_COLOR_LDR
    sc.fov_angle = 24.0
    sc.capture_every_frame = False
    pp = sc.post_process_settings
    pp.override_auto_exposure_method = True
    pp.auto_exposure_method = unreal.AutoExposureMethod.AEM_MANUAL
    pp.override_auto_exposure_bias = True
    pp.auto_exposure_bias = float(os.environ.get("CHARACTER_EV", "4.0"))
    sc.post_process_settings = pp
    sc.post_process_blend_weight = 1.0
    dist = float(os.environ.get("CHARACTER_DIST", "95"))
    shots = []
    for label, deg in SHOTS:
        r = math.radians(deg)
        loc = eyes + unreal.Vector(-math.sin(r) * dist, math.cos(r) * dist, 0.0)
        shots.append((label, loc, unreal.MathLibrary.find_look_at_rotation(loc, eyes + unreal.Vector(0, 0, -4.0))))
    wait = float(os.environ.get("CHARACTER_WAIT", "150"))
    tag = os.environ.get("CHARACTER_TAG", "")
    start = time.time()
    state = {"handle": None, "i": 0}

    def tick(dt):
        if time.time() - start < wait:
            return
        if state["i"] < len(shots):
            label, loc, rot = shots[state["i"]]
            cap.set_actor_location_and_rotation(loc, rot, False, False)
            sc.capture_scene()
            sc.capture_scene()
            out = f"{name.lower()}{tag}-{label}"
            unreal.RenderingLibrary.export_render_target(world, rt, RENDER, out)
            os.replace(os.path.join(RENDER, out), os.path.join(RENDER, out + ".png"))
            log(f"rendered {os.path.join(RENDER, out)}.png")
            state["i"] += 1
            return
        unreal.unregister_slate_post_tick_callback(state["handle"])
        finish(c, sub)

    state["handle"] = unreal.register_slate_post_tick_callback(tick)


def finish(c, sub):
    if sub.is_object_added_for_editing(c):
        sub.remove_object_to_edit(c)
    log("done")
    unreal.SystemLibrary.quit_editor()


def main():
    with open(RECIPE) as f:
        recipe = json.load(f)
    name = recipe["name"]
    sub = unreal.get_editor_subsystem(unreal.MetaHumanCharacterEditorSubsystem)
    c = character(recipe)
    log(f"opened {c.get_path_name()}: {sub.try_add_object_to_edit(c)}")
    try:
        if "look" in STEPS:
            step("face", lambda: face(c, sub, recipe))
            step("skin", lambda: skin(c, sub, recipe))
            step("eyes", lambda: eyes(c, sub, recipe))
            step("grooms", lambda: grooms(c, sub, recipe))
        if "save" in STEPS:
            step("save", lambda: unreal.EditorAssetLibrary.save_loaded_asset(c, False))
        if "autorig" in STEPS:
            step("face rig", lambda: autorig(c, sub))
            step("save", lambda: unreal.EditorAssetLibrary.save_loaded_asset(c, False))
        if "textures" in STEPS:
            step("textures", lambda: textures(c, sub))
            step("save", lambda: unreal.EditorAssetLibrary.save_loaded_asset(c, False))
        if "export" in STEPS:
            os.makedirs(EXPORT, exist_ok=True)
            step("export", lambda: export(c, name))
        if "cards" in STEPS:
            step("cards", lambda: cards(recipe))
        if "render" in STEPS:
            if step("render", lambda: render(c, sub, name) or "waiting for shaders") is not None:
                return
    except Exception:
        log("failed: " + traceback.format_exc().replace("\n", " | "))
    finish(c, sub)


main()
