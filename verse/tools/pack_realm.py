"""A realm's asset pack (docs/verse/realm-host.md, "Asset packs"): kit art
compiled into one file the realm page streams on entry. Offline tooling;
never shipped or run by a page.

    python3 verse/tools/pack_realm.py verse/packs/NAME.json OUT.pack [LICENCES.md]

The manifest names the kit meshes (from Aethyr's SCN4 scene.bin, packed from
the Quaternius kits by aethyr-cpp tools/pack_assets.py), the sounds, the
texture size, the budget and the licence of every source. Meshes take their
kit's licence from the SCN4 mesh kind (FROM_KIND below); each texture layer
takes the licence of the meshes that use it; a sound names its own. The
compiler refuses a pack over its budget, an asset without a licence, and a
mesh it can't find.

The pack (little endian):
  header, 32 bytes: "VPK1", u32 entry count, u32 file bytes, u32 budget bytes,
                    u32 table offset (32), u32 data offset, u32 0, u32 0
  entries, 48 bytes each: u8 kind, u8 licence, u16 0, u32 offset, u32 bytes,
                    u32 0, char[32] name
  data, each entry from a 4-byte boundary
Kinds: 1 the catalog, 2 vertices, 3 indices, 4 a texture layer (JPEG, or PNG
with a cutout), 5 a sound (MP3). There is one catalog, one vertices and one indices entry, in
that order, then the layers in layer order, then the sounds.

The catalog is text, a line each:
  pack NAME / budget BYTES / bytes BYTES
  L K NAME|LICENCE|URL            licence K
  M NAME FIRST COUNT K X0 Y0 Z0 X1 Y1 Z1   a mesh: its indices, licence, bounds (metres)
  T LAYER SOURCE K                a texture layer and its licence
  S NAME K                        a sound and its licence
  I FIRST COUNT                   identity indices (0, 1, ... COUNT-1) for shapes the realm's shader
                                  draws from the vertex index alone (manifest "identity": COUNT)
Vertices are the Haunted House's (realm.wgsl): 48 bytes, position, normal,
uv, colour with its layer in the alpha byte, then 12 zero bytes; the layer
is the pack's own (0 first).
"""
import io, json, os, struct, sys
from pathlib import Path

KIND_CATALOG, KIND_VERTICES, KIND_INDICES, KIND_LAYER, KIND_SOUND = 1, 2, 3, 4, 5
MAGIC = b"VPK1"
HEADER, ENTRY = 32, 48
# SCN4 mesh kinds (aethyr-cpp tools/pack_assets.py) and the kit each came from.
FROM_KIND = {0: "quaternius-fantasy-props", 5: "quaternius-medieval-village", 7: "quaternius-stylized-nature"}


def home(p):
    return Path(os.path.expanduser(p))


def scene_meshes(path):
    """Every mesh in an SCN4 scene.bin: name -> (kind, list of 44-byte vertices, one per index)."""
    b = path.read_bytes()
    if b[:4] != b"SCN4":
        raise SystemExit(f"{path}: not an SCN4 scene")
    n, nv, ni = struct.unpack_from("<III", b, 4)
    vb = 16 + n * 100
    ib = vb + nv * 44
    out = {}
    for k in range(n):
        at = 16 + k * 100
        name = b[at:at + 48].split(b"\0")[0].decode()
        kind = struct.unpack_from("<I", b, at + 48)[0]
        first, count = struct.unpack_from("<II", b, at + 68)
        out[name] = (kind, first, count)
    return b, vb, ib, out


def image(stem, size):
    """A texture layer at size x size (PIL): JPEG, or PNG where the source has a cutout."""
    from PIL import Image
    path = next((stem.with_suffix(x) for x in (".jpg", ".png") if stem.with_suffix(x).exists()), None)
    if not path:
        raise SystemExit(f"{stem}: no .jpg or .png")
    im = Image.open(path)
    cutout = path.suffix == ".png"
    im = im.convert("RGBA" if cutout else "RGB")
    if im.size != (size, size):
        im = im.resize((size, size), Image.LANCZOS)
    buf = io.BytesIO()
    if cutout:
        im.save(buf, "PNG", optimize=True)
    else:
        im.save(buf, "JPEG", quality=82, optimize=True)
    return path, buf.getvalue()


def build(manifest_path):
    m = json.loads(Path(manifest_path).read_text())
    licences = m["licences"]
    keys = list(licences)
    def licence_of(key, what):
        if key not in licences:
            raise SystemExit(f"{what}: no licence (give it one of {', '.join(keys)})")
        return keys.index(key)
    b, vb, ib, meshes = scene_meshes(home(m["scene"]))
    vertices = bytearray()
    indices = []
    layers = []          # SCN4 layer numbers, in the pack's order
    layer_licence = {}
    mesh_lines = []
    for item in m["meshes"]:
        name = item["name"] if isinstance(item, dict) else item
        if name not in meshes:
            raise SystemExit(f"{name}: not in {m['scene']}")
        kind, first, count = meshes[name]
        key = (item.get("licence") if isinstance(item, dict) else None) or FROM_KIND.get(kind)
        lic = licence_of(key, f"mesh {name}")
        start = len(indices)
        unique = {}
        lo, hi = [1e9] * 3, [-1e9] * 3
        for i in range(count):
            v = struct.unpack_from("<I", b, ib + (first + i) * 4)[0]
            raw = b[vb + v * 44: vb + v * 44 + 44]
            layer = raw[35]
            if layer not in layers:
                layers.append(layer)
            prev = layer_licence.setdefault(layer, lic)
            if prev != lic:
                layer_licence[layer] = 255   # shared by two sources: listed under both below
            if raw not in unique:
                unique[raw] = len(vertices) // 48
                p = struct.unpack_from("<3f", raw)
                lo = [min(lo[k], p[k]) for k in range(3)]
                hi = [max(hi[k], p[k]) for k in range(3)]
                vertices += raw[:35] + bytes([layers.index(layer)]) + bytes(12)
            indices.append(unique[raw])
        mesh_lines.append(f"M {name} {start} {len(indices) - start} {lic} " + " ".join(f"{x:.3f}" for x in lo + hi))
    size = int(m.get("texture_size", 512))
    entries = []   # (kind, licence, name, bytes)
    layer_lines, sound_lines = [], []
    for k, layer in enumerate(layers):
        lic = layer_licence[layer]
        if lic == 255:
            raise SystemExit(f"layer {layer} is used by meshes of two licences; split them")
        src, data = image(home(m["layers"].format(n=layer)), size)
        entries.append((KIND_LAYER, lic, f"layer{k}", data))
        layer_lines.append(f"T {k} {src.name} {lic}")
    for s in m.get("sounds", []):
        lic = licence_of(s.get("licence"), f"sound {s['name']}")
        entries.append((KIND_SOUND, lic, s["name"], home(s["file"]).read_bytes()))
        sound_lines.append(f"S {s['name']} {lic}")
    identity_lines = []
    if int(m.get("identity", 0)) > 0:
        identity_lines.append(f"I {len(indices)} {int(m['identity'])}")
        indices += list(range(int(m["identity"])))
    vbytes = bytes(vertices)
    ibytes = struct.pack(f"<{len(indices)}I", *indices)
    budget = int(m["budget"])
    def catalog(total):
        lines = [f"pack {m['pack']}", f"budget {budget}", f"bytes {total}"]
        lines += [f"L {k} {licences[key]['name']}|{licences[key]['licence']}|{licences[key]['url']}" for k, key in enumerate(keys)]
        return ("\n".join(lines + mesh_lines + identity_lines + layer_lines + sound_lines) + "\n").encode()
    # The catalog names the file's size, which depends on the catalog: settle it.
    total = 0
    for _ in range(4):
        body = [(KIND_CATALOG, 255, "catalog", catalog(total)), (KIND_VERTICES, 255, "vertices", vbytes), (KIND_INDICES, 255, "indices", ibytes)] + entries
        at = HEADER + ENTRY * len(body)
        data_at = at
        offsets = []
        for kind, lic, name, data in body:
            offsets.append(at)
            at += (len(data) + 3) & ~3
        if at == total:
            break
        total = at
    out = bytearray(total)
    struct.pack_into("<4sIIIIIII", out, 0, MAGIC, len(body), total, budget, HEADER, data_at, 0, 0)
    for k, ((kind, lic, name, data), off) in enumerate(zip(body, offsets)):
        if len(name.encode()) > 31:
            raise SystemExit(f"{name}: a name is at most 31 bytes")
        struct.pack_into("<BBHIII32s", out, HEADER + k * ENTRY, kind, lic, 0, off, len(data), 0, name.encode())
        out[off:off + len(data)] = data
    if total > budget:
        raise SystemExit(f"{m['pack']}: {total} bytes, over its budget of {budget}")
    return m, keys, bytes(out), mesh_lines, layer_lines, sound_lines


def licences_md(m, keys, mesh_lines, layer_lines, sound_lines, total):
    lic = m["licences"]
    name = lambda k: lic[keys[int(k)]]["name"]
    lines = [f"# Licences: the {m['title']} pack", "",
             f"Every asset in `{m['pack']}.pack` ({total:,} bytes of its {int(m['budget']):,}-byte budget) and its licence. Written by `verse/tools/pack_realm.py` from `verse/packs/{m['pack']}.json`.", "",
             "## Sources", ""]
    for key in keys:
        l = lic[key]
        lines.append(f"- {l['name']}: {l['licence']}, {l['url']}")
    lines += ["", "## Assets", "", "| Asset | Kind | Licence |", "| --- | --- | --- |"]
    for x in mesh_lines:
        p = x.split()
        lines.append(f"| {p[1]} | mesh | {name(p[4])} |")
    for x in layer_lines:
        p = x.split()
        lines.append(f"| texture {p[1]} ({p[2]}) | texture | {name(p[3])} |")
    for x in sound_lines:
        p = x.split()
        lines.append(f"| {p[1]} | sound | {name(p[2])} |")
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    if len(sys.argv) not in (3, 4):
        raise SystemExit(__doc__.split("\n\n")[1])
    m, keys, data, ml, ll, sl = build(sys.argv[1])
    Path(sys.argv[2]).parent.mkdir(parents=True, exist_ok=True)
    Path(sys.argv[2]).write_bytes(data)
    if len(sys.argv) == 4:
        Path(sys.argv[3]).write_text(licences_md(m, keys, ml, ll, sl, len(data)))
    print(f"{m['pack']}: {len(data):,} bytes of {int(m['budget']):,}; {len(ml)} meshes, {len(ll)} layers, {len(sl)} sound{"" if len(sl) == 1 else "s"}")
