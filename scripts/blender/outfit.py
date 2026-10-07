"""Original garments a private character can wear: a tailored black jacket
and black pumps (`private_character.py --overlay jacket,heels`).

The garments are our own geometry, built by this script around the body it
is given, so they may be committed; the body they fit, and anything built
from it, stays private (docs/verse/private-assets.md).

The body arrives seated, as `private_character.seat` leaves it: feet at the
origin, front toward -Y, Z up, arms hanging beside the body. It is one fused
mesh whose head material also holds the hair.

The jacket:

- Measures the body in horizontal slices. Each slice's torso outline is the
  convex hull of its skin and dress points, without the hair and without the
  arms, so the cloth bridges hollows the way fabric does. The arms are found
  as the two columns of points beside the torso and fit with a straight axis
  each.
- Lofts a torso shell and two sleeves over those outlines, eased outward,
  with the cloth falling from the bust rather than following under it, and
  joins them into one smooth surface with a voxel remesh.
- Relaxes the surface and pushes it back out of the body, so nothing pokes
  through; cuts the hem, the cuffs, the neck, and the open front; and gives
  the edges a turned thickness.
- Adds notched lapels that roll back from the opening, a collar around the
  back of the neck, and a button at the waist; and marks seams, topstitching,
  pocket flaps, and cuff buttons in a vertex color the material reads.

The pumps fit the feet as the seated pose leaves them, pointed with the
heels lifted: a smooth shell over the toes, the sides, and the heel, cut low
over the instep, a pointed toe, a sole, and a tapered heel to the floor.

Then the body is fitted to the garments: hair the jacket would pass through
is lifted onto it, dress and skin that would show through are drawn in, and
skin and dress the garments hide are removed, so the levels spend their
triangles and texture on what shows.
"""

import math

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Vector
from mathutils.bvhtree import BVHTree

LUMA = np.array([0.2126, 0.7152, 0.0722])


# ---------------------------------------------------------------- arrays


def positions(obj):
    me = obj.data
    co = np.zeros(len(me.vertices) * 3)
    me.vertices.foreach_get("co", co)
    return co.reshape(-1, 3)


def set_positions(obj, co):
    obj.data.vertices.foreach_set("co", np.asarray(co, dtype=np.float64).ravel())
    obj.data.update()


def edges(obj):
    me = obj.data
    e = np.zeros(len(me.edges) * 2, dtype=np.int64)
    me.edges.foreach_get("vertices", e)
    return e.reshape(-1, 2)


def vertex_colors(obj):
    """Each vertex's base color, sampled from its faces' base-color images,
    and the fraction of its faces in each material."""
    me = obj.data
    nl = len(me.loops)
    lv = np.zeros(nl, dtype=np.int64)
    me.loops.foreach_get("vertex_index", lv)
    uv = np.zeros(nl * 2)
    me.uv_layers.active.data.foreach_get("uv", uv)
    uv = uv.reshape(-1, 2)
    mi = np.zeros(len(me.polygons), dtype=np.int64)
    me.polygons.foreach_get("material_index", mi)
    lt = np.zeros(len(me.polygons), dtype=np.int64)
    me.polygons.foreach_get("loop_total", lt)
    loop_mat = np.repeat(mi, lt)
    col = np.full((nl, 3), 0.5)
    for k, m in enumerate(me.materials):
        img = base_image(m)
        if img is None:
            continue
        w, h = img.size
        px = np.array(img.pixels[:], dtype=np.float32).reshape(h, w, -1)
        s = loop_mat == k
        xs = np.clip((uv[s, 0] % 1.0) * w, 0, w - 1).astype(int)
        ys = np.clip((uv[s, 1] % 1.0) * h, 0, h - 1).astype(int)
        col[s] = px[ys, xs, :3]
    nv = len(me.vertices)
    vc = np.zeros((nv, 3))
    cnt = np.zeros(nv)
    np.add.at(vc, lv, col)
    np.add.at(cnt, lv, 1)
    vc /= np.maximum(cnt, 1)[:, None]
    share = np.zeros((nv, max(1, len(me.materials))))
    np.add.at(share, (lv, loop_mat), 1.0)
    share /= np.maximum(cnt, 1)[:, None]
    return vc, share


def base_image(material):
    if not material or not material.use_nodes:
        return None
    for n in material.node_tree.nodes:
        if n.type == "TEX_IMAGE" and n.image and any(
                l.to_socket.name == "Base Color" for l in n.outputs[0].links):
            return n.image
    return None


def hair_mask(obj, co, e):
    """The hair: dark vertices of the material that holds the top of the
    head, below which no hair falls past the waist. Smoothed over the mesh's
    edges, so highlights in the hair count as hair."""
    vc, share = vertex_colors(obj)
    top = int(np.argmax(co[:, 2]))
    head = int(np.argmax(share[top]))
    h = co[:, 2].max()
    dark = (share[:, head] > 0.5) & (vc @ LUMA < 0.3) & (co[:, 2] > 0.6 * h)
    f = dark.astype(float)
    for _ in range(3):
        acc = np.zeros_like(f)
        cnt = np.zeros_like(f)
        np.add.at(acc, e[:, 0], f[e[:, 1]])
        np.add.at(acc, e[:, 1], f[e[:, 0]])
        np.add.at(cnt, e[:, 0], 1)
        np.add.at(cnt, e[:, 1], 1)
        f = 0.5 * f + 0.5 * acc / np.maximum(cnt, 1)
    return (f > 0.5) & (share[:, head] > 0.5)


# ---------------------------------------------------------------- outlines


def hull(points):
    """The convex hull of 2D `points`, counterclockwise."""
    p = np.unique(np.round(points, 6), axis=0)
    if len(p) < 3:
        return p
    p = p[np.lexsort((p[:, 1], p[:, 0]))]

    def half(pts):
        out = []
        for q in pts:
            while len(out) >= 2:
                a, b = out[-2], out[-1]
                if (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0]) > 0:
                    break
                out.pop()
            out.append(q)
        return out

    lower = half(p)
    upper = half(p[::-1])
    return np.array(lower[:-1] + upper[:-1])


def extremes(points, center, bins=180):
    """The farthest of `points` from `center` in each of `bins` directions:
    the few a hull needs, out of many."""
    d = points - center
    a = np.floor((np.arctan2(d[:, 1], d[:, 0]) + math.pi) / (2 * math.pi) * bins).astype(int) % bins
    r = np.hypot(d[:, 0], d[:, 1])
    order = np.lexsort((-r, a))
    first = np.ones(len(order), bool)
    first[1:] = a[order][1:] != a[order][:-1]
    return points[order[first]]


def radii(poly, center, angles):
    """Where rays from `center` at `angles` leave the convex polygon `poly`."""
    d = np.stack([np.cos(angles), np.sin(angles)], 1)
    a = poly - center
    b = np.roll(poly, -1, 0) - center
    s = b - a
    out = np.zeros(len(angles))
    for i, (dx, dy) in enumerate(d):
        den = dx * s[:, 1] - dy * s[:, 0]
        ok = np.abs(den) > 1e-12
        t = np.where(ok, (a[:, 0] * s[:, 1] - a[:, 1] * s[:, 0]) / np.where(ok, den, 1), -1)
        u = np.where(ok, (a[:, 0] * dy - a[:, 1] * dx) / np.where(ok, den, 1), -1)
        hit = ok & (u >= -1e-9) & (u <= 1 + 1e-9) & (t > 0)
        out[i] = t[hit].max() if hit.any() else 0.0
    return out


def smooth_cyclic(r, width):
    """A moving average of each row of `r` around its columns."""
    k = np.ones(2 * width + 1) / (2 * width + 1)
    pad = np.concatenate([r[:, -width:], r, r[:, :width]], 1)
    return np.array([np.convolve(row, k, mode="valid") for row in pad])


def smooth_rows(r, width):
    """A moving average of `r` down its rows, holding the ends."""
    if width < 1:
        return r
    k = np.ones(2 * width + 1) / (2 * width + 1)
    pad = np.concatenate([np.repeat(r[:1], width, 0), r, np.repeat(r[-1:], width, 0)], 0)
    return np.stack([np.convolve(pad[:, j], k, mode="valid") for j in range(r.shape[1])], 1)


# ---------------------------------------------------------------- meshes


def grid_mesh(name, rings, cap_bottom=True, cap_top=True):
    """A closed tube through `rings` (R x N x 3), capped with a fan at
    each end."""
    rings = np.asarray(rings)
    R, N = rings.shape[:2]
    verts = [tuple(p) for p in rings.reshape(-1, 3)]
    faces = []
    for i in range(R - 1):
        for j in range(N):
            a, b = i * N + j, i * N + (j + 1) % N
            faces.append((a, b, b + N, a + N))
    if cap_bottom:
        verts.append(tuple(rings[0].mean(0)))
        c = len(verts) - 1
        faces += [(c, (j + 1) % N, j) for j in range(N)]
    if cap_top:
        verts.append(tuple(rings[-1].mean(0)))
        c = len(verts) - 1
        base = (R - 1) * N
        faces += [(c, base + j, base + (j + 1) % N) for j in range(N)]
    me = bpy.data.meshes.new(name)
    me.from_pydata(verts, [], faces)
    me.validate()
    obj = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(obj)
    return obj


def apply(obj, mod):
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier=mod.name)


def join(objs, name):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.join()
    o = bpy.context.view_layer.objects.active
    o.name = o.data.name = name
    return o


def bvh(obj, faces=None):
    me = obj.data
    co = [v.co.copy() for v in me.vertices]
    polys = [tuple(p.vertices) for p in me.polygons] if faces is None else faces
    return BVHTree.FromPolygons(co, polys)


def triangles_of(obj):
    me = obj.data
    me.calc_loop_triangles()
    t = np.zeros(len(me.loop_triangles) * 3, dtype=np.int64)
    me.loop_triangles.foreach_get("vertices", t)
    return t.reshape(-1, 3)


class Slicer:
    """Horizontal sections of a triangle mesh: where its triangles cross a
    plane z = c, as points spaced `step` m along the crossing segments."""

    def __init__(self, co, tris, step=0.002):
        self.co, self.step = co, step
        z = co[tris, 2]
        self.lo, self.hi = z.min(1), z.max(1)
        order = np.argsort(self.lo)
        self.tris, self.lo, self.hi = tris[order], self.lo[order], self.hi[order]

    def at(self, c):
        n = np.searchsorted(self.lo, c, side="right")
        t = self.tris[:n][self.hi[:n] > c]
        if len(t) == 0:
            return np.zeros((0, 2))
        p = self.co[t]
        above = p[:, :, 2] > c
        hits = []
        for a, b in ((0, 1), (1, 2), (2, 0)):
            pa, pb = p[:, a], p[:, b]
            dz = pb[:, 2] - pa[:, 2]
            f = (c - pa[:, 2]) / np.where(np.abs(dz) > 1e-12, dz, 1.0)
            hits.append((above[:, a] != above[:, b], pa[:, :2] + (pb[:, :2] - pa[:, :2]) * f[:, None]))
        cross = np.stack([h[0] for h in hits], 1)
        pts = np.stack([h[1] for h in hits], 1)
        two = cross.sum(1) == 2
        cross, pts = cross[two], pts[two]
        first = np.argmax(cross, 1)
        second = 2 - np.argmax(cross[:, ::-1], 1)
        a = pts[np.arange(len(pts)), first]
        b = pts[np.arange(len(pts)), second]
        k = np.maximum(1, (np.hypot(*(b - a).T) / self.step).astype(int))
        s = np.concatenate([np.linspace(0, 1, n + 1) for n in k]) if len(k) else np.zeros(0)
        rep = np.repeat(np.arange(len(k)), k + 1)
        return a[rep] + (b[rep] - a[rep]) * s[:, None]


def parts(points, cell=0.003):
    """Labels `points` (2D) by which connected piece of a `cell`-m grid
    they fall in, eight-connected: two outlines closer than a cell or two
    are one piece."""
    ij = np.floor(points / cell).astype(int)
    ij -= ij.min(0) - 2
    shape = tuple(ij.max(0) + 3)
    occ = np.zeros(shape, bool)
    occ[ij[:, 0], ij[:, 1]] = True
    lab = np.where(occ, np.arange(occ.size).reshape(shape) + 1, 0)
    while True:
        m = lab.copy()
        for dx in (-1, 0, 1):
            for dy in (-1, 0, 1):
                if dx or dy:
                    m = np.maximum(m, np.roll(np.roll(lab, dx, 0), dy, 1))
        m = np.where(occ, m, 0)
        if (m == lab).all():
            break
        lab = m
    return lab[ij[:, 0], ij[:, 1]]


# ---------------------------------------------------------------- the jacket

# Heights, m, on the seated body `private_character.seat` makes of a body
# 1.62 m tall: they scale with the body's standing height.
HEM = 0.555  # the jacket's hem, over the hips
ARMS = (0.60, 0.86)  # where the arms hang apart from the torso: wrist to armpit
ARMPIT = 0.89  # above it the torso's outline takes in the arms
SHOULDER = 1.036  # the top of the shoulders
TOP = 1.06  # the highest slice the torso shell measures
EASE = 0.013  # how far the cloth stands off the body, m
SLEEVE_EASE = 0.011
FALL = 0.55  # how steeply cloth falls inward below a fullness, m per m
RISE = 1.1  # and above one
ANGLES = 96  # columns around the torso
ARM_ANGLES = 48  # and around a sleeve


class Body:
    """The seated body as the garments see it: positions, the hair, and
    horizontal sections of everything else."""

    def __init__(self, obj, height):
        self.obj = obj
        self.k = height / 1.62
        self.co = positions(obj)
        self.e = edges(obj)
        self.hair = hair_mask(obj, self.co, self.e)
        self.tris = triangles_of(obj)
        self.skin_tris = self.tris[~self.hair[self.tris].any(1)]
        self.slicer = Slicer(self.co, self.skin_tris)

    def z(self, value):
        return value * self.k


def arm_rings(body, side):
    """The arm on `side` (+1 or -1 in x) from wrist to armpit, as rings: for
    each slice, its height, center, and the radius at each of ARM_ANGLES
    directions; slices where the arm touches the torso borrow from their
    neighbors."""
    lo, hi = body.z(ARMS[0]), body.z(ARMS[1])
    zs = np.arange(lo, hi + 1e-6, 0.005)
    angles = np.linspace(-math.pi, math.pi, ARM_ANGLES, endpoint=False)
    found = []
    for z in zs:
        p = body.slicer.at(z)
        p = p[p[:, 0] * side > 0]
        if len(p) == 0:
            found.append(None)
            continue
        lab = parts(p)
        best = None
        for l in np.unique(lab):
            q = p[lab == l]
            w = np.ptp(q[:, 0])
            if len(q) > 40 and np.abs(q[:, 0]).min() > 0.085 * body.k and w < 0.1 * body.k:
                if best is None or len(q) > len(best):
                    best = q
        found.append(best)
    centers = np.full((len(zs), 2), np.nan)
    radius = np.full((len(zs), ARM_ANGLES), np.nan)
    for i, q in enumerate(found):
        if q is None:
            continue
        h = hull(q)
        c = (h.min(0) + h.max(0)) / 2
        centers[i] = c
        radius[i] = radii(h, c, angles)
    ok = ~np.isnan(centers[:, 0])
    for j in range(2):
        centers[:, j] = np.interp(zs, zs[ok], centers[ok, j])
    for j in range(ARM_ANGLES):
        radius[:, j] = np.interp(zs, zs[ok], radius[ok, j])
    radius = smooth_rows(smooth_cyclic(radius, 2), 3)
    centers = smooth_rows(centers, 4)
    return zs, centers, angles, radius


def inside_rings(points, rings, margin):
    """Which 2D `points` of one slice fall within an arm's ring there."""
    c, angles, r = rings
    d = points - c
    a = np.arctan2(d[:, 1], d[:, 0])
    rr = np.interp(a, angles, r, period=2 * math.pi)
    return np.hypot(d[:, 0], d[:, 1]) < rr + margin


def torso_rings(body, arms):
    """The torso from hem to neck as rings around a center line, each the
    convex outline of its slice without the arms below the armpit, with the
    arms above it, blended across the armpit."""
    zs = np.concatenate([np.arange(body.z(HEM) - 0.01, body.z(0.98), 0.006),
                         np.arange(body.z(0.98), body.z(TOP) + 1e-6, 0.003)])
    angles = np.linspace(-math.pi, math.pi, ANGLES, endpoint=False)
    rows, centers = [], []
    armpit = body.z(ARMPIT)
    for z in zs:
        p = body.slicer.at(z)
        if z < armpit:
            keep = np.ones(len(p), bool)
            for zs_a, cs, ang, rad in arms:
                i = int(np.clip(np.searchsorted(zs_a, z), 0, len(zs_a) - 1))
                if z <= zs_a[-1] + 0.03:
                    keep &= ~inside_rings(p, (cs[i], ang, rad[i]), 0.006)
            torso = p[keep]
        elif z > body.z(SHOULDER):
            # Above the shoulders, only the neck: the largest piece.
            lab = parts(p)
            labels, counts = np.unique(lab, return_counts=True)
            torso = p[lab == labels[np.argmax(counts)]]
        else:
            torso = p
        c = np.array([0.0, (torso[:, 1].min() + torso[:, 1].max()) / 2])
        h = hull(extremes(torso, c))
        rows.append(radii(h, c, angles))
        centers.append(c)
    r = np.array(rows)
    centers = smooth_rows(np.array(centers), 3)
    # Cloth falls from a fullness, like the bust or the shoulder blades,
    # rather than following the body in under it.
    dz = np.diff(zs)
    for i in range(len(zs) - 2, -1, -1):
        r[i] = np.maximum(r[i], r[i + 1] - FALL * dz[i])
    chest = body.z(ARMPIT)
    for i in range(1, len(zs)):
        if zs[i] < chest:
            r[i] = np.maximum(r[i], r[i - 1] - RISE * dz[i - 1])
    r = smooth_rows(smooth_cyclic(r, 2), 2)
    return zs, centers, angles, r


def loft(name, zs, centers, angles, r, ease):
    rings = []
    for i, z in enumerate(zs):
        rr = r[i] + ease
        x = centers[i, 0] + rr * np.cos(angles)
        y = centers[i, 1] + rr * np.sin(angles)
        rings.append(np.stack([x, y, np.full_like(x, z)], 1))
    return grid_mesh(name, rings)


def jacket_form(body):
    """The jacket's closed form before it is cut: the torso shell and both
    sleeves joined in one surface."""
    arms = [arm_rings(body, s) for s in (1, -1)]
    torso = torso_rings(body, arms)
    shell = loft("jacket_torso", *torso, EASE)
    sleeves = []
    for s, (zs, cs, ang, rad) in zip((1, -1), arms):
        # Carry the sleeve up into the shoulder, where the torso's outline
        # already holds the arm.
        top = np.arange(zs[-1] + 0.005, body.z(ARMPIT) + 0.05, 0.005)
        zs2 = np.concatenate([zs, top])
        cs2 = np.concatenate([cs, np.repeat(cs[-1:], len(top), 0)])
        rad2 = np.concatenate([rad, np.repeat(rad[-1:], len(top), 0)])
        sleeves.append(loft("jacket_sleeve", zs2, cs2, ang, rad2, SLEEVE_EASE))
    form = join([shell] + sleeves, "jacket")
    mod = form.modifiers.new("Remesh", "REMESH")
    mod.mode = "VOXEL"
    mod.voxel_size = 0.004
    apply(form, mod)
    return form, arms, torso


def vertex_normals(obj):
    obj.data.update()
    n = np.zeros(len(obj.data.vertices) * 3)
    obj.data.vertices.foreach_get("normal", n)
    return n.reshape(-1, 3)


def laplacian(co, e, iterations, lam=0.5, fixed=None):
    """Moves each vertex part way toward the mean of its neighbors."""
    n = len(co)
    deg = np.zeros(n)
    np.add.at(deg, e[:, 0], 1)
    np.add.at(deg, e[:, 1], 1)
    deg = np.maximum(deg, 1)[:, None]
    for _ in range(iterations):
        acc = np.zeros_like(co)
        np.add.at(acc, e[:, 0], co[e[:, 1]])
        np.add.at(acc, e[:, 1], co[e[:, 0]])
        step = lam * (acc / deg - co)
        if fixed is not None:
            step[fixed] = 0
        co = co + step
    return co


def push_out(co, normals, tree, gap, reach=0.06):
    """Moves each point of a garment outward along its normal until it
    stands at least `gap` off the body in `tree`: a point inside the body
    (the body lies just outside it) or too near it. Returns the new points
    and how many moved."""
    out = co.copy()
    moved = 0
    for i, (p, n) in enumerate(zip(co, normals)):
        p, n = Vector(p), Vector(n)
        if n.length < 1e-9:
            continue
        hit, _, _, d = tree.ray_cast(p, n, reach)
        if hit is not None:
            # The body is outside this point: it pokes through.
            out[i] = hit + n * gap
            moved += 1
            continue
        hit, _, _, d = tree.ray_cast(p, -n, gap)
        if hit is not None:
            out[i] = hit + n * gap
            moved += 1
    return out, moved


# The front: one button at the waist; above it the opening widens in a V
# to the gorge, where the lapel meets the collar; below it the fronts curve
# apart to the hem.
BUTTON = 0.712
GORGE = (0.062, 0.99)  # the V's half width and height at the gorge
CUTAWAY = 0.075  # the opening's half width at the hem
NECK_CUT = 1.05
CUFF = 0.613  # the cuff's height on the forearm


def surface_point(tree, origin, direction):
    hit, normal, _, _ = tree.ray_cast(Vector(origin), Vector(direction).normalized(), 2.0)
    return hit, normal


class Front:
    """The front opening's planes, from the button: two V planes up to the
    gorge and two cutaway planes down to the hem, each standing upright
    along Y."""

    def __init__(self, body, form_tree):
        z_b = body.z(BUTTON)
        b, _ = surface_point(form_tree, (0, -1, z_b), (0, 1, 0))
        self.button = b
        x_g, z_g = GORGE[0] * body.k, body.z(GORGE[1])
        x_h, z_h = CUTAWAY * body.k, body.z(HEM)
        self.v = [self.plane(b, Vector((s * x_g, b.y, z_g))) for s in (1, -1)]
        self.cut = [self.plane(b, Vector((s * x_h, b.y, z_h))) for s in (1, -1)]

    @staticmethod
    def plane(a, c):
        n = (c - a).cross(Vector((0, 1, 0))).normalized()
        # Point the normal into the opening: toward the middle line level
        # with `c`.
        if (Vector((0, a.y, c.z)) - a).dot(n) < 0:
            n = -n
        return a.copy(), n

    def opening(self, p, center_y):
        """Whether point `p` lies in the opening."""
        if p.y > center_y:
            return False
        if p.z > self.button.z:
            planes = self.v
        else:
            planes = self.cut
        return all((p - a).dot(n) > 0 for a, n in planes)


def keep_largest(bm):
    faces = set(bm.faces)
    best = []
    while faces:
        seed = faces.pop()
        island, stack = [seed], [seed]
        while stack:
            f = stack.pop()
            for e in f.edges:
                for g in e.link_faces:
                    if g in faces:
                        faces.remove(g)
                        island.append(g)
                        stack.append(g)
        if len(island) > len(best):
            best = island
    keep = set(best)
    bmesh.ops.delete(bm, geom=[f for f in bm.faces if f not in keep], context="FACES")
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")


def cut_jacket(form, body, arms, torso):
    """Cuts the closed form at the hem, the neck, the cuffs, and the front."""
    tree = bvh(form)
    front = Front(body, tree)
    bm = bmesh.new()
    bm.from_mesh(form.data)
    cuts = [((0, 0, body.z(HEM)), (0, 0, 1)), ((0, 0, body.z(NECK_CUT)), (0, 0, 1))]
    cuts += [(tuple(a), tuple(n)) for a, n in front.v + front.cut]
    cuffs = []
    for zs, cs, _, _ in arms:
        z = body.z(CUFF)
        i = int(np.searchsorted(zs, z))
        c = Vector((*cs[i], zs[i]))
        j = min(i + 6, len(zs) - 1)
        axis = (Vector((*cs[j], zs[j])) - c).normalized()
        cuffs.append((c, axis))
        cuts.append((tuple(c), tuple(axis)))
    for co, no in cuts:
        geom = bm.verts[:] + bm.edges[:] + bm.faces[:]
        bmesh.ops.bisect_plane(bm, geom=geom, plane_co=co, plane_no=no, dist=1e-6)
    hem, neck = body.z(HEM), body.z(NECK_CUT)
    doomed = []
    for f in bm.faces:
        c = f.calc_center_median()
        if c.z < hem or c.z > neck:
            doomed.append(f)
        elif any((c - a).dot(n) < 0 and (c - a).length < 0.12 for a, n in cuffs):
            doomed.append(f)
        elif front.opening(c, float(np.interp(c.z, torso[0], torso[1][:, 1]))):
            doomed.append(f)
    bmesh.ops.delete(bm, geom=doomed, context="FACES")
    keep_largest(bm)
    bm.to_mesh(form.data)
    bm.free()
    form.data.update()
    return front, cuffs


def boundary(obj):
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    out = np.zeros(len(bm.verts), bool)
    for v in bm.verts:
        out[v.index] = v.is_boundary
    bm.free()
    return out


def relax_open(form, tree, gap=0.008, rounds=3):
    """Smooths an open garment with its edges held, and keeps it off the
    body."""
    e = edges(form)
    edge = boundary(form)
    co = positions(form)
    moved = 0
    for _ in range(rounds):
        co = laplacian(co, e, 12, fixed=edge)
        set_positions(form, co)
        co, moved = push_out(co, vertex_normals(form), tree, gap)
        set_positions(form, co)
    return moved


LAPEL = 0.068  # the lapel's widest, m
LAPEL_LIFT = 0.005  # how far a lapel stands off the front it lies on
ROWS, COLS = 56, 10


def mesh_from_grid(name, grid):
    """An open quad mesh through `grid` (R x C x 3)."""
    grid = np.asarray(grid)
    R, C = grid.shape[:2]
    verts = [tuple(p) for p in grid.reshape(-1, 3)]
    faces = [(i * C + j, i * C + j + 1, (i + 1) * C + j + 1, (i + 1) * C + j)
             for i in range(R - 1) for j in range(C - 1)]
    me = bpy.data.meshes.new(name)
    me.from_pydata(verts, [], faces)
    me.validate()
    obj = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(obj)
    return obj


def lapel_width(t):
    """The lapel's width along its roll line, from the button (0) to the
    gorge (1) and on up the neckline, as a fraction of LAPEL: it widens to
    a point below the gorge, its top edge runs back in to the notch, and
    above the notch the collar's end runs narrow up to the neck."""
    if t < 0.78:
        return (t / 0.78) ** 0.8
    if t < 0.86:
        return 1.0
    if t < 0.98:
        return 1.0 - 0.6 * (t - 0.86) / 0.12
    if t < 1.03:
        return 0.4 - 0.25 * (t - 0.98) / 0.05
    if t < 1.07:
        return 0.15 + 0.3 * (t - 1.03) / 0.04
    return 0.45


def lapels(body, front, form_tree):
    """Two notched lapels rolled back from the opening's edges, from the
    button to the gorge, lying on the fronts."""
    out = []
    b = front.button
    x_g, z_g = GORGE[0] * body.k, body.z(GORGE[1])
    for s in (1, -1):
        grid = []
        top = (body.z(NECK_CUT) - 0.002 - b.z) / (z_g - b.z)
        ts = np.linspace(0.0, top, ROWS)
        edge = []
        for t in ts:
            p = Vector((s * (x_g * t + 0.0015), -1.0, b.z + (z_g - b.z) * t))
            hit, n, _, _ = form_tree.ray_cast(p, Vector((0, 1, 0)), 2.0)
            edge.append((hit, n))
        for i, t in enumerate(ts):
            e, n = edge[i]
            j = min(i + 1, ROWS - 1)
            k = max(i - 1, 0)
            tangent = (edge[j][0] - edge[k][0]).normalized()
            across = n.cross(tangent).normalized()
            if across.x * s < 0:
                across = -across
            w = LAPEL * body.k * lapel_width(t)
            row = []
            for u in np.linspace(0.0, 1.0, COLS):
                q = e + across * (w * u) + n * 0.04
                hit, hn, _, _ = form_tree.ray_cast(q, -n, 0.1)
                if hit is None:
                    hit, hn, _, _ = form_tree.find_nearest(q)
                lift = LAPEL_LIFT * smoothstep(u * 6.0)
                row.append(tuple(hit + hn * lift))
            grid.append(row)
        out.append(mesh_from_grid("lapel", grid))
    return out


def smoothstep(t):
    t = min(max(t, 0.0), 1.0)
    return t * t * (3.0 - 2.0 * t)


def collar(body, form, front, form_tree):
    """The collar: a band that stands around the back of the neck from one
    gorge to the other and turns down over the neckline."""
    bm = bmesh.new()
    bm.from_mesh(form.data)
    z_g = body.z(GORGE[1])
    loop = [v.co.copy() for v in bm.verts if v.is_boundary and v.co.z > z_g - 0.005
            and abs(v.co.x) < 0.11 * body.k]
    bm.free()
    pts = np.array([tuple(p) for p in loop])
    c = np.array([0.0, pts[:, 1].mean()])
    ang = np.arctan2(pts[:, 1] - c[1], pts[:, 0] - c[0])
    rad = np.hypot(pts[:, 0] - c[0], pts[:, 1] - c[1])
    order = np.argsort(ang)
    ang, rad, zz = ang[order], rad[order], pts[order, 2]
    # The collar stands where the neckline is the neck cut, round the back
    # (+Y) from one side of the opening to the other.
    top = zz > body.z(NECK_CUT) - 0.004
    around = (ang[top] + math.pi / 2) % (2 * math.pi)
    span_l = np.linspace(around.min(), around.max(), 72) - math.pi / 2
    profile = [(-0.004, 0.0), (-0.006, 0.016), (-0.003, 0.028), (0.004, 0.034),
               (0.012, 0.027), (0.018, 0.010), (0.023, -0.008), (0.027, -0.022)]
    grid = []
    for k, a in enumerate(span_l):
        # The stand eases down toward the gorges, where the collar meets
        # the lapels' ends.
        f = k / (len(span_l) - 1)
        taper = 0.35 + 0.65 * smoothstep(min(f, 1.0 - f) / 0.22)
        aa = (a + math.pi) % (2 * math.pi) - math.pi
        r0 = float(np.interp(aa, ang, rad, period=2 * math.pi))
        z0 = float(np.interp(aa, ang, zz, period=2 * math.pi))
        d = np.array([math.cos(aa), math.sin(aa)])
        row = []
        for dr, dz in profile:
            xy = c + d * (r0 + dr * taper * body.k)
            p = Vector((xy[0], xy[1], z0 + dz * taper * body.k))
            row.append(tuple(p))
        grid.append(row)
    return mesh_from_grid("collar", grid)


# ---------------------------------------------------------------- fitting


def neighbor_mean(values, e, n):
    acc = np.zeros((n,) + values.shape[1:])
    cnt = np.zeros(n)
    np.add.at(acc, e[:, 0], values[e[:, 1]])
    np.add.at(acc, e[:, 1], values[e[:, 0]])
    np.add.at(cnt, e[:, 0], 1)
    np.add.at(cnt, e[:, 1], 1)
    return acc / np.maximum(cnt, 1).reshape((-1,) + (1,) * (values.ndim - 1))


SWING = 0.22  # how fast a hair lift eases off up the strand, m per m


def lift_hair(body, garment_tree, gap=0.005, reach=0.04):
    """Lifts the hair the garments would pass through out onto them, so the
    hair falls over the cloth. The lift is a smooth field over the angle
    around the neck and the height, the same for every layer of hair at an
    angle and height, so curls and the hair's layers keep their shape and
    order rather than flattening onto the cloth or passing through each
    other. Returns how many vertices needed lifting."""
    co = body.co
    idx = np.nonzero(body.hair)[0]
    pts, needs = [], []
    for i in idx:
        v = Vector(co[i])
        p, n, _, _ = garment_tree.find_nearest(v, reach)
        if p is None:
            continue
        depth = (v - p).dot(n)
        if depth < gap:
            pts.append(co[i])
            needs.append(np.array(p + n * gap - v))
    if not pts:
        return 0
    pts, needs = np.array(pts), np.array(needs)
    neck = np.array([0.0, float(np.median(co[idx, 1]))])
    na, nz = 72, 64
    z0, z1 = pts[:, 2].min() - 0.05, pts[:, 2].max() + 0.2
    zs = np.linspace(z0, z1, nz)

    def coords(p):
        a = np.arctan2(p[:, 1] - neck[1], p[:, 0] - neck[0])
        fa = (a + math.pi) / (2 * math.pi) * na
        fz = np.clip((p[:, 2] - z0) / (z1 - z0) * (nz - 1), 0, nz - 1)
        return fa, fz

    def bins(p):
        fa, fz = coords(p)
        return fa.astype(int) % na, fz.round().astype(int)

    ia, iz = bins(pts)
    field = np.zeros((nz, na, 3))
    size = np.zeros((nz, na))
    for a, z, d in zip(ia, iz, needs):
        m = np.linalg.norm(d)
        field[z, a] += d
        size[z, a] = max(size[z, a], m)
    # Hair hangs: whatever lifts a strand lifts it all the way down.
    for z in range(nz - 2, -1, -1):
        size[z] = np.maximum(size[z], size[z + 1])
        field[z] = np.where((np.linalg.norm(field[z], axis=1) < 1e-9)[:, None], field[z + 1], field[z])
    # And up a strand toward the head the lift eases off gradually, as hair
    # swings out from where it hangs.
    step = SWING * (z1 - z0) / (nz - 1)
    for z in range(1, nz):
        size[z] = np.maximum(size[z], size[z - 1] - step)
        field[z] = np.where((np.linalg.norm(field[z], axis=1) < 1e-9)[:, None], field[z - 1], field[z])
    # Spread around the neck.
    for _ in range(10):
        size = np.maximum(size, 0.96 * (np.roll(size, 1, 1) + np.roll(size, -1, 1)) / 2)
    k = np.array([1, 4, 6, 4, 1]) / 16.0
    for _ in range(6):
        size = sum(w * np.roll(size, j - 2, 1) for j, w in enumerate(k))
        pad = np.vstack([size[:1]] * 2 + [size] + [size[-1:]] * 2)
        size = sum(w * pad[j:j + nz] for j, w in enumerate(k))
        for c in range(3):
            f = field[:, :, c]
            f = sum(w * np.roll(f, j - 2, 1) for j, w in enumerate(k))
            pad = np.vstack([f[:1]] * 2 + [f] + [f[-1:]] * 2)
            field[:, :, c] = sum(w * pad[j:j + nz] for j, w in enumerate(k))
    norm = np.linalg.norm(field, axis=2)
    direction = np.where(norm[:, :, None] > 1e-9, field / np.maximum(norm, 1e-9)[:, :, None], 0)
    lift = direction * size[:, :, None]
    near = idx[co[idx, 2] < z1]
    fa, fz = coords(co[near])
    a0 = np.floor(fa).astype(int)
    za = np.minimum(np.floor(fz).astype(int), nz - 2)
    ta, tz = (fa - a0)[:, None], (fz - za)[:, None]
    a0 %= na
    a1 = (a0 + 1) % na
    value = ((1 - tz) * ((1 - ta) * lift[za, a0] + ta * lift[za, a1])
             + tz * ((1 - ta) * lift[za + 1, a0] + ta * lift[za + 1, a1]))
    # Nothing moves above the reach of the swing, where the face is.
    top = pts[:, 2].max()
    fade = 1.0 - np.clip((co[near, 2] - top - 0.03) / 0.07, 0.0, 1.0)
    out = co.copy()
    out[near] += value * (fade * fade * (3 - 2 * fade))[:, None]
    body.co = out
    set_positions(body.obj, out)
    return len(pts)


def hidden_faces(body, garment_tree, bounds, reach=0.12, tilt=1.4, keep_rings=3):
    """The body's faces the garments cover from every side: each of their
    vertices meets cloth along its normal and along four directions tilted
    from it. Only vertices within `bounds`, the garments' box, are tested.
    Hair is never hidden."""
    me = body.obj.data
    normals = vertex_normals(body.obj)
    co = body.co
    lo, hi = np.asarray(bounds[0]) - 0.02, np.asarray(bounds[1]) + 0.02
    near = np.all((co > lo) & (co < hi), 1) & ~body.hair
    covered = np.zeros(len(co), bool)
    for i in np.nonzero(near)[0]:
        n = Vector(normals[i])
        if n.length < 1e-6:
            continue
        p = Vector(co[i]) + n * 0.0005
        t = n.orthogonal().normalized()
        b = n.cross(t)
        ok = True
        for d in (n, (n + t * tilt).normalized(), (n - t * tilt).normalized(),
                  (n + b * tilt).normalized(), (n - b * tilt).normalized()):
            if garment_tree.ray_cast(p, d, reach)[0] is None:
                ok = False
                break
        covered[i] = ok
    # Keep a few rings of faces past the last uncovered vertex, so an
    # opening never shows a cut edge of the body under it.
    e = body.e
    for _ in range(keep_rings):
        shown = ~covered
        spread = shown.copy()
        spread[e[:, 0]] |= shown[e[:, 1]]
        spread[e[:, 1]] |= shown[e[:, 0]]
        covered = ~spread
    poly = np.zeros(len(me.polygons), bool)
    for f in me.polygons:
        poly[f.index] = all(covered[v] for v in f.vertices)
    return poly



def body_tree(body):
    return BVHTree.FromPolygons([tuple(p) for p in body.co], [tuple(t) for t in body.skin_tris])


def jacket(body):
    """The jacket, one object: shell, lapels, and collar, with turned edges."""
    form, arms, torso = jacket_form(body)
    tree = body_tree(body)
    front, _ = cut_jacket(form, body, arms, torso)
    relax_open(form, tree)
    ft = bvh(form)
    pieces = [form, collar(body, form, front, ft)] + lapels(body, front, ft)
    for o in pieces:
        mod = o.modifiers.new("Solidify", "SOLIDIFY")
        mod.thickness = 0.004 * body.k
        mod.offset = -1
        mod.use_rim = True
        mod.use_rim_only = True
        apply(o, mod)
    return join(pieces, "jacket"), front


def delete_faces(obj, mask):
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bm.faces.ensure_lookup_table()
    bmesh.ops.delete(bm, geom=[bm.faces[i] for i in np.nonzero(mask)[0]], context="FACES")
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    bm.to_mesh(obj.data)
    bm.free()
    obj.data.update()


def bounds(obj):
    co = positions(obj)
    return co.min(0), co.max(0)


# ---------------------------------------------------------------- the pumps

SHOE_EASE = 0.0035  # the upper's stand-off from the foot, m
SOLE = 0.004  # the sole's thickness under the ball of the foot, m
TOE = 0.026  # how far the pointed toe reaches past the toes, m
FOOT_TOP = 0.16  # the highest a foot reaches, as a fraction of standing height / 1.62 m


def foot_points(body, side):
    """The foot on `side`, as positions: everything below the ankle on that
    side of the middle."""
    co = body.co
    m = (co[:, 2] < body.z(FOOT_TOP)) & (co[:, 0] * side > 0) & ~body.hair
    return co[m]


def shoe(body, side):
    """One pump fitted to the foot on `side` as the seated pose leaves it:
    an upper lofted over the foot's sections, a pointed toe, a rounded heel
    cup, cut low over the instep, and a tapered heel to the floor."""
    pts = foot_points(body, side)
    y_toe, y_heel = pts[:, 1].min(), pts[:, 1].max()
    length = y_heel - y_toe
    y_ball = y_toe + 0.19 * length
    ys = np.linspace(y_heel - 0.0012 * body.k, y_ball, 44)
    angles = np.linspace(-math.pi, math.pi, 48, endpoint=False)
    sl = Slicer(body.co[:, [0, 2, 1]].copy(), body.skin_tris, step=0.0015)
    rows, centers, kept = [], [], []
    for y in ys:
        p = sl.at(y)  # (x, z) of the section at y
        p = p[(p[:, 0] * side > 0) & (p[:, 1] < body.z(FOOT_TOP) * 0.8)]
        if len(p) < 6:
            continue
        h = hull(p)
        c = (h.min(0) + h.max(0)) / 2
        kept.append(y)
        centers.append(c)
        rows.append(radii(h, c, angles))
    ys = np.array(kept)
    centers = smooth_rows(np.array(centers), 2)
    r = smooth_rows(smooth_cyclic(np.array(rows), 1), 2) + SHOE_EASE

    def ring(c, rr):
        return np.stack([c[0] + rr * np.cos(angles), np.zeros_like(rr), c[1] + rr * np.sin(angles)], 1)

    grid = []
    for y, c, rr in zip(ys, centers, r):
        g = ring(c, rr)
        g[:, 1] = y
        grid.append(g)
    # The pointed toe: from the ball of the foot, past the toes, to a point
    # a little above the sole.
    c_b, ring_b = centers[-1], ring(centers[-1], r[-1])
    tip_y = y_toe - TOE * body.k
    tip = np.array([c_b[0] - side * 0.004 * body.k, -SOLE + 0.007 * body.k])
    for f in np.linspace(0.08, 1.0, 12):
        c = c_b + (tip - c_b) * f
        sc = max((1.0 - f ** 1.8) ** 0.55, 0.02)
        g = np.stack([c[0] + (ring_b[:, 0] - c_b[0]) * sc, np.full(len(angles), y_ball + (tip_y - y_ball) * f),
                      c[1] + (ring_b[:, 2] - c_b[1]) * sc], 1)
        grid.append(g)
    for g in grid:
        g[:, 2] = np.maximum(g[:, 2], -SOLE)
    obj = grid_mesh("pump", grid)
    recalc_normals(obj)
    return obj, (y_toe, y_heel)


def recalc_normals(obj):
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    bm.to_mesh(obj.data)
    bm.free()
    obj.data.update()


COUNTER = 0.042  # the heel counter's height over the heel's underside, m
VAMP = 0.29  # where the throat crosses the foot, as a fraction from the toes
HEEL_TOP = (0.024, 0.027)  # the heel's width and depth where it meets the shoe, m
HEEL_TIP = (0.009, 0.010)  # and at the floor


def heel(body, upper, side, y_heel):
    """The tapered heel under the heel cup, from inside the shoe to the
    floor, its back in line with the counter's."""
    co = positions(upper)
    near = co[(co[:, 1] > y_heel - 0.03 * body.k)]
    cx = float(near[:, 0].mean())
    under = float(near[:, 2].min())
    top_z, bottom_z = under + 0.01 * body.k, -SOLE
    back_top, back_tip = y_heel - 0.004 * body.k, y_heel - 0.016 * body.k
    rings = []
    a = np.linspace(-math.pi, math.pi, 24, endpoint=False)
    for t in np.linspace(0.0, 1.0, 14):
        f = t ** 0.55
        w = (HEEL_TOP[0] + (HEEL_TIP[0] - HEEL_TOP[0]) * f) * body.k / 2
        d = (HEEL_TOP[1] + (HEEL_TIP[1] - HEEL_TOP[1]) * f) * body.k / 2
        back = back_top + (back_tip - back_top) * t
        z = top_z + (bottom_z - top_z) * t
        # A rounded rectangle, by a superellipse.
        x = cx + w * np.sign(np.cos(a)) * np.abs(np.cos(a)) ** 0.6
        y = back - d + d * np.sign(np.sin(a)) * np.abs(np.sin(a)) ** 0.6
        rings.append(np.stack([x, y, np.full_like(x, z)], 1))
    obj = grid_mesh("heel", rings)
    recalc_normals(obj)
    return obj


def pumps(body):
    """Both pumps: uppers pushed clear of the feet, smoothed, cut low over
    the instep, with turned edges and tapered heels."""
    tree = body_tree(body)
    out = []
    for side in (1, -1):
        upper, (y_toe, y_heel) = shoe(body, side)
        e = edges(upper)
        co = positions(upper)
        for _ in range(3):
            co, _ = push_out(co, vertex_normals(upper), tree, 0.003, reach=0.025)
            set_positions(upper, co)
            co = laplacian(co, e, 3, lam=0.4)
            set_positions(upper, co)
        co, _ = push_out(co, vertex_normals(upper), tree, 0.003, reach=0.025)
        co[:, 2] = np.maximum(co[:, 2], -SOLE)
        set_positions(upper, co)
        # The throat: a plane from the top of the heel counter down to the
        # vamp over the toes.
        pts = foot_points(body, side)
        length = y_heel - y_toe
        under_heel = pts[pts[:, 1] > y_heel - 0.02 * body.k][:, 2].min()
        back = Vector((0, y_heel, under_heel + COUNTER * body.k))
        y_v = y_toe + VAMP * length
        top_v = pts[np.abs(pts[:, 1] - y_v) < 0.004][:, 2].max()
        vamp = Vector((0, y_v, top_v - 0.003 * body.k))
        along = (vamp - back).normalized()
        normal = along.cross(Vector((1, 0, 0))).normalized()
        if normal.z < 0:
            normal = -normal
        bm = bmesh.new()
        bm.from_mesh(upper.data)
        bmesh.ops.bisect_plane(bm, geom=bm.verts[:] + bm.edges[:] + bm.faces[:],
                               plane_co=back, plane_no=normal, dist=1e-6)
        doomed = [f for f in bm.faces if (f.calc_center_median() - back).dot(normal) > 0]
        bmesh.ops.delete(bm, geom=doomed, context="FACES")
        keep_largest(bm)
        bm.to_mesh(upper.data)
        bm.free()
        e = edges(upper)
        edge = boundary(upper)
        co = laplacian(positions(upper), e, 4, lam=0.3, fixed=edge)
        set_positions(upper, co)
        mod = upper.modifiers.new("Solidify", "SOLIDIFY")
        mod.thickness = 0.0022 * body.k
        mod.offset = -1
        mod.use_rim = True
        mod.use_rim_only = True
        apply(upper, mod)
        out.append(join([upper, heel(body, upper, side, y_heel)], "pump"))
    return join(out, "pumps")


# ---------------------------------------------------------------- materials


def node(nt, kind, location, **inputs):
    n = nt.nodes.new(kind)
    n.location = location
    for key, value in inputs.items():
        if key in n.inputs:
            n.inputs[key].default_value = value
        else:
            setattr(n, key, value)
    return n


def cloth_material():
    """Black suiting: a near-black wool with a fine diagonal twill and a
    faint mottle, darkened in its folds. Only base color survives the bake,
    so the folds' shade is part of it."""
    mat = bpy.data.materials.new("jacket")
    mat.use_nodes = True
    nt = mat.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = 0.72
    coord = node(nt, "ShaderNodeTexCoord", (-1200, 0))
    twill = node(nt, "ShaderNodeTexWave", (-900, 200), wave_type="BANDS", bands_direction="DIAGONAL",
                 Scale=180.0, Distortion=1.5, Detail=2.0)
    nt.links.new(coord.outputs["Object"], twill.inputs["Vector"])
    mottle = node(nt, "ShaderNodeTexNoise", (-900, -100), Scale=60.0, Detail=6.0, Roughness=0.6)
    nt.links.new(coord.outputs["Object"], mottle.inputs["Vector"])
    ao = node(nt, "ShaderNodeAmbientOcclusion", (-900, -400), Distance=0.06, samples=16)
    # base = 0.020 * (0.9 + 0.12 twill) * (0.85 + 0.3 mottle) * (0.5 + 0.5 ao)
    t = node(nt, "ShaderNodeMath", (-600, 200), operation="MULTIPLY_ADD")
    t.inputs[1].default_value, t.inputs[2].default_value = 0.12, 0.9
    nt.links.new(twill.outputs["Fac"], t.inputs[0])
    m = node(nt, "ShaderNodeMath", (-600, -100), operation="MULTIPLY_ADD")
    m.inputs[1].default_value, m.inputs[2].default_value = 0.3, 0.85
    nt.links.new(mottle.outputs["Fac"], m.inputs[0])
    a = node(nt, "ShaderNodeMath", (-600, -400), operation="MULTIPLY_ADD")
    a.inputs[1].default_value, a.inputs[2].default_value = 0.5, 0.5
    nt.links.new(ao.outputs["AO"], a.inputs[0])
    tm = node(nt, "ShaderNodeMath", (-400, 0), operation="MULTIPLY")
    nt.links.new(t.outputs[0], tm.inputs[0])
    nt.links.new(m.outputs[0], tm.inputs[1])
    ta = node(nt, "ShaderNodeMath", (-250, 0), operation="MULTIPLY")
    nt.links.new(tm.outputs[0], ta.inputs[0])
    nt.links.new(a.outputs[0], ta.inputs[1])
    level = node(nt, "ShaderNodeMath", (-100, 0), operation="MULTIPLY")
    level.inputs[1].default_value = JACKET_BLACK
    nt.links.new(ta.outputs[0], level.inputs[0])
    tint = node(nt, "ShaderNodeCombineColor", (50, 0))
    for k in range(3):
        nt.links.new(level.outputs[0], tint.inputs[k])
    nt.links.new(tint.outputs[0], bsdf.inputs["Base Color"])
    return mat


JACKET_BLACK = 0.034  # the jacket's linear base color before its folds
SHOE_BLACK = 0.010
SHINE = 0.30  # the brightest of the pumps' baked highlight


def patent_material():
    """Black patent leather: glossy where it is rendered, and, since only
    base color survives the bake, with a soft highlight painted along the
    tops of its curves, as a lamp overhead would catch it."""
    mat = bpy.data.materials.new("pumps")
    mat.use_nodes = True
    nt = mat.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = 0.12
    if "Coat Weight" in bsdf.inputs:
        bsdf.inputs["Coat Weight"].default_value = 0.6
    geo = node(nt, "ShaderNodeNewGeometry", (-1000, 0))
    light = node(nt, "ShaderNodeVectorMath", (-800, 0), operation="DOT_PRODUCT")
    light.inputs[1].default_value = (0.25, -0.45, 0.86)
    nt.links.new(geo.outputs["Normal"], light.inputs[0])
    clamp = node(nt, "ShaderNodeMath", (-600, 0), operation="MAXIMUM")
    clamp.inputs[1].default_value = 0.0
    nt.links.new(light.outputs["Value"], clamp.inputs[0])
    sharp = node(nt, "ShaderNodeMath", (-450, 0), operation="POWER")
    sharp.inputs[1].default_value = 12.0
    nt.links.new(clamp.outputs[0], sharp.inputs[0])
    ramp = node(nt, "ShaderNodeMath", (-300, 0), operation="MULTIPLY_ADD")
    ramp.inputs[1].default_value, ramp.inputs[2].default_value = SHINE, SHOE_BLACK
    nt.links.new(sharp.outputs[0], ramp.inputs[0])
    tint = node(nt, "ShaderNodeCombineColor", (-100, 0))
    for k in range(3):
        nt.links.new(ramp.outputs[0], tint.inputs[k])
    nt.links.new(tint.outputs[0], bsdf.inputs["Base Color"])
    return mat


def finish(obj, material):
    obj.data.materials.clear()
    obj.data.materials.append(material)
    for p in obj.data.polygons:
        p.use_smooth = True
    obj.data.update()


# ---------------------------------------------------------------- dressing

OVERLAYS = ("jacket", "heels")
MATERIALS = ("jacket", "pumps")  # the garments' material names


def dress(body_obj, height, overlays):
    """Dresses the seated body in `overlays` (`jacket`, `heels`): builds
    each garment, lifts the hair onto the jacket, removes the skin and
    dress the garments hide, and joins the garments into the body with
    their own materials. Returns the dressed body, how far everything
    rose so the pumps' soles stand on the floor, m, and a report."""
    body = Body(body_obj, height)
    garments, report = [], {}
    if "jacket" in overlays:
        jk, _ = jacket(body)
        finish(jk, cloth_material())
        garments.append(jk)
        lifted = lift_hair(body, bvh(jk))
        report["jacket_triangles"] = triangles_count(jk)
        report["hair_lifted"] = lifted
    if "heels" in overlays:
        pm = pumps(body)
        finish(pm, patent_material())
        garments.append(pm)
        report["pumps_triangles"] = triangles_count(pm)
    if not garments:
        return body_obj, 0.0, report
    whole = join(garments_copy(garments), "garment_probe")
    tree = bvh(whole)
    lo, hi = bounds(whole)
    bpy.data.objects.remove(whole, do_unlink=True)
    hidden = hidden_faces(body, tree, (lo, hi))
    delete_faces(body_obj, hidden)
    report["hidden_faces"] = int(hidden.sum())
    for g in garments:
        uv = g.data.uv_layers.new(name=body_obj.data.uv_layers.active.name)
        del uv
    joined = join([body_obj] + garments, body_obj.name)
    low = positions(joined)[:, 2].min()
    lift = max(0.0, -float(low))
    if lift:
        joined.data.transform(Matrix.Translation((0, 0, lift)))
    report["lift_m"] = round(lift, 4)
    return joined, lift, report


def garments_copy(garments):
    out = []
    for g in garments:
        c = g.copy()
        c.data = g.data.copy()
        bpy.context.scene.collection.objects.link(c)
        out.append(c)
    return out


def triangles_count(obj):
    obj.data.calc_loop_triangles()
    return len(obj.data.loop_triangles)
