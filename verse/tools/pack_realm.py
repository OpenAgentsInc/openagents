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
  M ... canopy SHAPE R G B         a tree's leaves (a "_Leaves" mesh) end their M line so: SHAPE
                                  sphere, or ellipsoid for a pine or cypress, and the leaves' mean
                                  colour (linear, thousandths: the texels under its vertices times
                                  their colour), so the far trees that stand for it match it
  T LAYER SOURCE K                a texture layer and its licence
  S NAME K                        a sound and its licence
  I FIRST COUNT                   identity indices (0, 1, ... COUNT-1) for shapes the realm's shader
                                  draws from the vertex index alone (manifest "identity": COUNT)
  C NAME FIRST COUNT K WORD VERTICES JOINTS
                                  a skinned character: its indices (numbered from its own first
                                  vertex), licence, the vertex buffer word its vertices start at,
                                  how many there are and its joints
A pack names its art style (manifest "style": lowpoly, painterly or
photoreal; none is lowpoly) and the catalog says it first: "style NAME".
A painterly pack's layers must pass the style's texture rules
(docs/verse/art-styles.md, 10. Textures and packs): at most 512 texels a
side (256 when the manifest names no texture_size), no normal, roughness or
detail maps, fine-detail energy at most 0.035 (the RMS of the luminance less
its own 4x-down-then-up copy over opaque texels, sRGB 0..1), and mean HSV
saturation 0.20 to 0.70 and value 0.18 to 0.92. "paint": true (or a list of
SCN4 layer numbers) runs the paint pass first: a Kuwahara filter (radius 4
at 512, 2 below), 24 colours, saturation and value pulled into range. A
layer that still fails is refused with its numbers; the source stays as it
was.

Vertices are the Haunted House's (realm.wgsl): 48 bytes, position, normal,
uv, colour with its layer in the alpha byte, then 12 bytes; the layer is the
pack's own (0 first). The 12 are zero but on a canopy's vertices, where the
first 4 are its blob (docs/verse/art-styles.md, 7. Foliage): the direction
from the canopy's centre (the middle of its bounds) as three signed bytes
(n * 127; over the half sizes squared for an ellipsoid, then normalized),
and its share of the tree's height (y over the leaves' top, 255ths), which
the wind's sway scales by. A character's vertices follow the kit's, 32
bytes each: position (3 x f32), the normal as three signed bytes (n / 127)
with the layer in the fourth, uv (2 x f32), four joints (u8) and four
weights (u8, 255ths summing to 255).

A character (manifest "character") is taken whole from a skinned character
in a VTP3 pack of the old Rust Verse engine (crates/verse-zone-everglade/src/
zones/everglade_pack/format.rs before the Nuke), read from git at a pinned
revision and checked against its SHA-256: its mesh, its base-colour images
(each a layer, scaled up to texture_size by repeating texels, PNG, so no
texel changes) and, written to the manifest's "rig" path as Coax, its
skeleton and clips, keys as they were. Its triangles are counted apart
from the kit's.
"""
import hashlib, io, json, os, struct, subprocess, sys, zlib
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


STYLES = ("lowpoly", "painterly", "photoreal")
# The painterly texture rules (art-styles.md section 9 and 10).
PAINT_MAX_SIZE, PAINT_DEFAULT_SIZE = 512, 256
FINE_MAX = 0.035
SAT_RANGE, VAL_RANGE = (0.20, 0.70), (0.18, 0.92)
PAINT_COLOURS = 24
# Map kinds a painterly pack never carries (the pack format has none; a manifest naming one is refused).
MAP_KEYS = ("normal", "normals", "normal_map", "roughness", "roughness_map", "detail", "detail_map", "metallic", "metallic_map")


def layer_measures(im):
    """(fine-detail energy, mean saturation, mean value) of an image over its opaque texels."""
    from PIL import Image, ImageChops
    im = im.convert("RGBA")
    w, h = im.size
    lum = im.convert("RGB").convert("L")
    # the 4x-down copy of the opaque texels only (weighted by coverage), so a cutout's hidden texels don't count
    cover = im.getchannel("A").point(lambda x: 255 if x >= 128 else 0)
    small = (max(1, w // 4), max(1, h // 4))
    held = ImageChops.multiply(lum, cover).convert("F").resize(small, Image.BOX).resize((w, h), Image.BILINEAR)
    seen = cover.convert("F").resize(small, Image.BOX).resize((w, h), Image.BILINEAR)
    flat = lambda i: getattr(i, "get_flattened_data", i.getdata)()
    held, seen = flat(held), flat(seen)
    alpha, a = cover.tobytes(), lum.tobytes()
    b = [held[i] * 255.0 / seen[i] if seen[i] > 0 else a[i] for i in range(w * h)]
    hsv = im.convert("RGB").convert("HSV").tobytes()
    n, e, sat, val = 0, 0.0, 0.0, 0.0
    for i in range(w * h):
        if alpha[i] < 128:
            continue
        d = (a[i] - b[i]) / 255.0
        e += d * d; sat += hsv[3 * i + 1]; val += hsv[3 * i + 2]; n += 1
    if not n:
        return 0.0, 0.0, 0.0
    return (e / n) ** 0.5, sat / n / 255.0, val / n / 255.0


def layer_refusal(name, im):
    """Why a layer fails the painterly rules, or None."""
    e, sat, val = layer_measures(im)
    why = []
    if e > FINE_MAX:
        why.append(f"fine-detail energy {e:.3f} (at most {FINE_MAX})")
    if not SAT_RANGE[0] <= sat <= SAT_RANGE[1]:
        why.append(f"mean saturation {sat:.2f} (from {SAT_RANGE[0]} to {SAT_RANGE[1]})")
    if not VAL_RANGE[0] <= val <= VAL_RANGE[1]:
        why.append(f"mean value {val:.2f} (from {VAL_RANGE[0]} to {VAL_RANGE[1]})")
    return f"{name}: " + "; ".join(why) if why else None


def kuwahara(rgb, r):
    """A Kuwahara filter of radius r over an RGB image: each texel takes the mean of the
    least varied of the four (r+1)x(r+1) squares it is a corner of (summed-area tables)."""
    from PIL import Image
    w, h = rgb.size
    px = rgb.tobytes()
    W = w + 1
    sums = [[0] * (W * (h + 1)) for _ in range(5)]   # r, g, b, luminance, luminance squared
    for y in range(h):
        run = [0] * 5
        row, up = (y + 1) * W, y * W
        for x in range(w):
            i = 3 * (y * w + x)
            R, G, B = px[i], px[i + 1], px[i + 2]
            L = (R * 299 + G * 587 + B * 114) // 1000
            v = (R, G, B, L, L * L)
            for k in range(5):
                run[k] += v[k]
                sums[k][row + x + 1] = sums[k][up + x + 1] + run[k]
    def box(k, x0, y0, x1, y1):   # inclusive texels, clamped
        t = sums[k]
        return t[(y1 + 1) * W + x1 + 1] - t[y0 * W + x1 + 1] - t[(y1 + 1) * W + x0] + t[y0 * W + x0]
    out = bytearray(3 * w * h)
    for y in range(h):
        for x in range(w):
            best, pick = None, None
            for x0, x1 in ((max(0, x - r), x), (x, min(w - 1, x + r))):
                for y0, y1 in ((max(0, y - r), y), (y, min(h - 1, y + r))):
                    n = (x1 - x0 + 1) * (y1 - y0 + 1)
                    m = box(3, x0, y0, x1, y1) / n
                    var = box(4, x0, y0, x1, y1) / n - m * m
                    if best is None or var < best:
                        best, pick = var, (x0, y0, x1, y1, n)
            x0, y0, x1, y1, n = pick
            i = 3 * (y * w + x)
            out[i] = box(0, x0, y0, x1, y1) // n
            out[i + 1] = box(1, x0, y0, x1, y1) // n
            out[i + 2] = box(2, x0, y0, x1, y1) // n
    return Image.frombytes("RGB", (w, h), bytes(out))


def paint(im):
    """The paint pass: Kuwahara (radius 4 at 512, 2 below), 24 colours, then each opaque texel's
    saturation and value pulled into the albedo rules. Keeps the alpha (cutouts)."""
    from PIL import Image
    im = im.convert("RGBA")
    alpha = im.getchannel("A")
    rgb = kuwahara(im.convert("RGB"), 4 if max(im.size) >= 512 else 2)
    rgb = rgb.quantize(colors=PAINT_COLOURS, method=Image.Quantize.MEDIANCUT).convert("RGB")
    hsv = bytearray(rgb.convert("HSV").tobytes())
    lo_s, hi_s = round(SAT_RANGE[0] * 255) + 1, round(SAT_RANGE[1] * 255) - 1
    lo_v, hi_v = round(VAL_RANGE[0] * 255) + 1, round(VAL_RANGE[1] * 255) - 1
    for i in range(1, len(hsv), 3):
        hsv[i] = min(hi_s, max(lo_s, hsv[i]))
        hsv[i + 1] = min(hi_v, max(lo_v, hsv[i + 1]))
    out = Image.frombytes("HSV", rgb.size, bytes(hsv)).convert("RGB").convert("RGBA")
    out.putalpha(alpha)
    return out


def image(stem, size, style="lowpoly", painted=False):
    """A texture layer at size x size (PIL): JPEG, or PNG where the source has a cutout.
    A painterly pack's layer is painted first when asked, then must pass the rules."""
    from PIL import Image
    path = next((stem.with_suffix(x) for x in (".jpg", ".png") if stem.with_suffix(x).exists()), None)
    if not path:
        raise SystemExit(f"{stem}: no .jpg or .png")
    im = Image.open(path)
    cutout = path.suffix == ".png"
    im = im.convert("RGBA" if cutout else "RGB")
    if im.size != (size, size):
        im = im.resize((size, size), Image.LANCZOS)
    if style == "painterly":
        if painted:
            im = paint(im).convert("RGBA" if cutout else "RGB")
        why = layer_refusal(path.name + (" (painted)" if painted else ""), im)
        if why:
            raise SystemExit(f"painterly layer refused: {why}" + ("" if painted else '; "paint": true runs the paint pass'))
    buf = io.BytesIO()
    if cutout:
        im.save(buf, "PNG", optimize=True)
    else:
        im.save(buf, "JPEG", quality=82, optimize=True)
    return path, buf.getvalue()


def canopy_shape(name):
    """A tree's leaves shade as a blob: an ellipsoid for a pine or cypress, else a sphere; None for any other mesh."""
    if not name.endswith("_Leaves"):
        return None
    return "ellipsoid" if any(k in name for k in ("Pine", "Cypress")) else "sphere"


def canopy_blob(vertices, vstart, lo, hi, ellipsoid):
    """Writes each canopy vertex's blob direction and height share into its first spare 4 bytes."""
    c = [(lo[k] + hi[k]) / 2 for k in range(3)]
    half = [max((hi[k] - lo[k]) / 2, 0.001) for k in range(3)]
    top = max(hi[1], 0.001)
    for v in range(vstart, len(vertices) // 48):
        p = struct.unpack_from("<3f", vertices, v * 48)
        d = [(p[k] - c[k]) / (half[k] * half[k] if ellipsoid else 1.0) for k in range(3)]
        n = sum(x * x for x in d) ** 0.5
        d = [x / n for x in d] if n > 1e-9 else [0.0, 1.0, 0.0]
        share = min(max(p[1] / top, 0.0), 1.0)
        struct.pack_into("<3bB", vertices, v * 48 + 36, *[max(-127, min(127, round(x * 127))) for x in d], round(share * 255))


def canopy_colour(vertices, vstart, vend, layer_images):
    """A canopy's mean colour (linear, thousandths): the opaque texels of the layers its vertices use
    (a leaf card's corners are clear, so not the texel under each vertex), times their mean colour."""
    from PIL import Image
    lin = [((x / 255) / 12.92) if x <= 10 else (((x / 255) + 0.055) / 1.055) ** 2.4 for x in range(256)]
    layers, colour, n = {}, [0.0, 0.0, 0.0], 0
    for v in range(vstart, vend):
        layers[vertices[v * 48 + 35]] = 1
        for k in range(3):
            colour[k] += vertices[v * 48 + 32 + k] / 255
        n += 1
    total, texels = [0.0, 0.0, 0.0], 0
    for layer in layers:
        im = Image.open(io.BytesIO(layer_images[layer])).convert("RGBA")
        px = im.tobytes()
        for i in range(0, len(px), 4):
            if px[i + 3] >= 90:
                total[0] += lin[px[i]]; total[1] += lin[px[i + 1]]; total[2] += lin[px[i + 2]]; texels += 1
    return [round(1000 * total[k] / max(texels, 1) * colour[k] / max(n, 1)) for k in range(3)]


class Reader:
    """Little-endian reads over bytes, refusing to run past their end."""
    def __init__(self, b):
        self.b, self.o = b, 0
    def take(self, n):
        if self.o + n > len(self.b):
            raise SystemExit("character pack: runs past its end")
        v = self.b[self.o:self.o + n]; self.o += n; return v
    def u8(self): return self.take(1)[0]
    def u16(self): return struct.unpack("<H", self.take(2))[0]
    def i16(self): return struct.unpack("<h", self.take(2))[0]
    def u32(self): return struct.unpack("<I", self.take(4))[0]
    def f32(self, n=1):
        v = struct.unpack(f"<{n}f", self.take(4 * n)); return v if n > 1 else v[0]
    def name(self): return self.take(self.u8()).decode()


def vtp_planes(r, colored):
    """A VTP3 primitive's quantized vertex planes: positions, uvs, i8 normals."""
    n = r.u32()
    po, ps, uo, us = r.f32(3), r.f32(3), r.f32(2), r.f32(2)
    x, y, z, u, v = (struct.unpack(f"<{n}H", r.take(2 * n)) for _ in range(5))
    normals = r.take(3 * n)
    if colored:
        r.take(4 * n)
    pos = [(po[0] + ps[0] * x[i], po[1] + ps[1] * y[i], po[2] + ps[2] * z[i]) for i in range(n)]
    uv = [(uo[0] + us[0] * u[i], uo[1] + us[1] * v[i]) for i in range(n)]
    return pos, uv, normals


def vtp_character(r):
    c = {"name": r.name(), "joints": [], "prims": [], "clips": []}
    for _ in range(r.u16()):
        c["joints"].append({"parent": r.i16(), "t": r.f32(3), "r": r.f32(4), "s": r.f32(3), "bind": r.f32(16)})
    for _ in range(r.u8()):
        material = r.u16()
        pos, uv, normals = vtp_planes(r, False)
        n = len(pos)
        joints, weights = r.take(4 * n), r.take(4 * n)
        count = r.u32()
        c["prims"].append({"material": material, "pos": pos, "uv": uv, "normals": normals, "joints": joints,
                           "weights": weights, "indices": struct.unpack(f"<{count}H", r.take(2 * count))})
    for _ in range(r.u8()):
        clip = {"name": r.name(), "duration": r.f32(), "distance": r.f32(), "tracks": []}
        for _ in range(r.u16()):
            track = {"joint": r.u16()}
            for channel, width in (("t", 3), ("r", 4), ("s", 3)):
                track[channel] = [(r.f32(), r.f32(width)) for _ in range(r.u16())]
            clip["tracks"].append(track)
        c["clips"].append(clip)
    return c


def vtp_pack(b):
    """The textures, materials and character of a VTP3 pack (its models are skipped)."""
    r = Reader(b)
    if r.take(8) != b"VTP3\r\n\x1a\n":
        raise SystemExit("character pack: not VTP3")
    textures = []
    for _ in range(r.u32()):
        name, w, h = r.name(), r.u32(), r.u32()
        textures.append({"name": name, "w": w, "h": h, "png": r.take(r.u32())})
    size, stored = r.u32(), r.u32()
    body = zlib.decompress(r.take(stored), -15)
    if len(body) != size:
        raise SystemExit("character pack: its body is not its stated length")
    r = Reader(body)
    materials = []
    for _ in range(r.u32()):
        name, texture, colour = r.name(), r.u16(), r.f32(4)
        alpha, cutoff, sides = r.u8(), r.f32(), r.u8()
        materials.append({"name": name, "texture": texture, "colour": colour, "alpha": alpha})
    for _ in range(r.u32()):
        r.name()
        for _ in range(r.u8()):
            r.u16(); colored = r.u8(); vtp_planes(r, colored); r.take(2 * r.u32())
    if not r.u8():
        raise SystemExit("character pack: it has no character")
    return textures, materials, vtp_character(r)


def character_source(spec):
    """The pinned VTP3 pack, read from git at its revision and checked."""
    root = Path(__file__).resolve().parents[2]
    b = subprocess.run(["git", "-C", str(root), "show", f"{spec['revision']}:{spec['path']}"],
                       capture_output=True, check=True).stdout
    if hashlib.sha256(b).hexdigest() != spec["sha256"]:
        raise SystemExit(f"{spec['path']}: not the pinned character pack")
    return vtp_pack(b)


def texels_up(png, size):
    """A PNG at size x size, each source texel repeated (the image must divide it)."""
    from PIL import Image
    im = Image.open(io.BytesIO(png)).convert("RGBA")
    if size % im.size[0] or size % im.size[1]:
        raise SystemExit(f"a {im.size[0]} x {im.size[1]} character image doesn't divide {size}")
    if im.getextrema()[3] == (255, 255):
        im = im.convert("RGB")
    im = im.resize((size, size), Image.NEAREST)
    buf = io.BytesIO()
    im.save(buf, "PNG", optimize=True)
    return buf.getvalue()


def posed_joints(c, clip, time):
    """Every joint's position `time` seconds into `clip`, as the old engine sampled it
    (crates/verse-engine/src/animation.rs sample, matrices), in f64: the check the
    rig's Coax playback is held to."""
    import math
    def pair(keys, t, default):
        if not keys:
            return default, default, 0.0
        i = max(sum(1 for k in keys if k[0] <= t) - 1, 0)
        a = keys[i]; b = keys[i + 1] if i + 1 < len(keys) else a
        f = min(max((t - a[0]) / (b[0] - a[0]), 0.0), 1.0) if b[0] > a[0] else 0.0
        return a[1], b[1], f
    def unit(q):
        l = math.sqrt(sum(x * x for x in q)); return tuple(x / l for x in q) if l * l > 1e-5 else (0, 0, 0, 1)
    def slerp(a, b, f):
        d = sum(x * y for x, y in zip(a, b))
        if d < 0: b = tuple(-x for x in b); d = -d
        if d > 0.9995: return unit(tuple(x + (y - x) * f for x, y in zip(a, b)))
        th = math.acos(d); k = 1 / math.sin(th)
        return tuple((x * math.sin(th * (1 - f)) + y * math.sin(th * f)) * k for x, y in zip(a, b))
    def qmul(a, b):
        ax, ay, az, aw = a; bx, by, bz, bw = b
        return (aw*bx + ax*bw + ay*bz - az*by, aw*by - ax*bz + ay*bw + az*bx, aw*bz + ax*by - ay*bx + az*bw, aw*bw - ax*bx - ay*by - az*bz)
    def rot(q, v):
        u = q[:3]; w = q[3]
        cr = lambda a, b: (a[1]*b[2] - a[2]*b[1], a[2]*b[0] - a[0]*b[2], a[0]*b[1] - a[1]*b[0])
        t = tuple(2 * x for x in cr(u, v)); t2 = cr(u, t)
        return tuple(v[i] + w * t[i] + t2[i] for i in range(3))
    k = c["clips"][clip]; t = time % k["duration"]
    tracks = {tr["joint"]: tr for tr in k["tracks"]}
    gq, gt = [], []
    for j, joint in enumerate(c["joints"]):
        tr = tracks.get(j)
        a, b, f = pair(tr["t"] if tr else [], t, joint["t"]); lt = tuple(x + (y - x) * f for x, y in zip(a, b))
        a, b, f = pair(tr["r"] if tr else [], t, joint["r"]); lq = slerp(unit(a), unit(b), f) if tr and tr["r"] else tuple(joint["r"])
        if joint["parent"] < 0: gq.append(lq); gt.append(lt)
        else:
            p = joint["parent"]; gq.append(unit(qmul(gq[p], lq))); gt.append(tuple(x + y for x, y in zip(gt[p], rot(gq[p], lt))))
    return gt, gq


def skinned_point(c, clip, time, j, x):
    """Point x (in the character's space) skinned wholly by joint j: posed times inverse bind."""
    gt, gq = posed_joints(c, clip, time)
    b = c["joints"][j]["bind"]
    y = tuple(b[r] * x[0] + b[4 + r] * x[1] + b[8 + r] * x[2] + b[12 + r] for r in range(3))
    q = gq[j]; u = q[:3]; w = q[3]
    cr = lambda a, v: (a[1]*v[2] - a[2]*v[1], a[2]*v[0] - a[0]*v[2], a[0]*v[1] - a[1]*v[0])
    t = tuple(2 * k for k in cr(u, y)); t2 = cr(u, t)
    return tuple(y[i] + w * t[i] + t2[i] + gt[j][i] for i in range(3))


def f9(x):
    """An f32 as Coax reads it back exactly."""
    t = "%.9g" % x
    return t if any(c in t for c in ".en") else t + ".0"


# The joints the rig's checks pose: the root, the spine, a hand, a foot and the last.
CHECK_JOINTS = (0, 13, 20, 26, 33, 45, 60, 74)


def rig_coax(m, c, prefix, triangles):
    """The character's skeleton and clips as Coax constants, for verse/skin.coax."""
    J = c["joints"]
    for j in J:
        if max(abs(v - 1) for v in j["s"]) > 1e-5:
            raise SystemExit(f"{c['name']}: a joint's rest scale isn't 1")
    tt, tr, kt, kr = [], [], [], []
    for clip in c["clips"]:
        tracks = {t["joint"]: t for t in clip["tracks"]}
        for j in range(len(J)):
            t = tracks.get(j)
            if t and any(max(abs(v - 1) for v in k[1]) > 1e-5 for k in t["s"]):
                raise SystemExit(f"{c['name']}: clip {clip['name']} scales a joint")
            for keys, out, track in ((t["t"] if t else [], kt, tt), (t["r"] if t else [], kr, tr)):
                track += [len(out), len(keys)]
                for time, value in keys:
                    out.append((time,) + tuple(value))
    def arr(name, kind, values, per_line, fmt):
        lines = [", ".join(fmt(v) for v in values[i:i + per_line]) for i in range(0, len(values), per_line)]
        return f"{name} :: [{len(values)}] {kind}.[\n  " + ",\n  ".join(lines) + "\n];\n"
    flat = lambda rows: [v for row in rows for v in row]
    P = prefix
    sizes = {"PARENT": len(J), "REST": 7 * len(J), "BIND": 12 * len(J), "DURATION": len(c["clips"]),
             "DISTANCE": len(c["clips"]), "TRACK_T": len(tt), "TRACK_R": len(tr), "KEYS_T": 4 * len(kt), "KEYS_R": 5 * len(kr)}
    out = [f"// Generated by verse/tools/pack_realm.py from verse/packs/{m['pack']}.json; do not edit.",
           f"// The skeleton and clips of {c['name']}, the old Rust Verse engine's character, as its last",
           f"// pinned pack held them ({m['character']['path']} at {m['character']['revision'][:10]}):",
           "// per joint its parent (parents first), its rest translation and rotation (x, y, z, w) and",
           "// its inverse bind (three rows of four); per clip its seconds, the metres a loop travels (0",
           "// for a clip on the clock) and, per joint, its translation and rotation keys (first, count)",
           "// into the key lists: time then the value. Scale is 1 everywhere (within 1e-5). verse/skin.coax plays it.",
           "",
           '#load "../skin.coax";',
           "",
           f"{P}_JOINTS :: {len(J)};",
           f"{P}_CLIPS :: {len(c['clips'])};",
           f"{P}_TRIANGLES :: {triangles};",
           f"{P}_CLIP_NAME :: [{len(c['clips'])}] string.[" + ", ".join(f'"{k["name"]}"' for k in c["clips"]) + "];",
           arr(f"{P}_DURATION", "f32", [k["duration"] for k in c["clips"]], 8, f9).rstrip(),
           arr(f"{P}_DISTANCE", "f32", [k["distance"] for k in c["clips"]], 8, f9).rstrip(),
           arr(f"{P}_PARENT", "int", [j["parent"] for j in J], 25, str).rstrip(),
           arr(f"{P}_REST", "f32", flat(tuple(j["t"]) + tuple(j["r"]) for j in J), 7, f9).rstrip(),
           arr(f"{P}_BIND", "f32", flat([j["bind"][r], j["bind"][4 + r], j["bind"][8 + r], j["bind"][12 + r]]
                                        for j in J for r in range(3)), 12, f9).rstrip(),
           arr(f"{P}_TRACK_T", "int", tt, 30, str).rstrip(),
           arr(f"{P}_TRACK_R", "int", tr, 30, str).rstrip(),
           arr(f"{P}_KEYS_T", "f32", flat(kt), 8, f9).rstrip(),
           arr(f"{P}_KEYS_R", "f32", flat(kr), 10, f9).rstrip(),
           "// Checks, by the old engine's sampling (posed_joints in pack_realm.py, in f64): part way",
           "// into each clip, where a joint stands and where the point (0.1, 1, 0.05) skinned wholly",
           "// by it goes: clip, seconds, joint, x, y, z, then the point's x, y, z.",
           arr(f"{P}_CHECK", "f32", flat((ci, tm, j) + tuple(posed_joints(c, ci, tm)[0][j])
                                         + skinned_point(c, ci, tm, j, (0.1, 1.0, 0.05))
                                         for ci in range(len(c["clips"])) for tm in (0.0, c["clips"][ci]["duration"] * 0.37)
                                         for j in CHECK_JOINTS if j < len(J)), 9, f9).rstrip(),
           "",
           "// The rig in a page's state (a view needs an array it may write through), and its views.",
           f"{P.title()} :: struct {{",
           *[f"  {n.split(':')[0].lower()}: [{sizes[n.split(':')[0]]}] {n.split(':')[1]};" for n in ['PARENT:int', 'REST:f32', 'BIND:f32', 'DURATION:f32', 'DISTANCE:f32', 'TRACK_T:int', 'TRACK_R:int', 'KEYS_T:f32', 'KEYS_R:f32']],
           "}",
           f"{P.lower()}_load :: (d: [] {P.title()}) {{",
           *[f"  d[0].{n.split(':')[0].lower()} = {P}_{n.split(':')[0]};" for n in ['PARENT:int', 'REST:f32', 'BIND:f32', 'DURATION:f32', 'DISTANCE:f32', 'TRACK_T:int', 'TRACK_R:int', 'KEYS_T:f32', 'KEYS_R:f32']],
           "}",
           f"{P.lower()}_rig :: (d: [] {P.title()}) -> SkinRig {{",
           f"  return SkinRig.{{joints = {P}_JOINTS, clips = {P}_CLIPS, parent = d[0].parent[..], rest = d[0].rest[..], bind = d[0].bind[..],",
           "                  duration = d[0].duration[..], distance = d[0].distance[..], track_t = d[0].track_t[..],",
           "                  track_r = d[0].track_r[..], keys_t = d[0].keys_t[..], keys_r = d[0].keys_r[..]};",
           "}"]
    return "\n".join(out) + "\n"


def build(manifest_path):
    m = json.loads(Path(manifest_path).read_text()) if not isinstance(manifest_path, dict) else manifest_path
    style = m.get("style", "lowpoly")
    if style not in STYLES:
        raise SystemExit(f"{m['pack']}: style is one of {', '.join(STYLES)}, not {style!r}")
    if style == "painterly":
        named = [k for k in MAP_KEYS if k in m] + [f"{x['name']}.{k}" for x in m["meshes"] if isinstance(x, dict) for k in MAP_KEYS if k in x]
        if named:
            raise SystemExit(f"{m['pack']}: a painterly pack carries no normal, roughness or detail maps ({', '.join(named)})")
        if int(m.get("texture_size", PAINT_DEFAULT_SIZE)) > PAINT_MAX_SIZE:
            raise SystemExit(f"{m['pack']}: a painterly layer is at most {PAINT_MAX_SIZE} texels a side, not {m['texture_size']}")
    paint_what = m.get("paint", False)
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
    canopies = []        # (its mesh line, its first vertex and the one after its last, sphere or ellipsoid)
    for item in m["meshes"]:
        name = item["name"] if isinstance(item, dict) else item
        if name not in meshes:
            raise SystemExit(f"{name}: not in {m['scene']}")
        kind, first, count = meshes[name]
        key = (item.get("licence") if isinstance(item, dict) else None) or FROM_KIND.get(kind)
        lic = licence_of(key, f"mesh {name}")
        start = len(indices)
        vstart = len(vertices) // 48
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
        line = f"M {name} {start} {len(indices) - start} {lic} " + " ".join(f"{x:.3f}" for x in lo + hi)
        if canopy_shape(name):
            canopy_blob(vertices, vstart, lo, hi, canopy_shape(name) == "ellipsoid")
            canopies.append((len(mesh_lines), vstart, len(vertices) // 48, canopy_shape(name)))
        mesh_lines.append(line)
    size = int(m.get("texture_size", PAINT_DEFAULT_SIZE if style == "painterly" else 512))
    entries = []   # (kind, licence, name, bytes)
    layer_lines, sound_lines = [], []
    layer_images = []
    for k, layer in enumerate(layers):
        lic = layer_licence[layer]
        if lic == 255:
            raise SystemExit(f"layer {layer} is used by meshes of two licences; split them")
        painted = paint_what is True or (isinstance(paint_what, list) and layer in paint_what)
        src, data = image(home(m["layers"].format(n=layer)), size, style, painted)
        layer_images.append(data)
        entries.append((KIND_LAYER, lic, f"layer{k}", data))
        layer_lines.append(f"T {k} {src.name} {lic}")
    for at, vstart, vend, shape in canopies:
        mesh_lines[at] += f" canopy {shape} " + " ".join(str(x) for x in canopy_colour(vertices, vstart, vend, layer_images))
    character_lines, rig = [], None
    if "character" in m:
        spec = m["character"]
        lic = licence_of(spec.get("licence"), f"character {spec['name']}")
        textures, materials, c = character_source(spec)
        if c["name"] != spec["name"]:
            raise SystemExit(f"the pinned pack's character is {c['name']}, not {spec['name']}")
        first, word, count = len(indices), len(vertices) // 4, 0
        layer_of = {}
        for pr in c["prims"]:
            mat = materials[pr["material"]]
            if mat["alpha"] != 0 or any(abs(x - 1) > 1e-6 for x in mat["colour"]):
                raise SystemExit(f"{c['name']}: a material isn't plain opaque base colour")
            t = mat["texture"]
            if t not in layer_of:
                layer_of[t] = len(layer_lines)
                entries.append((KIND_LAYER, lic, f"layer{layer_of[t]}", texels_up(textures[t]["png"], size)))
                layer_lines.append(f"T {layer_of[t]} {textures[t]['name']} {lic}")
            layer = layer_of[t]
            for i, p in enumerate(pr["pos"]):
                n = pr["normals"][3 * i:3 * i + 3]
                vertices += struct.pack("<3f", *p) + bytes(n) + bytes([layer]) + struct.pack("<2f", *pr["uv"][i])
                vertices += pr["joints"][4 * i:4 * i + 4] + pr["weights"][4 * i:4 * i + 4]
                if sum(pr["weights"][4 * i:4 * i + 4]) != 255 or max(pr["joints"][4 * i:4 * i + 4]) >= len(c["joints"]):
                    raise SystemExit(f"{c['name']}: a vertex's skin is out of range")
            indices += [count + k for k in pr["indices"]]
            count += len(pr["pos"])
        character_lines.append(f"C {c['name']} {first} {len(indices) - first} {lic} {word} {count} {len(c['joints'])}")
        if "rig" in spec:
            rig = (spec["rig"], rig_coax(m, c, spec.get("prefix", "RIG"), (len(indices) - first) // 3))
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
        lines = [f"style {style}"] if "style" in m else []
        lines += [f"pack {m['pack']}", f"budget {budget}", f"bytes {total}"]
        lines += [f"L {k} {licences[key]['name']}|{licences[key]['licence']}|{licences[key]['url']}" for k, key in enumerate(keys)]
        return ("\n".join(lines + mesh_lines + character_lines + identity_lines + layer_lines + sound_lines) + "\n").encode()
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
    return m, keys, bytes(out), mesh_lines + character_lines, layer_lines, sound_lines, rig


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
        lines.append(f"| {p[1]} | {'character' if p[0] == 'C' else 'mesh'} | {name(p[4])} |")
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
    m, keys, data, ml, ll, sl, rig = build(sys.argv[1])
    Path(sys.argv[2]).parent.mkdir(parents=True, exist_ok=True)
    Path(sys.argv[2]).write_bytes(data)
    if rig:
        Path(__file__).resolve().parents[2].joinpath(rig[0]).write_text(rig[1])
    if len(sys.argv) == 4:
        Path(sys.argv[3]).write_text(licences_md(m, keys, ml, ll, sl, len(data)))
    print(f"{m['pack']}: {len(data):,} bytes of {int(m['budget']):,}; {len(ml)} meshes, {len(ll)} layers, {len(sl)} sound{"" if len(sl) == 1 else "s"}")
