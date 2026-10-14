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
    pts = [(0.0, 0.56), (0.35, 0.51), (0.75, 0.30), (1.05, 0.22), (1.35, 0.30), (1.6, 0.05), (2.0, -0.45), (2.6, -0.62), (math.pi, -0.66)]
    for (a0, e0), (a1, e1) in zip(pts, pts[1:]):
        if a <= a1:
            t = (a - a0) / (a1 - a0)
            return e0 + (e1 - e0) * t - _drop(a)
    return pts[-1][1] - _drop(a)


def _drop(a):
    """HAIRLINE_DROP at azimuth a: one value, or (front, temples and
    sides), blended over the first radian from the front."""
    if isinstance(HAIRLINE_DROP, tuple):
        f, sd = HAIRLINE_DROP
        return f + (sd - f) * min(1.0, a / 1.0)
    return HAIRLINE_DROP


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
        self.skin = []  # per vertex: [(joint, weight), ...]

    def add(self, pts, normals, width, variant, taper=0.6, free=None, skin=None):
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
            ws = skin(i, t) if skin else [("Head", 1.0)]
            self.skin += [ws, ws]
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
        groups = {}
        for i, ws in enumerate(self.skin):
            for j, w in ws:
                if j not in groups:
                    groups[j] = obj.vertex_groups.new(name=j)
                groups[j].add([i], w, "REPLACE")
        return obj


def chain_skin(links, t0=0.0):
    """Weights along a chain of joints `links` for a strand vertex at
    fraction t of its length: link k sits at fraction t0 + (1 - t0) k / n;
    a vertex before t0 rides the head."""

    def f(i, t):
        if t <= t0:
            return [("Head", 1.0)]
        n = len(links)
        x = (t - t0) / max(1 - t0, 1e-6) * n
        k = min(int(x), n - 1)
        frac = min(x - k, 1.0)
        if k + 1 < n:
            return [(links[k], 1 - frac), (links[k + 1], frac)]
        return [(links[k], 1.0)]

    return f


def _noise3(rng):
    """A smooth random 3D offset field from a few random sine waves."""
    waves = [(Vector((rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-1, 1))).normalized() * rng.uniform(40, 140), rng.uniform(0, 6.28), Vector((rng.uniform(-1, 1), rng.uniform(-1, 1), rng.uniform(-1, 1))).normalized()) for _ in range(6)]

    def f(p):
        o = Vector()
        for k, ph, d in waves:
            o += d * math.sin(k.dot(p) + ph)
        return o / 6

    return f


def grow(skin, marks, seed=7, style="updo"):
    """The hair objects and its extra joints [(name, parent, position)]."""
    if style == "long":
        return grow_long(skin, marks, seed)
    rng = random.Random(seed)
    nrng = np.random.default_rng(seed)
    centre, lo, hi, eye_z = _head_frame(skin, marks)
    me = skin.data
    bvh = BVHTree.FromObject(skin, bpy.context.evaluated_depsgraph_get())
    scalp = _scalp_faces(skin, centre)
    # The knot: behind and above the occiput.
    knot = Vector((0.004, hi.y - 0.022, centre.z + 0.07))
    knot_axis = Vector((0.15, 1.0, 0.35)).normalized()  # out of the back of the head
    knot_r = 0.034

    def surface(p):
        loc, nrm, _, _ = bvh.find_nearest(p)
        return loc, nrm

    def comb(root, rnrm, lift, mess, wander):
        """March from the root over the scalp toward the knot."""
        pts, nrms = [], []
        p = root.copy()
        # A messy part: strands near the middle pick their side with some
        # randomness (PART_JITTER, metres), so the line wanders.
        side = 1.0 if root.x + rng.gauss(0, PART_JITTER) >= 0 else -1.0
        step = 0.004
        # Curtains (round 15): strands rooted toward the front first fall
        # sideways and down from the part over the temple, to a point above
        # the ear, and only then turn back to the knot (the reference's loose
        # centre-parted updo), instead of combing straight back.
        az0 = abs(_angles(root, centre)[0])
        via = None
        if CURTAIN and az0 < CURTAIN_AZ:
            via = Vector((centre.x + side * CURTAIN_X, centre.y + CURTAIN_Y + rng.uniform(-0.01, 0.01), centre.z + CURTAIN_Z + rng.uniform(-0.012, 0.012)))
        for k in range(120 + (60 if via else 0)):
            loc, nrm = surface(p)
            if via is not None and (abs(loc.x - centre.x) > abs(via.x - centre.x) - 0.035 and loc.z < via.z + 0.02):
                via = None
            to_knot = (via if via is not None else knot) - loc
            dist = to_knot.length if via is None else (knot - loc).length
            # Away from the part near it, then toward the knot.
            part_push = Vector((side, 0, 0)) * math.exp(-abs(loc.x) / 0.02) * PART
            d = to_knot.normalized() * (1.0 if via is None else CURTAIN) + part_push
            d = d - nrm * d.dot(nrm)
            if d.length < 1e-6:
                break
            d.normalize()
            t = k / 60
            off = wander(p * 1.0) * mess * min(1.0, t * 1.5)
            # Loose, not slicked: the hair puffs out over the sides and the
            # crown (most at mid-path, over the ears), and each strand
            # waves along its own clump's offset.
            along = min(1.0, k * step / max(dist + k * step, 1e-3))
            puff = PUFF * (0.006 + 0.012 * max(0.0, 1.0 - abs(loc.z - (centre.z - 0.01)) / 0.08)) * math.sin(math.pi * min(along * 1.4, 1.0))
            h = 0.0004 + lift * min(1.0, k * step / 0.03) ** 0.7 + puff * (0.3 + 0.7 * mess / 1.8) + max(0.0, off.dot(nrm)) * 0.006
            q = loc + nrm * h + (off - nrm * off.dot(nrm)) * 0.008
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
    roots = _sample_surface(me, scalp, BODY_ROOTS, rng)
    # Extra roots along the hairline, so the edge is dense and soft.
    edge = [f for f in scalp if _angles(me.polygons[f].center, centre)[1] < _hairline_el(_angles(me.polygons[f].center, centre)[0]) + 0.12]
    roots += _sample_surface(me, edge, EDGE_ROOTS, rng)
    for idx, (root, rn) in enumerate(roots if "body" in PARTS else []):
        layer = rng.random()
        # A loose updo: the outer layers ride well off the scalp (up to
        # 2 cm over the sides), the inner ones close.
        lift = 0.0015 + LIFT * layer**1.4
        pts, nrms = comb(root, rn, lift, mess=MESS[0] + MESS[1] * layer**2, wander=wander)
        if len(pts) < 2:
            continue
        loops = wrap(pts[-1], rng.uniform(0.6, 1.8), knot_r * rng.uniform(0.6, 1.25), rng)
        all_pts = pts + loops
        all_n = nrms + [None] * len(loops)
        width = rng.uniform(0.0009, 0.0019) * (1.3 - 0.5 * layer) * BODY_WIDTH
        body.add(all_pts, all_n, width, rng.randrange(VARIANTS), taper=0.5, free=knot_axis)
    # Loose loops that stand off the bun, and strands through its middle.
    for k in range(520 if "body" in PARTS else 0):
        start = knot + Vector((rng.uniform(-1, 1), rng.uniform(-0.3, 0.6), rng.uniform(-1, 1))).normalized() * knot_r * 0.9
        pts = wrap(start, rng.uniform(0.3, 0.9), knot_r * rng.uniform(1.0, 1.7), rng)
        body.add(pts, None, rng.uniform(0.0006, 0.0016), rng.randrange(VARIANTS), taper=0.7, free=knot_axis)

    # Face-framing locks and wisps: roots near the front hairline and the
    # temples, falling past the cheeks in loose waves.
    locks = Ribbons()
    # Locks leave the hairline at the temples (azimuth 0.45-1.25 rad from
    # the front), never the middle of the forehead, so they fall beside the
    # face rather than across it.
    front_roots = [(r, n) for (r, n) in _sample_surface(me, scalp, 8000, rng) if LOCK_AZ[0] < abs(_angles(r, centre)[0]) < LOCK_AZ[1] and _angles(r, centre)[1] < _hairline_el(_angles(r, centre)[0]) + 0.16]
    rng.shuffle(front_roots)
    clumps = []
    for side in (1, -1):
        cands = [fr for fr in front_roots if fr[0].x * side > 0.012]
        # Many loose locks: the reference's updo lets strands fall from the
        # temples and the front hairline past the cheeks to the jaw.
        for k in range(LOCKS_PER_SIDE):
            if not cands:
                break
            root, rn = cands[k * 7 % len(cands)]
            clumps.append((root, rn, side))
    joints = []
    for c, (root, rn, side) in enumerate(clumps):
        length = rng.uniform(*LOCK_LENGTH)
        links = [f"hair_{c}_{k}" for k in range(5)]
        chain_made = False
        sway = rng.uniform(0.6, 1.4)
        phase = rng.uniform(0, 6.28)
        n_hairs = rng.randint(*LOCK_HAIRS)
        spread = rng.uniform(0.001, 0.003)
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
                d = Vector((side * (LOCK_OUT - 0.35 * LOCK_OUT / 0.6 * t), 0.12, -1.0))
                wave = Vector((side, -0.4, 0)) * math.sin(phase + t * 13 * sway) * 0.8 * LOCK_WAVE
                p = p + (d.normalized() + wave * 0.6) * seg
                loc, nrm = surface(p)
                if (p - loc).length < 0.003 + 0.004 * t or (p - loc).dot(nrm) < 0:
                    p = loc + nrm * (0.003 + 0.004 * t)
                curl = Vector()
                if LOCK_CURL:
                    a_ = phase + t * LOCK_CURL_TURNS * 2 * math.pi
                    curl = Vector((math.cos(a_), -math.sin(a_) * 0.6, math.sin(a_) * 0.4)) * LOCK_CURL * t**1.5
                pts.append(p + off * t * 1.5 + curl)
                nrms.append(None)
            if not chain_made and len(pts) > 5:
                # The clump's first hair is its guide: five links along it.
                chain_made = True
                L = len(pts)
                for k, name in enumerate(links):
                    at = pts[min(int(k / 5 * (L - 1)), L - 1)]
                    joints.append((name, "Head" if k == 0 else links[k - 1], at.copy()))
            locks.add(pts, None, rng.uniform(0.00025, 0.0006), rng.randrange(VARIANTS), taper=0.75, free=Vector((0, -1, 0)), skin=chain_skin(links) if chain_made else None)
    # Wisps at the temples and hairline: short curls of a few hairs.
    for k in range(WISPS):
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
    for root, rn in _sample_surface(me, scalp, FLY_COUNT if "fly" in PARTS else 0, rng):
        pts = []
        loc, nrm = surface(root)
        p = loc + nrm * 0.008
        d = (nrm * rng.uniform(0.3, 1.0) + (knot - loc).normalized() * rng.uniform(-0.6, 0.6) + Vector((rng.uniform(-0.5, 0.5), rng.uniform(-0.5, 0.5), rng.uniform(-0.2, 0.5)))).normalized()
        L = rng.uniform(*FLY_LENGTH)
        for s in range(int(L / 0.003)):
            d = (d + Vector((0, 0, -FLY_DROOP)) + Vector((rng.uniform(-0.12, 0.12), rng.uniform(-0.12, 0.12), rng.uniform(-0.12, 0.12)))).normalized()
            p = p + d * 0.003
            pts.append(p.copy())
        fly.add(pts, None, rng.uniform(0.00008, 0.00016), rng.randrange(VARIANTS), taper=0.5)

    # The cap: the scalp offset by 1.5 mm, in the hair's root colour.
    bm = bmesh.new()
    bm.from_mesh(me)
    # Cut the scalp's faces finer first, so the cap's edge follows the
    # hairline instead of stepping along big polygons.
    scalp_set = set(scalp)
    near = [f for f in bm.faces if f.index in scalp_set or _angles(f.calc_center_median(), centre)[1] > _hairline_el(_angles(f.calc_center_median(), centre)[0]) - 0.1]
    edges = list({e for f in near for e in f.edges})
    bmesh.ops.subdivide_edges(bm, edges=edges, cuts=2, use_grid_fill=True)
    bm.faces.ensure_lookup_table()
    drop = []
    for f in bm.faces:
        c = f.calc_center_median()
        az, el = _angles(c, centre)
        if el <= _hairline_el(az) + CAP_INSET:
            drop.append(f)
    bmesh.ops.delete(bm, geom=drop, context="FACES")
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

    if "cap" not in PARTS:
        bpy.data.objects.remove(cap)
        cap = None
    objs = ([cap] if cap else []) + [r.object(n) for r, n in ((body, "hair_body"), (locks, "hair_locks"), (fly, "hair_fly")) if r.count]
    img = paint_strips(1024, 2048)
    mat = hair_material(img)
    cap_mat = bpy.data.materials.new("draconia_hair_cap")
    cap_mat.use_nodes = True
    cap_mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (*(c * 0.8 for c in HAIR_ROOT), 1) if CAP_FROM_MID is None else (*(c * CAP_FROM_MID for c in HAIR_MID), 1)
    cap_mat.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.95
    cap_mat["verse_shading"] = "standard"
    if cap:
        cap.data.materials.append(cap_mat)
    for o in objs[1 if cap else 0 :]:
        o.data.materials.append(mat)
        for p in o.data.polygons:
            p.use_smooth = True
    print("HAIR ribbons", body.count, locks.count, fly.count, "joints", len(joints))
    return objs, joints


def grow_long(skin, marks, seed=7, guides=18, links=6):
    """Waist-length, loose, straight platinum hair with a centre part and a
    slight wave at the ends (the battle and peacock look in
    docs/verse/draconia/character.md), on joint chains.

    The hair is combed from the part down over the scalp to a release ring
    around the head (from the temples round the back, about ear level),
    then falls free. `guides` chains of `links` joints hang from the ring
    around the head, each a guide curve that drapes over the shoulders and
    back to the waist; every strand follows the two guides beside it,
    weighted by azimuth, so wind on the chains moves the whole fall."""
    rng = random.Random(seed)
    centre, lo, hi, eye_z = _head_frame(skin, marks)
    me = skin.data
    bvh = BVHTree.FromObject(skin, bpy.context.evaluated_depsgraph_get())
    scalp = _scalp_faces(skin, centre)
    waist_z = centre.z - 0.62

    def surface(p):
        loc, nrm, _, _ = bvh.find_nearest(p)
        return loc, nrm

    # Guides: azimuths from one temple round the back to the other.
    azs = [(-1.25 + 2.5 * g / (guides - 1)) for g in range(guides)]
    azs = [math.copysign(abs(a) + 0.0, a) for a in azs]
    ring_el = -0.15
    guide_pts, joints = [], []
    for g, az in enumerate(azs):
        # Seen from the head centre: az 0 at the back here (she faces -Y),
        # so the ring runs from her right temple round the back.
        a = math.pi - az * 1.1 if az >= 0 else -math.pi - az * 1.1
        d = Vector((math.sin(a), -math.cos(a), 0)).normalized()
        start = centre + d * 0.11 + Vector((0, 0, -0.02))
        loc, nrm = surface(start)
        p = loc + nrm * 0.012
        pts = [p.copy()]
        out = Vector((d.x, d.y, 0)).normalized()
        end = waist_z + rng.uniform(-0.03, 0.03)
        for _ in range(400):
            if p.z <= end:
                break
            # Fall, held off the head by its skin while beside it, then off
            # the shoulders and back by a body capsule (the skin mesh stops
            # at the collar, so it can't hold the fall below the neck).
            p = p + Vector((0, 0, -0.012)) + out * 0.0012
            if p.z > centre.z - 0.11:
                loc, nrm = surface(p)
                if (p - loc).dot(nrm) < 0.012:
                    p = loc + nrm * 0.012
                    p.z = min(p.z, pts[-1].z - 0.004)
            # Over the shoulders and back: keep outside a body capsule.
            r_xy = Vector((p.x, p.y - 0.01, 0))
            body_r = 0.09 if p.z > centre.z - 0.17 else 0.15
            if p.z < centre.z - 0.17 and r_xy.length < body_r:
                p.xy = (r_xy.normalized() * body_r).xy + Vector((0, 0.01))
            pts.append(p.copy())
        guide_pts.append(pts)
        names = [f"hair_{g}_{k}" for k in range(links)]
        for k, name in enumerate(names):
            at = pts[min(int(k / links * (len(pts) - 1)), len(pts) - 1)]
            joints.append((name, "Head" if k == 0 else names[k - 1], at.copy()))

    hair = Ribbons()
    roots = _sample_surface(me, scalp, 8000, rng)
    # Extra roots along the front hairline and temples, so no scalp shows.
    edge = [f for f in scalp if _angles(me.polygons[f].center, centre)[1] < _hairline_el(_angles(me.polygons[f].center, centre)[0]) + 0.18 and abs(_angles(me.polygons[f].center, centre)[0]) < 1.6]
    roots += _sample_surface(me, edge, 2500, rng)
    for root, rn in roots:
        az, el = _angles(root, centre)
        # Which guides: the strand's azimuth, folded to the ring's range.
        back_az = math.atan2(root.x, root.y - centre.y)  # 0 straight back
        x = (back_az / 1.1 + 1.25) / 2.5 * (guides - 1)
        x = min(max(x, 0.0), guides - 1.0001)
        g0 = int(x)
        f = x - g0
        g1 = min(g0 + 1, guides - 1)
        # Comb from the root down over the scalp to the ring point.
        target = guide_pts[g0][0].lerp(guide_pts[g1][0], f)
        pts = []
        p = root.copy()
        layer = rng.random()
        # Volume over the crown and temples: outer layers stand off.
        lift = 0.0015 + 0.011 * layer**1.3
        for k in range(40):
            loc, nrm = surface(p)
            to = target - loc
            if to.length < 0.009:
                break
            d = to - nrm * to.dot(nrm)
            d.normalize()
            pts.append(loc + nrm * (0.0006 + lift * min(1.0, k / 4)))
            p = loc + d * 0.007
        n_scalp = len(pts)
        # Then the fall: between the two guides, with a per-strand offset
        # and a slight wave toward the ends.
        off = Vector((rng.gauss(0, 0.01), rng.gauss(0, 0.01), 0)) * (0.4 + layer)
        phase = rng.uniform(0, 6.28)
        A, B = guide_pts[g0], guide_pts[g1]
        n = min(len(A), len(B))
        length_frac = rng.uniform(0.82, 1.0)
        for k in range(int(n * length_frac)):
            t = k / max(n - 1, 1)
            q = A[k].lerp(B[k], f) + off * (0.3 + t) + Vector((math.sin(phase + t * 14) * 0.006 * t * t, 0, 0))
            pts.append(q)
        if len(pts) < 3:
            continue
        t0 = n_scalp / len(pts)
        la = [f"hair_{g0}_{k}" for k in range(links)]
        lb = [f"hair_{g1}_{k}" for k in range(links)]
        fa, fb = chain_skin(la, t0), chain_skin(lb, t0)

        def skin_fn(i, t, fa=fa, fb=fb, f=f):
            out = {}
            for j, w in fa(i, t):
                out[j] = out.get(j, 0) + w * (1 - f)
            for j, w in fb(i, t):
                out[j] = out.get(j, 0) + w * f
            return sorted(out.items(), key=lambda kv: -kv[1])[:4]

        hair.add(pts, None, rng.uniform(0.002, 0.0038) * (1.2 - 0.4 * layer), rng.randrange(VARIANTS), taper=0.6, free=Vector((0, -1, 0)), skin=skin_fn)
    # Strands across her forehead and between her eyes: a few clumps from
    # the front hairline beside the part, over the brow, hanging in front
    # of the face to the nose, each on a chain of its own.
    front = [(r, n) for (r, n) in _sample_surface(me, scalp, 6000, rng) if abs(_angles(r, centre)[0]) < 0.6 and _angles(r, centre)[1] < _hairline_el(_angles(r, centre)[0]) + 0.2]
    nose_z = marks.nose_tip.z if hasattr(marks, "nose_tip") else eye_z - 0.04
    g = guides
    for c in range(26):
        if not front:
            break
        root, rn = front[(c * 37) % len(front)]
        # A side-swept fringe: most clumps sweep across the forehead toward
        # her left and stop at the brows; two fall between the eyes to the
        # nose. They ride the head (no chain): a fringe holds its shape.
        to_nose = c in (3, 11)
        stop_z = nose_z + 0.01 if to_nose else eye_z + 0.012 + rng.uniform(-0.004, 0.01)
        lean = (rng.uniform(-0.2, 0.2) if to_nose else rng.uniform(0.6, 1.1))
        for h in range(rng.randint(12, 20)):
            off = Vector((rng.gauss(0, 0.003), 0, 0))
            p = root + off
            pts = []
            for k in range(70):
                p = p + Vector((lean * 0.0025, -0.0012, -0.0035))
                loc, nrm = surface(p)
                clear = (0.0025 + 0.004 * (h % 3) / 2) if p.z > eye_z + 0.012 else 0.01
                if (p - loc).dot(nrm) < clear:
                    p = loc + nrm * clear
                pts.append(p.copy())
                if p.z < stop_z:
                    break
            hair.add(pts, None, rng.uniform(0.0008, 0.0018) if not to_nose else rng.uniform(0.0003, 0.0007), rng.randrange(VARIANTS), taper=0.7, free=Vector((0, -1, 0)))
    # Hair framing the temples: locks from the temple hairline falling in
    # front of the ear tops to the jaw, riding the head so they hold.
    temples = [(r, n) for (r, n) in _sample_surface(me, scalp, 6000, rng) if 0.8 < abs(_angles(r, centre)[0]) < 1.5 and _angles(r, centre)[1] < _hairline_el(_angles(r, centre)[0]) + 0.25]
    for c in range(16):
        if not temples:
            break
        root, rn = temples[(c * 53) % len(temples)]
        side = 1.0 if root.x > 0 else -1.0
        for h in range(rng.randint(10, 18)):
            off = Vector((0, rng.gauss(0, 0.004), rng.gauss(0, 0.002)))
            p = root + off
            pts = []
            stop = eye_z - rng.uniform(0.03, 0.09)
            for k in range(60):
                p = p + Vector((side * 0.0006, 0.0006, -0.004))
                loc, nrm = surface(p)
                clear = 0.003 + 0.002 * (h % 3)
                if (p - loc).dot(nrm) < clear:
                    p = loc + nrm * clear
                pts.append(p.copy())
                if p.z < stop:
                    break
            hair.add(pts, None, rng.uniform(0.0008, 0.0016), rng.randrange(VARIANTS), taper=0.75, free=Vector((side, 0, 0)))
    # The cap under it.
    bm = bmesh.new()
    bm.from_mesh(me)
    # Cut the scalp's faces finer first, so the cap's edge follows the
    # hairline instead of stepping along big polygons.
    scalp_set = set(scalp)
    near = [f for f in bm.faces if f.index in scalp_set or _angles(f.calc_center_median(), centre)[1] > _hairline_el(_angles(f.calc_center_median(), centre)[0]) - 0.1]
    edges = list({e for f in near for e in f.edges})
    bmesh.ops.subdivide_edges(bm, edges=edges, cuts=2, use_grid_fill=True)
    bm.faces.ensure_lookup_table()
    drop = []
    for f in bm.faces:
        c = f.calc_center_median()
        az, el = _angles(c, centre)
        if el <= _hairline_el(az) + CAP_INSET:
            drop.append(f)
    bmesh.ops.delete(bm, geom=drop, context="FACES")
    for v in bm.verts:
        v.co += v.normal * 0.0012
    cap_me = bpy.data.meshes.new("hair_cap")
    bm.to_mesh(cap_me)
    bm.free()
    cap = bpy.data.objects.new("hair_cap", cap_me)
    bpy.context.collection.objects.link(cap)
    cap_mat = bpy.data.materials.new("draconia_hair_cap")
    cap_mat.use_nodes = True
    cap_mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.42, 0.37, 0.27, 1)
    cap_mat.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.9
    cap_mat["verse_shading"] = "standard"
    cap.data.materials.append(cap_mat)
    body = hair.object("hair_long")
    body.data.materials.append(hair_material(paint_strips(1024, 2048)))
    for p in body.data.polygons:
        p.use_smooth = True
    print("HAIR long ribbons", hair.count, "joints", len(joints))
    return [cap, body], joints


# The strand colours (linear): ash roots, pale platinum lengths, lighter
# tips. A caller may set them, and the strip texture's path, before growing.
HAIR_ROOT = (0.36, 0.30, 0.21)
HAIR_MID = (0.66, 0.60, 0.47)
HAIR_TIP = (0.80, 0.76, 0.64)
STRIPS_PATH = os.path.expanduser("~/work/film-out/draconia/build/textures/draconia_hair.png")
# Face-framing locks: per side, hairs per lock, and length range (m).
LOCKS_PER_SIDE = 9
LOCK_HAIRS = (3, 8)
LOCK_LENGTH = (0.08, 0.2)
# How far the locks lean out from the cheek as they leave the head, and a
# curl toward their ends (radius in metres at the tip, turns over the lock).
LOCK_OUT = 0.6
LOCK_CURL = 0.0
LOCK_CURL_TURNS = 2.0
# The share of a strand's length (from the root) that shades from the root
# colour to the lengths'.
ROOT_SPAN = 0.15
# Scale of the updo body's ribbon widths (wider ribbons cover the scalp
# with fewer gaps, so the crown averages smooth rather than grainy).
BODY_WIDTH = 1.0
# How many strands the combed body grows (fewer, wider ones read as clumped
# sections with gaps between them).
BODY_ROOTS = 14000
# The centre part: how hard strands near it comb away, and how much the
# line wanders (the side a root falls to is decided with this much noise).
PART = 0.8
PART_JITTER = 0.0
# Curtains from the centre part (0: off): roots within CURTAIN_AZ radians
# of the front first comb to a point CURTAIN_X out from the head's centre,
# CURTAIN_Y forward and CURTAIN_Z up (m; above the ear), then to the knot;
# CURTAIN weighs that pull against the part's push.
CURTAIN = 0.0
CURTAIN_AZ = 1.0
CURTAIN_X = 0.11
CURTAIN_Y = 0.0
CURTAIN_Z = 0.01
# Which parts to grow: the combed body (and its bun), the face-framing
# locks and wisps, the flyaways (the cap always). With MetaHuman's own
# hair cards on the head, only the locks are grown, as extra wisps.
PARTS = ("cap", "body", "locks", "fly")
# The locks' wave amplitude (1: the original loose waves; small: nearly
# straight, fine wisps).
LOCK_WAVE = 1.0
# Where the locks root, as azimuth from the front (radians): the temples by
# default; lower bounds near 0.4 bring strands to the front of the face.
LOCK_AZ = (0.7, 1.35)
# Short wisps and baby hairs along the hairline (from the locks' roots).
WISPS = 320
# The updo's body: how far the outer layers stand off the scalp (m), how
# much the strands puff mid-path and wander (frizz), the hairline's drop
# (radians, lower is further down the forehead and temples), extra roots
# along the hairline, and the flyaways' count and droop.
LIFT = 0.017
PUFF = 1.0
MESS = (0.4, 1.4)
HAIRLINE_DROP = 0.0
EDGE_ROOTS = 1600
FLY_COUNT = 900
FLY_DROOP = 0.06
FLY_LENGTH = (0.02, 0.08)
# The cap under the strands: how far behind the hairline it starts
# (radians) and its colour as a share of the roots' (None) or the
# lengths' colour.
CAP_INSET = 0.07
CAP_FROM_MID = None


def paint_strips(w, h):
    """Eight strand variants side by side, each a band of u: a few hairs per
    band with alpha gaps between them, root (v=0, image top) ash-beige,
    the length pale platinum with a lighter, thinner tip."""
    rng = np.random.default_rng(5)
    u = (np.arange(w) + 0.5) / w
    v = (np.arange(h) + 0.5) / h
    img = np.zeros((h, w, 4), np.float32)
    band = w // VARIANTS
    root = np.array(HAIR_ROOT)
    mid = np.array(HAIR_MID)
    tip = np.array(HAIR_TIP)
    vv = v[:, None]
    rs = ROOT_SPAN
    grad = np.where(vv < rs, root + (mid - root) * (vv / rs), mid + (tip - mid) * np.clip((vv - rs) / (1 - rs), 0, 1) ** 1.5)
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
    path = STRIPS_PATH
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
