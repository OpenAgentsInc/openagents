"""Map MetaHuman Creator's skin-tone chart (u, v) to albedo colour (UE 5.8).

Run headless on a copy of the project:

    DRACONIA_UE_PROJECT=... scripts/unreal/run_ue.sh scripts/unreal/skin_tone_probe.py

For each (u, v) on a coarse grid it commits the skin tone (local texture
synthesis; no cloud call), draws the synthesized face albedo onto a render
target and exports it to ~/work/film-out/draconia/metahuman/tone-probe/,
so a tone can be picked by number to match the reference instead of by
eye. Transient textures can't go through an asset exporter (it asserts),
hence the render target. Nothing is saved back to the character. Private
MetaHuman data, never used to train AI models.
"""

import os
import traceback

import unreal

OUT = os.path.expanduser("~/work/film-out/draconia/metahuman/tone-probe")
GRID = [float(x) for x in os.environ.get("DRACONIA_TONE_GRID", "0.05,0.35,0.7").split(",")]


def log(msg):
    print(f"DRACONIA {msg}", flush=True)
    unreal.log(f"DRACONIA {msg}")


def face_albedo(c):
    for k, t in c.synthesized_face_textures.items():
        if str(k).endswith("BASECOLOR: 0>") or k == unreal.FaceTextureType.BASECOLOR:
            return t
    return None


def main():
    os.makedirs(OUT, exist_ok=True)
    c = unreal.load_asset("/Game/Draconia/Draconia.Draconia")
    sub = unreal.get_editor_subsystem(unreal.MetaHumanCharacterEditorSubsystem)
    sub.try_add_object_to_edit(c)
    world = unreal.EditorLevelLibrary.get_editor_world()
    try:
        for u in GRID:
            for v in GRID:
                s = unreal.MetaHumanCharacterSkinProperties()
                s.u = u
                s.v = v
                s.roughness = 0.9
                settings = unreal.MetaHumanCharacterSkinSettings()
                settings.skin = s
                sub.commit_skin_settings(character=c, skin_settings=settings)
                face = face_albedo(c)
                if face is None:
                    log("no face albedo")
                    return
                rt = unreal.RenderingLibrary.create_render_target2d(world, 512, 512, unreal.TextureRenderTargetFormat.RTF_RGBA8_SRGB)
                canvas, size, ctx = unreal.RenderingLibrary.begin_draw_canvas_to_render_target(world, rt)
                canvas.draw_texture(face, unreal.Vector2D(0, 0), unreal.Vector2D(512, 512), unreal.Vector2D(0, 0), unreal.Vector2D(1, 1), unreal.LinearColor(1, 1, 1, 1), unreal.BlendMode.BLEND_OPAQUE)
                unreal.RenderingLibrary.end_draw_canvas_to_render_target(world, ctx)
                unreal.RenderingLibrary.export_render_target(world, rt, OUT, f"tone_{u:.2f}_{v:.2f}")
                log(f"tone {u:.2f} {v:.2f}")
    except Exception:
        log("probe failed: " + traceback.format_exc().replace("\n", " | "))
    finally:
        if sub.is_object_added_for_editing(c):
            sub.remove_object_to_edit(c)
        log("done")
        unreal.SystemLibrary.quit_editor()


main()
