"""MetaHuman Lab's human pack (docs/verse/metahuman-support.md, "MetaHuman Lab"): one skinned
human, its meshes, skin weights on the 65-joint people rig and its texture maps, compiled into
one VPK1 file (verse/tools/pack_realm.py has the container) that the lab's page loads with
`private:human` (coax/web/host.js). Offline tooling; never shipped or run by a page.

    python3 verse/tools/humans/pack_human.py standin [OUT.pack] [--outfit NAME]
    python3 verse/tools/humans/pack_human.py metahuman EXPORT_DIR [OUT.pack] [--outfit NAME]

--outfit dresses the body in a Quaternius fantasy outfit (outfits.py: fantasy, hooded, robe)
instead of the portrait's clothes (a MetaHuman) or none (the stand-in). OUT.pack defaults to
~/.openagents/verse/private-build/web/NAME.pack (standin.pack or human.pack, or with the
outfit's name after a dash: human-fantasy.pack), the directory coax/web/serve.c serves as /private/ on a local address. Nothing
this writes goes into git: a MetaHuman pack is a MetaHuman asset (private, never used to train
AI models), and the stand-in stays beside it so both load the same way.

Sources:
  standin    the Quaternius Universal Base Characters' superhero male (CC0), with its eyes,
             brows and parted hair, already on the people rig's 65 joints (verse/tools/people.py
             rebinds it onto the rig's male rest pose).
  metahuman  a MetaHuman export's DNA meshes, dumped by OpenRigLogic's dna_dump --skin
             (scripts in the export's tree; see metahuman.py), each joint folded onto the
             rig's 65 (aethyr/mannequin.coax mannequin_fold) and posed from MetaHuman's A pose
             into the rig's T pose.

The pack's entries: the catalog (text), the vertices, the indices (u32), then the layers
(JPEG, or PNG where a layer has alpha), every layer the same size.

The catalog, a line each:
  human NAME                    the source's name
  body male|female              the people rig's body whose clips it plays
  licence TEXT                  what its art is under
  outfit NAME BASE              what it wears (student, bare, or an outfits.py outfit) and the
                                pack's name without it (the lab's OUTFIT button: ?pack=BASE&outfit=)
  B J X Y Z                     joint J's bind position (metres, the figure's own space, the
                                rig's rest pose: +y up, +z forward, the figure's left +x)
  M MATERIAL R G B ROUGH        a material's tint (thousandths, linear) and roughness scale
  P NAME FIRST COUNT MATERIAL   a piece: its indices into the index buffer, its material
  L LAYER ROLE SOURCE           a layer: base, normal (tangent space, green toward +v, down the
                                image: a map whose green points up is flipped, green_up()) or
                                rough (G)
  box X0 Y0 Z0 X1 Y1 Z1         the bind pose's bounds
  D LOD NAME FIRST COUNT MATERIAL  a piece at a lower level of detail (1, 2: reduce()), after
                                every P line, the levels in order; the eyes' blended layers
                                have none
  F FIRST COUNT BYTES           a face rig (face_rig.py): its vertices' range in the vertex
                                buffer, and its block's length; the block follows the text
                                after a NUL (aethyr/facerig.coax reads it)

A vertex is 32 bytes (vertex() below): position (x, y, z unorm16 across SPAN from LOW), its
piece's material, the normal and tangent (octahedral, 2 x unorm16 each), the bitangent's
sign, uv (2 x unorm16), four joints (u8) and their weights (u8, 255ths summing to 255), and the
layers its material reads (base, normal, rough; 255 none).

Materials: 1 skin, 2 eye, 3 hair, 4 brow, 5 cloth, 6 teeth, 7 eye wet layer, 8 to 16 more
cloths (a colour each, from the portrait's materials; 8 to 24), 25 the eyes' occlusion. A material
with no base layer is its tint. The wet layer and the occlusion are blended over the eyes (the lab draws
them last, as the host's water: one pass multiplies what lies behind, one adds): their uv is not
a texture's but lid_coords', u from the lid's margin (0) inward (1), v up the eye's opening, and
the wet layer's tint is its roughness (the occlusion's colour is scene_fs's: past the lab's 24
material rows).
"""
import io, math, os, struct, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import people  # noqa: E402
import outfits  # noqa: E402

MAGIC, HEADER, ENTRY = b"VPK1", 32, 48
KIND_CATALOG, KIND_VERTICES, KIND_INDICES, KIND_LAYER = 1, 2, 3, 4
SPAN, LOW = 2.5, -0.25
JOINTS = 65
NONE = 255
MATERIALS = {"skin": 1, "eye": 2, "hair": 3, "brow": 4, "cloth": 5, "teeth": 6, "wet": 7, "occlusion": 25}
BLENDED = (7, 25)       # drawn last, over what is behind them
# the eyes' wet layer: its roughness
EYE_TINTS = {"wet": (1.0, 1.0, 1.0, 0.06)}
MATERIALS.update({f"cloth{k}": 8 + k for k in range(17)})   # more cloths, each its own colour (8 to 24)
OUT_DIR = Path.home() / ".openagents/verse/private-build/web"

QUATERNIUS = "~/Downloads/Universal Base Characters[Standard]"
OUTFITS = "~/Downloads/Modular Character Outfits - Fantasy[Standard]/Exports/glTF (Godot-Unreal)/Outfits"


# ---- small vector maths

def sub(a, b): return tuple(x - y for x, y in zip(a, b))
def dot(a, b): return sum(x * y for x, y in zip(a, b))
def cross(a, b): return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])
def unit(a):
    l = math.sqrt(dot(a, a))
    return tuple(x / l for x in a) if l > 1e-12 else (0.0, 1.0, 0.0)


def octa(n):
    """A unit vector as two unorm16s (octahedral)."""
    x, y, z = unit(n)
    s = abs(x) + abs(y) + abs(z)
    x, y = x / s, y / s
    if z < 0:
        x, y = (1 - abs(y)) * (1 if x >= 0 else -1), (1 - abs(x)) * (1 if y >= 0 else -1)
    q = lambda v: max(0, min(65535, round((v * 0.5 + 0.5) * 65535)))
    return q(x) | (q(y) << 16)


def tangents(verts, tris):
    """Per vertex a tangent along +u and the bitangent's sign, from the triangles' uvs."""
    acc_t = [[0.0, 0.0, 0.0] for _ in verts]
    acc_b = [[0.0, 0.0, 0.0] for _ in verts]
    for a, b, c in tris:
        p0, p1, p2 = verts[a]["p"], verts[b]["p"], verts[c]["p"]
        u0, u1, u2 = verts[a]["uv"], verts[b]["uv"], verts[c]["uv"]
        e1, e2 = sub(p1, p0), sub(p2, p0)
        du1, dv1, du2, dv2 = u1[0] - u0[0], u1[1] - u0[1], u2[0] - u0[0], u2[1] - u0[1]
        d = du1 * dv2 - du2 * dv1
        if abs(d) < 1e-14:
            continue
        r = 1.0 / d
        t = tuple((e1[k] * dv2 - e2[k] * dv1) * r for k in range(3))
        bt = tuple((e2[k] * du1 - e1[k] * du2) * r for k in range(3))
        for v in (a, b, c):
            for k in range(3):
                acc_t[v][k] += t[k]; acc_b[v][k] += bt[k]
    out = []
    for i, v in enumerate(verts):
        n = v["n"]
        t = acc_t[i]
        t = sub(t, tuple(x * dot(n, t) for x in n))
        if dot(t, t) < 1e-16:
            t = cross(n, (0.0, 0.0, 1.0)) if abs(n[2]) < 0.9 else cross(n, (1.0, 0.0, 0.0))
        t = unit(t)
        sign = 1 if dot(cross(n, t), acc_b[i]) >= 0 else -1
        out.append((t, sign))
    return out


def vertex(v, tangent, material, layers):
    q = []
    for x, lo in zip(v["p"], (-SPAN / 2, LOW, -SPAN / 2)):
        k = (x - lo) / SPAN
        if not 0 <= k <= 1:
            raise SystemExit(f"pack_human: a vertex past the figure's box ({v['p']})")
        q.append(round(k * 65535))
    t, sign = tangent
    u = min(max(v["uv"][0], 0.0), 1.0); w = min(max(v["uv"][1], 0.0), 1.0)
    base, normal, rough = layers
    return struct.pack("<IIIIIBBBBBBBBBBBB",
                       q[0] | (q[1] << 16),
                       q[2] | (material << 16) | ((1 if sign < 0 else 0) << 31),
                       octa(v["n"]), octa(t),
                       round(u * 65535) | (round(w * 65535) << 16),
                       *v["j"], *v["w"], base, normal, rough, material)


# Levels of detail below the full mesh: a vertex-clustering cell size each (metres). The lab
# switches by how tall the figure stands on the screen.
LODS = (0.008, 0.025)


def octant(n):
    """A normal's dominant axis and sign (0..5): clustering keeps a cloth's two sides apart."""
    k = max(range(3), key=lambda i: abs(n[i]))
    return 2 * k + (n[k] < 0)


def reduce(verts, tris, cell):
    """A piece at a lower level of detail, by vertex clustering: the vertices in one cell of the
    grid all move to one place (the one nearest their middle) and take its skin weights, so the
    surface stays closed in every pose; those
    facing another way or in another 1/8 of uv (a seam, a cloth's other side) keep their own uv,
    normal and weights there. Triangles that collapse go."""
    cells = {}
    for i, v in enumerate(verts):
        cells.setdefault(tuple(math.floor(c / cell) for c in v["p"]), []).append(i)
    keep, out = {}, []
    for idx in cells.values():
        mid = [sum(verts[i]["p"][k] for i in idx) / len(idx) for k in range(3)]
        anchor = verts[min(idx, key=lambda i: math.dist(verts[i]["p"], mid))]
        parts = {}
        for i in idx:
            v = verts[i]
            parts.setdefault((octant(v["n"]), math.floor(v["uv"][0] * 8), math.floor(v["uv"][1] * 8)), []).append(i)
        for part in parts.values():
            best = min(part, key=lambda i: math.dist(verts[i]["p"], mid))
            for i in part:
                keep[i] = len(out)
            v = dict(verts[best]); v["p"] = anchor["p"]; v["j"] = list(anchor["j"]); v["w"] = list(anchor["w"])
            out.append(v)
    seen, tri = set(), []
    for t in tris:
        a, b, c = (keep[i] for i in t)
        if len({a, b, c}) < 3 or len({out[a]["p"], out[b]["p"], out[c]["p"]}) < 3:
            continue
        k = tuple(sorted((a, b, c)))
        if k in seen:
            continue
        seen.add(k); tri.append((a, b, c))
    used = sorted({i for t in tri for i in t})
    at = {i: k for k, i in enumerate(used)}
    return [out[i] for i in used], [tuple(at[i] for i in t) for t in tri]


def lid_coords(verts, tris):
    """Rewrites the uv of an eye's blended layer (a disc in the eye's opening, or a ring along
    the lids) to lid coordinates: u from its outer border, the lids' margin (0), to its middle or
    its inner border (1), along the surface; v up the opening, 0 at its lowest point, 1 at its
    highest. Each connected part (an eye) on its own. Vertices split at uv seams are one place."""
    import heapq
    from collections import defaultdict
    ids, at = {}, []
    for v in verts:
        k = tuple(round(c, 6) for c in v["p"])
        if k not in ids:
            ids[k] = len(at); at.append(v["p"])
        v["_k"] = ids[k]
    edges, nbr = defaultdict(int), defaultdict(set)
    for t in tris:
        q = [verts[i]["_k"] for i in t]
        for a, b in ((q[0], q[1]), (q[1], q[2]), (q[2], q[0])):
            edges[(min(a, b), max(a, b))] += 1; nbr[a].add(b); nbr[b].add(a)
    # a quad's diagonal is inside it: only an edge one triangle uses, both ways round a quad
    # split in two, is the border
    rim = defaultdict(set)
    for (a, b), c in edges.items():
        if c == 1:
            rim[a].add(b); rim[b].add(a)
    def walk(seed, links):
        seen, stack = {seed}, [seed]
        while stack:
            for y in links[stack.pop()]:
                if y not in seen:
                    seen.add(y); stack.append(y)
        return seen
    def dist(seeds):
        d = {i: 0.0 for i in seeds}; q = [(0.0, i) for i in seeds]
        while q:
            dd, i = heapq.heappop(q)
            if dd > d[i]:
                continue
            for j in nbr[i]:
                nd = dd + math.dist(at[i], at[j])
                if nd < d.get(j, 1e9):
                    d[j] = nd; heapq.heappush(q, (nd, j))
        return d
    uv, done = {}, set()
    for start in range(len(at)):
        if start in done:
            continue
        part = walk(start, nbr); done |= part
        loops, left = [], {i for i in part if i in rim}
        while left:
            loop = walk(next(iter(left)), rim); left -= loop; loops.append(loop)
        if not loops:
            continue
        # the outer border reaches lowest and highest (a ring's inner one doesn't)
        loops.sort(key=lambda l: -(max(at[i][1] for i in l) - min(at[i][1] for i in l)))
        out = dist(loops[0])
        inner = dist(set().union(*loops[1:])) if len(loops) > 1 else None
        far = max(out.get(i, 0.0) for i in part) or 1.0
        y0 = min(at[i][1] for i in part); y1 = max(at[i][1] for i in part)
        for i in part:
            a = out.get(i, 0.0)
            u = a / (a + inner[i]) if inner and a + inner.get(i, 0.0) > 0 else a / far
            uv[i] = (min(max(u, 0.0), 1.0), (at[i][1] - y0) / max(y1 - y0, 1e-6))
    for v in verts:
        v["uv"] = uv.get(v.pop("_k"), (0.0, 0.0))
    return verts


def eye_layers(pack, eyes, segments=32, rings=6):
    """Procedural blended layers for a figure whose eyes are bare balls (the CC0 stand-in): per eye
    a cap on its ball (the occlusion) and a ring round the cap's edge (the wet layer), skinned as
    the nearest eye vertex, so the lab's eye passes can be seen on art anyone may look at."""
    from collections import defaultdict
    sides = defaultdict(list)
    for v in eyes["verts"]:
        sides[v["p"][0] > 0].append(v)
    cap, ring = ([], []), ([], [])
    for side, vs in sides.items():
        c = [sum(v["p"][k] for v in vs) / len(vs) for k in range(3)]
        rad = max(math.dist(v["p"], c) for v in vs)
        r_open = 0.75 * rad
        near = lambda p: min(vs, key=lambda v: math.dist(v["p"], p))
        def at(x, y):
            z = c[2] + math.sqrt(max(rad * rad - x * x - y * y, 0.0)) + 0.0004
            p = (c[0] + x, c[1] + y, z); n = tuple((p[k] - c[k]) / rad for k in range(3))
            w = near(p)
            return {"p": p, "n": n, "uv": (0.0, 0.0), "j": list(w["j"]), "w": list(w["w"])}
        def grid(verts, tris, radii):
            base = len(verts)
            for r in radii:
                for k in range(segments):
                    a = 2 * math.pi * k / segments
                    verts.append(at(r * math.cos(a), 0.62 * r * math.sin(a)))
            for i in range(len(radii) - 1):
                for k in range(segments):
                    a, b = base + i * segments + k, base + i * segments + (k + 1) % segments
                    tris += [(a, b + segments, b), (a, a + segments, b + segments)]
        grid(*cap, [r_open * (1 - i / rings) + 1e-5 for i in range(rings + 1)])
        grid(*ring, [r_open * 0.98, r_open * 0.8])
    for name, (verts, tris), material in (("eyes/occlusion", cap, "occlusion"), ("eyes/wet", ring, "wet")):
        pack.piece(name, lid_coords(verts, tris), tris, material)


def green_up(path, n=256):
    """Whether a tangent-space normal map's green points up its image (OpenGL's convention, as
    glTF's) rather than down (DirectX's, as Unreal's): of the two readings, the one whose slopes
    are a surface's (curl free) is the map's."""
    from PIL import Image
    im = Image.open(path).convert("RGB").resize((n, n))
    px = im.load()
    nx = [[px[x, y][0] / 127.5 - 1 for x in range(n)] for y in range(n)]
    ny = [[px[x, y][1] / 127.5 - 1 for x in range(n)] for y in range(n)]
    nz = [[max(px[x, y][2] / 127.5 - 1, 0.2) for x in range(n)] for y in range(n)]
    curl = [0.0, 0.0]
    for y in range(1, n - 1):
        for x in range(1, n - 1):
            dgx_dy = (-nx[y + 1][x] / nz[y + 1][x] + nx[y - 1][x] / nz[y - 1][x]) * 0.5
            dny_dx = (ny[y][x + 1] / nz[y][x + 1] - ny[y][x - 1] / nz[y][x - 1]) * 0.5
            curl[0] += abs(dny_dx - dgx_dy)     # green up: the slope down the rows is +ny
            curl[1] += abs(-dny_dx - dgx_dy)    # green down: it is -ny
    return curl[0] <= curl[1], curl


def layer_bytes(img, size, flip_green=False):
    from PIL import Image
    im = img if isinstance(img, Image.Image) else Image.open(img)
    alpha = im.mode in ("RGBA", "LA") and im.getextrema()[-1][0] < 250
    im = im.convert("RGBA" if alpha else "RGB")
    if flip_green:
        ch = list(im.split()); ch[1] = ch[1].point(lambda x: 255 - x); im = Image.merge(im.mode, ch)
    if im.size != (size, size):
        im = im.resize((size, size), Image.LANCZOS)
    buf = io.BytesIO()
    if alpha: im.save(buf, "PNG", optimize=True)
    else: im.save(buf, "JPEG", quality=92, optimize=True)
    return buf.getvalue()


def empty(path):
    """An image with nothing in it (every channel zero), as the export's SRMF maps: no layer."""
    if hasattr(path, "size"):
        return False
    from PIL import Image
    return all(hi == 0 for _, hi in Image.open(path).getextrema())


class Pack:
    """What a source hands the writer: pieces (verts in the rig's rest pose, tris, material,
    layer images), bind positions and material tints."""
    def __init__(self, name, body, licence):
        self.name, self.body, self.licence = name, body, licence
        self.pieces, self.binds, self.tints = [], [], {}
        self.images = []        # (role, source name, image path or PIL image, flip green)
        self.face = None        # a face rig: given the face pieces as written, its block (face_rig.pack)
        self.image_of = {}
        self.outfit = None      # (its outfit's name, the pack's name without it): the lab's outfit button

    def layer(self, role, path, flip=False):
        if path is None or empty(path):
            return NONE
        key = (role, str(path) if not hasattr(path, "size") else id(path))
        if key not in self.image_of:
            self.image_of[key] = len(self.images)
            self.images.append((role, Path(str(path)).name if not hasattr(path, "size") else role, path, flip))
        return self.image_of[key]

    def piece(self, name, verts, tris, material, base=None, normal=None, rough=None):
        # every normal layer is stored green down its image (+v), the way scene_fs reads it
        flip = normal is not None and green_up(normal)[0]
        layers = (self.layer("base", base), self.layer("normal", normal, flip), self.layer("rough", rough))
        self.pieces.append({"name": name, "verts": verts, "tris": tris, "material": MATERIALS[material], "layers": layers})


def write(pack, out, size):
    vb = bytearray(); ib = []; lines = [f"human {pack.name}", f"body {pack.body}", f"licence {pack.licence}"]
    if pack.outfit:
        lines.append(f"outfit {pack.outfit[0]} {pack.outfit[1]}")
    for j, b in enumerate(pack.binds):
        # a joint's bind position, and (a bind off the rig's rest) its turn to the rest, x y z w
        lines.append(("B %d" + " %.6f" * len(b)) % (j, *b))
    for m, (r, g, b, rough) in sorted(pack.tints.items()):
        lines.append("M %d %d %d %d %d" % (MATERIALS[m], round(r * 1000), round(g * 1000), round(b * 1000), round(rough * 1000)))
    lo, hi = [1e9] * 3, [-1e9] * 3
    count = 0
    # opaque pieces first, then the ones with cut-out or blended texels (hair, brows, the eyes' wet
    # layer); the face rig's pieces (head, teeth, eyes, lashes) together where the two groups meet,
    # so its vertices are one range
    # and the blended layers last of all (the lab draws its last pieces blended)
    def rank(p):
        cut = p["material"] in (3, 4)
        blended, face = p["material"] in BLENDED, "face" in p
        return (blended, face != blended, cut)
    order = sorted(pack.pieces, key=rank)
    face_first = None
    for p in order:
        ts = tangents(p["verts"], p["tris"])
        for v, t in zip(p["verts"], ts):
            if abs(sum(v["w"]) - 255) > 0:
                raise SystemExit(f"pack_human: {p['name']}: weights that don't sum to 255")
            if max(v["j"]) >= JOINTS:
                raise SystemExit(f"pack_human: {p['name']}: a joint past the rig's 65")
            vb += vertex(v, t, p["material"], p["layers"])
            lo = [min(a, b) for a, b in zip(lo, v["p"])]; hi = [max(a, b) for a, b in zip(hi, v["p"])]
        if "face" in p and face_first is None:
            face_first = count
        first = len(ib)
        ib += [count + k for t in p["tris"] for k in t]
        lines.append(f"P {p['name']} {first} {len(ib) - first} {p['material']}")
        count += len(p["verts"])
    for level, cell in enumerate(LODS, 1):
        for p in order:
            if p["material"] in BLENDED:
                continue
            verts, tris = reduce(p["verts"], p["tris"], cell)
            if not tris:
                continue
            for v, t in zip(verts, tangents(verts, tris)):
                vb += vertex(v, t, p["material"], p["layers"])
            first = len(ib)
            ib += [count + k for t in tris for k in t]
            lines.append(f"D {level} {p['name']} {first} {len(ib) - first} {p['material']}")
            count += len(verts)
    images = [layer_bytes(img, size, flip) for _, _, img, flip in pack.images]
    for k, (role, source, _, _) in enumerate(pack.images):
        lines.append(f"L {k} {role} {source}")
    lines.append("box %.4f %.4f %.4f %.4f %.4f %.4f" % (*lo, *hi))
    lines.append(f"vertices {count} triangles {len(ib) // 3} layers {len(images)} size {size}")
    block = b""
    if pack.face and face_first is not None:
        faces = [p for p in order if "face" in p]
        n = sum(len(p["verts"]) for p in faces)
        block = pack.face(faces)
        # the face rig: its vertices' range and its block's length (the block follows the text, after a NUL)
        lines.append(f"F {face_first} {n} {len(block)}")
    catalog = ("\n".join(lines) + "\n").encode() + (b"\0" + block if block else b"")
    entries = [(KIND_CATALOG, "catalog", catalog), (KIND_VERTICES, "vertices", bytes(vb)),
               (KIND_INDICES, "indices", struct.pack(f"<{len(ib)}I", *ib))]
    entries += [(KIND_LAYER, f"layer{k}", b) for k, b in enumerate(images)]
    data_at = HEADER + ENTRY * len(entries)
    table, blob, at = bytearray(), bytearray(), data_at
    for kind, name, b in entries:
        pad = (-at) % 4
        blob += b"\0" * pad; at += pad
        table += struct.pack("<BBHIII32s", kind, 0, 0, at, len(b), 0, name.encode()[:31])
        blob += b; at += len(b)
    total = data_at + len(blob)
    head = MAGIC + struct.pack("<IIIIIII", len(entries), total, total, HEADER, data_at, 0, 0)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(head + table + blob)
    return catalog.split(b"\0")[0].decode(), total


# ---- the stand-in: Quaternius' superhero male, CC0

def found(path):
    """An image a glTF names, or its sibling without the exporter's "_png" (the kit ships those)."""
    if path.exists():
        return path
    alt = path.with_name(path.name.replace("_png.png", ".png"))
    return alt if alt.exists() else None


def standin(pack, outfit=None):
    skel = people.Skeleton(people.load(f"{OUTFITS}/Male_Peasant.gltf"))
    if len(skel.names) != JOINTS:
        raise SystemExit("pack_human: the people rig isn't 65 joints")
    pack.binds = [tuple(m[r][3] for r in range(3)) for m in skel.world]
    skeletons = {"male": skel}
    base = people.home_path(f"{QUATERNIUS}/Base Characters/Godot - UE")
    hair = people.home_path(f"{QUATERNIUS}/Hairstyles/Rigged to Head Bone/glTF (Godot -Unreal)")
    kinds = {"MI_Superhero_Male": "skin", "MI_Eyes": "eye", "MI_Hair_1": "brow"}
    sources = [(base / "Superhero_Male_FullBody.gltf", None)]
    if outfit is None or outfits.OUTFITS[outfit]["hair"]:
        sources.append((hair / "Hair_SimpleParted.gltf", "hair"))
    for path, force in sources:
        g = people.load(str(path))
        maps = {}
        for m in g["materials"]:
            img = lambda t: found(g["dir"] / g["images"][g["textures"][t["index"]]["source"]]["uri"]) if t else None
            pbr = m.get("pbrMetallicRoughness", {})
            maps[m["name"]] = (img(pbr.get("baseColorTexture")), img(m.get("normalTexture")), img(pbr.get("metallicRoughnessTexture")))
        spec = {"name": path.stem, "body": "male"}
        for pr in people.part_file(spec, str(path), None, skeletons):
            material = force or kinds.get(pr["material"], "cloth")
            b, n, r = maps[pr["material"]]
            if outfit and material == "skin":
                pr["tris"] = dressed_standin(pack, outfit, pr, skel)
            pack.piece(f"{path.stem}/{pr['material']}", pr["verts"], pr["tris"], material, b, n, r)
    pack.tints.update({"skin": (1.0, 1.0, 1.0, 1.0), "eye": (1.0, 1.0, 1.0, 0.4), "hair": (0.16, 0.1, 0.06, 0.55),
                       "brow": (0.16, 0.1, 0.06, 0.6), "cloth": (1.0, 1.0, 1.0, 1.0)})   # the outfit's cloths are in already
    pack.tints.update(EYE_TINTS)
    eyes = next(p for p in pack.pieces if p["material"] == MATERIALS["eye"])
    eye_layers(pack, eyes)


def dressed_standin(pack, outfit, body, skel):
    """The outfit fitted onto the stand-in's body (outfits.fit; the same steps as a MetaHuman's), its
    edges checked in the lab's clips; returns the body's triangles not under the cloth."""
    import metahuman
    xs = [tuple(c * 100.0 for c in v["p"]) for v in body["verts"]]
    ws = [{j: w / 255.0 for j, w in zip(v["j"], v["w"]) if w > 0} for v in body["verts"]]
    target = {"x": xs, "n": [v["n"] for v in body["verts"]], "w": ws,
              "joints": [tuple(m[r][3] * 100.0 for r in range(3)) for m in skel.world]}
    pieces = outfits.fit(outfit, target)
    poses = metahuman.clip_poses([(*b, 0.0, 0.0, 0.0, 1.0) for b in pack.binds], "male")
    bad = outfits.edges(pieces, poses, metahuman.edge_check)
    if bad:
        raise SystemExit(f"standin: torn cloth (edge outliers) in {bad}")
    outfits.packed(pack, pieces, {})
    hide = outfits.covered(xs, target["n"], ws, pieces) | outfits.legs(outfit, ws, [n.lower() for n in skel.names])
    print(f"standin: body points under the cloth: {len(hide)} of {len(xs)}")
    return [t for t in body["tris"] if not all(i in hide for i in t)]


def main(argv):
    if not argv or argv[0] not in ("standin", "metahuman"):
        raise SystemExit(__doc__)
    size = 1024
    if "--size" in argv:
        k = argv.index("--size"); size = int(argv[k + 1]); argv = argv[:k] + argv[k + 2:]
    outfit = None
    if "--outfit" in argv:
        k = argv.index("--outfit"); outfit = argv[k + 1]; argv = argv[:k] + argv[k + 2:]
        if outfit not in outfits.OUTFITS:
            raise SystemExit(f"pack_human: outfits are {sorted(outfits.OUTFITS)}")
    suffix = f"-{outfit}" if outfit else ""
    if argv[0] == "standin":
        pack = Pack("standin", "male", "Quaternius Universal Base Characters|CC0 1.0" + (f"; {outfits.LICENCE}" if outfit else ""))
        pack.outfit = (outfit or "bare", "standin")
        standin(pack, outfit)
        out = Path(argv[1]) if len(argv) > 1 else OUT_DIR / f"standin{suffix}.pack"
    else:
        import metahuman
        if len(argv) < 2:
            raise SystemExit("pack_human: metahuman EXPORT_DIR [OUT.pack]")
        pack = Pack("metahuman", "male", "MetaHuman (private; never committed, never used to train AI models)")
        pack.outfit = (outfit or "student", "human")
        metahuman.build(pack, Path(os.path.expanduser(argv[1])), outfit)
        out = Path(argv[2]) if len(argv) > 2 else OUT_DIR / f"human{suffix}.pack"
        size = max(size, 2048) if "--size" not in sys.argv else size
    catalog, total = write(pack, out, size)
    print(catalog.splitlines()[-1], f"-> {out} ({total} bytes)")


if __name__ == "__main__":
    main(sys.argv[1:])
