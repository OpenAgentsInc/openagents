"""Procedural device models for the launch film's studio shots (#11432).

`macbook(...)` and `iphone(...)` build a laptop and a phone at real size, in
meters, in Blender's +Z-up frame, and return the device's root empty. Each has
a screen object whose emissive material plays a movie file, cover-fitted to
the display so the footage is never stretched.

The models are built from rounded-rectangle prisms with real bevels, so the
edges catch the studio light. The MacBook keyboard is cut into the deck with
an exact boolean, so the aluminum shows between the keys.

Front of every device faces -Y (MacBook) or +Z (iPhone, lying screen-up); the
studio poses the root.
"""

import math
import os

import bmesh
import bpy
from mathutils import Matrix, Vector

# Linear base colors for the finishes. Anodized aluminum reads best as a
# partly metallic, fairly rough surface: a pure metal with a dark base color
# looks like chrome painted black.
FINISHES = {
    "space-black": dict(rgb=(0.018, 0.019, 0.021), metal=0.2, rough=0.5),
    "silver": dict(rgb=(0.80, 0.81, 0.82), metal=1.0, rough=0.30),
    "deep-blue": dict(rgb=(0.045, 0.065, 0.12), metal=0.7, rough=0.34),
    "cosmic-orange": dict(rgb=(0.80, 0.28, 0.075), metal=0.85, rough=0.26),
}

# ---------------------------------------------------------------- geometry


def rrect(w, h, radii, segs=12, inset=0.0):
    """Points of a w×h rounded rectangle centered on the origin, CCW.

    `radii` is one radius or (bottom-right, top-right, top-left, bottom-left).
    `inset` shrinks the outline and its radii together.
    """
    if not isinstance(radii, (tuple, list)):
        radii = (radii,) * 4
    w, h = w - 2 * inset, h - 2 * inset
    pts = []
    corners = [(w / 2, -h / 2, -90), (w / 2, h / 2, 0), (-w / 2, h / 2, 90), (-w / 2, -h / 2, 180)]
    for (cx, cy, a0), r in zip(corners, radii):
        r = max(0.0, r - inset)
        if r <= 1e-6:
            pts.append((cx, cy))
            continue
        ox = cx - math.copysign(r, cx)
        oy = cy - math.copysign(r, cy)
        for i in range(segs + 1):
            a = math.radians(a0 + 90 * i / segs)
            pts.append((ox + r * math.cos(a), oy + r * math.sin(a)))
    return pts


def circle(r, segs=48):
    return [(r * math.cos(2 * math.pi * i / segs), r * math.sin(2 * math.pi * i / segs)) for i in range(segs)]


def _smooth_by_angle(bm, angle=35):
    lim = math.radians(angle)
    for f in bm.faces:
        f.smooth = True
    for e in bm.edges:
        if not e.is_manifold:
            e.smooth = False
        else:
            e.smooth = e.calc_face_angle(math.pi) < lim


def prism(name, pts, z0, z1, mat=None, bevel=0.0, segs=4, rims="both", offset=(0, 0), uv=None):
    """Extrude outline `pts` from z0 to z1 and bevel its top/bottom rims.

    `uv` = (x0, y0, w, h) projects UVs from XY onto that rectangle.
    """
    bm = bmesh.new()
    ox, oy = offset
    bot = [bm.verts.new((x + ox, y + oy, z0)) for x, y in pts]
    top = [bm.verts.new((x + ox, y + oy, z1)) for x, y in pts]
    fb = bm.faces.new(list(reversed(bot)))
    ft = bm.faces.new(top)
    n = len(pts)
    for i in range(n):
        j = (i + 1) % n
        bm.faces.new((bot[i], bot[j], top[j], top[i]))
    if bevel > 0:
        edges = []
        if rims in ("both", "top"):
            edges += list(ft.edges)
        if rims in ("both", "bottom"):
            edges += list(fb.edges)
        bmesh.ops.bevel(bm, geom=edges, offset=bevel, segments=segs, profile=0.5,
                        affect="EDGES", clamp_overlap=False)
    _smooth_by_angle(bm)
    me = bpy.data.meshes.new(name)
    if uv:
        x0, y0, w, h = uv
        layer = bm.loops.layers.uv.new("UVMap")
        for f in bm.faces:
            for l in f.loops:
                l[layer].uv = ((l.vert.co.x - x0) / w, (l.vert.co.y - y0) / h)
    bm.to_mesh(me)
    bm.free()
    obj = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(obj)
    if mat:
        me.materials.append(mat)
    return obj


def flat(name, pts, z, mat=None, uv=None, offset=(0, 0)):
    """A single flat face at height z (screens, decals)."""
    bm = bmesh.new()
    ox, oy = offset
    f = bm.faces.new([bm.verts.new((x + ox, y + oy, z)) for x, y in pts])
    for e in f.edges:
        e.smooth = False
    f.smooth = False
    me = bpy.data.meshes.new(name)
    if uv:
        x0, y0, w, h = uv
        layer = bm.loops.layers.uv.new("UVMap")
        for l in f.loops:
            l[layer].uv = ((l.vert.co.x - x0) / w, (l.vert.co.y - y0) / h)
    bm.to_mesh(me)
    bm.free()
    obj = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(obj)
    if mat:
        me.materials.append(mat)
    return obj


def join(name, objs):
    """Join objects into one mesh, keeping each part's material."""
    bm = bmesh.new()
    me = bpy.data.meshes.new(name)
    mats = []
    for o in objs:
        part = bmesh.new()
        part.from_mesh(o.data)
        part.transform(o.matrix_world)
        local = {}
        for i, m in enumerate(o.data.materials):
            if m not in mats:
                mats.append(m)
            local[i] = mats.index(m)
        for f in part.faces:
            f.material_index = local.get(f.material_index, 0)
        tmp = bpy.data.meshes.new("tmp")
        part.to_mesh(tmp)
        bm.from_mesh(tmp)
        bpy.data.meshes.remove(tmp)
        part.free()
        bpy.data.objects.remove(o)
    bm.to_mesh(me)
    bm.free()
    for m in mats:
        me.materials.append(m)
    obj = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(obj)
    return obj


def apply_modifiers(obj):
    dg = bpy.context.evaluated_depsgraph_get()
    me = bpy.data.meshes.new_from_object(obj.evaluated_get(dg))
    old = obj.data
    obj.modifiers.clear()
    obj.data = me
    bpy.data.meshes.remove(old)
    bm = bmesh.new()
    bm.from_mesh(me)
    _smooth_by_angle(bm)
    bm.to_mesh(me)
    bm.free()


def transform(obj, m):
    obj.data.transform(m)
    obj.data.update()


def parent(objs, root):
    for o in objs:
        o.parent = root


def empty(name, loc=(0, 0, 0)):
    e = bpy.data.objects.new(name, None)
    e.empty_display_size = 0.05
    e.location = loc
    bpy.context.scene.collection.objects.link(e)
    return e


# ---------------------------------------------------------------- materials


def _bsdf(name):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    return m, m.node_tree.nodes, m.node_tree.links, m.node_tree.nodes["Principled BSDF"]


def metal_mat(name, finish, rough_add=0.0):
    f = FINISHES[finish]
    m, nodes, links, b = _bsdf(name)
    b.inputs["Base Color"].default_value = (*f["rgb"], 1)
    b.inputs["Metallic"].default_value = f["metal"]
    # Bead-blasted aluminum: a faint, fine roughness variation, no visible grain.
    tc = nodes.new("ShaderNodeTexCoord")
    noise = nodes.new("ShaderNodeTexNoise")
    noise.inputs["Scale"].default_value = 900.0
    noise.inputs["Detail"].default_value = 4.0
    links.new(tc.outputs["Object"], noise.inputs["Vector"])
    ramp = nodes.new("ShaderNodeMapRange")
    ramp.inputs["To Min"].default_value = f["rough"] + rough_add - 0.03
    ramp.inputs["To Max"].default_value = f["rough"] + rough_add + 0.03
    links.new(noise.outputs["Fac"], ramp.inputs["Value"])
    links.new(ramp.outputs["Result"], b.inputs["Roughness"])
    return m


def plain(name, rgb, rough, metal=0.0, coat=0.0, ior=1.5, spec=0.5):
    m, nodes, links, b = _bsdf(name)
    b.inputs["Base Color"].default_value = (*rgb, 1)
    b.inputs["Roughness"].default_value = rough
    b.inputs["Metallic"].default_value = metal
    b.inputs["IOR"].default_value = ior
    b.inputs["Specular IOR Level"].default_value = spec
    if coat:
        b.inputs["Coat Weight"].default_value = coat
        b.inputs["Coat Roughness"].default_value = 0.02
    return m


def grille_mat(name, finish, pitch=0.0011, hole=0.00034):
    """Aluminum perforated with a hex grid of speaker holes."""
    m = metal_mat(name, finish)
    nodes, links = m.node_tree.nodes, m.node_tree.links
    b = nodes["Principled BSDF"]
    tc = nodes.new("ShaderNodeTexCoord")
    vor = nodes.new("ShaderNodeTexVoronoi")  # not random: randomness 0 gives a grid
    vor.inputs["Scale"].default_value = 1.0 / pitch
    vor.inputs["Randomness"].default_value = 0.0
    vor.feature = "F1"
    links.new(tc.outputs["Object"], vor.inputs["Vector"])
    lt = nodes.new("ShaderNodeMath")
    lt.operation = "LESS_THAN"
    lt.inputs[1].default_value = hole / pitch
    links.new(vor.outputs["Distance"], lt.inputs[0])
    mix = nodes.new("ShaderNodeMix")
    mix.data_type = "RGBA"
    mix.inputs[7].default_value = (0.0, 0.0, 0.0, 1)
    mix.inputs[6].default_value = b.inputs["Base Color"].default_value
    links.new(lt.outputs["Value"], mix.inputs["Factor"])
    links.new(mix.outputs[2], b.inputs["Base Color"])
    inv = nodes.new("ShaderNodeMath")
    inv.operation = "SUBTRACT"
    inv.inputs[0].default_value = b.inputs["Metallic"].default_value
    links.new(lt.outputs["Value"], inv.inputs[1])
    links.new(inv.outputs["Value"], b.inputs["Metallic"])
    return m


def screen_mat(name, video, screen_aspect, in_frame=0, brightness=1.0, frames=None, fit="cover"):
    """An emissive display under glass, playing `video` cover-fitted.

    The emission is the footage's sRGB color times `brightness`, so at 1.0 a
    white pixel emits exactly scene white; the studio's view transform keeps
    that near display white. A clear glossy layer on top takes reflections.
    """
    m, nodes, links, b = _bsdf(name)
    b.inputs["Base Color"].default_value = (0.0, 0.0, 0.0, 1)
    # Low-reflection display glass: a faint, slightly soft reflection.
    b.inputs["Roughness"].default_value = 0.12
    b.inputs["Specular IOR Level"].default_value = 0.12
    b.inputs["Emission Strength"].default_value = brightness
    if not video:
        b.inputs["Emission Color"].default_value = (0.02, 0.02, 0.025, 1)
        return m
    img = bpy.data.images.load(os.path.abspath(os.path.expanduser(video)), check_existing=True)
    still = os.path.splitext(video)[1].lower() in (".png", ".jpg", ".jpeg")
    if not still:
        img.source = "MOVIE"
    img.colorspace_settings.name = "sRGB"
    tex = nodes.new("ShaderNodeTexImage")
    tex.image = img
    tex.interpolation = "Linear"  # Cubic is a B-spline in Cycles: it blurs text
    tex.extension = "CLIP"
    if not still:
        iu = tex.image_user
        iu.frame_duration = frames or img.frame_duration
        # A negative in-point starts the clip that many frames into the
        # shot, holding its first frame until then (a boot clip's dark screen).
        iu.frame_start = 1 + max(0, -int(in_frame))
        iu.frame_offset = max(0, int(in_frame))
        iu.use_auto_refresh = True
        iu.use_cyclic = False
    # Never stretch. "cover" fills the display, cropping the sides (wide
    # footage) or the bottom (tall footage); "contain" shows all of it,
    # leaving black bars below or beside it.
    iw, ih = img.size
    img_aspect = iw / ih if ih else screen_aspect
    pick = min if fit == "cover" else max
    sx = pick(1.0, screen_aspect / img_aspect)
    sy = pick(1.0, img_aspect / screen_aspect)
    uvn = nodes.new("ShaderNodeUVMap")
    mp = nodes.new("ShaderNodeMapping")
    mp.inputs["Scale"].default_value = (sx, sy, 1)
    mp.inputs["Location"].default_value = ((1 - sx) / 2, 1 - sy, 0)  # centered, top-anchored
    links.new(uvn.outputs["UV"], mp.inputs["Vector"])
    links.new(mp.outputs["Vector"], tex.inputs["Vector"])
    links.new(tex.outputs["Color"], b.inputs["Emission Color"])
    m["video"] = video
    return m


# ---------------------------------------------------------------- MacBook

MACBOOKS = {
    # outer width, depth, base height, lid thickness, display px, plan radius
    "14": dict(W=0.3126, D=0.2212, Hb=0.0098, T=0.0052, px=(3024, 1964), r=0.0105),
    "16": dict(W=0.3560, D=0.2481, Hb=0.0110, T=0.0058, px=(3456, 2234), r=0.0115),
}
PPM = 254 / 0.0254  # MacBook Pro Liquid Retina XDR: 254 pixels per inch


def _keyboard_layout(u):
    """(x, y, w, h) key rectangles in key units → meters, centered on the board."""
    rows = [
        (0.72, [1.5] + [1.0] * 12 + [1.0]),
        (1.0, [1.0] * 13 + [1.5]),
        (1.0, [1.5] + [1.0] * 12 + [1.0]),
        (1.0, [1.75] + [1.0] * 11 + [1.75]),
        (1.0, [2.25] + [1.0] * 10 + [2.25]),
        (1.0, [1.0, 1.0, 1.0, 1.25, 5.0, 1.25, 1.0, "arrows"]),
    ]
    width = 14.5
    depth = sum(h for h, _ in rows)
    keys = []
    y = depth / 2
    for h, ws in rows:
        x = -width / 2
        for w in ws:
            if w == "arrows":
                keys.append((x + 0.5, y - h * 0.75, 1, h / 2))
                keys.append((x + 1.5, y - h * 0.25, 1, h / 2))
                keys.append((x + 1.5, y - h * 0.75, 1, h / 2))
                keys.append((x + 2.5, y - h * 0.75, 1, h / 2))
                x += 3
                continue
            keys.append((x + w / 2, y - h / 2, w, h))
            x += w
        y -= h
    return [(kx * u, ky * u, kw * u, kh * u) for kx, ky, kw, kh in keys], width * u, depth * u


def macbook(video=None, finish="space-black", size="14", open_deg=112, in_frame=0, brightness=1.0, fit="cover"):
    p = MACBOOKS[size]
    W, D, Hb, T, r = p["W"], p["D"], p["Hb"], p["T"], p["r"]
    sw, sh = p["px"][0] / PPM, p["px"][1] / PPM
    feet = 0.0010
    top = feet + Hb

    alu = metal_mat("mb_aluminum", finish)
    well = plain("mb_well", (0.004, 0.004, 0.005), 0.6)
    keycap = plain("mb_keycap", (0.005, 0.005, 0.006), 0.68, spec=0.15)
    pad = metal_mat("mb_trackpad", finish, rough_add=-0.08)
    grille = grille_mat("mb_grille", finish)
    glass = plain("mb_bezel", (0.002, 0.002, 0.0025), 0.03, spec=0.6)
    hinge_m = plain("mb_hinge", (0.008, 0.008, 0.009), 0.4, metal=0.3)
    rubber = plain("mb_feet", (0.01, 0.01, 0.01), 0.8)
    lens = plain("mb_cam", (0.005, 0.006, 0.012), 0.05)

    root = empty("macbook")
    parts = []

    # Base: a unibody slab with the keyboard and trackpad cut in.
    base = prism("mb_base", rrect(W, D, r), feet, top, alu, bevel=0.0024, segs=6)
    u = 0.0190 * (W / 0.3126) if size == "16" else 0.0190
    keys, kw, kd = _keyboard_layout(u)
    kcy = D / 2 - 0.0185 - kd / 2
    gap = 0.0013
    cutters = []
    for kx, ky, w, h in keys:
        c = prism("cut", rrect(w - gap + 0.0007, h - gap + 0.0007, 0.0013), top - 0.0016, top + 0.003,
                  well, offset=(kx, ky + kcy))
        cutters.append(c)
    tp_w, tp_d = (0.1270, 0.0780) if size == "14" else (0.1500, 0.0900)
    tp_y = -D / 2 + 0.0100 + tp_d / 2
    cutters.append(prism("cut", rrect(tp_w + 0.0006, tp_d + 0.0006, 0.0062), top - 0.0012, top + 0.003,
                         well, offset=(0, tp_y)))
    cutter = join("mb_cutter", cutters)
    mod = base.modifiers.new("keys", "BOOLEAN")
    mod.operation = "DIFFERENCE"
    mod.solver = "EXACT"
    mod.object = cutter
    mod.material_mode = "TRANSFER"
    cutter.hide_render = cutter.hide_viewport = True
    apply_modifiers(base)
    bpy.data.objects.remove(cutter)
    parts.append(base)

    caps = [prism("key", rrect(w - gap, h - gap, 0.0014), top - 0.0012, top + 0.0004, keycap,
                  bevel=0.00035, segs=3, rims="top", offset=(kx, ky + kcy)) for kx, ky, w, h in keys]
    parts.append(join("mb_keys", caps))
    parts.append(prism("mb_trackpad", rrect(tp_w, tp_d, 0.006), top - 0.0012, top - 0.00005, pad,
                       bevel=0.0003, segs=3, rims="top", offset=(0, tp_y)))
    gx = kw / 2 + (W / 2 - kw / 2) / 2
    gw = (W / 2 - kw / 2) * 0.42
    for s in (-1, 1):
        parts.append(flat("mb_grille", rrect(gw, kd, gw / 2), top + 0.00004, grille, offset=(s * gx, kcy)))
    for fx in (-1, 1):
        for fy in (-1, 1):
            parts.append(prism("mb_foot", circle(0.0055, 32), 0.0, feet + 0.0003, rubber, bevel=0.0003,
                               segs=2, rims="bottom", offset=(fx * (W / 2 - 0.022), fy * (D / 2 - 0.022))))

    # Hinge barrel along the back edge.
    hy, hz = D / 2 - 0.0062, top + 0.0012
    hl = W - 0.052
    hinge = prism("mb_hinge", circle(0.0042, 40), -hl / 2, hl / 2, hinge_m, bevel=0.0008, segs=3)
    transform(hinge, Matrix.Rotation(math.radians(90), 4, "Y"))
    hinge.location = (0, hy, hz)
    parts.append(hinge)
    parent(parts, root)

    # Lid, built in a frame where +z is the screen's facing and y runs up the
    # lid from the hinge, then rotated into place (Rx(+90): z → -y).
    lid = empty("mb_lid", (0, hy, hz))
    lid.parent = root
    Hl = D - 0.0060
    y0 = 0.0016
    lid_r = (0.0045, r, r, 0.0045)
    lp = []
    lp.append(prism("mb_shell", rrect(W, Hl, lid_r), -T / 2, T / 2 - 0.0007, alu, bevel=0.0017, segs=5,
                    offset=(0, y0 + Hl / 2)))
    lp.append(prism("mb_bezel", rrect(W, Hl, lid_r, inset=0.0006), T / 2 - 0.0012, T / 2, glass,
                    bevel=0.00035, segs=3, rims="top", offset=(0, y0 + Hl / 2)))
    s_top = y0 + Hl - 0.0042
    s_cy = s_top - sh / 2
    smat = screen_mat("mb_screen", video, sw / sh, in_frame, brightness, fit=fit)
    screen = flat("mb_screen", rrect(sw, sh, (0.0012, 0.0098, 0.0098, 0.0012)), T / 2 + 0.00003, smat,
                  uv=(-sw / 2, s_cy - sh / 2, sw, sh), offset=(0, s_cy))
    lp.append(screen)
    notch_h = 32 * 2 / PPM  # the 32 pt menu-bar band the notch sits in
    lp.append(flat("mb_notch", rrect(0.0318, notch_h + 0.001, (0.0028, 0, 0, 0.0028)), T / 2 + 0.00006, glass,
                   offset=(0, s_top - notch_h / 2 + 0.0005)))
    lp.append(flat("mb_cam", circle(0.0011, 24), T / 2 + 0.00009, lens, offset=(0, s_top - notch_h / 2)))
    rx = Matrix.Rotation(math.radians(90), 4, "X")
    for o in lp:
        transform(o, rx)
        o.parent = lid
    lid.rotation_euler = (-math.radians(open_deg - 90), 0, 0)
    root["screen"] = screen.name
    root["screen_size"] = (sw, sh)
    root["front_glass"] = [o.name for o in lp if o.name.startswith(("mb_bezel", "mb_notch", "mb_cam", "mb_screen"))]
    return root


# ---------------------------------------------------------------- iPhone


def iphone(video=None, finish="cosmic-orange", in_frame=0, brightness=1.0, fit="cover"):
    """An iPhone 17 Pro-like phone lying screen-up (+Z), centered on its root."""
    px_w, px_h = 1206, 2622  # iPhone 17 Pro: 402×874 pt at 3x
    ppm = 460 / 0.0254
    sw, sh = px_w / ppm, px_h / ppm  # 65.1 × 141.1 mm
    W, H = sw + 0.0064, sh + 0.0064
    Tz = 0.00875
    R = 0.0118
    z0, z1 = -Tz / 2, Tz / 2

    alu = metal_mat("ip_aluminum", finish)
    back = metal_mat("ip_back", finish, rough_add=0.14)
    back.node_tree.nodes["Principled BSDF"].inputs["Metallic"].default_value = 0.35
    glass = plain("ip_glass", (0.002, 0.002, 0.0025), 0.025, spec=0.6)
    lensglass = plain("ip_lens", (0.004, 0.005, 0.012), 0.02, coat=1.0)
    ring = plain("ip_ring", (0.55, 0.56, 0.58), 0.18, metal=1.0)
    flashm = plain("ip_flash", (0.75, 0.72, 0.62), 0.25)
    sapphire = plain("ip_sapphire", (0.02, 0.02, 0.022), 0.08, metal=0.3)

    root = empty("iphone")
    parts = []
    # Unibody frame with generously rounded sides.
    parts.append(prism("ip_body", rrect(W, H, R), z0, z1 - 0.0008, alu, bevel=0.0027, segs=8))
    # Front cover glass, slightly proud, with a soft edge.
    parts.append(prism("ip_front", rrect(W, H, R, inset=0.0011), z1 - 0.0016, z1, glass, bevel=0.0005,
                       segs=4, rims="top"))
    smat = screen_mat("ip_screen", video, sw / sh, in_frame, brightness, fit=fit)
    screen = flat("ip_screen", rrect(sw, sh, 0.0092), z1 + 0.00003, smat, uv=(-sw / 2, -sh / 2, sw, sh))
    parts.append(screen)
    pt = sw / 402
    di_w, di_h = 126 * pt, 37 * pt
    parts.append(flat("ip_island", rrect(di_w, di_h, di_h / 2), z1 + 0.00006, glass,
                      offset=(0, sh / 2 - 11 * pt - di_h / 2)))

    # Back: matte panel, and the full-width camera plateau across the top.
    parts.append(prism("ip_backpanel", rrect(W, H, R, inset=0.0016), z0 - 0.0002, z0 + 0.0004, back,
                       bevel=0.0003, segs=3, rims="bottom"))
    pl_h = 0.0330
    pl_y = H / 2 - 0.0032 - pl_h / 2
    pz = z0 - 0.0018
    parts.append(prism("ip_plateau", rrect(W - 0.0064, pl_h, 0.0082), pz, z0, alu, bevel=0.0009, segs=5,
                       rims="bottom", offset=(0, pl_y)))
    # Seen from the back, +x on the body is on the viewer's left: the lens
    # triangle sits at body +x.
    top_y = pl_y + pl_h / 2
    for lx, ly in ((0.0218, top_y - 0.0098), (0.0218, top_y - 0.0232), (0.0090, top_y - 0.0165)):
        parts.append(prism("ip_lensring", circle(0.0066, 64), pz - 0.0013, pz + 0.0002, ring, bevel=0.0004,
                           segs=3, rims="bottom", offset=(lx, ly)))
        parts.append(prism("ip_lensglass", circle(0.0054, 64), pz - 0.00145, pz - 0.0012, lensglass,
                           offset=(lx, ly)))
    parts.append(flat("ip_flash", circle(0.0026, 32)[::-1], pz - 0.00002, flashm, offset=(-0.0215, top_y - 0.0098)))
    parts.append(flat("ip_lidar", circle(0.0024, 32)[::-1], pz - 0.00002, glass, offset=(-0.0215, top_y - 0.0232)))

    # Side buttons: left side action + volume, right side power + camera control.
    def button(x_side, y, length, mat, depth=0.0006):
        bx = x_side * (W / 2 + depth / 2 - 0.0004)
        w = depth + 0.0008
        b = prism("ip_button", rrect(w, length, min(0.0006, w * 0.45)), -0.0012, 0.0012, mat, bevel=min(0.0004, w * 0.3),
                  segs=3, offset=(bx, y))
        return b

    parts += [button(-1, 0.037, 0.0068, alu), button(-1, 0.0205, 0.0105, alu), button(-1, 0.0075, 0.0105, alu),
              button(1, 0.024, 0.017, alu), button(1, -0.024, 0.0118, sapphire, depth=0.0002)]
    parent(parts, root)
    root["screen"] = screen.name
    root["screen_size"] = (sw, sh)
    root["front_glass"] = [o.name for o in parts if o.name.startswith(("ip_front", "ip_island", "ip_screen"))]
    return root


# ---------------------------------------------------------------- Studio Display


def display(video=None, finish="silver", in_frame=0, brightness=1.0, fit="cover"):
    """A Studio Display-style 27-inch 5K monitor on its tilting stand, the
    screen facing -Y, standing on the floor at its root."""
    ppm = 218 / 0.0254  # 5K at 218 pixels per inch
    sw, sh = 5120 / ppm, 2880 / ppm  # 596.5 × 335.5 mm
    W, H, T = sw + 0.0264, sh + 0.0264, 0.0300
    r = 0.0105
    cz = 0.288  # panel center height
    alu = metal_mat("sd_aluminum", finish)
    glass = plain("sd_glass", (0.002, 0.002, 0.0025), 0.03, spec=0.6)
    vent = grille_mat("sd_vent", finish, pitch=0.0016, hole=0.0005)
    stand = metal_mat("sd_stand", finish, rough_add=0.18)  # satin, so it never flares

    root = empty("display")
    # The panel is built facing +z with y up, then stood up (Rx(+90): z → -y).
    pp = []
    pp.append(prism("sd_shell", rrect(W, H, r), -T / 2, T / 2 - 0.0008, alu, bevel=0.0022, segs=5))
    pp.append(prism("sd_bezel", rrect(W, H, r, inset=0.0007), T / 2 - 0.0013, T / 2, glass, bevel=0.0004,
                    segs=3, rims="top"))
    smat = screen_mat("sd_screen", video, sw / sh, in_frame, brightness, fit=fit)
    screen = flat("sd_screen", rrect(sw, sh, 0.002), T / 2 + 0.00003, smat, uv=(-sw / 2, -sh / 2, sw, sh))
    pp.append(screen)
    # the perforated band across the top of the back
    pp.append(flat("sd_vent", rrect(W - 0.06, 0.05, 0.004)[::-1], -T / 2 - 0.00004, vent,
                   offset=(0, H / 2 - 0.045)))
    rx = Matrix.Rotation(math.radians(90), 4, "X")
    for o in pp:
        transform(o, rx)
        o.location = (0, 0, cz)
        o.parent = root
    # Stand: a foot on the floor and one bent blade rising to the panel's back.
    sw_ = 0.175
    foot = prism("sd_foot", rrect(sw_, 0.19, 0.012), 0.0, 0.006, stand, bevel=0.0015, segs=3,
                 offset=(0, 0.035))
    a, b = Vector((0.12, 0.006)), Vector((T / 2 + 0.004, cz - 0.02))
    d = b - a
    blade = prism("sd_blade", rrect(sw_, d.length, 0.004), -0.004, 0.004, stand, bevel=0.0012, segs=3,
                  offset=(0, d.length / 2))
    transform(blade, Matrix.Rotation(math.atan2(d.y, d.x), 4, "X"))
    blade.location = (0, a.x, a.y)
    hinge = prism("sd_hinge", circle(0.009, 32), -sw_ / 2, sw_ / 2, alu, bevel=0.001, segs=2)
    transform(hinge, Matrix.Rotation(math.radians(90), 4, "Y"))
    hinge.location = (0, b.x, b.y)
    for o in (foot, blade, hinge):
        o.parent = root
    root["screen"] = screen.name
    root["screen_size"] = (sw, sh)
    root["front_glass"] = [o.name for o in pp if o.name.startswith(("sd_bezel", "sd_screen"))]
    return root


def build(kind, **kw):
    return {"macbook": macbook, "iphone": iphone, "display": display}[kind](**kw)
