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
SWEATER_TOP = -0.005
# Where "front" ends for the sweater's V (m behind the `neck` mark's
# default); the MetaHuman `neck` mark sits at the neck's front, so its V
# reaches back over the upper chest only when this is raised (round 15).
V_FRONT = 0.0
# The robe's front opening: half-width at the chest (m), how much wider it
# grows per metre above the chest (round 15: open at the collarbones, as
# the reference's), and the green facing's width.
ROBE_OPEN_X = 0.075
ROBE_FLARE = 0.0
FACING_W = 0.03
# The tie's stripes: repeats per metre along the diagonal.
TIE_STRIPES = 8.0
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
    # (Where the sweater stops under the neck, the shirt goes all round.)
    shirt = _shell(body, "shirt", 0.0025, lambda c: torso(c) and ((c.z > chest.z - 0.1 and abs(c.x) < 0.1 and c.y < neck.y + V_FRONT) or (SWEATER_TOP > 0 and c.z > neck_base_z - SWEATER_TOP - 0.01)), smooth_iters=4)
    shirt.data.materials.append(_mat("draconia_shirt", WHITE, rough=0.9, sheen=0.2, weave="plain"))
    out.append(shirt)

    # Sweater: charcoal knit, V-neck from the collar to mid-chest.
    v_bottom = chest.z - 0.04
    v_top = neck_base_z - 0.01
    v_half = 0.065

    def v_open(c):
        if c.y > neck.y - 0.01 + V_FRONT:
            return False  # front only
        h = (c.z - v_bottom) / (v_top - v_bottom)
        return h > 0 and abs(c.x) < v_half * min(h, 1.0) + 0.004

    # (SWEATER_TOP: how far under the neck's base the sweater stops; at 0
    # it rode up the neck as a dark choker above the collar, round 14.)
    sweater = _shell(body, "sweater", 0.007, lambda c: torso(c, 0.05) and c.z < neck_base_z - SWEATER_TOP, smooth_iters=6)
    v_planes = []
    for side in (1, -1):
        # The V's edge on this side: from the bottom point up to the collar.
        dx, dz = side * v_half, v_top - v_bottom
        n = Vector((dz, 0, -dx)).normalized() * side
        v_planes.append((Vector((side * 0.004, 0, v_bottom)), n))
    _cut(sweater, v_planes, drop=v_open, front_y=neck.y - 0.01 + V_FRONT)
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
        return ROBE_OPEN_X + 0.02 * (chest.z - z) + ROBE_FLARE * max(0.0, z - chest.z)

    # (Round 17: with ROBE_NECK_R the robe's top follows the neckline, all
    # faces further than that from the neck's axis, instead of stopping at
    # one height, which on MetaHuman's body cut it below the shoulders'
    # line and left a shelf.)
    if ROBE_NECK_R:
        nring = [v.co for v in body.data.vertices if abs(v.co.z - (neck_base_z + 0.02)) < 0.004 and abs(v.co.x) < 0.075]
        nys = [p.y for p in nring] or [neck.y + 0.06]
        n_y = (min(nys) + max(nys)) / 2

    def above_top(c):
        if ROBE_NECK_R:
            if c.z > neck_base_z + 0.1 or (c.z > neck_base_z and c.y < n_y - 0.02):
                return True
            return c.z > neck_base_z - 0.05 and math.hypot(c.x - neck.x, c.y - n_y) < ROBE_NECK_R
        return c.z > neck_base_z + 0.012

    def robe_keep(c):
        if above_top(c):
            return False
        if c.z < hip_z - 0.25:
            return False
        if abs(c.x) > 0.16 and c.z < hand_z + 0.05:
            return False
        if c.y < neck.y + 0.02 + V_FRONT and abs(c.x) < cut_x(c.z) and c.z < neck_base_z - ROBE_OPEN_TOP:
            return False
        return True

    def robe_body(c):
        # The shell runs a little past the top edge; the straight cut
        # below trims it.
        if ROBE_NECK_R:
            if above_top(c) or c.z < hip_z - 0.25 or (abs(c.x) > 0.16 and c.z < hand_z + 0.05):
                return False
            return True
        if neck_base_z + 0.012 < c.z <= neck_base_z + 0.03 and not (abs(c.x) > 0.16 and c.z < hand_z + 0.05):
            return True
        return robe_keep(c) or (c.y < neck.y + 0.02 + V_FRONT and c.z < neck_base_z - 0.02 and c.z > hip_z - 0.25 and not (abs(c.x) > 0.16 and c.z < hand_z + 0.05))

    robe = _shell(body, "robe", 0.02, robe_body, smooth_iters=10)
    planes = []
    for side in (1, -1):
        for w in (0.0, FACING_W):
            # x = side * (ROBE_OPEN_X + w + 0.02 (chest.z - z)): a plane in x, z.
            n = Vector((side, 0, 0.02)).normalized()
            planes.append((Vector((side * (ROBE_OPEN_X + w + 0.02 * chest.z), 0, 0)), n))
            if ROBE_FLARE:
                # Above the chest: side x = X + w + (flare - 0.02) (z - chest.z).
                k = ROBE_FLARE - 0.02
                n = Vector((side, 0, -k)).normalized()
                planes.append((Vector((side * (ROBE_OPEN_X + w - k * chest.z), 0, 0)), n))
    # A straight top edge round the neck (round 13: it stepped along the
    # body's faces and read jagged in the medium shots).
    if not ROBE_NECK_R:
        _cut(robe, [(Vector((0, 0, neck_base_z + 0.012)), Vector((0, 0, 1)))])
    _cut(robe, planes, drop=lambda c: not robe_keep(c), front_y=neck.y + 0.02 + V_FRONT)
    # Split the facing as its own material by distance to the opening edge.
    me = robe.data
    me.materials.append(_mat("draconia_robe", BLACK, rough=0.9, sheen=0.55, weave="twill"))
    me.materials.append(_mat("draconia_robe_facing", GREEN, rough=0.75, sheen=0.7, weave="twill"))
    bm = bmesh.new()
    bm.from_mesh(me)
    # The facing: a 4 cm band beside the opening, rising around the neck.
    for f in bm.faces:
        c = f.calc_center_median()
        if c.y < neck.y + 0.03 + V_FRONT and abs(c.x) < cut_x(c.z) + FACING_W:
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
    if HOOD_DRAPE:
        out.extend(_hood_drape(body, robe, neck, neck_base_z))
    else:
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

    if COLLAR_STYLE == "shell":
        out.extend(_shell_collar(body, neck, neck_base_z, [shirt, sweater]))
        return out

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
    stripes = _stripe_image("draconia_tie", [(0.0, 0.62, (0.012, 0.06, 0.042, 1)), (0.62, 0.78, (0.40, 0.42, 0.42, 1)), (0.78, 0.84, (0.012, 0.06, 0.042, 1)), (0.84, 0.9, (0.40, 0.42, 0.42, 1)), (0.9, 1.0, (0.012, 0.06, 0.042, 1))])
    tie_mat = _mat("draconia_tie", GREEN, rough=0.6, sheen=0.4, image=stripes)
    if COLLAR_STYLE == "points":
        # (Round 14: a modelled four-in-hand knot, not a tube.)
        out.append(_tie_knot(front, tie_mat))
    else:
        knot = _tube("tie_knot", [front + Vector((0, -0.006 - ko, 0.012)), front + Vector((0, -0.009 - ko, -0.006))], lambda t: 0.012 + kr - 0.004 * t, segs=12, flatten=0.55)
        knot.data.materials.append(tie_mat)
        _diag_uv(knot, TIE_STRIPES)
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
    _diag_uv(blade, TIE_STRIPES)
    blade.data.materials.append(tie_mat)
    out.append(blade)
    return out


# The "shell" collar (round 15): the band is an offset shell of the neck
# itself, its top higher at the back than the front; the points fold over
# it and lie on the shirt and sweater (projected, then optionally settled
# by a short cloth simulation pinned at the fold).
COLLAR_FRONT_DROP = 0.018  # how much lower the band's top is at the front (m)
COLLAR_POINT_LEN = 0.075  # fold to tip, down the front (m)
COLLAR_POINT_SPREAD = 0.05  # tip's distance from the centre line (m)
COLLAR_FALL = 0.032  # the fall's depth at the sides and back (m)
COLLAR_TH0 = 0.12  # where the points' front edges start, from the front (rad)
COLLAR_POINT_ARC = 1.3  # how far round the point's lower edge rises to the fall (rad)
COLLAR_SIM = 0  # cloth-sim frames for the points (0: projection only)
KNOT_DROP = 0.0  # how far under the fold's front the knot sits (m)
KNOT_SIZE = 1.0  # the knot's scale


def _shell_collar(body, neck, neck_base_z, under):
    out = []
    collar_mat = _mat("draconia_collar", WHITE, rough=0.85, sheen=0.15, weave="plain")
    top_z = neck_base_z + COLLAR_BAND
    lo_z = neck_base_z - 0.005
    # The neck's own centre and depth at the band (the `neck` mark sits at
    # its front).
    ring = [v.co for v in body.data.vertices if abs(v.co.z - top_z) < 0.004 and abs(v.co.x) < 0.075]
    ys = [p.y for p in ring]
    ny, depth = (min(ys) + max(ys)) / 2, (max(ys) - min(ys)) / 2
    print("COLLAR neck centre y", round(ny, 4), "half depth", round(depth, 4), len(ring))
    neck = Vector((neck.x, ny, neck.z))

    def top_at(c):
        # Lower at the front (y < ny), level round the back.
        f = max(0.0, min(1.0, (ny - c.y) / depth))
        return top_z - COLLAR_FRONT_DROP * f

    band = _shell(body, "collar_band", 0.003, lambda c: lo_z - 0.01 < c.z < top_z + 0.02, smooth_iters=2)
    # A clean top edge: bisect along a few tilted planes, then drop faces
    # above the line.
    planes = [
        (Vector((0, neck.y, top_z)), Vector((0, 0, 1))),
        (Vector((0, neck.y, top_z)), Vector((0, -COLLAR_FRONT_DROP / depth, 1)).normalized()),
        (Vector((0, neck.y, top_z - COLLAR_FRONT_DROP)), Vector((0, 0, 1))),
        (Vector((0, 0, lo_z)), Vector((0, 0, 1))),
    ]
    _cut(band, planes, drop=lambda c: c.z > top_at(c) + 1e-4 or c.z < lo_z)
    _solidify(band, COLLAR_THICK)
    band.data.materials.append(collar_mat)
    out.append(band)

    # The fall: from the band's top edge (the fold) down over the band all
    # round, 3 cm deep at the sides and back, drawn out at the front into
    # the points, whose front edges run from beside the knot diagonally
    # down and out to tips on the chest. Every vertex is laid on the
    # outermost layer under it (band, shirt, sweater), measured along the
    # body's normal.
    deps = bpy.context.evaluated_depsgraph_get()
    rest = BVHTree.FromObject(band, deps)
    skin = BVHTree.FromObject(body, deps)
    layers = [rest] + [BVHTree.FromObject(o, deps) for o in under]

    def lie(p, off):
        loc, n, _, _ = skin.find_nearest(p)
        if loc is None:
            return p
        start = loc + n * 0.04
        best = None
        for t in layers:
            hit = t.ray_cast(start, -n, 0.045)
            if hit[0] is not None and (best is None or hit[3] < best):
                best = hit[3]
        h = (0.04 - best) if best is not None else 0.0
        return loc + n * (max(h, 0.0) + off)

    def fold_at(th, s):
        d = Vector((s * math.sin(th), -math.cos(th), 0))
        z = top_z - COLLAR_FRONT_DROP * max(0.0, math.cos(th)) - 0.001
        p = None
        for _ in range(3):
            o = Vector((neck.x, neck.y, z))
            hit = rest.ray_cast(o, d, 0.2)
            if hit[0] is None:
                return None
            p = hit[0]
            z = top_at(p) - 0.0008
        return lie(p, COLLAR_THICK * 0.5 + 0.0005)

    for s in (1.0, -1.0):
        nu, nv = 21, 12
        ths = [COLLAR_TH0 + (math.pi - COLLAR_TH0) * i / (nu - 1) for i in range(nu)]
        fold = [fold_at(t, s) for t in ths]
        miss = sum(f is None for f in fold)
        if miss:
            print("COLLAR: fold ray missed", miss)
            ok = next(f for f in fold if f is not None)
            fold = [f if f is not None else ok for f in fold]
        tip = fold[0] + Vector((s * COLLAR_POINT_SPREAD, -0.03, -COLLAR_POINT_LEN))
        bm = bmesh.new()
        grid = []
        for j in range(nv):
            v = j / (nv - 1)
            row = []
            for i in range(nu):
                w = max(0.0, 1.0 - (ths[i] - COLLAR_TH0) / (COLLAR_POINT_ARC - COLLAR_TH0))
                w = w * w * (3 - 2 * w)
                low = (fold[i] - Vector((0, 0, COLLAR_FALL))).lerp(tip, w)
                p = fold[i].lerp(low, v)
                if j > 0:
                    p = lie(p, COLLAR_THICK * 0.5 + 0.0012)
                row.append(bm.verts.new(p))
            grid.append(row)
        for j in range(nv - 1):
            for i in range(nu - 1):
                bm.faces.new((grid[j][i], grid[j][i + 1], grid[j + 1][i + 1], grid[j + 1][i]))
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
        me2 = bpy.data.meshes.new(f"collar_fall_{'l' if s > 0 else 'r'}")
        bm.to_mesh(me2)
        bm.free()
        flap = bpy.data.objects.new(me2.name, me2)
        bpy.context.collection.objects.link(flap)
        if COLLAR_SIM:
            _settle(flap, nu, [band] + list(under), COLLAR_SIM)
        mod = flap.modifiers.new("solid", "SOLIDIFY")
        mod.thickness = COLLAR_THICK
        mod.offset = 0.0
        bpy.context.view_layer.objects.active = flap
        bpy.ops.object.modifier_apply(modifier=mod.name)
        mod = flap.modifiers.new("s", "SUBSURF")
        mod.levels = 1
        bpy.ops.object.modifier_apply(modifier=mod.name)
        for p in flap.data.polygons:
            p.use_smooth = True
        _box_uv(flap)
        flap.data.materials.append(collar_mat)
        out.append(flap)
    knot_top = fold_at(0.0, 1.0) or Vector((neck.x, neck.y - 0.06, top_z))

    # Tie: the knot seated against the band at the front, its top at the
    # band's top; the blade from under it down the shirt.
    stripes = _stripe_image("draconia_tie", [(0.0, 0.62, (0.012, 0.06, 0.042, 1)), (0.62, 0.78, (0.40, 0.42, 0.42, 1)), (0.78, 0.84, (0.012, 0.06, 0.042, 1)), (0.84, 0.9, (0.40, 0.42, 0.42, 1)), (0.9, 1.0, (0.012, 0.06, 0.042, 1))])
    tie_mat = _mat("draconia_tie", GREEN, rough=0.6, sheen=0.4, image=stripes)
    kz = knot_top.z - 0.012 - KNOT_DROP
    hit = rest.ray_cast(Vector((neck.x, neck.y - 0.3, kz)), Vector((0, 1, 0)))
    front = hit[0] if hit[0] is not None else Vector((neck.x, neck.y - 0.06, kz))
    # (_tie_knot centres the knot 1.1 cm in front of `front`, 4 mm up; its
    # back then rests on the band.)
    out.append(_tie_knot(front + Vector((0, 0.011 - 0.0065 - COLLAR_THICK, -0.004)), tie_mat))
    bvh = BVHTree.FromObject(under[0], deps)
    # The blade lies on the shirt's most forward point across its width
    # (round 15: along the centre line alone it dipped behind a ridge in
    # the shirt and showed broken), its line smoothed.
    cs = []
    for k in range(41):
        t = k / 40
        z = kz - 0.012 - t * 0.2
        hw = 0.009 + 0.02 * min(t * 1.6, 1.0)
        ys = []
        for dx in (-hw, -hw / 2, 0.0, hw / 2, hw):
            hit = bvh.ray_cast(Vector((neck.x + dx, neck.y - 0.3, z)), Vector((0, 1, 0)))
            if hit[0] is not None:
                ys.append(hit[0].y)
        y = min(ys) if ys else neck.y - 0.08
        cs.append((Vector((neck.x, y - 0.003 - 0.0015 * t, z)), hw))
    raw = [c.y for c, _ in cs]
    for _ in range(3):
        cs = [cs[0]] + [((cs[k - 1][0] + cs[k][0] * 2 + cs[k + 1][0]) / 4, cs[k][1]) for k in range(1, len(cs) - 1)] + [cs[-1]]
    # (Never behind where it lay before smoothing: over the convex chest
    # the average sinks in.)
    for (c, _), y in zip(cs, raw):
        c.y = min(c.y, y)
    left, right = [], []
    for c, hw in cs:
        left.append(c + Vector((hw, 0, 0)))
        right.append(c + Vector((-hw, 0, 0)))
    blade = _strip("tie_blade", left, right)
    _solidify(blade, 0.0025)
    _diag_uv(blade, TIE_STRIPES)
    blade.data.materials.append(tie_mat)
    out.append(blade)
    return out


# The hood (round 17): a soft roll lying on the robe round the back of the
# neck and forward over the shoulders, with a green lining showing along
# its edge toward the neck (0: the CC0 builds' roll).
HOOD_DRAPE = False
HOOD_R = 0.105  # its distance out from the neck's centre (m)
HOOD_A0 = 0.2  # where its ends lie, from straight out at the sides toward the back (rad)
HOOD_COLOR = BLACK
# The robe's top round the neck: faces nearer the neck's axis than this
# are cut away (0: one height, the CC0 builds').
ROBE_NECK_R = 0.0


def _hood_drape(body, robe, neck, neck_base_z):
    deps = bpy.context.evaluated_depsgraph_get()
    ring = [v.co for v in body.data.vertices if abs(v.co.z - (neck_base_z + 0.02)) < 0.004 and abs(v.co.x) < 0.075]
    ys = [p.y for p in ring] or [neck.y + 0.06]
    ny = (min(ys) + max(ys)) / 2
    # On the body's shoulders, out by the robe's 2 cm shell and thickness
    # (the robe's own top is open by the neck, so rays fell through it).
    tree = BVHTree.FromObject(body, deps)
    pts, lin, rads = [], [], []
    n = 33
    for k in range(n):
        t = k / (n - 1)
        a = HOOD_A0 + (math.pi - 2 * HOOD_A0) * t  # her left side, round the back, to her right side
        back = max(0.0, math.sin(a))
        d = Vector((math.cos(a), math.sin(a) * 0.95, 0))
        o = Vector((neck.x, ny, 0)) + d * (HOOD_R + 0.03 * back)
        # On the body's nearest point to a ring at the neck's base (rays
        # down met the head, or started inside the thick back of the neck).
        hit = tree.find_nearest(Vector((o.x, o.y, neck_base_z - 0.01 * back)))
        rad = (0.012 + 0.018 * back) * min(1.0, 0.4 + 3.0 * min(t, 1 - t))
        base = (hit[0] + hit[1] * (0.02 + ROBE_THICK + rad * 0.55)) if hit[0] is not None else Vector((o.x, o.y, neck_base_z))
        p = base
        pts.append(p)
        rads.append(rad)
        lin.append(p - d * rad * 0.85 + Vector((0, 0, rad * 0.3)))
    hood = _tube("hood", pts, lambda t: rads[min(int(t * (n - 1) + 0.5), n - 1)], segs=16, flatten=0.55)
    hood.data.materials.append(_mat("draconia_hood", HOOD_COLOR, rough=0.9, sheen=0.6, weave="twill"))
    lining = _tube("hood_lining", lin, lambda t: 0.0045 + 0.004 * math.sin(math.pi * t), segs=8, flatten=0.6)
    lining.data.materials.append(_mat("draconia_hood_lining", GREEN, rough=0.75, sheen=0.7, weave="twill"))
    for o in (hood, lining):
        _box_uv(o)
    return [hood, lining]


def _settle(flap, nu, colliders, frames):
    """A short cloth simulation: the fold row (the first `nu` vertices)
    pinned, the rest falling onto `colliders` under gravity."""
    g = flap.vertex_groups.new(name="pin")
    g.add(list(range(nu)), 1.0, "REPLACE")
    for o in colliders:
        if not any(m.type == "COLLISION" for m in o.modifiers):
            c = o.modifiers.new("col", "COLLISION")
            o.collision.thickness_outer = 0.001
    cl = flap.modifiers.new("cloth", "CLOTH")
    cl.settings.vertex_group_mass = "pin"
    cl.settings.quality = 10
    cl.settings.mass = 0.05
    cl.settings.tension_stiffness = 40
    cl.settings.compression_stiffness = 40
    cl.settings.bending_stiffness = 2.0
    cl.collision_settings.distance_min = 0.001
    cl.collision_settings.use_self_collision = False
    sc = bpy.context.scene
    cl.point_cache.frame_start = 1
    cl.point_cache.frame_end = frames
    for f in range(1, frames + 1):
        sc.frame_set(f)
    deps = bpy.context.evaluated_depsgraph_get()
    ev = flap.evaluated_get(deps)
    co = [v.co.copy() for v in ev.data.vertices]
    flap.modifiers.remove(cl)
    for v, c in zip(flap.data.vertices, co):
        v.co = c
    flap.vertex_groups.remove(g)
    sc.frame_set(1)
    for o in colliders:
        for m in [m for m in o.modifiers if m.type == "COLLISION"]:
            o.modifiers.remove(m)


def _collar_points(bvh, ring, hi, neck, neck_base_z):
    """A shirt collar's two points (round 14), modelled: each a curved
    triangular flap with thickness, its top edge running along the band's
    top from beside the knot to the side of the neck, its lower edge from
    just under the knot out to a crisp tip on the chest, the surface
    between lying on the sweater (9.5 mm off the body, over its 7 mm
    shell), then thickened and subdivided once so its edges are soft."""
    out = []
    lift = 0.0095
    band_bottom = min(p.z for p, _ in ring)

    def on_body(p):
        loc, nrm, _, _ = bvh.find_nearest(p)
        return loc + nrm * lift if loc is not None else p

    def front_angle(d, s):
        return math.atan2(s * d.x, -d.y)

    for s in (1.0, -1.0):
        side = [(front_angle(d, s), top) for (p, d), top in zip(ring, hi) if front_angle(d, s) > 0]
        side.sort(key=lambda q: q[0])

        def top_at(a):
            return min(side, key=lambda q: abs(q[0] - a))[1]

        a0 = side[0][0]
        inner_low = on_body(top_at(a0) + Vector((-s * 0.004, -0.004, -0.028)))
        tip = on_body(Vector((s * 0.07, neck.y - 0.09, band_bottom - 0.07)))
        bm = bmesh.new()
        nu, nv = 7, 8
        grid = []
        for j in range(nv):
            v = j / (nv - 1)
            row = []
            for i in range(nu):
                u = i / (nu - 1)
                # (Round 14: the top edge spans only the front of the band,
                # so the point lies flat on the chest facing forward rather
                # than wrapping round the neck's side edge-on.)
                top = top_at(a0 + u * 0.55)
                bottom = inner_low.lerp(tip, u)
                p = top.lerp(bottom, v)
                # Onto the sweater below the band, blending in so the flap
                # turns over the band's top edge without tearing.
                w = min(1.0, max(0.0, (v - 0.05) / 0.3))
                p = p.lerp(on_body(p), w)
                row.append(bm.verts.new(p))
            grid.append(row)
        for j in range(nv - 1):
            for i in range(nu - 1):
                bm.faces.new((grid[j][i], grid[j][i + 1], grid[j + 1][i + 1], grid[j + 1][i]))
        me = bpy.data.meshes.new(f"collar_point_{'l' if s > 0 else 'r'}")
        bm.to_mesh(me)
        bm.free()
        flap = bpy.data.objects.new(me.name, me)
        bpy.context.collection.objects.link(flap)
        _solidify(flap, COLLAR_THICK)
        mod = flap.modifiers.new("s", "SUBSURF")
        mod.levels = 1
        bpy.context.view_layer.objects.active = flap
        bpy.ops.object.modifier_apply(modifier=mod.name)
        for p in flap.data.polygons:
            p.use_smooth = True
        _box_uv(flap)
        flap.data.materials.append(bpy.data.materials["draconia_collar"])
        out.append(flap)
    return out


def _tie_knot(front, mat):
    """A four-in-hand knot (round 14): a trapezoid, wider at the top
    (3 cm) than the bottom (1.7 cm), 2.4 cm tall and 1.3 cm deep, its front
    bowed and creased by one subdivision, in the tie's stripes."""
    bm = bmesh.new()
    top_w, bot_w, h, dep = (x * KNOT_SIZE for x in (0.03, 0.017, 0.024, 0.013))
    c = front + Vector((0, -0.011, 0.004))
    pts = []
    for zz, w in ((h / 2, top_w), (-h / 2, bot_w)):
        for yy in (-dep / 2, dep / 2):
            for xx in (-w / 2, w / 2):
                pts.append(bm.verts.new(c + Vector((xx, yy, zz))))
    # 0..3 top (front l, front r, back l, back r), 4..7 bottom.
    for f in ((0, 1, 3, 2), (4, 6, 7, 5), (0, 4, 5, 1), (2, 3, 7, 6), (0, 2, 6, 4), (1, 5, 7, 3)):
        bm.faces.new([pts[i] for i in f])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new("tie_knot")
    bm.to_mesh(me)
    bm.free()
    knot = bpy.data.objects.new("tie_knot", me)
    bpy.context.collection.objects.link(knot)
    # Rounded edges (a subdivided box shrinks to a ball): a bevel, then
    # one gentle cut for the bow below.
    mod = knot.modifiers.new("b", "BEVEL")
    mod.width = 0.003
    mod.segments = 3
    bpy.context.view_layer.objects.active = knot
    bpy.ops.object.modifier_apply(modifier=mod.name)
    bm = bmesh.new()
    bm.from_mesh(knot.data)
    bmesh.ops.subdivide_edges(bm, edges=bm.edges[:], cuts=2, use_grid_fill=True)
    bm.to_mesh(knot.data)
    bm.free()
    # Bow the front out a little and pinch a crease across it.
    for v in knot.data.vertices:
        if v.co.y < c.y:
            dz = (v.co.z - c.z) / (h / 2)
            v.co.y -= 0.003 * (1 - dz * dz) - 0.0015 * math.exp(-((dz + 0.2) / 0.15) ** 2)
    for p in knot.data.polygons:
        p.use_smooth = True
    knot.data.materials.append(mat)
    _diag_uv(knot, TIE_STRIPES)
    return knot


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
