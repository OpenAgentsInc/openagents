"""Render one studio device shot for the launch film (#11432) from a JSON spec.

    Blender -b --factory-startup -P scripts/film/studio/studio.py -- \\
        --shot scripts/film/studio/shots/macbook-chat.json \\
        --out ~/work/film-out/studio/macbook-chat.mp4 [--preview] \\
        [--video PATH] [--still SECONDS] [--frames A:B] [--keep-frames]

The spec (see `shots/*.json`) names the device and its finish, the screen
footage and its in-point, the background sweep, the light rig, the camera and
device keyframes, and the duration. Frames render with Cycles to PNG, then
ffmpeg encodes 1920×1080 H.264 yuv420p at CRF 12 per
`docs/verse/film/cinematic.md`.

Camera and device keys are interpolated with Catmull-Rom splines through
every key, after the shot's time is remapped by its ease curve, so a move
starts and stops softly and never jerks at an inner key. A camera key gives
either `pos` [x, y, z] or `orbit` [azimuth°, elevation°, distance] around its
`target`; azimuth 0 looks from the front (-Y), positive swings to the right.

`--preview` renders at a quarter of the pixels with few samples. `--still T`
writes one PNG at T seconds, next to `--out`, for look development.
"""

import argparse
import json
import math
import os
import shutil
import subprocess
import sys
import time

import bpy
from mathutils import Quaternion, Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import devices  # noqa: E402

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))


def parse():
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    ap = argparse.ArgumentParser(prog="studio.py")
    ap.add_argument("--shot", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--video", help="override the spec's screen footage")
    ap.add_argument("--preview", action="store_true")
    ap.add_argument("--still", type=float, help="render one PNG at this time (s)")
    ap.add_argument("--frames", help="render only frames A:B (1-based, inclusive)")
    ap.add_argument("--samples", type=int)
    ap.add_argument("--resolution", type=lambda s: [int(v) for v in s.lower().split("x")],
                    help="WxH, overriding the spec (default 3840x2160)")
    ap.add_argument("--keep-frames", action="store_true")
    ap.add_argument("--blend", help="also save the built scene to this .blend")
    return ap.parse_args(argv)


# ---------------------------------------------------------------- easing


def ease(kind, x):
    x = min(1.0, max(0.0, x))
    if kind == "linear":
        return x
    if kind == "in":
        return x * x * x
    if kind == "out":
        return 1 - (1 - x) ** 3
    if kind == "gentle":  # in-out with a long cruise, for slow product moves
        return x * x * (3 - 2 * x) * 0.6 + x * 0.4
    return x * x * x * (x * (6 * x - 15) + 10)  # "inout": smootherstep


def spline(keys, t, field):
    """Catmull-Rom through the keys' `field` (scalars or vectors) at time t."""
    ts = [k["t"] for k in keys]
    vals = [k[field] for k in keys]
    scalar = not isinstance(vals[0], (list, tuple))
    vs = [Vector((v, 0.0)) if scalar else Vector(v) for v in vals]
    if len(vs) == 1 or t <= ts[0]:
        out = vs[0]
    elif t >= ts[-1]:
        out = vs[-1]
    else:
        i = max(j for j in range(len(ts) - 1) if ts[j] <= t)
        u = (t - ts[i]) / (ts[i + 1] - ts[i])
        p0 = vs[i - 1] if i > 0 else vs[i] * 2 - vs[i + 1]
        p1, p2 = vs[i], vs[i + 1]
        p3 = vs[i + 2] if i + 2 < len(vs) else vs[i + 1] * 2 - vs[i]
        out = 0.5 * ((2 * p1) + (-p0 + p2) * u + (2 * p0 - 5 * p1 + 4 * p2 - p3) * u * u
                     + (-p0 + 3 * p1 - 3 * p2 + p3) * u ** 3)
    return out[0] if scalar else out


def fill(keys, field, default):
    """Carry a field forward through keys that omit it."""
    last = default
    for k in keys:
        if field in k:
            last = k[field]
        else:
            k[field] = last


# ---------------------------------------------------------------- scene


def setup_render(spec, args):
    sc = bpy.context.scene
    r = spec.get("render", {})
    fps = spec.get("fps", 30)
    sc.render.fps = fps
    sc.frame_start = 1
    sc.frame_end = int(round(spec["duration"] * fps))
    res = args.resolution or r.get("resolution", [3840, 2160])
    sc.render.resolution_x, sc.render.resolution_y = res
    sc.render.resolution_percentage = 25 if args.preview else 100
    sc.render.filter_size = 1.0  # a tight pixel filter keeps UI text crisp
    sc.render.engine = "CYCLES"
    cy = sc.cycles
    prefs = bpy.context.preferences.addons["cycles"].preferences
    try:
        prefs.compute_device_type = "METAL"
        prefs.get_devices()
        for d in prefs.devices:
            d.use = d.type != "CPU"
        cy.device = "GPU"
    except Exception as e:  # CPU fallback keeps the script portable
        print("studio: GPU unavailable, using CPU:", e)
    cy.samples = args.samples or (24 if args.preview else r.get("samples", 40))
    cy.use_adaptive_sampling = True
    cy.adaptive_threshold = 0.015
    cy.use_denoising = True
    cy.denoiser = "OPENIMAGEDENOISE"
    cy.denoising_use_gpu = True
    cy.denoising_input_passes = "RGB_ALBEDO_NORMAL"
    cy.max_bounces = 8
    cy.glossy_bounces = 6
    cy.diffuse_bounces = 3
    cy.transmission_bounces = 6
    cy.caustics_reflective = False
    cy.caustics_refractive = False
    cy.blur_glossy = 0.5
    cy.film_exposure = 1.0
    sc.render.use_persistent_data = True
    bpy.context.preferences.filepaths.use_scripts_auto_execute = True  # grid line driver
    sc.render.use_motion_blur = r.get("motion_blur", True)
    sc.render.motion_blur_shutter = r.get("shutter", 0.5)
    sc.render.film_transparent = False
    sc.render.image_settings.file_format = "PNG"
    sc.render.image_settings.color_depth = "8"
    vs = sc.view_settings
    vs.view_transform = r.get("view_transform", "Khronos PBR Neutral")
    vs.look = r.get("look", "None")
    vs.exposure = r.get("exposure", 0.0)
    vs.gamma = 1.0
    sc.display_settings.display_device = "sRGB"
    sc.sequencer_colorspace_settings.name = "sRGB"


def color(c):
    return (*c, 1.0) if len(c) == 3 else tuple(c)


def setup_background(bg):
    """A seamless cyclorama: a floor that curves up into a back wall.

    kind "sweep" is one color; "gradient" fades floor → wall top by height.
    `floor_rough` below ~0.3 gives the floor a soft reflection of the device.
    """
    w = bpy.context.scene.world or bpy.data.worlds.new("world")
    bpy.context.scene.world = w
    w.use_nodes = True
    bgn = w.node_tree.nodes["Background"]
    bgn.inputs["Color"].default_value = color(bg.get("world", [0.0, 0.0, 0.0]))
    bgn.inputs["Strength"].default_value = 1.0
    if bg.get("kind") == "none":
        return
    if bg.get("kind") == "grid":
        setup_grid(bg)
        return
    import bmesh

    bm = bmesh.new()
    radius = bg.get("curve", 0.9)
    depth = bg.get("depth", 1.4)  # how far behind the device the curve starts
    near, height, half = 6.0, 4.0, 6.0
    prof = [(-near, 0.0)]
    for i in range(25):
        a = math.radians(-90 + 90 * i / 24)
        prof.append((depth + radius * math.cos(a), radius + radius * math.sin(a)))
    prof.append((depth + radius, height))
    rows = []
    for y, z in prof:
        rows.append([bm.verts.new((x, y, z)) for x in (-half, half)])
    for a, b in zip(rows, rows[1:]):
        f = bm.faces.new((a[0], a[1], b[1], b[0]))
        f.smooth = True
    me = bpy.data.meshes.new("cyc")
    bm.to_mesh(me)
    bm.free()
    cyc = bpy.data.objects.new("cyclorama", me)
    bpy.context.scene.collection.objects.link(cyc)
    m = bpy.data.materials.new("cyc")
    m.use_nodes = True
    nodes, links = m.node_tree.nodes, m.node_tree.links
    b = nodes["Principled BSDF"]
    b.inputs["Roughness"].default_value = bg.get("floor_rough", 0.55)
    b.inputs["Specular IOR Level"].default_value = bg.get("floor_spec", 0.35)
    if bg.get("kind") == "gradient":
        geo = nodes.new("ShaderNodeNewGeometry")
        sep = nodes.new("ShaderNodeSeparateXYZ")
        links.new(geo.outputs["Position"], sep.inputs["Vector"])
        mr = nodes.new("ShaderNodeMapRange")
        mr.inputs["From Min"].default_value = 0.0
        mr.inputs["From Max"].default_value = bg.get("gradient_height", 1.6)
        links.new(sep.outputs["Z"], mr.inputs["Value"])
        mix = nodes.new("ShaderNodeMix")
        mix.data_type = "RGBA"
        mix.inputs[6].default_value = color(bg["color"])
        mix.inputs[7].default_value = color(bg.get("color_top", bg["color"]))
        links.new(mr.outputs["Result"], mix.inputs["Factor"])
        links.new(mix.outputs[2], b.inputs["Base Color"])
    else:
        b.inputs["Base Color"].default_value = color(bg.get("color", [0.02, 0.02, 0.02]))
    me.materials.append(m)


# The Verse's Grid, from crates/verse-core/src/world.rs and the grid pack
# (crates/verse/src/grid_pack.rs, world::ground): a line every GRID meters
# out to ±HALF, every LOT-th "street" line (offset LOT/2) one ladder step
# brighter, drawn unlit in the neutral palette over the near-black field.
GRID, LOT, HALF = 4.0, 24.0, 264.0
QUARTER = 0x656565  # Intensity::Quarter = coder_noir::CONTENT_TERTIARY
HALF_STEP = 0x818181  # Intensity::Half = coder_noir::CONTENT_SECONDARY
FIELD = 0x0A0A0A  # coder_noir::CANVAS, the clear color


def srgb_hex(rgb):
    def ch(c):
        c = c / 255.0
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4
    return (ch((rgb >> 16) & 255), ch((rgb >> 8) & 255), ch(rgb & 255), 1.0)


def setup_grid(bg):
    """The Grid as the stage: the black field with the ground lattice's
    lines, about one pixel wide at any distance like the engine's line pass.

    The floor under the lines is black glass (`floor_rough`), so the device
    stands on a faint reflection of itself instead of floating. Line width is
    set from the camera per frame (`pixel_angle`, filled in by `build`).
    """
    import bmesh

    sc = bpy.context.scene
    # `scale` shrinks the whole lattice about the device, so a laptop sits on
    # the Grid the way an avatar does (one cell is GRID * scale meters).
    s = bg.get("scale", 1.0)
    G, L_, H = GRID * s, LOT * s, HALF * s
    w = sc.world
    w.node_tree.nodes["Background"].inputs["Color"].default_value = srgb_hex(FIELD)
    bm = bmesh.new()
    ext = H + G
    bm.faces.new([bm.verts.new((x, y, 0.0)) for x, y in ((-ext, -ext), (ext, -ext), (ext, ext), (-ext, ext))])
    me = bpy.data.meshes.new("grid")
    bm.to_mesh(me)
    bm.free()
    floor = bpy.data.objects.new("grid", me)
    sc.collection.objects.link(floor)
    off = bg.get("offset", [0.0, 0.0])
    floor.location = (off[0], off[1], 0.0)
    m = bpy.data.materials.new("grid")
    m.use_nodes = True
    nt = m.node_tree
    nodes, links = nt.nodes, nt.links
    b = nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = srgb_hex(FIELD)
    b.inputs["Roughness"].default_value = bg.get("floor_rough", 0.22)
    b.inputs["Specular IOR Level"].default_value = bg.get("floor_spec", 0.5)

    def math(op, a, b_=None):
        n = nodes.new("ShaderNodeMath")
        n.operation = op
        for i, v in enumerate((a, b_)):
            if v is None:
                continue
            if isinstance(v, (int, float)):
                n.inputs[i].default_value = v
            else:
                links.new(v, n.inputs[i])
        return n.outputs[0]

    tc = nodes.new("ShaderNodeTexCoord")
    sep = nodes.new("ShaderNodeSeparateXYZ")
    links.new(tc.outputs["Object"], sep.inputs["Vector"])
    vl = nodes.new("ShaderNodeVectorMath")
    vl.operation = "LENGTH"
    links.new(tc.outputs["Camera"], vl.inputs[0])
    dist = vl.outputs["Value"]
    # Half a line's width in meters at this distance: `width_px` pixels.
    half_w = math("MULTIPLY", dist, 0.5)
    half_w.node.name = "halfwidth"
    inside = math("LESS_THAN", math("ABSOLUTE", sep.outputs["X"]), H + 1e-4)
    inside = math("MULTIPLY", inside, math("LESS_THAN", math("ABSOLUTE", sep.outputs["Y"]), H + 1e-4))

    # The engine draws lines a pixel wide on screen whatever their angle. A
    # pixel's footprint on the floor stretches along the view direction, by
    # 1/sqrt(1 - (I·axis)²) for I the unit ray toward the camera.
    geo = nodes.new("ShaderNodeNewGeometry")
    isep = nodes.new("ShaderNodeSeparateXYZ")
    links.new(geo.outputs["Incoming"], isep.inputs["Vector"])

    def stretch(component):
        s2 = math("SUBTRACT", 1.0, math("MULTIPLY", component, component))
        return math("DIVIDE", half_w, math("SQRT", math("MAXIMUM", s2, 0.0004)))

    hw_x, hw_y = stretch(isep.outputs["X"]), stretch(isep.outputs["Y"])

    def lines(coord, period, phase, hw):
        # distance to the nearest line of this family
        u = math("SUBTRACT", coord, phase)
        d = math("ABSOLUTE", math("SUBTRACT", u, math("MULTIPLY", math("ROUND", math("DIVIDE", u, period)), period)))
        return math("LESS_THAN", d, hw)

    X, Y = sep.outputs["X"], sep.outputs["Y"]
    minor = math("MAXIMUM", lines(X, G, 0.0, hw_x), lines(Y, G, 0.0, hw_y))
    street = math("MAXIMUM", lines(X, L_, L_ / 2, hw_x), lines(Y, L_, L_ / 2, hw_y))
    q, h = srgb_hex(QUARTER)[0], srgb_hex(HALF_STEP)[0]
    level = math("ADD", math("MULTIPLY", minor, q), math("MULTIPLY", street, h - q))
    # Lines dim toward the horizon, as the engine's fog does.
    fade = math("POWER", math("ADD", math("DIVIDE", dist, bg.get("fade", 90.0) * s), 1.0), -1.5)
    level = math("MULTIPLY", math("MULTIPLY", level, fade), inside)
    level = math("MULTIPLY", level, bg.get("line_gain", 1.0))
    em = nodes.new("ShaderNodeEmission")
    em.inputs["Color"].default_value = (1, 1, 1, 1)
    links.new(level, em.inputs["Strength"])
    add = nodes.new("ShaderNodeAddShader")
    links.new(b.outputs[0], add.inputs[0])
    links.new(em.outputs[0], add.inputs[1])
    out = nodes["Material Output"]
    links.new(add.outputs[0], out.inputs["Surface"])
    me.materials.append(m)
    sc["grid_width_px"] = bg.get("width_px", 1.2)


def kelvin(k):
    """Approximate linear RGB of a blackbody at k kelvin, normalized."""
    t = k / 100.0
    r = 255 if t <= 66 else 329.7 * (t - 60) ** -0.1332
    g = 99.47 * math.log(t) - 161.12 if t <= 66 else 288.12 * (t - 60) ** -0.0755
    bl = 255 if t >= 66 else (0 if t <= 19 else 138.52 * math.log(t - 10) - 305.04)
    c = [min(255, max(0, v)) / 255 for v in (r, g, bl)]
    c = [v ** 2.2 for v in c]
    m = max(c)
    return tuple(v / m for v in c)


def setup_lights(rig, target):
    """Area lights placed on a sphere around the target, all aimed at it.

    The rig's `rotation` swings every light around the device (Ultramock's
    environment rotation) and `brightness` scales them all.
    """
    rot = rig.get("rotation", 0.0)
    gain = rig.get("brightness", 1.0)
    aim = bpy.data.objects.new("light_target", None)
    aim.location = target
    bpy.context.scene.collection.objects.link(aim)
    for i, L in enumerate(rig["lights"]):
        az = math.radians(L.get("azimuth", 0) + rot)
        el = math.radians(L.get("elevation", 30))
        d = L.get("distance", 1.5)
        pos = Vector(target) + d * Vector((math.sin(az) * math.cos(el), -math.cos(az) * math.cos(el), math.sin(el)))
        ld = bpy.data.lights.new(L.get("name", f"light{i}"), "AREA")
        size = L.get("size", 1.0)
        if isinstance(size, (list, tuple)):
            ld.shape = "RECTANGLE"
            ld.size, ld.size_y = size
        else:
            ld.shape = L.get("shape", "DISK")
            ld.size = size
        ld.energy = L.get("power", 100) * gain
        ld.color = kelvin(L["kelvin"]) if "kelvin" in L else tuple(L.get("color", (1, 1, 1)))
        ld.spread = math.radians(L.get("spread", 180))
        ld.use_shadow = L.get("shadow", True)
        ob = bpy.data.objects.new(ld.name, ld)
        ob.location = pos
        bpy.context.scene.collection.objects.link(ob)
        c = ob.constraints.new("TRACK_TO")
        if "aim" in L:  # aim this light somewhere else, e.g. a glow on the sweep
            own = bpy.data.objects.new(ld.name + "_aim", None)
            own.location = L["aim"]
            bpy.context.scene.collection.objects.link(own)
            c.target = own
        else:
            c.target = aim
        c.track_axis = "TRACK_NEGATIVE_Z"
        c.up_axis = "UP_Y"
        if not L.get("visible", True):
            ob.visible_glossy = False
        ob.visible_camera = False


def unlink_screen(root):
    """Keep the studio lights off the display with light linking.

    A softbox mirrored in the display glass washes the UI out, and no single
    light placement avoids every screen through a whole move. The front glass
    (display, bezel, notch or island) still reflects the lit room and floor,
    so it reads as glass; the metal keeps every highlight.
    """
    lit = bpy.data.collections.new("lit")
    glass = set(root.get("front_glass", [root.get("screen")]))
    glass.add("grid")  # the Grid's black glass floor mirrors the device, never a softbox
    for o in bpy.context.scene.objects:
        if o.type == "MESH" and o.name not in glass:
            lit.objects.link(o)
    for o in bpy.context.scene.objects:
        if o.type == "LIGHT":
            o.light_linking.receiver_collection = lit


def animate_camera(spec, cam, fps, nframes):
    c = spec["camera"]
    keys = [dict(k) for k in c["keys"]]
    for f, d in (("lens", 50.0), ("fstop", 5.6), ("roll", 0.0), ("target", [0, 0, 0])):
        fill(keys, f, d)
    orbit = "orbit" in keys[0]
    dur = spec["duration"]
    kind = c.get("ease", "inout")
    cam.rotation_mode = "QUATERNION"
    cd = cam.data
    cd.sensor_width = 36.0
    cd.dof.use_dof = c.get("dof", True)
    cd.clip_start = 0.01
    cd.clip_end = 50
    prev = None
    for fr in range(1, nframes + 1):
        t = ease(kind, (fr - 1) / max(1, nframes - 1)) * dur
        tgt = spline(keys, t, "target")
        if orbit:
            az, el, dist = spline(keys, t, "orbit")
            az, el = math.radians(az), math.radians(el)
            pos = tgt + dist * Vector((math.sin(az) * math.cos(el), -math.cos(az) * math.cos(el), math.sin(el)))
        else:
            pos = spline(keys, t, "pos")
        q = (tgt - pos).to_track_quat("-Z", "Y")
        q = q @ Quaternion((0, 0, 1), math.radians(spline(keys, t, "roll")))
        if prev is not None and prev.dot(q) < 0:
            q.negate()
        prev = q
        cam.location = pos
        cam.rotation_quaternion = q
        cd.lens = spline(keys, t, "lens")
        cd.dof.aperture_fstop = spline(keys, t, "fstop")
        if "focus_target" in keys[0]:  # focus on a point other than the aim
            focus = (spline(keys, t, "focus_target") - pos).length
        elif "focus" in keys[0]:
            focus = spline(keys, t, "focus")
        else:
            focus = (tgt - pos).length
        cd.dof.focus_distance = focus
        cam.keyframe_insert("location", frame=fr)
        cam.keyframe_insert("rotation_quaternion", frame=fr)
        cd.keyframe_insert("lens", frame=fr)
        cd.dof.keyframe_insert("aperture_fstop", frame=fr)
        cd.dof.keyframe_insert("focus_distance", frame=fr)
    for owner in (cam, cd):
        if owner.animation_data and owner.animation_data.action:
            for fc in _fcurves(owner.animation_data):
                for kp in fc.keyframe_points:
                    kp.interpolation = "LINEAR"


def _fcurves(ad):
    act = ad.action
    if hasattr(act, "fcurves"):
        return list(act.fcurves)
    # Blender 5 slotted actions: curves live in layers → strips → channelbags.
    out = []
    for layer in act.layers:
        for strip in layer.strips:
            for bag in strip.channelbags:
                out += list(bag.fcurves)
    return out


def animate_device(spec, root, nframes):
    d = spec.get("device_motion")
    pose = spec["device"].get("pose", {})
    root.location = pose.get("location", (0, 0, 0))
    root.rotation_euler = [math.radians(a) for a in pose.get("rotation", (0, 0, 0))]
    if not d:
        return
    keys = [dict(k) for k in d["keys"]]
    fill(keys, "rot", pose.get("rotation", [0, 0, 0]))
    fill(keys, "loc", pose.get("location", [0, 0, 0]))
    kind = d.get("ease", "inout")
    for fr in range(1, nframes + 1):
        t = ease(kind, (fr - 1) / max(1, nframes - 1)) * spec["duration"]
        root.location = spline(keys, t, "loc")
        root.rotation_euler = [math.radians(a) for a in spline(keys, t, "rot")]
        root.keyframe_insert("location", frame=fr)
        root.keyframe_insert("rotation_euler", frame=fr)
    for fc in _fcurves(root.animation_data):
        for kp in fc.keyframe_points:
            kp.interpolation = "LINEAR"


def build(spec, args):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    setup_render(spec, args)
    sc = bpy.context.scene
    fps = sc.render.fps
    nframes = sc.frame_end
    dev = spec["device"]
    scr = spec.get("screen", {})
    video = args.video or scr.get("video")
    if video:
        video = os.path.expanduser(video)
        if not os.path.isabs(video):
            video = os.path.join(REPO, video)
        if not os.path.exists(video):
            sys.exit(f"studio: screen footage not found: {video}")
    kw = dict(video=video, finish=dev.get("finish", "space-black"),
              in_frame=int(round(scr.get("in", 0.0) * fps)), brightness=scr.get("brightness", 1.0))
    if dev["type"] == "macbook":
        kw.update(size=str(dev.get("size", "14")), open_deg=dev.get("open", 112))
    if dev["type"] == "none":  # an empty stage, e.g. the title plate
        root = devices.empty("stage")
    else:
        root = devices.build(dev["type"], **kw)
    animate_device(spec, root, nframes)
    setup_background(spec.get("background", {}))
    setup_lights(spec["lights"], spec["lights"].get("target", spec["camera"]["keys"][0].get("target", [0, 0, 0])))
    if not spec.get("screen", {}).get("glare", False):
        unlink_screen(root)
    cd = bpy.data.cameras.new("cam")
    cam = bpy.data.objects.new("cam", cd)
    sc.collection.objects.link(cam)
    sc.camera = cam
    animate_camera(spec, cam, fps, nframes)
    grid_width(cam)
    return nframes


def grid_width(cam):
    """Keep grid lines `grid_width_px` wide: a driver turns the camera's
    lens and the output width into meters per unit of distance."""
    m = bpy.data.materials.get("grid")
    if not m:
        return
    sc = bpy.context.scene
    px = 1920.0  # width_px is in 1080p pixels, so lines keep their weight at 4K
    node = m.node_tree.nodes["halfwidth"]
    fc = node.inputs[1].driver_add("default_value")
    drv = fc.driver
    drv.type = "SCRIPTED"
    var = drv.variables.new()
    var.name = "lens"
    var.targets[0].id_type = "CAMERA"
    var.targets[0].id = cam.data
    var.targets[0].data_path = "lens"
    drv.expression = f"0.5 * {sc['grid_width_px']} * 2 * atan(18 / lens) / {px}"


def encode(frames_dir, out, fps):
    os.makedirs(os.path.dirname(out), exist_ok=True)
    cmd = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-framerate", str(fps), "-i",
           os.path.join(frames_dir, "%04d.png"), "-c:v", "libx264", "-preset", "slow", "-crf", "12",
           "-pix_fmt", "yuv420p", "-movflags", "+faststart", "-an", out]
    subprocess.run(cmd, check=True)


def load_spec(path):
    """Read a shot spec; `"extends": "other.json"` inherits that spec's
    top-level fields (background, lights, ...) and overrides the rest."""
    with open(path) as f:
        spec = json.load(f)
    base = spec.pop("extends", None)
    if base:
        merged = load_spec(os.path.join(os.path.dirname(path), base))
        merged.pop("device_motion", None)
        merged.update(spec)
        spec = merged
    return spec


def main():
    args = parse()
    shot = args.shot if os.path.isabs(args.shot) else os.path.abspath(args.shot)
    spec = load_spec(shot)
    out = os.path.abspath(os.path.expanduser(args.out))
    t0 = time.time()
    nframes = build(spec, args)
    sc = bpy.context.scene
    if args.blend:
        bpy.ops.wm.save_as_mainfile(filepath=os.path.abspath(os.path.expanduser(args.blend)))
    if args.still is not None:
        sc.frame_set(1 + int(round(args.still * sc.render.fps)))
        png = os.path.splitext(out)[0] + f".t{args.still:g}.png"
        sc.render.filepath = png
        bpy.ops.render.render(write_still=True)
        print(f"studio: wrote {png} in {time.time() - t0:.1f}s")
        return
    frames_dir = os.path.splitext(out)[0] + ".frames"
    shutil.rmtree(frames_dir, ignore_errors=True)
    os.makedirs(frames_dir)
    if args.frames:
        a, b = (int(x) for x in args.frames.split(":"))
        sc.frame_start, sc.frame_end = a, b
    sc.render.filepath = os.path.join(frames_dir, "")
    bpy.ops.render.render(animation=True)
    if args.frames:  # renumber so ffmpeg starts at 0001
        for i, fr in enumerate(range(sc.frame_start, sc.frame_end + 1)):
            os.rename(os.path.join(frames_dir, f"{fr:04d}.png"), os.path.join(frames_dir, f"n{i + 1:04d}.png"))
        for i in range(sc.frame_end - sc.frame_start + 1):
            os.rename(os.path.join(frames_dir, f"n{i + 1:04d}.png"), os.path.join(frames_dir, f"{i + 1:04d}.png"))
    t1 = time.time()
    encode(frames_dir, out, sc.render.fps)
    if not args.keep_frames:
        shutil.rmtree(frames_dir, ignore_errors=True)
    n = sc.frame_end - sc.frame_start + 1
    print(f"studio: wrote {out}: {n} frames, render {t1 - t0:.0f}s ({(t1 - t0) / n:.2f}s/frame), "
          f"total {time.time() - t0:.0f}s")


if __name__ == "__main__":
    main()
