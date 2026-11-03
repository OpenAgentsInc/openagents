"""MetaHuman Lab's human pack (docs/verse/metahuman-support.md, "MetaHuman Lab"): one skinned
human, its meshes, skin weights on the 65-joint people rig and its texture maps, compiled into
one VPK1 file (verse/tools/pack_realm.py has the container) that the lab's page loads with
`private:human` (coax/web/host.js). Offline tooling; never shipped or run by a page.

    python3 verse/tools/humans/pack_human.py standin [OUT.pack]
    python3 verse/tools/humans/pack_human.py metahuman EXPORT_DIR [OUT.pack]

OUT.pack defaults to ~/.openagents/verse/private-build/web/NAME.pack (standin.pack or
human.pack), the directory coax/web/serve.c serves as /private/ on a local address. Nothing
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
  B J X Y Z                     joint J's bind position (metres, the figure's own space, the
                                rig's rest pose: +y up, +z forward, the figure's left +x)
  M MATERIAL R G B ROUGH        a material's tint (thousandths, linear) and roughness scale
  P NAME FIRST COUNT MATERIAL   a piece: its indices into the index buffer, its material
  L LAYER ROLE SOURCE           a layer: base, normal (tangent space, green toward +v, down the
                                image: a map whose green points up is flipped, green_up()) or
                                rough (G)
  box X0 Y0 Z0 X1 Y1 Z1         the bind pose's bounds

A vertex is 32 bytes (vertex() below): position (x, y, z unorm16 across SPAN from LOW), its
piece's material, the normal and tangent (octahedral, 2 x unorm16 each), the bitangent's
sign, uv (2 x unorm16), four joints (u8) and their weights (u8, 255ths summing to 255), and the
layers its material reads (base, normal, rough; 255 none).

Materials: 1 skin, 2 eye, 3 hair, 4 brow, 5 cloth, 6 teeth, 7 eye wet layer, 8 to 16 more
cloths (a colour each, from the portrait's materials). A material with no base layer is its tint.
"""
import io, math, os, struct, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
import people  # noqa: E402

MAGIC, HEADER, ENTRY = b"VPK1", 32, 48
KIND_CATALOG, KIND_VERTICES, KIND_INDICES, KIND_LAYER = 1, 2, 3, 4
SPAN, LOW = 2.5, -0.25
JOINTS = 65
NONE = 255
MATERIALS = {"skin": 1, "eye": 2, "hair": 3, "brow": 4, "cloth": 5, "teeth": 6, "wet": 7}
MATERIALS.update({f"cloth{k}": 8 + k for k in range(9)})   # more cloths, each its own colour
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


class Pack:
    """What a source hands the writer: pieces (verts in the rig's rest pose, tris, material,
    layer images), bind positions and material tints."""
    def __init__(self, name, body, licence):
        self.name, self.body, self.licence = name, body, licence
        self.pieces, self.binds, self.tints = [], [], {}
        self.images = []        # (role, source name, image path or PIL image, flip green)
        self.image_of = {}

    def layer(self, role, path, flip=False):
        if path is None:
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
    for j, b in enumerate(pack.binds):
        lines.append("B %d %.6f %.6f %.6f" % (j, *b))
    for m, (r, g, b, rough) in sorted(pack.tints.items()):
        lines.append("M %d %d %d %d %d" % (MATERIALS[m], round(r * 1000), round(g * 1000), round(b * 1000), round(rough * 1000)))
    lo, hi = [1e9] * 3, [-1e9] * 3
    count = 0
    # opaque pieces first, then the ones with cut-out or blended texels (hair, brows, the eyes' wet layer)
    order = sorted(pack.pieces, key=lambda p: p["material"] in (3, 4, 7))
    for p in order:
        ts = tangents(p["verts"], p["tris"])
        for v, t in zip(p["verts"], ts):
            if abs(sum(v["w"]) - 255) > 0:
                raise SystemExit(f"pack_human: {p['name']}: weights that don't sum to 255")
            if max(v["j"]) >= JOINTS:
                raise SystemExit(f"pack_human: {p['name']}: a joint past the rig's 65")
            vb += vertex(v, t, p["material"], p["layers"])
            lo = [min(a, b) for a, b in zip(lo, v["p"])]; hi = [max(a, b) for a, b in zip(hi, v["p"])]
        first = len(ib)
        ib += [count + k for t in p["tris"] for k in t]
        lines.append(f"P {p['name']} {first} {len(ib) - first} {p['material']}")
        count += len(p["verts"])
    images = [layer_bytes(img, size, flip) for _, _, img, flip in pack.images]
    for k, (role, source, _, _) in enumerate(pack.images):
        lines.append(f"L {k} {role} {source}")
    lines.append("box %.4f %.4f %.4f %.4f %.4f %.4f" % (*lo, *hi))
    lines.append(f"vertices {count} triangles {len(ib) // 3} layers {len(images)} size {size}")
    catalog = ("\n".join(lines) + "\n").encode()
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
    return catalog.decode(), total


# ---- the stand-in: Quaternius' superhero male, CC0

def found(path):
    """An image a glTF names, or its sibling without the exporter's "_png" (the kit ships those)."""
    if path.exists():
        return path
    alt = path.with_name(path.name.replace("_png.png", ".png"))
    return alt if alt.exists() else None


def standin(pack):
    skel = people.Skeleton(people.load(f"{OUTFITS}/Male_Peasant.gltf"))
    if len(skel.names) != JOINTS:
        raise SystemExit("pack_human: the people rig isn't 65 joints")
    pack.binds = [tuple(m[r][3] for r in range(3)) for m in skel.world]
    skeletons = {"male": skel}
    base = people.home_path(f"{QUATERNIUS}/Base Characters/Godot - UE")
    hair = people.home_path(f"{QUATERNIUS}/Hairstyles/Rigged to Head Bone/glTF (Godot -Unreal)")
    kinds = {"MI_Superhero_Male": "skin", "MI_Eyes": "eye", "MI_Hair_1": "brow"}
    sources = [(base / "Superhero_Male_FullBody.gltf", None), (hair / "Hair_SimpleParted.gltf", "hair")]
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
            pack.piece(f"{path.stem}/{pr['material']}", pr["verts"], pr["tris"], material, b, n, r)
    pack.tints = {"skin": (1.0, 1.0, 1.0, 1.0), "eye": (1.0, 1.0, 1.0, 0.4), "hair": (0.16, 0.1, 0.06, 0.55),
                  "brow": (0.16, 0.1, 0.06, 0.6), "cloth": (1.0, 1.0, 1.0, 1.0)}


def main(argv):
    if not argv or argv[0] not in ("standin", "metahuman"):
        raise SystemExit(__doc__)
    size = 1024
    if "--size" in argv:
        k = argv.index("--size"); size = int(argv[k + 1]); argv = argv[:k] + argv[k + 2:]
    if argv[0] == "standin":
        pack = Pack("standin", "male", "Quaternius Universal Base Characters|CC0 1.0")
        standin(pack)
        out = Path(argv[1]) if len(argv) > 1 else OUT_DIR / "standin.pack"
    else:
        import metahuman
        if len(argv) < 2:
            raise SystemExit("pack_human: metahuman EXPORT_DIR [OUT.pack]")
        pack = Pack("metahuman", "male", "MetaHuman (private; never committed, never used to train AI models)")
        metahuman.build(pack, Path(os.path.expanduser(argv[1])))
        out = Path(argv[2]) if len(argv) > 2 else OUT_DIR / "human.pack"
        size = max(size, 2048) if "--size" not in sys.argv else size
    catalog, total = write(pack, out, size)
    print(catalog.splitlines()[-1], f"-> {out} ({total} bytes)")


if __name__ == "__main__":
    main(sys.argv[1:])
