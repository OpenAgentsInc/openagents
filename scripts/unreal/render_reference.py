"""Unreal's own render of a MetaHuman head, for the parity grid's middle
column (reference video | Unreal | Verse).

Run with scripts/unreal/run_ue.sh:

    DRACONIA_UE_KEEP=1 scripts/unreal/run_ue.sh scripts/unreal/render_reference.py

It makes an empty level, places the MetaHuman Creator plugin's archetype
face and body (`/MetaHumanCharacter/Face/SKM_Face`, `.../SKM_Body`) with
their own MetaHuman materials, or Draconia's fitted head
(DRACONIA_UE_FACE=/Game/Draconia/Fit/Draconia_Head, from fit_face.py), lights them as the reference frame at 0:54 (a warm soft key high to
screen left, a dim warm fill, a teal rim behind screen right, a black room)
and frames the full-face shot with a scene capture (Verse's 85 mm framing,
f/3.2). Unreal renders it with Lumen and Unreal's skin, eye and hair
shading; captures are exported (8-bit sRGB) after the shaders have had time to
compile, to ~/work/film-out/draconia/ue/render/ (or
$DRACONIA_UE_RENDER), and the editor quits.

The renders are MetaHuman content: private, never committed, never used to
train AI models.
"""

import os
import time

import unreal

OUT = os.environ.get("DRACONIA_UE_RENDER", os.path.expanduser("~/work/film-out/draconia/ue/render"))
FACE = os.environ.get("DRACONIA_UE_FACE", "/MetaHumanCharacter/Face/SKM_Face")
BODY = os.environ.get("DRACONIA_UE_BODY", "/MetaHumanCharacter/Body/IdentityTemplate/SKM_Body")
# Seconds after setup at which to shoot (shaders compile in between).
SHOTS = [float(s) for s in os.environ.get("DRACONIA_UE_SHOTS", "60,150").split(",")]


def log(msg):
    print(f"DRACONIA {msg}", flush=True)
    unreal.log(f"DRACONIA {msg}")


# The character's exported MetaHuman materials (export_materials), set by
# slot: MetaHuman's own skin, eyes, teeth, lashes and wet shells.
MATERIALS = os.environ.get("DRACONIA_UE_MATERIALS", "")
SLOTS = [
    ("teeth", "MI_Face_Teeth"),
    ("eyeleft", "MI_Face_Eye_Left"),
    ("eye_left", "MI_Face_Eye_Left"),
    ("eyeright", "MI_Face_Eye_Right"),
    ("eye_right", "MI_Face_Eye_Right"),
    ("lash", "MI_Face_EyelashesHiLODs"),
    ("shell", "MI_Face_EyeShell"),
    ("edge", "MI_Face_LacrimalFluid"),
    ("lacrimal", "MI_Face_LacrimalFluid"),
    ("saliva", "MI_Face_LacrimalFluid"),
    ("body", "MI_Body_Skin"),
    ("head", "MI_Face_Skin_LOD0"),
]


def dress(comp):
    if not MATERIALS:
        return
    reg = unreal.AssetRegistryHelpers.get_asset_registry()
    mis = {str(a.asset_name): a for a in reg.get_assets_by_path(MATERIALS, recursive=True) if str(a.asset_class_path.asset_name) == "MaterialInstanceConstant"}
    for i, name in enumerate(comp.get_material_slot_names()):
        n = str(name).lower()
        pick = next((mi for key, mi in SLOTS if key in n and mi in mis), None)
        if pick:
            comp.set_material(i, mis[pick].get_asset())
        log(f"slot {name}: {pick}")


def spawn(cls, loc, rot=unreal.Rotator(0, 0, 0)):
    sub = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
    return sub.spawn_actor_from_class(cls, loc, rot)


def setup():
    levels = unreal.get_editor_subsystem(unreal.LevelEditorSubsystem)
    levels.new_level("/Game/Draconia/RefLevel")
    # A black room: drop the template's sky, sun, sky light, fog and clouds.
    actors = unreal.get_editor_subsystem(unreal.EditorActorSubsystem)
    for a in actors.get_all_level_actors():
        if a.get_class().get_name() in ("SkyAtmosphere", "SkyLight", "DirectionalLight", "ExponentialHeightFog", "VolumetricCloud", "PostProcessVolume"):
            actors.destroy_actor(a)
    meshes = []
    for path in (FACE, BODY):
        mesh = unreal.load_asset(path)
        if mesh is None:
            log(f"missing {path}")
            continue
        a = spawn(unreal.SkeletalMeshActor, unreal.Vector(0, 0, 0))
        a.skeletal_mesh_component.set_skinned_asset_and_update(mesh)
        dress(a.skeletal_mesh_component)
        # Always LOD0: a scene capture's view doesn't drive LOD selection.
        a.skeletal_mesh_component.set_forced_lod(1)
        meshes.append(a)
        log(f"placed {path}")
    if not meshes:
        raise RuntimeError(f"no face mesh at {FACE}")
    face = meshes[0]
    origin, extent = face.get_actor_bounds(False)
    log(f"face bounds origin {origin} extent {extent}")
    # MetaHumans stand facing +Y in Unreal (Z up, centimetres). The eyes
    # sit about 10.5 cm under the crown, 2 cm in front of the bounds'
    # centre line.
    top = origin.z + extent.z
    eyes = unreal.Vector(origin.x, origin.y + 2.0, top - 10.5)
    front = unreal.Vector(0, 1, 0)
    # The full-face shot: 74 cm in front, a touch to screen left, aimed a
    # little under the eyes (as the Verse shot).
    # Screen left: the camera faces -Y; in Unreal's left-handed frame (X
    # forward, Y right at zero yaw) its right is then +X, so left is -X.
    left = unreal.Vector(-1, 0, 0)
    # DRACONIA_UE_FRAMING "distance,left,down" (cm) matches Verse's shots:
    # the full face (74, 6, 4.8, the default) or the 0:24 medium (125, 7, 7).
    dist, side, down = (float(x) for x in os.environ.get("DRACONIA_UE_FRAMING", "74,6,4.8").split(","))
    cam_loc = eyes + front * dist + left * side + unreal.Vector(0, 0, -down) * (0 if dist < 100 else 1)
    target = eyes + left * (side if dist >= 100 else 0) + unreal.Vector(0, 0, -down)
    rot = unreal.MathLibrary.find_look_at_rotation(cam_loc, target)
    # A scene capture, not a viewport screenshot: it renders on demand,
    # whatever the editor's window is doing. Its horizontal field of view
    # matches Verse's 85 mm on a 24 mm-high 16:9 gate (28.1 degrees).
    cap = spawn(unreal.SceneCapture2D, cam_loc, rot)
    sc = cap.capture_component2d
    rt = unreal.RenderingLibrary.create_render_target2d(unreal.EditorLevelLibrary.get_editor_world(), 1920, 1080, unreal.TextureRenderTargetFormat.RTF_RGBA8_SRGB)
    sc.texture_target = rt
    sc.capture_source = unreal.SceneCaptureSource.SCS_FINAL_COLOR_LDR
    sc.fov_angle = 28.1
    sc.capture_every_frame = False
    pp = sc.post_process_settings
    pp.override_auto_exposure_method = True
    pp.auto_exposure_method = unreal.AutoExposureMethod.AEM_MANUAL
    pp.override_auto_exposure_bias = True
    pp.auto_exposure_bias = float(os.environ.get("DRACONIA_UE_EV", "4.0"))
    pp.override_depth_of_field_fstop = True
    pp.depth_of_field_fstop = 3.2
    pp.override_depth_of_field_focal_distance = True
    pp.depth_of_field_focal_distance = (cam_loc - eyes).length()
    pp.override_depth_of_field_sensor_width = True
    pp.depth_of_field_sensor_width = 42.67
    sc.post_process_settings = pp
    sc.post_process_blend_weight = 1.0
    key = spawn(unreal.RectLight, eyes + left * 95 + front * 60 + unreal.Vector(0, 0, 40))
    key.set_actor_rotation(unreal.MathLibrary.find_look_at_rotation(key.get_actor_location(), eyes), False)
    k = key.light_component
    k.set_intensity(float(os.environ.get("DRACONIA_UE_KEY", "8")))
    k.set_light_color(unreal.LinearColor(1.0, 0.95, 0.9, 1.0))
    k.set_editor_property("source_width", 60.0)
    k.set_editor_property("source_height", 60.0)
    fill = spawn(unreal.RectLight, eyes - left * 90 + front * 100 + unreal.Vector(0, 0, 10))
    fill.set_actor_rotation(unreal.MathLibrary.find_look_at_rotation(fill.get_actor_location(), eyes), False)
    fill.light_component.set_intensity(float(os.environ.get("DRACONIA_UE_FILL", "1.5")))
    fill.light_component.set_light_color(unreal.LinearColor(0.95, 0.8, 0.7, 1.0))
    fill.light_component.set_editor_property("source_width", 80.0)
    fill.light_component.set_editor_property("source_height", 80.0)
    rim = spawn(unreal.PointLight, eyes - left * 45 - front * 55 + unreal.Vector(0, 0, 18))
    rim.light_component.set_intensity(float(os.environ.get("DRACONIA_UE_RIM", "120")))
    rim.light_component.set_light_color(unreal.LinearColor(0.35, 0.95, 0.85, 1.0))
    rim.light_component.set_attenuation_radius(110.0)
    return sc, rt


def main():
    os.makedirs(OUT, exist_ok=True)
    # Keep the editor rendering at full rate in the background.
    unreal.SystemLibrary.execute_console_command(None, "t.IdleWhenNotForeground 0")
    try:
        sc, rt = setup()
    except Exception as e:
        log(f"setup failed: {e}")
        unreal.SystemLibrary.quit_editor()
        return
    world = unreal.EditorLevelLibrary.get_editor_world()
    start = time.time()
    pending = list(SHOTS)
    state = {"handle": None}

    def tick(dt):
        t = time.time() - start
        if pending and t >= pending[0]:
            s = pending.pop(0)
            sc.capture_scene()
            name = f"{os.environ.get('DRACONIA_UE_NAME', 'fullface')}-{int(s)}s"
            unreal.RenderingLibrary.export_render_target(world, rt, OUT, name)
            log(f"captured {os.path.join(OUT, name)}")
        if not pending and t >= SHOTS[-1] + 5:
            unreal.unregister_slate_post_tick_callback(state["handle"])
            log("done")
            unreal.SystemLibrary.quit_editor()

    state["handle"] = unreal.register_slate_post_tick_callback(tick)


main()
