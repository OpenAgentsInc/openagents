"""Draconia's hair: a loose, messy platinum updo grown by script.

The hair is a groom of strand clumps rendered as thin ribbons (MetaHuman's
"cards" idea taken down to a few hairs per card), so each ribbon carries a
handful of painted hairs in its alpha, and thousands of them together give
the updo's volume, the loose face-framing locks and the flyaways:

- a scalp cap of the same colour under everything, so no skin shows through;
- the body: roots spread over the scalp from a centre part, combed back
  over the sides and up from the nape toward a knot at the back of the
  crown, riding a little off the scalp with soft lift, then wrapped into a
  messy bun of loose loops;
- face-framing locks from the temples and the front hairline, falling past
  the cheeks to the jaw and neck in loose waves;
- flyaways: single hairs that arc off the surface.

Every ribbon's UV runs u across its width (one of eight painted strand
variants, a band of u each) and v from root (0) to tip (1), which the Verse
hair shading reads for the strand direction and root-to-tip colour.

Nothing here is copied from a groom asset; the hairstyle is an original
take on a common loose updo.
"""

import math
import os
import random

import bmesh
import bpy
import numpy as np
from mathutils import Vector
from mathutils.bvhtree import BVHTree

VARIANTS = 8


def _head_frame(skin, marks):
    eye_z = (marks.eye_l.z + marks.eye_r.z) / 2
    pts = [v.co for v in skin.data.vertices if v.co.z > eye_z - 0.03]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    centre = Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, eye_z + 0.02))
    return centre, lo, hi, eye_z


def _hairline_el(az):
    """The hairline's elevation (radians) at azimuth az (0 front, +-pi
    back), seen from the head centre."""
    a = abs(az)
    pts = [(0.0, 0.62), (0.35, 0.56), (0.75, 0.30), (1.05, 0.22), (1.35, 0.30), (1.6, 0.05), (2.0, -0.45), (2.6, -0.62), (math.pi, -0.66)]
    for (a0, e0), (a1, e1) in zip(pts, pts[1:]):
        if a <= a1:
            t = (a - a0) / (a1 - a0)
            return e0 + (e1 - e0) * t
    return pts[-1][1]


def _angles(p, centre):
    d = (p - centre).normalized()
    el = math.asin(max(-1, min(1, d.z)))
    az = math.atan2(d.x, -d.y)
    return az, el


def _scalp_faces(skin, centre):
    faces = []
    me = skin.data
    for f in me.polygons:
        c = f.center
        az, el = _angles(c, centre)
        if el > _hairline_el(az):
            faces.append(f.index)
    return faces


def _sample_surface(me, faces, n, rng):
    areas = np.array([me.polygons[i].area for i in faces])
    cum = np.cumsum(areas) / areas.sum()
    out = []
    for _ in range(n):
        k = int(np.searchsorted(cum, rng.random()))
        f = me.polygons[faces[min(k, len(faces) - 1)]]
        vs = [me.vertices[i].co for i in f.vertices]
        a, b = rng.random(), rng.random()
        if a + b > 1:
            a, b = 1 - a, 1 - b
        p = vs[0] + (vs[1] - vs[0]) * a + (vs[2] - vs[0]) * b
        out.append((p, f.normal.copy()))
    return out


class Ribbons:
    def __init__(self):
        self.bm = bmesh.new()
        self.uv = self.bm.loops.layers.uv.new("UVMap")
        self.count = 0

    def add(self, pts, normals, width, variant, taper=0.6, free=None):
        """A ribbon along `pts`; its width lies across the strand and the
        given surface normals (or faces `free` where there is no surface)."""
        n = len(pts)
        if n < 2:
            return
        u0 = variant / VARIANTS
        u1 = (variant + 1) / VARIANTS
        prev = None
        length = sum((pts[i + 1] - pts[i]).length for i in range(n - 1))
        s = 0.0
        for i in range(n):
            if i > 0:
                s += (pts[i] - pts[i - 1]).length
            t = s / max(length, 1e-9)
            tan = (pts[min(i + 1, n - 1)] - pts[max(i - 1, 0)]).normalized()
            nrm = normals[i] if normals and normals[i] is not None else (free or Vector((0, -1, 0)))
            side = tan.cross(nrm)
            if side.length < 1e-6:
                side = tan.cross(Vector((0, 0, 1)))
            side.normalize()
            w = width * (1 - taper * t * t) * 0.5
            a = self.bm.verts.new(pts[i] - side * w)
            b = self.bm.verts.new(pts[i] + side * w)
            if prev:
                pa, pb, pt = prev
                f = self.bm.faces.new([pa, pb, b, a])
                for loop, uvv in zip(f.loops, [(u0, pt), (u1, pt), (u1, t), (u0, t)]):
                    loop[self.uv].uv = uvv
            prev = (a, b, t)
        self.count += 1

    def object(self, name):
        me = bpy.data.meshes.new(name)
        self.bm.to_mesh(me)
        self.bm.free()
        obj = bpy.data.objects.new(name, me)
        bpy.context.collection.objects.link(obj)
        return obj


def _noise3(rng):
    """A smooth random 3D offset field from a few random sine waves."""
    waves = [(Vector((rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-1, 1))).normalized() * rng.uniform(40, 140), rng.uniform(0, 6.28), Vector((rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-1, 1))).normalized()) for _ in range(6)]

    def f(p):
        o = Vector()
        for k, ph, d in waves:
            o += d * math.sin(k.dot(p) + ph)
        return o / 6

    return f


def grow(skin, marks, seed=7):
    rng = random.Random(seed)
    nrng = np.random.default_rng(seed)
    centre, lo, hi, eye_z = _head_frame(skin, marks)
    me = skin.data
    bvh = BVHTree.FromObject(skin, bpy.context.evaluated_depsgraph_get())
    scalp = _scalp_faces(skin, centre)
    # The knot: behind and above the occiput.
    knot = Vector((0.004, hi.y - 0.012, centre.z + 0.045))
    knot_axis = Vector((0.15, 1.0, 0.35)).normalized()  # out of the back of the head
    knot_r = 0.028

    def surface(p):
        loc, nrm, _, _ = bvh.find_nearest(p)
        return loc, nrm

    def comb(root, rnrm, lift, mess, wander):
        """March from the root over the scalp toward the knot."""
        pts, nrms = [], []
        p = root.copy()
        side = 1.0 if root.x >= 0 else -1.0
        step = 0.004
        for k in range(120):
            loc, nrm = surface(p)
            to_knot = knot - loc
            dist = to_knot.length
            # Away from the part near it, then toward the knot.
            part_push = Vector((side, 0, 0)) * math.exp(-abs(loc.x) / 0.02) * 0.8
            d = to_knot.normalized() + part_push
            d = d - nrm * d.dot(nrm)
            if d.length < 1e-6:
                break
            d.normalize()
            t = k / 60
            off = wander(p * 1.0) * mess * min(1.0, t * 1.5)
            h = 0.0004 + lift * min(1.0, k * step / 0.03) ** 0.7 + max(0.0, off.dot(nrm)) * 0.004
            q = loc + nrm * h + (off - nrm * off.dot(nrm)) * 0.003
            pts.append(q)
            nrms.append(nrm)
            if dist < knot_r * 1.1:
                break
            p = loc + d * step
        return pts, nrms

    def wrap(start, turns, radius, rng_):
        """Loose loops around the knot from where the strand arrives, each
        strand on its own tilted axis so the knot reads as a messy twist."""
        a = (knot_axis + Vector((rng_.uniform(-0.6, 0.6), rng_.uniform(-0.3, 0.3), rng_.uniform(-0.6, 0.6)))).normalized()
        u = (start - knot)
        u = (u - a * u.dot(a))
        if u.length < 1e-6:
            u = a.orthogonal()
        u.normalize()
        v = a.cross(u)
        pts = []
        steps = int(turns * 28)
        ph0 = 0.0
        direction = rng_.choice((-1, 1))
        for k in range(steps):
            t = k / steps
            ang = ph0 + direction * 2 * math.pi * turns * t
            r = radius * (1 - 0.55 * t) * (1 + 0.15 * math.sin(7 * ang + rng_.uniform(0, 1)))
            out = 0.006 + 0.012 * math.sin(math.pi * t) + rng_.uniform(-0.002, 0.002)
            pts.append(knot + (u * math.cos(ang) + v * math.sin(ang)) * r + a * out)
        return pts

    body = Ribbons()
    wander = _noise3(rng)
    roots = _sample_surface(me, scalp, 9000, rng)
    # Extra roots along the hairline, so the edge is dense and soft.
    edge = [f for f in scalp if _angles(me.polygons[f].center, centre)[1] < _hairline_el(_angles(me.polygons[f].center, centre)[0]) + 0.12]
    roots += _sample_surface(me, edge, 1600, rng)
    for idx, (root, rn) in enumerate(roots):
        layer = rng.random()
        # A loose updo: the outer layers ride well off the scalp (up to
        # 2 cm over the sides), the inner ones close.
        lift = 0.0015 + 0.017 * layer**1.4
        pts, nrms = comb(root, rn, lift, mess=0.4 + 1.4 * layer**2, wander=wander)
        if len(pts) < 2:
            continue
        loops = wrap(pts[-1], rng.uniform(0.6, 1.8), knot_r * rng.uniform(0.6, 1.25), rng)
        all_pts = pts + loops
        all_n = nrms + [None] * len(loops)
        width = rng.uniform(0.0014, 0.0030) * (1.3 - 0.5 * layer)
        body.add(all_pts, all_n, width, rng.randrange(VARIANTS), taper=0.5, free=knot_axis)
    # Loose loops that stand off the bun, and strands through its middle.
    for k in range(520):
        start = knot + Vector((rng.uniform(-1, 1), rng.uniform(-0.3, 0.6), rng.uniform(-1, 1))).normalized() * knot_r * 0.9
        pts = wrap(start, rng.uniform(0.3, 0.9), knot_r * rng.uniform(1.0, 1.7), rng)
        body.add(pts, None, rng.uniform(0.0006, 0.0016), rng.randrange(VARIANTS), taper=0.7, free=knot_axis)

    # Face-framing locks and wisps: roots near the front hairline and the
    # temples, falling past the cheeks in loose waves.
    locks = Ribbons()
    # Locks leave the hairline at the temples (azimuth 0.45-1.25 rad from
    # the front), never the middle of the forehead, so they fall beside the
    # face rather than across it.
    front_roots = [(r, n) for (r, n) in _sample_surface(me, scalp, 8000, rng) if 0.45 < abs(_angles(r, centre)[0]) < 1.25 and _angles(r, centre)[1] < _hairline_el(_angles(r, centre)[0]) + 0.16]
    rng.shuffle(front_roots)
    clumps = []
    for side in (1, -1):
        cands = [fr for fr in front_roots if fr[0].x * side > 0.012]
        for k in range(9):
            if not cands:
                break
            root, rn = cands[k * 7 % len(cands)]
            clumps.append((root, rn, side))
    for root, rn, side in clumps:
        length = rng.uniform(0.11, 0.21)
        sway = rng.uniform(0.6, 1.4)
        phase = rng.uniform(0, 6.28)
        n_hairs = rng.randint(10, 26)
        spread = rng.uniform(0.002, 0.006)
        for h in range(n_hairs):
            off = Vector((rng.gauss(0, spread), rng.gauss(0, spread * 0.5), rng.gauss(0, spread * 0.5)))
            pts, nrms = [], []
            p = root + off * 0.3
            seg = 0.004
            n_seg = int(length * rng.uniform(0.75, 1.05) / seg)
            for k in range(n_seg):
                t = k / max(n_seg - 1, 1)
                loc, nrm = surface(p)
                # Lift off the head; fall down and slightly out from the cheek.
                d = Vector((side * (0.45 - 0.3 * t), 0.05, -1.0))
                wave = Vector((side, -0.4, 0)) * math.sin(phase + t * 9 * sway) * 0.5
                p = p + (d.normalized() + wave * 0.6) * seg
                loc, nrm = surface(p)
                if (p - loc).length < 0.003 + 0.004 * t or (p - loc).dot(nrm) < 0:
                    p = loc + nrm * (0.003 + 0.004 * t)
                pts.append(p + off * t * 1.5)
                nrms.append(None)
            locks.add(pts, None, rng.uniform(0.00025, 0.0006), rng.randrange(VARIANTS), taper=0.8, free=Vector((0, -1, 0)))
    # Wisps at the temples and hairline: short curls of a few hairs.
    for k in range(140):
        root, rn = front_roots[k % len(front_roots)]
        pts = []
        p = root.copy()
        d = (Vector((math.copysign(1, root.x) * 0.5, -0.3, -1.0)) + Vector((rng.uniform(-0.5, 0.5), rng.uniform(-0.3, 0.3), rng.uniform(-0.3, 0.6)))).normalized()
        L = rng.uniform(0.02, 0.07)
        for s in range(int(L / 0.003)):
            d = (d + Vector((rng.uniform(-0.25, 0.25), rng.uniform(-0.2, 0.2), rng.uniform(-0.25, 0.25)))).normalized()
            p = p + d * 0.003
            loc, nrm = surface(p)
            if (p - loc).dot(nrm) < 0.001:
                p = loc + nrm * 0.0015
            pts.append(p)
        locks.add(pts, None, rng.uniform(0.00015, 0.00035), rng.randrange(VARIANTS), taper=0.8)

    # Flyaways: single hairs arcing off the top and sides.
    fly = Ribbons()
    for root, rn in _sample_surface(me, scalp, 420, rng):
        pts = []
        loc, nrm = surface(root)
        p = loc + nrm * 0.008
        d = (nrm * rng.uniform(0.3, 1.0) + (knot - loc).normalized() * rng.uniform(-0.6, 0.6) + Vector((rng.uniform(-0.5, 0.5), rng.uniform(-0.5, 0.5), rng.uniform(-0.2, 0.5)))).normalized()
        L = rng.uniform(0.02, 0.08)
        for s in range(int(L / 0.003)):
            d = (d + Vector((0, 0, -0.06)) + Vector((rng.uniform(-0.12, 0.12), rng.uniform(-0.12, 0.12), rng.uniform(-0.12, 0.12)))).normalized()
            p = p + d * 0.003
            pts.append(p.copy())
        fly.add(pts, None, rng.uniform(0.00008, 0.00016), rng.randrange(VARIANTS), taper=0.5)

    # The cap: the scalp offset by 1.5 mm, in the hair's root colour.
    bm = bmesh.new()
    bm.from_mesh(me)
    keep = {f for f in scalp if _angles(me.polygons[f].center, centre)[1] > _hairline_el(_angles(me.polygons[f].center, centre)[0]) + 0.07}
    bmesh.ops.delete(bm, geom=[f for f in bm.faces if f.index not in keep], context="FACES")
    for v in bm.verts:
        v.co += v.normal * 0.0012
    cap_me = bpy.data.meshes.new("hair_cap")
    bm.to_mesh(cap_me)
    bm.free()
    cap = bpy.data.objects.new("hair_cap", cap_me)
    bpy.context.collection.objects.link(cap)
    # Cap UV: v = 0.15 so it samples the strip's root colour.
    for loop_uv in cap_me.uv_layers.active.data:
        loop_uv.uv = (loop_uv.uv.x % 1.0 * 0.999, 0.12)

    objs = [cap, body.object("hair_body"), locks.object("hair_locks"), fly.object("hair_fly")]
    img = paint_strips(1024, 2048)
    mat = hair_material(img)
    cap_mat = bpy.data.materials.new("draconia_hair_cap")
    cap_mat.use_nodes = True
    cap_mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.30, 0.26, 0.19, 1)
    cap_mat.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.95
    cap_mat["verse_shading"] = "standard"
    cap.data.materials.append(cap_mat)
    for o in objs[1:]:
        o.data.materials.append(mat)
        for p in o.data.polygons:
            p.use_smooth = True
    print("HAIR ribbons", body.count, locks.count, fly.count)
    return objs


def paint_strips(w, h):
    """Eight strand variants side by side, each a band of u: a few hairs per
    band with alpha gaps between them, root (v=0, image top) ash-beige,
    the length pale platinum with a lighter, thinner tip."""
    rng = np.random.default_rng(5)
    u = (np.arange(w) + 0.5) / w
    v = (np.arange(h) + 0.5) / h
    img = np.zeros((h, w, 4), np.float32)
    band = w // VARIANTS
    root = np.array([0.36, 0.30, 0.21])
    mid = np.array([0.66, 0.60, 0.47])
    tip = np.array([0.80, 0.76, 0.64])
    vv = v[:, None]
    grad = np.where(vv < 0.15, root + (mid - root) * (vv / 0.15), mid + (tip - mid) * np.clip((vv - 0.15) / 0.85, 0, 1) ** 1.5)
    for k in range(VARIANTS):
        x0 = k * band
        cols = np.zeros((h, band, 4), np.float32)
        hairs = rng.integers(7, 13)
        tone = rng.uniform(0.85, 1.12)
        for j in range(hairs):
            cx = rng.uniform(0.08, 0.92) * band
            wid = rng.uniform(0.7, 1.6) * band / 22
            drift = rng.uniform(-0.04, 0.04) * band
            ends = rng.uniform(0.82, 1.0)
            xs = np.arange(band)[None, :]
            centre = cx + drift * vv
            prof = np.clip(1 - np.abs(xs - centre) / wid, 0, 1)
            prof = prof * (vv < ends)
            shade = tone * rng.uniform(0.85, 1.15)
            cols[..., 3] = np.maximum(cols[..., 3], prof)
            cols[..., :3] = np.maximum(cols[..., :3], (grad * shade)[:, None, :] * (prof[..., None] > 0.01))
        img[:, x0 : x0 + band] = cols
    img[..., :3] = np.clip(img[..., :3], 0, 1)
    # Fill colour under transparent texels so filtering doesn't darken.
    empty = img[..., 3] < 0.01
    img[..., :3] = np.where(empty[..., None], grad[:, None, :] * 0.95, img[..., :3])
    srgb = np.where(img[..., :3] <= 0.0031308, img[..., :3] * 12.92, 1.055 * np.power(img[..., :3], 1 / 2.4) - 0.055)
    out = np.concatenate([srgb, img[..., 3:4]], 2)
    im = bpy.data.images.new("draconia_hair", w, h, alpha=True)
    im.pixels.foreach_set(np.ascontiguousarray(out[::-1]).astype(np.float32).ravel())
    path = os.path.join(os.path.expanduser("~/work/film-out/draconia/build/textures"), "draconia_hair.png")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    im.filepath_raw = path
    im.file_format = "PNG"
    im.save()
    return im


def hair_material(img):
    m = bpy.data.materials.new("draconia_hair")
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = img
    nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    nt.links.new(tex.outputs["Alpha"], bsdf.inputs["Alpha"])
    bsdf.inputs["Roughness"].default_value = 0.4
    try:
        m.blend_method = "CLIP"
        m.alpha_threshold = 0.35
    except AttributeError:
        pass
    m["verse_shading"] = "hair"
    return m
