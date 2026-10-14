"""Draconia's uniform, cut by script over her posed body.

An original school uniform of the common British type, with no crest of
any real school: a white collared shirt, a striped tie in deep green and
silver, a charcoal V-neck sweater with green and silver trim at the V, and
an open black wool robe faced and hood-lined in deep green.

Each garment is a shell over the body (or the garment under it): the body's
own faces, offset along their normals, smoothed, and cut where the garment
opens. The collar, the tie, the sweater's trim and the hood are built from
curves the script places around the neck and the V. Materials carry
`verse_shading = cloth` (sheen) except the shirt's starched collar, which
is matte cloth too.
"""

import math

import bmesh
import bpy
from mathutils import Vector
from mathutils.bvhtree import BVHTree

GREEN = (0.012, 0.055, 0.04, 1)
SILVER = (0.42, 0.44, 0.44, 1)
CHARCOAL = (0.022, 0.022, 0.025, 1)
BLACK = (0.012, 0.012, 0.014, 1)
WHITE = (0.70, 0.70, 0.67, 1)
# Fold depth (m) of the robe's drape below the shoulders (0: a smooth
# shell, the CC0 builds' default).
FOLDS = 0.0
# How far the shirt collar's points reach (1: long spread points, round 8;
# the CC0 builds keep their shorter ones at 0.6).
COLLAR_POINTS = 0.6
# Thickness (m) of the robe's shell and of the shirt collar's band and
# points.
ROBE_THICK = 0.003
COLLAR_THICK = 0.0015
# The shirt collar: "fall" (a band folding over all round, the CC0
# builds') or "points" (a band with two separate pointed flaps either side
# of the knot, round 13); its band's height (m) and the gap at the front
# for the knot (radians each side).
COLLAR_STYLE = "fall"
# How far under the neck's base the robe's front opening stops (m): 0.02
# leaves a band of robe across the chest (the CC0 builds'); a negative
# value opens it to the top, so the robe hangs from the shoulders and the
# collar and tie show (round 13).
ROBE_OPEN_TOP = 0.02
COLLAR_BAND = 0.026
COLLAR_GAP = 0.24


def _folds(obj, top_z, depth):
    """Vertical folds in a hanging garment: each vertex below `top_z` moves
    along its normal by a few overlapping waves around the body (so the
    folds run down, wavering), growing from nothing at the shoulders to
    `depth` 15 cm lower."""
    me = obj.data
    normals = [v.normal.copy() for v in me.vertices]
    for v, n in zip(me.vertices, normals):
        p = v.co.copy()
        fade = min(1.0, max(0.0, (top_z - p.z) / 0.15))
        if fade <= 0:
            continue
        a = math.atan2(p.x, p.y)
        w = math.sin(a * 23 + p.z * 9) * 0.6 + math.sin(a * 41 - p.z * 15 + 1.3) * 0.3 + math.sin(a * 9 + p.z * 4 + 0.4) * 0.4
        v.co = p + n * depth * fade * w
    me.update()


def _weave(name, kind, size=256):
    """A tiling fabric normal map: `twill` (the robe's wool: diagonal
    ribs), `knit` (the sweater: rows of V stitches), `plain` (the shirt:
    a fine square weave)."""
    import numpy as np

    y, x = np.mgrid[0:size, 0:size].astype(np.float32) / size * 2 * math.pi
    if kind == "twill":
        h = np.sin((x + y) * 24) * 0.6 + np.sin(x * 48) * 0.15
    elif kind == "knit":
        h = np.abs(np.sin(x * 16 + np.abs(np.sin(y * 32)) * 1.2)) * 0.8 + np.sin(y * 32) * 0.2
    else:
        h = np.sin(x * 64) * np.sin(y * 64) * 0.5
    gx = (np.roll(h, -1, 1) - np.roll(h, 1, 1)) * 0.5
    gy = (np.roll(h, -1, 0) - np.roll(h, 1, 0)) * 0.5
    # Gentle: at 2.5x the plain weave tilted the shirt and collar's normals
    # some 45 degrees and turned them grey (round 11's first build).
    s = 0.25 if kind == "plain" else 0.6
    n = np.stack([-gx * s, -gy * s, np.ones_like(h)], -1)
    n /= np.linalg.norm(n, axis=-1, keepdims=True)
    rgba = np.concatenate([n * 0.5 + 0.5, np.ones((size, size, 1), np.float32)], -1)
    # Non-Color before the pixels go in: set after, Blender had stored the
    # flat normal's 0.5 sRGB-encoded (about 0.73 once read back as data),
    # which tilted every normal some 25 degrees and turned white cloth grey.
    img = bpy.data.images.new(name, size, size, alpha=True)
    img.colorspace_settings.name = "Non-Color"
    img.pixels.foreach_set(rgba[::-1].astype(np.float32).ravel())
    img.pack()
    return img


# Fabric normal maps on the uniform (round 11): twill on the robe, a knit on
# the sweater, a fine weave on the shirt. Off by default for the CC0 builds.
FABRIC = False


def _mat(name, color, rough=0.85, sheen=0.5, image=None, weave=None):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = color
    bsdf.inputs["Roughness"].default_value = rough
    if FABRIC and weave:
        tex = m.node_tree.nodes.new("ShaderNodeTexImage")
        tex.image = _weave(f"{name}_weave", weave)
        nm = m.node_tree.nodes.new("ShaderNodeNormalMap")
        m.node_tree.links.new(tex.outputs["Color"], nm.inputs["Color"])
        m.node_tree.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    if image is not None:
        tex = m.node_tree.nodes.new("ShaderNodeTexImage")
        tex.image = image
        m.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    m["verse_shading"] = "cloth"
    m["verse_sheen"] = sheen
    return m


def _shell(body, name, offset, keep, smooth_iters=6, subdiv=1):
    """The body's faces that `keep(face centre)` accepts, pushed out by
    `offset` along the smoothed normals."""
    bm = bmesh.new()
    bm.from_mesh(body.data)
    bm.normal_update()
    drop = [f for f in bm.faces if not keep(f.calc_center_median())]
    bmesh.ops.delete(bm, geom=drop, context="FACES")
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    for _ in range(2):
        bmesh.ops.smooth_vert(bm, verts=bm.verts, factor=0.5, use_axis_x=True, use_axis_y=True, use_axis_z=True)
    bm.normal_update()
    for v in bm.verts:
        v.co += v.normal * offset
    for _ in range(smooth_iters):
        bmesh.ops.smooth_vert(bm, verts=[v for v in bm.verts if not v.is_boundary], factor=0.5, use_axis_x=True, use_axis_y=True, use_axis_z=True)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    obj = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(obj)
    # The shell keeps the body's skin weights vertex for vertex (the deform
    # layer came across with the faces); name the groups to match.
    for g in body.vertex_groups:
        obj.vertex_groups.new(name=g.name)
    if subdiv:
        mod = obj.modifiers.new("s", "SUBSURF")
        mod.levels = subdiv
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.modifier_apply(modifier=mod.name)
    for p in obj.data.polygons:
        p.use_smooth = True
    _box_uv(obj)
    return obj


def _cut(obj, planes, drop=None, front_y=None):
    """Splits the shell cleanly along planes (point, normal), only where it
    lies in front of `front_y` when given, then deletes the faces `drop`
    accepts, so openings have straight edges instead of the mesh's steps."""
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    for co, no in planes:
        geom = [g for g in list(bm.verts) + list(bm.edges) + list(bm.faces) if front_y is None or _centre(g).y < front_y]
        bmesh.ops.bisect_plane(bm, geom=geom, dist=1e-6, plane_co=co, plane_no=no)
    if drop:
        bmesh.ops.delete(bm, geom=[f for f in bm.faces if drop(f.calc_center_median())], context="FACES")
        bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    bm.to_mesh(obj.data)
    bm.free()
    _box_uv(obj)


def _centre(g):
    if isinstance(g, bmesh.types.BMVert):
        return g.co
    if isinstance(g, bmesh.types.BMEdge):
        return (g.verts[0].co + g.verts[1].co) / 2
    return g.calc_center_median()


def _box_uv(obj, scale=6.0):
    """World-scaled UVs (about 1/6 m per unit) for woven detail maps."""
    me = obj.data
    if not me.uv_layers:
        me.uv_layers.new(name="UVMap")
    uv = me.uv_layers.active
    for poly in me.polygons:
        n = poly.normal
        for li in poly.loop_indices:
            co = me.vertices[me.loops[li].vertex_index].co
            if abs(n.z) > max(abs(n.x), abs(n.y)):
                uv.data[li].uv = (co.x * scale, co.y * scale)
            elif abs(n.x) > abs(n.y):
                uv.data[li].uv = (co.y * scale, co.z * scale)
            else:
                uv.data[li].uv = (co.x * scale, co.z * scale)


def _tube(name, pts, radius_fn, segs=10, flatten=1.0, up=Vector((0, 0, 1))):
    bm = bmesh.new()
    uvl = bm.loops.layers.uv.new("UVMap")
    rings = []
    n = len(pts)
    for i, p in enumerate(pts):
        t = (pts[min(i + 1, n - 1)] - pts[max(i - 1, 0)]).normalized()
        a = t.cross(up)
        if a.length < 1e-6:
            a = t.orthogonal()
        a.normalize()
        b = a.cross(t).normalized()
        r = radius_fn(i / (n - 1))
        ring = []
        for j in range(segs):
            ang = 2 * math.pi * j / segs
            ring.append(bm.verts.new(p + a * math.cos(ang) * r + b * math.sin(ang) * r * flatten))
        rings.append(ring)
    for i in range(n - 1):
        for j in range(segs):
            f = bm.faces.new([rings[i][j], rings[i][(j + 1) % segs], rings[i + 1][(j + 1) % segs], rings[i + 1][j]])
            for loop, uv in zip(f.loops, [(j / segs, i / n), ((j + 1) / segs, i / n), ((j + 1) / segs, (i + 1) / n), (j / segs, (i + 1) / n)]):
                loop[uvl].uv = uv
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    obj = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(obj)
    for p in obj.data.polygons:
        p.use_smooth = True
    return obj


def _strip(name, left, right, uvs=None):
    """A quad strip between two point lists."""
    bm = bmesh.new()
    uvl = bm.loops.layers.uv.new("UVMap")
    L = [bm.verts.new(p) for p in left]
    R = [bm.verts.new(p) for p in right]
    n = len(L)
    for i in range(n - 1):
        f = bm.faces.new([L[i], R[i], R[i + 1], L[i + 1]])
        for loop, uv in zip(f.loops, [(0, i / (n - 1)), (1, i / (n - 1)), (1, (i + 1) / (n - 1)), (0, (i + 1) / (n - 1))]):
            loop[uvl].uv = uv
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    obj = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(obj)
    for p in obj.data.polygons:
        p.use_smooth = True
    return obj


def _solidify(obj, thickness):
    mod = obj.modifiers.new("solid", "SOLIDIFY")
    mod.thickness = thickness
    mod.offset = 1.0
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier=mod.name)


def _stripe_image(name, stripes, w=64, h=512):
    """A tie or trim stripe texture: (start, end, rgba) bands across v."""
    import numpy as np

    img = np.zeros((h, w, 4), np.float32)
    img[...] = stripes[0][2]
    vv = (np.arange(h) + 0.5) / h
    for a, b, c in stripes:
        sel = (vv % 1.0 >= a) & (vv % 1.0 < b)
        img[sel] = c
    lin = img[..., :3]
    srgb = np.where(lin <= 0.0031308, lin * 12.92, 1.055 * np.power(lin, 1 / 2.4) - 0.055)
    img[..., :3] = srgb
    im = bpy.data.images.new(name, w, h, alpha=True)
    im.pixels.foreach_set(np.ascontiguousarray(img[::-1]).ravel())
    import os

    path = os.path.expanduser(f"~/work/film-out/draconia/build/textures/{name}.png")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    im.filepath_raw = path
    im.file_format = "PNG"
    im.save()
    return im


def build(body, marks):
    bvh = BVHTree.FromObject(body, bpy.context.evaluated_depsgraph_get())
    neck = marks.neck
    chest = marks.chest
    neck_base_z = neck.z - 0.02
    hip_z = chest.z - 0.33
    hand_z = hip_z - 0.02

    def torso(c, below=0.0):
        # Torso and arms down to the wrists' cuffs, not the head or hands.
        if c.z > neck_base_z + 0.005:
            return False
        if c.z < hip_z - below:
            return False
        if abs(c.x) > 0.16 and c.z < hand_z + 0.12 + 0.0:
            return False  # hands
        return True

    out = []
    # Shirt: only shows at the collar and in the V; the body is close.
    # Shirt: only the front panel the sweater's V shows.
    shirt = _shell(body, "shirt", 0.0025, lambda c: torso(c) and c.z > chest.z - 0.1 and abs(c.x) < 0.1 and c.y < neck.y, smooth_iters=4)
    shirt.data.materials.append(_mat("draconia_shirt", WHITE, rough=0.9, sheen=0.2, weave="plain"))
    out.append(shirt)

    # Sweater: charcoal knit, V-neck from the collar to mid-chest.
    v_bottom = chest.z - 0.04
    v_top = neck_base_z - 0.01
    v_half = 0.065

    def v_open(c):
        if c.y > neck.y - 0.01:
            return False  # front only
        h = (c.z - v_bottom) / (v_top - v_bottom)
        return h > 0 and abs(c.x) < v_half * min(h, 1.0) + 0.004

    sweater = _shell(body, "sweater", 0.007, lambda c: torso(c, 0.05), smooth_iters=6)
    v_planes = []
    for side in (1, -1):
        # The V's edge on this side: from the bottom point up to the collar.
        dx, dz = side * v_half, v_top - v_bottom
        n = Vector((dz, 0, -dx)).normalized() * side
        v_planes.append((Vector((side * 0.004, 0, v_bottom)), n))
    _cut(sweater, v_planes, drop=v_open, front_y=neck.y - 0.01)
    sweater.data.materials.append(_mat("draconia_sweater", CHARCOAL, rough=0.95, sheen=0.6, weave="knit"))
    out.append(sweater)

    # The V's trim: a band along each edge of the V, green with a silver
    # stripe (as stripes across the band's width).
    def surf(p, off):
        loc, nrm, _, _ = bvh.find_nearest(p)
        return loc + nrm * off

    for side in (1, -1):
        inner, outer = [], []
        for k in range(24):
            t = k / 23
            z = v_bottom + (v_top - v_bottom) * t
            x = side * (v_half * t + 0.004)
            front_y = neck.y - 0.2
            p = Vector((x, front_y, z))
            hit = bvh.ray_cast(p, Vector((0, 1, 0)))
            base = hit[0] if hit[0] is not None else surf(p, 0)
            nrm = hit[1] if hit[1] is not None else Vector((0, -1, 0))
            edge = base + nrm * 0.0085
            inner.append(edge)
            outer.append(edge + Vector((side * 0.016, 0, 0.004)) + nrm * 0.0)
        trim = _strip(f"trim_{'l' if side > 0 else 'r'}", inner, outer)
        uvl = trim.data.uv_layers.active
        for d in uvl.data:
            d.uv = (d.uv.y, d.uv.x)  # stripes run along the band
        _solidify(trim, 0.002)
        out.append(trim)
        trim.data.materials.append(_trim_mat())

    # Robe: black wool over everything, open down the front; a green facing
    # 4 cm wide along the opening.
    def cut_x(z):
        return 0.075 + 0.02 * (chest.z - z)

    def robe_keep(c):
        if c.z > neck_base_z + 0.012:
            return False
        if c.z < hip_z - 0.25:
            return False
        if abs(c.x) > 0.16 and c.z < hand_z + 0.05:
            return False
        if c.y < neck.y + 0.02 and abs(c.x) < cut_x(c.z) and c.z < neck_base_z - ROBE_OPEN_TOP:
            return False
        return True

    def robe_body(c):
        # The shell runs a little past the top edge; the straight cut
        # below trims it.
        if neck_base_z + 0.012 < c.z <= neck_base_z + 0.03 and not (abs(c.x) > 0.16 and c.z < hand_z + 0.05):
            return True
        return robe_keep(c) or (c.y < neck.y + 0.02 and c.z < neck_base_z - 0.02 and c.z > hip_z - 0.25 and not (abs(c.x) > 0.16 and c.z < hand_z + 0.05))

    robe = _shell(body, "robe", 0.02, robe_body, smooth_iters=10)
    planes = []
    for side in (1, -1):
        for w in (0.0, 0.03):
            # x = side * (0.075 + w + 0.02 (chest.z - z)): a plane in x, z.
            n = Vector((side, 0, 0.02)).normalized()
            planes.append((Vector((side * (0.075 + w + 0.02 * chest.z), 0, 0)), n))
    # A straight top edge round the neck (round 13: it stepped along the
    # body's faces and read jagged in the medium shots).
    _cut(robe, [(Vector((0, 0, neck_base_z + 0.012)), Vector((0, 0, 1)))])
    _cut(robe, planes, drop=lambda c: not robe_keep(c), front_y=neck.y + 0.02)
    # Split the facing as its own material by distance to the opening edge.
    me = robe.data
    me.materials.append(_mat("draconia_robe", BLACK, rough=0.9, sheen=0.55, weave="twill"))
    me.materials.append(_mat("draconia_robe_facing", GREEN, rough=0.75, sheen=0.7, weave="twill"))
    bm = bmesh.new()
    bm.from_mesh(me)
    # The facing: a 4 cm band beside the opening, rising around the neck.
    for f in bm.faces:
        c = f.calc_center_median()
        if c.y < neck.y + 0.03 and abs(c.x) < cut_x(c.z) + 0.03:
            f.material_index = 1
    bm.to_mesh(me)
    bm.free()
    if FOLDS:
        _folds(robe, chest.z + 0.08, FOLDS)
    # (Round 11: a thicker robe on request, so its opening edge reads as
    # wool with a lapel, not a card.)
    _solidify(robe, ROBE_THICK)
    out.append(robe)

    # Hood: a soft roll lying around the back of the neck and over the
    # shoulders, its green lining toward the neck.
    pts = []
    for k in range(29):
        a = math.pi * (k / 28)  # 0 her left front, pi her right front
        r = 0.095 + 0.02 * math.sin(a)
        x = math.cos(a) * r
        y = neck.y + 0.01 + math.sin(a) * 0.075
        z = neck_base_z - 0.035 + 0.01 * math.sin(a) - 0.03 * (1 - math.sin(a))
        pts.append(Vector((x, y, z)))
    hood = _tube("hood", pts, lambda t: 0.022 + 0.01 * math.sin(math.pi * t), segs=14, flatten=0.6)
    hood.data.materials.append(_mat("draconia_hood", BLACK, rough=0.9, sheen=0.55))
    out.append(hood)

    # Collar: a stand around the neck base and two points folding down.
    stand_in, stand_out = [], []
    ring = []
    for k in range(33):
        a = -math.pi / 2 + 2 * math.pi * (k / 32)  # start at the front
        if abs(math.sin((a + math.pi / 2) / 2)) < math.sin(COLLAR_GAP / 2):
            continue
        d = Vector((math.cos(a), math.sin(a), 0))
        # Cast from inside the neck outward, so the collar hugs the neck,
        # never the shoulders.
        origin = Vector((neck.x, neck.y + 0.008, neck.z + 0.004 - 0.012 * max(0.0, -d.y)))
        hit = bvh.ray_cast(origin, d, 0.1)
        p = hit[0] if hit[0] is not None else origin + d * 0.05
        ring.append((p + d * 0.0035, d))
    # Sorted round from the front, so the gap for the knot falls between
    # the list's ends (sorted by plain angle, the strip bridged the gap and
    # closed the band over the knot).
    ring.sort(key=lambda q: (math.atan2(q[1].y, q[1].x) + math.pi / 2) % (2 * math.pi))
    lo = [p for p, d in ring]
    hi = [p + Vector((0, 0, COLLAR_BAND)) + d * 0.001 for p, d in ring]
    stand = _strip("collar_stand", lo, hi)
    _solidify(stand, COLLAR_THICK)
    stand.data.materials.append(_mat("draconia_collar", WHITE, rough=0.85, sheen=0.15, weave="plain"))
    out.append(stand)
    # Fall: from the top of the stand folding out and down to the points.
    fall_top = [top + d * 0.002 for (p0, d), top in zip(ring, hi)]
    fall_bot = []
    for (p, d), top in zip(ring, hi):
        front = max(0.0, -d.y)  # 1 at the front
        # Round 8: real collar points, longer and spread out over the
        # sweater at the front (the reference's), not a tube.
        # (Round 9: the points concentrated at the very front, so they read
        # as two pointed flaps either side of the knot, not a bib.)
        drop = 0.03 + COLLAR_POINTS * 0.06 * front**7
        fall_bot.append(top + d * (0.008 + COLLAR_POINTS * 0.03 * front**7) - Vector((0, 0, drop)))
    if COLLAR_STYLE == "points":
        out.extend(_collar_points(bvh, ring, hi, neck, neck_base_z))
    else:
        fall = _strip("collar_fall", fall_top, fall_bot)
        _solidify(fall, COLLAR_THICK)
        fall.data.materials.append(bpy.data.materials["draconia_collar"])
        out.append(fall)

    # Tie: a knot at the collar's front and a blade down the chest, deep
    # green with silver diagonal stripes.
    knot_c = Vector((0, neck.y - 0.0, neck.z - 0.008))
    hit = bvh.ray_cast(knot_c + Vector((0, -0.2, 0)), Vector((0, 1, 0)))
    front = hit[0] if hit[0] is not None else knot_c + Vector((0, -0.05, 0))
    # (Round 13: with collar points the knot sits further out, over the
    # shirt and between the points, and fuller.)
    ko = 0.006 if COLLAR_STYLE == "points" else 0.0
    kr = 0.004 if COLLAR_STYLE == "points" else 0.0
    knot = _tube("tie_knot", [front + Vector((0, -0.006 - ko, 0.012)), front + Vector((0, -0.009 - ko, -0.006))], lambda t: 0.012 + kr - 0.004 * t, segs=12, flatten=0.55)
    stripes = _stripe_image("draconia_tie", [(0.0, 0.62, (0.012, 0.06, 0.042, 1)), (0.62, 0.78, (0.40, 0.42, 0.42, 1)), (0.78, 0.84, (0.012, 0.06, 0.042, 1)), (0.84, 0.9, (0.40, 0.42, 0.42, 1)), (0.9, 1.0, (0.012, 0.06, 0.042, 1))])
    tie_mat = _mat("draconia_tie", GREEN, rough=0.6, sheen=0.4, image=stripes)
    knot.data.materials.append(tie_mat)
    _diag_uv(knot, 8.0)
    out.append(knot)
    left, right = [], []
    for k in range(16):
        t = k / 15
        z = neck.z - 0.022 - t * 0.2
        hw = 0.012 + 0.018 * min(t * 1.6, 1.0)
        p = Vector((0, neck.y - 0.2, z))
        hit = bvh.ray_cast(p, Vector((0, 1, 0)))
        c = (hit[0] if hit[0] is not None else Vector((0, neck.y - 0.08, z))) + Vector((0, -0.0045 - 0.004 * t, 0))
        left.append(c + Vector((hw, 0, 0)))
        right.append(c + Vector((-hw, 0, 0)))
    blade = _strip("tie_blade", left, right)
    _solidify(blade, 0.0025)
    _diag_uv(blade, 8.0)
    blade.data.materials.append(tie_mat)
    out.append(blade)
    return out


def _collar_points(bvh, ring, hi, neck, neck_base_z):
    """A shirt collar's two points (round 13), each a separate flap lying
    over the sweater: from the band's top, between an inner root beside the
    knot and an outer root at the side of the neck, down and out to a crisp
    tip on the chest, following the body 9.5 mm out (over the 7 mm
    sweater), so the knot shows between them."""
    out = []
    lift = 0.0095
    band_bottom = min(p.z for p, _ in ring)

    def on_body(p):
        loc, nrm, _, _ = bvh.find_nearest(p)
        return loc + nrm * lift if loc is not None else p

    for s in (1.0, -1.0):
        # The band's top at an angle from the front (0) round the neck.
        def top_at(a):
            best = min(zip(ring, hi), key=lambda q: abs(math.atan2(s * q[0][1].x, -q[0][1].y) - a))
            return best[1]

        inner_root = top_at(0.45)
        outer_root = top_at(1.35)
        tip = on_body(Vector((s * 0.062, neck.y - 0.08, band_bottom - 0.065)))
        inner, outer = [], []
        n = 8
        for k in range(n):
            t = k / (n - 1)
            # The inner edge runs down beside the knot to the tip; the outer
            # edge sweeps out over the shoulder side then in to the tip.
            pi = inner_root.lerp(tip, t) + Vector((0, 0, -0.004 * math.sin(math.pi * t)))
            po = outer_root.lerp(tip, t) + Vector((s * 0.012 * math.sin(math.pi * t), 0, 0))
            inner.append(on_body(pi) if t > 0.15 else pi)
            outer.append(on_body(po) if t > 0.15 else po)
        flap = _strip(f"collar_point_{'l' if s > 0 else 'r'}", inner, outer)
        _solidify(flap, COLLAR_THICK)
        flap.data.materials.append(bpy.data.materials["draconia_collar"])
        out.append(flap)
    return out


def _trim_mat():
    if "draconia_trim" in bpy.data.materials:
        return bpy.data.materials["draconia_trim"]
    img = _stripe_image("draconia_trim_bands", [(0.0, 0.35, (0.012, 0.06, 0.042, 1)), (0.35, 0.5, (0.40, 0.42, 0.42, 1)), (0.5, 0.65, (0.012, 0.06, 0.042, 1)), (0.65, 0.8, (0.40, 0.42, 0.42, 1)), (0.8, 1.0, (0.012, 0.06, 0.042, 1))])
    m = _mat("draconia_trim", GREEN, rough=0.9, sheen=0.6, image=img)
    return m


def _diag_uv(obj, scale):
    """Diagonal stripes: v runs along x + z, so the stripe texture's bands
    cross the tie at 45 degrees."""
    me = obj.data
    if not me.uv_layers:
        me.uv_layers.new(name="UVMap")
    uv = me.uv_layers.active
    for li, loop in enumerate(me.loops):
        co = me.vertices[loop.vertex_index].co
        uv.data[li].uv = (0.5, (co.x + co.z) * scale)
