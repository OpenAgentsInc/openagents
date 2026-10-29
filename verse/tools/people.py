"""A realm's people (manifest "people"): skinned characters put together from
the Quaternius Universal Base Characters, Modular Character Outfits - Fantasy
and Universal Animation Library glTFs, for verse/tools/pack_realm.py.

Every part (an outfit, a head cut from a base body, a hairstyle, eyebrows,
a beard) is rebound onto one of two skeletons, male or female (the outfits'
own: the same 65 joints in the same order), so a realm can draw any head and
hair on any outfit and play one set of clips on all of them:

  - a vertex is moved to where its own skin puts it in the target
    skeleton's rest pose (sum of weight x target rest x the part's inverse
    bind), and from then on is skinned with the target's inverse binds;
  - a head is the base body's triangles whose every vertex is mostly the
    head's or the neck's (the outfits' readme: use only the head);
  - the clips (Universal Animation Library 1 and 2) keep their joints'
    rotations; of the translations only the pelvis's is kept, as its change
    from the library's rest added to each skeleton's own, so a clip keeps
    every bone's length on either body.

A part's "drop_small" (metres) leaves out its pieces smaller than that every
way (the studs on a boot), and its "keep" (0 to 1) thins its triangles to that share (decimate()).

Keys are thinned where the straight line (the arc, for a rotation) between
the keys either side stays within KEY_TOLERANCE; a channel that never moves
from the skeleton's rest has none.

Each primitive's base-colour image is a texture layer. A layer can be cut to
the part of the image its part uses ("crop": material -> padding; the UVs'
bounds, padded) and its UVs moved to match, so a face keeps its detail in a
512 layer. An image named in "variants" brings its other colourway as the
layer after it, which a realm picks per person. "kinds" names what each
material's texels are (KINDS below), so a realm can tint skin and hair and dye
cloth per person without more layers.

A vertex is 24 bytes (vertex() below). A material named in "clamp" may have
UVs past the image's edge (the base bodies' eyes); they keep to its edge.
"""
import hashlib, io, json, math, os, struct
from pathlib import Path

CT = {5120: "b", 5121: "B", 5122: "h", 5123: "H", 5125: "I", 5126: "f"}
NC = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}
KEY_TOLERANCE_T = 0.0008       # metres
KEY_TOLERANCE_R = 0.0015       # radians of arc
DYE_MARK = 0x80


def load(path):
    p = Path(os.path.expanduser(path))
    b = p.read_bytes()
    if b[:4] == b"glTF":
        n = struct.unpack_from("<I", b, 12)[0]
        g = json.loads(b[20:20 + n])
        o = 20 + n
        bins = [b[o + 8:o + 8 + struct.unpack_from("<I", b, o)[0]]]
    else:
        g = json.loads(b)
        bins = [(p.parent / x["uri"]).read_bytes() for x in g["buffers"]]
    def acc(i):
        a = g["accessors"][i]; v = g["bufferViews"][a["bufferView"]]
        n, c = NC[a["type"]], CT[a["componentType"]]
        size = struct.calcsize(c)
        stride = v.get("byteStride", n * size); off = v.get("byteOffset", 0) + a.get("byteOffset", 0)
        buf = bins[v["buffer"]]
        out = []
        for k in range(a["count"]):
            t = struct.unpack_from(f"<{n}{c}", buf, off + k * stride)
            if a.get("normalized"):
                d = {"B": 255, "H": 65535, "b": 127, "h": 32767}[c]
                t = tuple(x / d for x in t)
            out.append(t if n > 1 else t[0])
        return out
    g["acc"] = acc
    g["dir"] = p.parent
    return g


# ---- small maths: quaternions (x, y, z, w) and 4x4 matrices as rows

def qmul(a, b):
    ax, ay, az, aw = a; bx, by, bz, bw = b
    return (aw*bx + ax*bw + ay*bz - az*by, aw*by - ax*bz + ay*bw + az*bx,
            aw*bz + ax*by - ay*bx + az*bw, aw*bw - ax*bx - ay*by - az*bz)
def qunit(q):
    l = math.sqrt(sum(x * x for x in q)); return tuple(x / l for x in q) if l > 1e-9 else (0, 0, 0, 1)
def qangle(a, b):
    d = min(1.0, abs(sum(x * y for x, y in zip(a, b)))); return 2 * math.acos(d)
def slerp(a, b, f):
    d = sum(x * y for x, y in zip(a, b))
    if d < 0: b = tuple(-x for x in b); d = -d
    if d > 0.9995: return qunit(tuple(x + (y - x) * f for x, y in zip(a, b)))
    th = math.acos(d); k = 1 / math.sin(th)
    return tuple((x * math.sin(th * (1 - f)) + y * math.sin(th * f)) * k for x, y in zip(a, b))
def mat_tr(t, q):
    x, y, z, w = q
    return [[1 - 2*(y*y + z*z), 2*(x*y - z*w), 2*(x*z + y*w), t[0]],
            [2*(x*y + z*w), 1 - 2*(x*x + z*z), 2*(y*z - x*w), t[1]],
            [2*(x*z - y*w), 2*(y*z + x*w), 1 - 2*(x*x + y*y), t[2]],
            [0, 0, 0, 1]]
def mmul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]
def col_major(m16):
    return [[m16[c * 4 + r] for c in range(4)] for r in range(4)]
def apply(m, v, w=1.0):
    return tuple(m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2] + m[i][3] * w for i in range(3))


class Skeleton:
    """A glTF skin: joint names, parents (parents first), rest t and q, inverse binds, rest world matrices."""
    def __init__(self, g):
        s = g["skins"][0]
        J = s["joints"]
        self.names = [g["nodes"][j]["name"] for j in J]
        parent_of = {}
        for i, n in enumerate(g["nodes"]):
            for c in n.get("children", []):
                parent_of[c] = i
        self.parent = []
        for k, j in enumerate(J):
            p = parent_of.get(j)
            self.parent.append(J.index(p) if p in J else -1)
            if p is not None and p not in J:
                a = g["nodes"][p]
                if any(k in a for k in ("translation", "rotation", "scale", "matrix")):
                    raise SystemExit(f"{self.names[k]}: the armature above the skeleton is not the identity")
        for k, p in enumerate(self.parent):
            if p >= k:
                raise SystemExit("a skeleton's joints aren't parents first")
        self.t = [tuple(g["nodes"][j].get("translation", (0, 0, 0))) for j in J]
        self.q = [qunit(tuple(g["nodes"][j].get("rotation", (0, 0, 0, 1)))) for j in J]
        for j in J:
            if max(abs(v - 1) for v in g["nodes"][j].get("scale", (1, 1, 1))) > 1e-4:
                raise SystemExit("a joint's rest scale isn't 1")
        self.ib = [col_major(m) for m in g["acc"](s["inverseBindMatrices"])]
        self.world = []
        for k in range(len(J)):
            m = mat_tr(self.t[k], self.q[k])
            self.world.append(m if self.parent[k] < 0 else mmul(self.world[self.parent[k]], m))
    def index(self, name):
        return self.names.index(name)


def quantize_weights(ws):
    """Four weights as u8 255ths summing to 255 (largest remainders)."""
    total = sum(ws) or 1.0
    raw = [w / total * 255 for w in ws]
    q = [int(math.floor(x)) for x in raw]
    order = sorted(range(4), key=lambda i: raw[i] - q[i], reverse=True)
    for i in order[:255 - sum(q)]:
        q[i] += 1
    return q


def part_mesh(spec, skeletons):
    """A part rebound onto its skeleton: per primitive, its vertices (position, normal, uv,
    joints, weights in the target's numbering), triangles and base-colour image. A part may
    take meshes from more than one file ("also": [{file, meshes}]); primitives sharing an
    image and material are one."""
    prims = part_file(spec, spec["file"], spec.get("meshes"), skeletons)
    for more in spec.get("also", []):
        prims += part_file(spec, more["file"], more.get("meshes"), skeletons)
    merged = {}
    for pr in prims:
        key = (str(pr["image"]), pr["material"])
        if key in merged:
            m = merged[key]; n = len(m["verts"])
            m["verts"] = m["verts"] + pr["verts"]; m["tris"] = m["tris"] + [tuple(x + n for x in t) for t in pr["tris"]]
        else:
            merged[key] = pr
    return list(merged.values())


def drop_small(verts, tris, size):
    """Without its pieces (triangles joined by shared positions) smaller than size metres every way."""
    parent = list(range(len(verts)))
    def root(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]; x = parent[x]
        return x
    seen = {}
    for i, v in enumerate(verts):
        k = tuple(round(c, 4) for c in v["p"])
        if k in seen: parent[root(i)] = root(seen[k])
        else: seen[k] = i
    for t in tris:
        parent[root(t[1])] = root(t[0]); parent[root(t[2])] = root(t[0])
    lo, hi = {}, {}
    for i, v in enumerate(verts):
        r = root(i)
        lo[r] = [min(a, b) for a, b in zip(lo.get(r, v["p"]), v["p"])]
        hi[r] = [max(a, b) for a, b in zip(hi.get(r, v["p"]), v["p"])]
    small = {r for r in lo if max(h - l for h, l in zip(hi[r], lo[r])) < size}
    tris = [t for t in tris if root(t[0]) not in small]
    used = sorted({x for t in tris for x in t})
    renum = {x: k for k, x in enumerate(used)}
    return [verts[x] for x in used], [tuple(renum[x] for x in t) for t in tris]


def part_file(spec, path, meshes, skeletons):
    g = load(path)
    target = skeletons[spec["body"]]
    own = Skeleton(g)
    remap = [target.index(n) for n in own.names]
    to_target = [mmul(target.world[remap[k]], own.ib[k]) for k in range(len(own.names))]
    keep_joints = {target.index(n) for n in spec.get("cut", [])}
    prims = []
    for mesh in g["meshes"]:
        if meshes and mesh["name"] not in meshes:
            continue
        for pr in mesh["primitives"]:
            a = pr["attributes"]
            pos, nor, uv = g["acc"](a["POSITION"]), g["acc"](a["NORMAL"]), g["acc"](a["TEXCOORD_0"])
            jo, we = g["acc"](a["JOINTS_0"]), g["acc"](a["WEIGHTS_0"])
            idx = g["acc"](pr["indices"])
            mat = g["materials"][pr["material"]]
            tex = mat.get("pbrMetallicRoughness", {}).get("baseColorTexture")
            image = g["dir"] / g["images"][g["textures"][tex["index"]]["source"]]["uri"]
            verts = []
            for i in range(len(pos)):
                ws = list(we[i])
                js = [remap[j] for j in jo[i]]
                m = [[sum(ws[k] * to_target[jo[i][k]][r][c] for k in range(4)) for c in range(4)] for r in range(4)]
                p = apply(m, pos[i])
                n = apply(m, nor[i], 0.0)
                l = math.sqrt(sum(x * x for x in n)) or 1.0
                n = tuple(x / l for x in n)
                # the four, heaviest first; unused slots point at joint 0 with no weight
                pairs = sorted(zip(ws, js), reverse=True)
                verts.append({"p": p, "n": n, "uv": uv[i], "j": [jj if w > 0 else 0 for w, jj in pairs],
                              "w": quantize_weights([w for w, _ in pairs]), "head": sum(w for w, jj in pairs if jj in keep_joints)})
            tris = [tuple(idx[k:k + 3]) for k in range(0, len(idx), 3)]
            if keep_joints:
                tris = [t for t in tris if all(verts[v]["head"] >= spec.get("cut_share", 0.5) for v in t)]
            used = sorted({v for t in tris for v in t})
            if not used:
                continue
            renum = {v: k for k, v in enumerate(used)}
            vs, ts = [verts[v] for v in used], [tuple(renum[v] for v in t) for t in tris]
            if spec.get("drop_small"):
                vs, ts = drop_small(vs, ts, spec["drop_small"])
            if spec.get("keep", 1) < 1:
                vs, ts = decimate(vs, ts, spec["keep"])
            prims.append({"image": image, "material": mat["name"], "verts": vs, "tris": ts})
    if not prims:
        raise SystemExit(f"{spec['name']}: nothing left of {path}")
    return prims


# ---- fewer triangles

def decimate(verts, tris, keep):
    """The mesh with about `keep` of its triangles: half-edge collapses (a vertex merged into a
    neighbour, which keeps its own uv, normal and skin), cheapest first by the quadric error of the
    planes round both, plus a centimetre's worth for each whole joint their skins differ by. A vertex
    on an edge of one triangle (the mesh's borders and every uv seam) never moves, and no collapse
    turns a triangle over or makes it a sliver."""
    import heapq
    target = int(len(tris) * keep)
    if target >= len(tris):
        return verts, tris
    P = [v["p"] for v in verts]
    faces = [list(t) for t in tris]
    around = [set() for _ in verts]
    for f, t in enumerate(faces):
        for x in t:
            around[x].add(f)
    edges = {}
    for t in faces:
        for i in range(3):
            e = tuple(sorted((t[i], t[(i + 1) % 3])))
            edges[e] = edges.get(e, 0) + 1
    locked = [False] * len(verts)
    for (a, b), n in edges.items():
        if n == 1:
            locked[a] = locked[b] = True
    def normal(t):
        a, b, c = (P[x] for x in t)
        u = [b[i] - a[i] for i in range(3)]; w = [c[i] - a[i] for i in range(3)]
        return (u[1]*w[2] - u[2]*w[1], u[2]*w[0] - u[0]*w[2], u[0]*w[1] - u[1]*w[0])
    Q = [[0.0] * 10 for _ in verts]
    for t in faces:
        n = normal(t); l = math.sqrt(sum(x * x for x in n))
        if l < 1e-12:
            continue
        a, b, c = n[0] / l, n[1] / l, n[2] / l; d = -(a * P[t[0]][0] + b * P[t[0]][1] + c * P[t[0]][2])
        k = [a*a, a*b, a*c, a*d, b*b, b*c, b*d, c*c, c*d, d*d]
        for x in t:
            q = Q[x]
            for i in range(10):
                q[i] += k[i] * l * 0.5
    def err(q, p):
        x, y, z = p
        return (q[0]*x*x + 2*q[1]*x*y + 2*q[2]*x*z + 2*q[3]*x + q[4]*y*y + 2*q[5]*y*z + 2*q[6]*y + q[7]*z*z + 2*q[8]*z + q[9])
    def skin(v):
        d = {}
        for j, w in zip(verts[v]["j"], verts[v]["w"]):
            if w: d[j] = d.get(j, 0) + w / 255
        return d
    skins = [skin(v) for v in range(len(verts))]
    def cost(u, v):
        su, sv = skins[u], skins[v]
        apart = sum(abs(su.get(j, 0) - sv.get(j, 0)) for j in set(su) | set(sv))
        return err([a + b for a, b in zip(Q[u], Q[v])], P[v]) + 1e-4 * apart
    heap = []
    stamp = [0] * len(verts)
    def push(u):
        if locked[u]:
            return
        stamp[u] += 1
        best = None
        for f in around[u]:
            for v in faces[f]:
                if v != u:
                    c = cost(u, v)
                    if best is None or c < best[0]:
                        best = (c, v)
        if best:
            heapq.heappush(heap, (best[0], u, best[1], stamp[u]))
    for u in range(len(verts)):
        push(u)
    alive = len(faces)
    while alive > target and heap:
        c, u, v, st = heapq.heappop(heap)
        if st != stamp[u] or locked[u]:
            continue
        shared = [f for f in around[u] if v in faces[f]]
        moved = [f for f in around[u] if v not in faces[f]]
        if not shared:
            push(u); continue
        ok = True
        for f in moved:
            t = faces[f]
            n0 = normal(t)
            t2 = [v if x == u else x for x in t]
            n1 = normal(t2)
            l0 = math.sqrt(sum(x * x for x in n0)); l1 = math.sqrt(sum(x * x for x in n1))
            if l1 < 1e-12 or l0 < 1e-12 or sum(a * b for a, b in zip(n0, n1)) < 0.5 * l0 * l1:
                ok = False; break
            # no slivers: the new triangle's area against its longest side squared
            a, b, cc = (P[x] for x in t2)
            longest = max(sum((a[i] - b[i]) ** 2 for i in range(3)), sum((b[i] - cc[i]) ** 2 for i in range(3)), sum((cc[i] - a[i]) ** 2 for i in range(3)))
            if l1 < 0.04 * longest:
                ok = False; break
        if not ok:
            stamp[u] += 1   # leave it where it is for now
            continue
        for f in shared:
            for x in faces[f]:
                around[x].discard(f)
            faces[f] = None
            alive -= 1
        for f in moved:
            faces[f] = [v if x == u else x for x in faces[f]]
            around[v].add(f)
        around[u] = set()
        Q[v] = [a + b for a, b in zip(Q[u], Q[v])]
        locked[u] = True
        near = {x for f in around[v] for x in faces[f]}
        for x in near:
            push(x)
    used = sorted({x for t in faces if t for x in t})
    renum = {x: k for k, x in enumerate(used)}
    return [verts[x] for x in used], [tuple(renum[x] for x in t) for t in faces if t]


# ---- clips

def channel(g, anim, node_name, path):
    for c in anim["channels"]:
        if g["nodes"][c["target"]["node"]]["name"] == node_name and c["target"]["path"] == path:
            s = anim["samplers"][c["sampler"]]
            if s.get("interpolation", "LINEAR") != "LINEAR":
                raise SystemExit(f"{anim['name']}: {node_name} {path} isn't linear")
            return list(zip(g["acc"](s["input"]), g["acc"](s["output"])))
    return []


def thin(keys, rotation):
    """Drop keys the line (arc) between their neighbours reproduces within the tolerance."""
    if len(keys) <= 2:
        return keys
    tol = KEY_TOLERANCE_R if rotation else KEY_TOLERANCE_T
    out = [keys[0]]
    i = 0
    while i < len(keys) - 1:
        j = i + 2
        while j < len(keys):
            a, b = keys[i], keys[j]
            ok = True
            for k in range(i + 1, j):
                f = (keys[k][0] - a[0]) / (b[0] - a[0])
                if rotation:
                    if qangle(slerp(a[1], b[1], f), keys[k][1]) > tol: ok = False; break
                elif max(abs(x + (y - x) * f - z) for x, y, z in zip(a[1], b[1], keys[k][1])) > tol: ok = False; break
            if not ok:
                break
            j += 1
        i = j - 1
        out.append(keys[i])
    return out


def clips(spec, skeletons, library_rest):
    """Each clip: name, seconds, metres a loop, per joint its rotation keys and per body its
    pelvis translation keys."""
    libs = {k: load(v) for k, v in spec["libraries"].items()}
    names = skeletons["male"].names
    pelvis = names.index("pelvis")
    out = []
    for c in spec["clips"]:
        g = libs[c["library"]]
        anim = next((a for a in g["animations"] if a["name"] == c["clip"]), None)
        if not anim:
            raise SystemExit(f"{c['clip']}: not in {c['library']}")
        duration = max(t for ch in anim["channels"] for t in g["acc"](anim["samplers"][ch["sampler"]]["input"]))
        rot = []
        for j, name in enumerate(names):
            keys = [(t, qunit(q)) for t, q in channel(g, anim, name, "rotation")]
            for k in range(1, len(keys)):   # one hemisphere, so the arcs between keys are the short ones
                if sum(x * y for x, y in zip(keys[k - 1][1], keys[k][1])) < 0:
                    keys[k] = (keys[k][0], tuple(-x for x in keys[k][1]))
            if keys and all(qangle(k[1], skeletons["male"].q[j]) < KEY_TOLERANCE_R and qangle(k[1], skeletons["female"].q[j]) < KEY_TOLERANCE_R for k in keys):
                keys = []
            elif keys and all(qangle(k[1], keys[0][1]) < KEY_TOLERANCE_R for k in keys):
                keys = [keys[0]]
            rot.append(thin(keys, True))
        base = channel(g, anim, "pelvis", "translation")
        lib = library_rest[pelvis]
        male = thin([(t, tuple(v[i] - lib[i] for i in range(3))) for t, v in base], False)
        move = {body: [(t, tuple(sk.t[pelvis][i] + v[i] for i in range(3))) for t, v in male] for body, sk in skeletons.items()}
        for name in names:
            if name != "pelvis" and any(max(abs(x - y) for x, y in zip(v, library_rest_by_name(libs, c["library"], name))) > 1e-4
                                        for _, v in channel(g, anim, name, "translation")):
                if name == "root":
                    raise SystemExit(f"{c['clip']}: moves the root (use the library's in-place clips)")
        out.append({"name": c["name"], "duration": duration, "distance": float(c.get("distance", 0)),
                    "rot": rot, "move": move})
    return out


_rest_cache = {}
def library_rest_by_name(libs, key, name):
    if key not in _rest_cache:
        sk = Skeleton(libs[key]); _rest_cache[key] = dict(zip(sk.names, sk.t))
    return _rest_cache[key][name]


# ---- textures and vertices

# What a vertex's texel is, for the realm's shader (its kind byte): as it is; skin (the person's skin tone); hair (their hair colour); cloth, whose pale
# cloth (the peasant's shirts) or green cloth (the ranger's) takes the person's dye.
KINDS = {"plain": 0, "skin": 1, "hair": 2, "pale": 3, "green": 4}


def crop_box(verts, pad):
    us = [v["uv"][0] for v in verts]; vs = [v["uv"][1] for v in verts]
    return (max(0.0, min(us) - pad), max(0.0, min(vs) - pad), min(1.0, max(us) + pad), min(1.0, max(vs) + pad))


def layer_bytes(path, size, quality, box=(0.0, 0.0, 1.0, 1.0)):
    from PIL import Image
    im = Image.open(path).convert("RGB")
    W, H = im.size
    if box != (0.0, 0.0, 1.0, 1.0):
        im = im.crop((round(box[0] * W), round(box[1] * H), round(box[2] * W), round(box[3] * H)))
    im = im.resize((size, size), Image.LANCZOS)
    buf = io.BytesIO()
    im.save(buf, "JPEG", quality=quality, optimize=True)
    return buf.getvalue()


# Where a vertex may stand, in metres of the character's own space: x and z within +-SPAN/2,
# y from LOW; its position is three unorm16s across SPAN.
SPAN, LOW = 2.5, -0.25


def vertex(v, layer, kind, box, clamp=False):
    """24 bytes: position (3 x unorm16, x and z from -SPAN/2, y from LOW, across SPAN), the layer
    (u8) and kind (u8), the normal (3 x i8, then 0), uv (2 x unorm16, in the layer's box), four
    joints (u8) and their weights (u8, 255ths)."""
    u0, v0, u1, v1 = box
    u = (v["uv"][0] - u0) / (u1 - u0); w = (v["uv"][1] - v0) / (v1 - v0)
    if not clamp and not (-1e-4 <= u <= 1 + 1e-4 and -1e-4 <= w <= 1 + 1e-4):
        raise SystemExit("people: a uv outside its layer (0 to 1)")
    q = []
    for x, lo in zip(v["p"], (-SPAN / 2, LOW, -SPAN / 2)):
        k = (x - lo) / SPAN
        if not 0 <= k <= 1:
            raise SystemExit(f"people: a vertex past the character's box ({v['p']})")
        q.append(round(k * 65535))
    n = bytes((round(x * 127) & 255) for x in v["n"])
    return (struct.pack("<3H", *q) + bytes([layer, kind]) + n + b"\0"
            + struct.pack("<2H", round(min(max(u, 0), 1) * 65535), round(min(max(w, 0), 1) * 65535))
            + bytes(v["j"]) + bytes(v["w"]))


def build(spec, size, first_layer):
    """The people's parts, layers and rig, for pack_realm.build: parts [{name, vertices, indices,
    count, body}], layers [(source, JPEG)], and what the rig needs."""
    skeletons = {k: Skeleton(load(v)) for k, v in spec["skeletons"].items()}
    if skeletons["male"].names != skeletons["female"].names or skeletons["male"].parent != skeletons["female"].parent:
        raise SystemExit("people: the male and female skeletons aren't the same joints")
    if len(skeletons["male"].names) > 96:
        raise SystemExit("people: more joints than verse/skin.coax holds")
    layers = []          # (source name, image bytes)
    layer_of = {}        # (image, box) -> layer number
    variants = {k: home_path(v) for k, v in spec.get("variants", {}).items()}
    parts = []
    quality = int(spec.get("quality", 80))
    kinds = spec.get("kinds", {})
    for ps in spec["parts"]:
        prims = part_mesh(ps, skeletons)
        vb = bytearray(); tris = []; count = 0
        for pr in prims:
            crop = ps.get("crop", {}).get(pr["material"])
            box = crop_box(pr["verts"], crop) if crop is not None else (0.0, 0.0, 1.0, 1.0)
            key = (hashlib.sha256(pr["image"].read_bytes()).hexdigest(), box)
            if key not in layer_of:
                layer_of[key] = first_layer + len(layers)
                layers.append((pr["image"].name + (" (cut)" if crop is not None else ""), layer_bytes(pr["image"], size, quality, box)))
                alt = variants.get(pr["image"].name)
                if alt:   # its other colourway, the layer after it
                    layers.append((alt.name, layer_bytes(alt, size, quality, box)))
            layer = layer_of[key]
            if layer > 255:
                raise SystemExit("people: more than 256 layers")
            kind = KINDS[kinds.get(pr["material"], "plain")]
            for v in pr["verts"]:
                vb += vertex(v, layer, kind, box, pr["material"] in spec.get("clamp", []))
            tris += [count + k for t in pr["tris"] for k in t]
            count += len(pr["verts"])
        parts.append({"name": ps["name"], "vertices": bytes(vb), "indices": tris, "count": count, "body": ps["body"]})
    library_rest = Skeleton(load(next(iter(spec["libraries"].values())))).t
    cl = clips(spec, skeletons, library_rest)
    return parts, layers, (skeletons, cl)


def home_path(p):
    return Path(os.path.expanduser(p))


# ---- the rig as Coax

def f9(x):
    t = "%.9g" % x
    return t if any(c in t for c in ".en") else t + ".0"


def bind_rows(sk):
    return [v for m in sk.ib for r in range(3) for v in m[r]]


def posed(sk, clip, body, time):
    """Every joint's world position `time` seconds into clip, sampled as verse/skin.coax does, in f64."""
    def pair(keys, t):
        i = max(sum(1 for k in keys if k[0] <= t) - 1, 0)
        a = keys[i]; b = keys[i + 1] if i + 1 < len(keys) else a
        f = min(max((t - a[0]) / (b[0] - a[0]), 0.0), 1.0) if b[0] > a[0] else 0.0
        return a[1], b[1], f
    t = time % clip["duration"]
    pel = sk.index("pelvis")
    gq, gt = [], []
    for j in range(len(sk.names)):
        lt = sk.t[j]
        if j == pel and clip["move"][body]:
            a, b, f = pair(clip["move"][body], t); lt = tuple(x + (y - x) * f for x, y in zip(a, b))
        lq = sk.q[j]
        if clip["rot"][j]:
            a, b, f = pair(clip["rot"][j], t); lq = slerp(qunit(a), qunit(b), f)
        p = sk.parent[j]
        if p < 0:
            gq.append(lq); gt.append(lt)
        else:
            gq.append(qunit(qmul(gq[p], lq)))
            m = mat_tr((0, 0, 0), gq[p])
            r = apply(m, lt)
            gt.append(tuple(x + y for x, y in zip(gt[p], r)))
    return gt


CHECK_JOINTS = ("pelvis", "Head", "hand_l", "hand_r", "foot_l", "foot_r")


def rig_coax(manifest, rig, parts, prefix):
    skeletons, cl = rig
    m, f = skeletons["male"], skeletons["female"]
    pel = m.index("pelvis")
    tr, kr, tt = [], [], []
    keys_t = {"male": [], "female": []}
    for c in cl:
        for j in range(len(m.names)):
            tr += [len(kr), len(c["rot"][j])]
            kr += [(t,) + tuple(q) for t, q in c["rot"][j]]
            mv = c["move"]["male"] if j == pel else []   # the bodies' keys share their times
            tt += [len(keys_t["male"]), len(mv)]
            if j == pel:
                keys_t["male"] += [(t,) + tuple(v) for t, v in c["move"]["male"]]
                keys_t["female"] += [(t,) + tuple(v) for t, v in c["move"]["female"]]
    def arr(name, kind, values, per_line, fmt):
        lines = [", ".join(fmt(v) for v in values[i:i + per_line]) for i in range(0, len(values), per_line)]
        return f"{name} :: [{len(values)}] {kind}.[\n  " + ",\n  ".join(lines) + "\n];"
    flat = lambda rows: [v for row in rows for v in row]
    P = prefix
    J = len(m.names)
    out = [f"// Generated by verse/tools/pack_realm.py (verse/tools/people.py) from verse/packs/{manifest['pack']}.json; do not edit.",
           "// The people's skeletons and clips: the Quaternius Universal Base Characters' two bodies (the",
           "// Modular Character Outfits' male and female skeletons, the same 65 joints) and the Universal",
           "// Animation Library's clips, every part rebound onto its body's skeleton. Per joint its parent",
           "// (parents first); per body its rest translation and rotation (x, y, z, w) and inverse bind",
           "// (three rows of four); per clip its seconds and the metres a loop travels (0 on the clock), per",
           "// joint its rotation keys (first, count) into PEOPLE_KEYS_R (time, x, y, z, w) and its",
           "// translation keys into the body's PEOPLE_KEYS_T_* (time, x, y, z: the pelvis's only, the",
           "// library's move from its rest added to the body's rest). verse/skin.coax plays it.",
           "",
           '#load "../skin.coax";',
           "",
           f"{P}_JOINTS :: {J};",
           f"{P}_CLIPS :: {len(cl)};",
           f"{P}_HEAD :: {m.index('Head')};",
           f"{P}_NECK :: {m.index('neck_01')};",
           f"{P}_SPINE :: {m.index('spine_03')};",
           f"{P}_CLIP_NAME :: [{len(cl)}] string.[" + ", ".join(f'"{c["name"]}"' for c in cl) + "];",
           *[f"{P}_{c['name'].upper()} :: {k};" for k, c in enumerate(cl)],
           f"{P}_PARTS :: {len(parts)};",
           f"{P}_PART_NAME :: [{len(parts)}] string.[" + ", ".join(f'"people/{p["name"]}"' for p in parts) + "];",
           f"{P}_PART_FEMALE :: [{len(parts)}] bool.[" + ", ".join("true" if p["body"] == "female" else "false" for p in parts) + "];",
           f"{P}_PART_TRIANGLES :: [{len(parts)}] int.[" + ", ".join(str(len(p["indices"]) // 3) for p in parts) + "];",
           arr(f"{P}_DURATION", "f32", [c["duration"] for c in cl], 8, f9),
           arr(f"{P}_DISTANCE", "f32", [c["distance"] for c in cl], 8, f9),
           arr(f"{P}_PARENT", "int", m.parent, 25, str),
           arr(f"{P}_REST_M", "f32", flat(tuple(m.t[j]) + tuple(m.q[j]) for j in range(J)), 7, f9),
           arr(f"{P}_REST_F", "f32", flat(tuple(f.t[j]) + tuple(f.q[j]) for j in range(J)), 7, f9),
           arr(f"{P}_BIND_M", "f32", bind_rows(m), 12, f9),
           arr(f"{P}_BIND_F", "f32", bind_rows(f), 12, f9),
           arr(f"{P}_TRACK_T", "int", tt, 30, str),
           arr(f"{P}_TRACK_R", "int", tr, 30, str),
           arr(f"{P}_KEYS_T_M", "f32", flat(keys_t["male"]), 8, f9),
           arr(f"{P}_KEYS_T_F", "f32", flat(keys_t["female"]), 8, f9),
           arr(f"{P}_KEYS_R", "f32", flat(kr), 10, f9),
           "// Checks, sampled in f64 as verse/skin.coax samples (posed in people.py): part way into each",
           "// clip, where a joint stands: body (0 male, 1 female), clip, seconds, joint, x, y, z.",
           arr(f"{P}_CHECK", "f32", flat((b, ci, tm, j) + tuple(posed(sk, c, body, tm)[j])
                                         for b, (body, sk) in enumerate((("male", m), ("female", f)))
                                         for ci, c in enumerate(cl) for tm in (0.0, c["duration"] * 0.37)
                                         for j in (m.index(n) for n in CHECK_JOINTS)), 7, f9),
           "",
           "// The rig in a page's state (a view needs an array it may write through), and its views.",
           f"{P.title()} :: struct {{",
           f"  parent: [{J}] int; rest_m: [{7 * J}] f32; rest_f: [{7 * J}] f32; bind_m: [{12 * J}] f32; bind_f: [{12 * J}] f32;",
           f"  duration: [{len(cl)}] f32; distance: [{len(cl)}] f32; track_t: [{len(tt)}] int; track_r: [{len(tr)}] int;",
           f"  keys_t_m: [{4 * len(keys_t['male'])}] f32; keys_t_f: [{4 * len(keys_t['female'])}] f32; keys_r: [{5 * len(kr)}] f32;",
           "}",
           f"{P.lower()}_load :: (d: [] {P.title()}) {{",
           *[f"  d[0].{n} = {P}_{n.upper()};" for n in ("parent", "rest_m", "rest_f", "bind_m", "bind_f", "duration", "distance",
                                                        "track_t", "track_r", "keys_t_m", "keys_t_f", "keys_r")],
           "}",
           "// Body 0 is the male skeleton, 1 the female; they share the clips' rotations.",
           f"{P.lower()}_rig :: (d: [] {P.title()}, female: bool) -> SkinRig {{",
           f"  if female {{",
           f"    return SkinRig.{{joints = {P}_JOINTS, clips = {P}_CLIPS, parent = d[0].parent[..], rest = d[0].rest_f[..], bind = d[0].bind_f[..],",
           "                    duration = d[0].duration[..], distance = d[0].distance[..], track_t = d[0].track_t[..],",
           "                    track_r = d[0].track_r[..], keys_t = d[0].keys_t_f[..], keys_r = d[0].keys_r[..]};",
           "  }",
           f"  return SkinRig.{{joints = {P}_JOINTS, clips = {P}_CLIPS, parent = d[0].parent[..], rest = d[0].rest_m[..], bind = d[0].bind_m[..],",
           "                  duration = d[0].duration[..], distance = d[0].distance[..], track_t = d[0].track_t[..],",
           "                  track_r = d[0].track_r[..], keys_t = d[0].keys_t_m[..], keys_r = d[0].keys_r[..]};",
           "}"]
    return "\n".join(out) + "\n"
