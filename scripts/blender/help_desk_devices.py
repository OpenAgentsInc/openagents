"""Export the launch film's MacBook and iPhone for the Grid's help desk.

Run headless:
    Blender -b --factory-startup --python scripts/blender/help_desk_devices.py -- [OUT_DIR]

The devices come from the studio's `scripts/film/studio/devices.py`, built
exactly as the film's `four-apps-intro` shot builds them (a silver 14-inch
MacBook open to 112 degrees and a silver iPhone), with no video. Each model
is written as two meshes:

- `body`: every part but the display, joined, with its materials kept, and
  decimated to the model's triangle budget.
- `screen`: the display alone, with the material `screen` and UVs that span
  the display from 0 to 1, so the Grid draws its own screen image there.

Origins sit at the center of the base, front toward glTF +Z, 1 unit = 1 m.
The phone lies screen up; the Grid poses it on the desk.
"""

import json
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, "..", "film", "studio"))

import bpy  # noqa: E402
import kit  # noqa: E402
import devices  # noqa: E402

# Triangle budgets for the body of each device. The screen is a few dozen
# more. The Grid draws each face dark and its sharp edges as lines, so the
# budget only has to keep the silhouette and the keyboard's rows.
BUDGET = {"macbook": 3000, "iphone": 800}

a = kit.args()
OUT = a[0] if a else os.path.join(kit.REPO, "assets", "verse", "generated", "help_desk")


def meshes_under(root):
    out = []
    stack = [root]
    while stack:
        o = stack.pop()
        stack.extend(o.children)
        if o.type == "MESH":
            out.append(o)
    return out


def bake(objs):
    """Bake every parent transform into the mesh and unparent."""
    for o in objs:
        m = o.matrix_world.copy()
        o.parent = None
        o.data.transform(m)
        o.matrix_world.identity()


def decimate(obj, budget):
    obj.data.calc_loop_triangles()
    tris = len(obj.data.loop_triangles)
    if tris <= budget:
        return
    mod = obj.modifiers.new("budget", "DECIMATE")
    mod.decimate_type = "COLLAPSE"
    mod.ratio = budget / tris
    mod.use_collapse_triangulate = True
    devices.apply_modifiers(obj)


def triangles(obj):
    obj.data.calc_loop_triangles()
    return len(obj.data.loop_triangles)


def build(kind):
    kit.reset()
    if kind == "macbook":
        root = devices.macbook(video=None, finish="silver", size="14", open_deg=112)
    else:
        root = devices.iphone(video=None, finish="silver")
    screen_name = root["screen"]
    parts = meshes_under(root)
    bpy.context.view_layer.update()
    bake(parts)
    for o in [o for o in bpy.data.objects if o.type == "EMPTY"]:
        bpy.data.objects.remove(o)

    screen = bpy.data.objects[screen_name]
    others = [o for o in parts if o.name != screen_name]
    body = devices.join("body", others)
    decimate(body, BUDGET[kind])

    # The display keeps its UVs and takes a plain material the Grid
    # recognizes by name; the film's video material is dropped.
    plain = kit.mat("screen", (0.0, 0.0, 0.0), 0.2, emit=(1.0, 1.0, 1.0), strength=1.0)
    screen.data.materials.clear()
    screen.data.materials.append(plain)
    screen.name = "screen"
    for m in list(bpy.data.materials):
        if m.users == 0:
            bpy.data.materials.remove(m)

    # Base center at the origin.
    pts = [v.co for o in (body, screen) for v in o.data.vertices]
    lo = [min(p[i] for p in pts) for i in range(3)]
    hi = [max(p[i] for p in pts) for i in range(3)]
    shift = ((lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, lo[2])
    for o in (body, screen):
        for v in o.data.vertices:
            v.co.x -= shift[0]
            v.co.y -= shift[1]
            v.co.z -= shift[2]
        o.data.update()

    out = os.path.join(OUT, kind + ".glb")
    info = kit.export(out, extra={"body_triangles": triangles(body), "screen_triangles": triangles(screen)})
    return info


# The screens' still images: the film's frames of the OpenAgents chat on the
# desktop and the OpenAgents app's home on the phone, cropped to each
# display's aspect (never stretched) and scaled down. (x0, y0, x1, y1) in
# source pixels, and the output size.
SCREENS = {
    "macbook": ("docs/verse/film/shots/chat-desktop.png", (0, 0, 1218, 790), (384, 249)),
    "iphone": ("docs/verse/film/shots/home-phone.png", (60, 0, 689, 1368), (144, 313)),
}


def screen_image(kind):
    import numpy as np

    src, (x0, y0, x1, y1), (w, h) = SCREENS[kind]
    img = bpy.data.images.load(os.path.join(kit.REPO, src))
    iw, ih = img.size
    px = np.array(img.pixels[:], dtype=np.float32).reshape(ih, iw, 4)
    # Blender's rows run bottom up.
    crop = px[ih - y1 : ih - y0, x0:x1]
    # The Grid admits eight-bit RGBA only.
    crop = crop.copy()
    crop[..., 3] = 1.0
    out = bpy.data.images.new(kind + "-screen", x1 - x0, y1 - y0, alpha=True)
    out.pixels = crop.ravel()
    out.scale(w, h)
    out.filepath_raw = os.path.join(OUT, kind + "-screen.png")
    out.file_format = "PNG"
    scene = bpy.context.scene
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"
    scene.render.image_settings.color_depth = "8"
    scene.render.image_settings.compression = 100
    scene.view_settings.view_transform = "Standard"
    scene.view_settings.look = "None"
    out.save_render(out.filepath_raw, scene=scene)
    print("SCREEN", json.dumps({"out": os.path.relpath(out.filepath_raw, kit.REPO), "size": [w, h]}))


if __name__ == "__main__":
    for kind in ("macbook", "iphone"):
        build(kind)
        screen_image(kind)
