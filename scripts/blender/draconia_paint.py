"""Draconia's skin and eye maps, painted by script from baked position maps.

The skin's UV layout is fresh (draconia.unwrap), so painting happens in 3D:
Cycles bakes each texel's object-space position and normal, and numpy
paints from those, so freckles, pores and colour zones sit on the face, not
on the layout. Nothing here comes from a photograph or a scan.

Maps (sizes are `tex`, 4096 by default):
- skin base colour: fair, cool-neutral skin with warmth that varies;
  redness on the cheeks, nose, chin, ears and eyelids; a faint blue-purple
  under the eyes; a light dusting of freckles over the nose and cheeks;
  rose lips; ash-brown brows of painted strands.
- skin normal: pores (denser and deeper on the nose and cheeks), fine
  crossing skin lines, lip lines, and soft creases under the eyes. Tangent
  space along +u and +v as glTF stores UVs, the frame Verse's renderer
  derives from UV gradients.
- skin ORM: R occlusion (1), G roughness (an oilier T-zone, wet lips,
  rougher pores), B 0.
- eye base colour: a grey-green iris with radial fibres, crypts, a darker
  collarette and limbal ring, a hint of warm flecks by the pupil; a sclera
  no brighter than her skin, pinker and veined toward the corners.

Images go to `<out>/textures/`; the glb embeds them.
"""

import math
import os

import bpy
import numpy as np

# Soot and blood for the battle shots (`draconia.py --battle`).
BATTLE = False
# `natural` (ash brown, the parity reference) or `pale` (platinum, thinner:
# the battle and peacock look); `draconia.py --brows`.
BROWS = "natural"
# The expression being built: a glare adds furrow creases between the brows.
EXPRESSION = "neutral"
# (path, repeats over the map, strength): a tiled micro-normal to blend in.
DETAIL_TILE = None
# Brow height relative to the eye centres (m), for heads whose brow ridge
# sits lower than the CC0 base's (MetaHuman: -0.006).
BROW_DZ = 0.0
# The painted brow's soft fill under its strands and its overall strength
# (natural brows); a head with its own brow shape wants less of both.
BROW_FILL = 0.55
LAST_BROW = None
# The under-eye region's mask (as LAST_BROW), for lifting another albedo's
# dark under-eye band.
LAST_UNDER = None
# The lips' vermilion mask (as LAST_BROW), for deepening another albedo's
# lip colour (round 18).
LAST_LIPS = None
# The painted nasolabial folds' depth (a head whose mesh has its own folds
# wants none: painted on top they age the face).
FOLD_DEPTH = 1.2
# Under-eye darkening (a share), and the freckles' density and strength.
UNDER_EYE = 0.5
FRECKLES = (0.95, 0.6)
# How much glossier the forehead and nose (the T-zone) are than the rest of
# the face: a dewy sheen.
TZONE_GLOSS = 0.09
BROW_STRENGTH = 0.62
# Skin base albedo (linear) and an optional cavity tile (path, repeats).
SKIN_BASE = (0.64, 0.53, 0.50)
CAVITY_TILE = None

# ------------------------------------------------------------------ noise ---


def _hash(ix, iy, iz, seed=0):
    h = (ix.astype(np.int64) * 73856093) ^ (iy.astype(np.int64) * 19349663) ^ (iz.astype(np.int64) * 83492791) ^ (seed * 2654435761)
    h = (h ^ (h >> 13)) * 1274126177
    h = h ^ (h >> 16)
    return (h & 0xFFFFFF).astype(np.float32) / float(0xFFFFFF)


def value_noise(p, scale, seed=0):
    """Smooth 3D value noise in [0, 1] at points p (N, 3), cell `scale` m."""
    q = p / scale
    i = np.floor(q)
    f = q - i
    f = f * f * (3 - 2 * f)
    ix, iy, iz = i[:, 0].astype(np.int64), i[:, 1].astype(np.int64), i[:, 2].astype(np.int64)
    out = np.zeros(len(p), np.float32)
    for dx in (0, 1):
        wx = f[:, 0] if dx else 1 - f[:, 0]
        for dy in (0, 1):
            wy = f[:, 1] if dy else 1 - f[:, 1]
            for dz in (0, 1):
                wz = f[:, 2] if dz else 1 - f[:, 2]
                out += wx * wy * wz * _hash(ix + dx, iy + dy, iz + dz, seed)
    return out


def fbm(p, scale, octaves=4, seed=0, gain=0.5):
    total, amp, norm = np.zeros(len(p), np.float32), 1.0, 0.0
    for o in range(octaves):
        total += amp * value_noise(p, scale / (2**o), seed + o * 17)
        norm += amp
        amp *= gain
    return total / norm


def cells(p, scale, seed=0, jitter=0.9):
    """Cellular noise: distance (in cells) to the nearest jittered feature
    point, and that point's random id in [0, 1)."""
    q = p / scale
    i = np.floor(q).astype(np.int64)
    best = np.full(len(p), 9.0, np.float32)
    ident = np.zeros(len(p), np.float32)
    for dx in (-1, 0, 1):
        for dy in (-1, 0, 1):
            for dz in (-1, 0, 1):
                cx, cy, cz = i[:, 0] + dx, i[:, 1] + dy, i[:, 2] + dz
                fx = cx + 0.5 + jitter * (_hash(cx, cy, cz, seed) - 0.5)
                fy = cy + 0.5 + jitter * (_hash(cx, cy, cz, seed + 1) - 0.5)
                fz = cz + 0.5 + jitter * (_hash(cx, cy, cz, seed + 2) - 0.5)
                d = np.sqrt((q[:, 0] - fx) ** 2 + (q[:, 1] - fy) ** 2 + (q[:, 2] - fz) ** 2)
                closer = d < best
                best = np.where(closer, d, best)
                ident = np.where(closer, _hash(cx, cy, cz, seed + 3), ident)
    return best, ident


def gauss(p, centre, radii):
    d = (p - np.asarray(centre, np.float32)) / np.asarray(radii, np.float32)
    return np.exp(-0.5 * np.sum(d * d, axis=1))


def smooth(a, b, x):
    t = np.clip((x - a) / (b - a), 0, 1)
    return t * t * (3 - 2 * t)


def mix(a, b, t):
    t = t[:, None] if np.ndim(t) == 1 else t
    return a * (1 - t) + b * t


# ------------------------------------------------------------------ bakes ---


def _bake_attribute(obj, size, kind):
    """Bakes object-space position or normal of each texel to a float image;
    returns (pixels (size, size, 4) top row first, coverage mask)."""
    img = bpy.data.images.new(f"bake_{kind}", size, size, alpha=True, float_buffer=True)
    img.generated_color = (0, 0, 0, 0)
    mat = bpy.data.materials.new(f"bake_{kind}")
    mat.use_nodes = True
    nt = mat.node_tree
    for n in list(nt.nodes):
        nt.nodes.remove(n)
    geo = nt.nodes.new("ShaderNodeNewGeometry")
    emit = nt.nodes.new("ShaderNodeEmission")
    outn = nt.nodes.new("ShaderNodeOutputMaterial")
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = img
    nt.nodes.active = tex
    if kind in ("position", "normal"):
        nt.links.new(geo.outputs["Position" if kind == "position" else "Normal"], emit.inputs["Color"])
    else:
        # A per-vertex attribute the build stored (such as `lipmask`).
        attr = nt.nodes.new("ShaderNodeAttribute")
        attr.attribute_name = kind
        nt.links.new(attr.outputs["Color"], emit.inputs["Color"])
    nt.links.new(emit.outputs["Emission"], outn.inputs["Surface"])
    old = list(obj.data.materials)
    obj.data.materials.clear()
    obj.data.materials.append(mat)
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.samples = 1
    scene.render.bake.margin = 6
    for o in bpy.context.scene.objects:
        o.select_set(False)
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.bake(type="EMIT", margin=6, use_clear=True)
    px = np.array(img.pixels[:], np.float32).reshape(size, size, 4)[::-1]
    obj.data.materials.clear()
    for m in old:
        obj.data.materials.append(m)
    bpy.data.images.remove(img)
    return px


def _bake_ao(obj, size, distance=0.03):
    """Ambient occlusion over the skin from everything around it (eyes,
    lashes, hair, collar), Cycles, 64 samples: the creases, the eye
    sockets, under the nose and the hairline darken."""
    img = bpy.data.images.new("bake_ao", size, size, alpha=False, float_buffer=True)
    mat = bpy.data.materials.new("bake_ao")
    mat.use_nodes = True
    tex = mat.node_tree.nodes.new("ShaderNodeTexImage")
    tex.image = img
    mat.node_tree.nodes.active = tex
    old = list(obj.data.materials)
    obj.data.materials.clear()
    obj.data.materials.append(mat)
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.samples = 64
    world = scene.world or bpy.data.worlds.new("w")
    scene.world = world
    world.light_settings.distance = distance
    # The posed skin and the rest copy coincide; only one may occlude.
    hidden = [o for o in bpy.context.scene.objects if o is not obj and (o.name.startswith("skin") or o.name.startswith("body"))]
    for o in hidden:
        o.hide_render = True
    for o in bpy.context.scene.objects:
        o.select_set(False)
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.bake(type="AO", margin=6, use_clear=True)
    for o in hidden:
        o.hide_render = False
    px = np.array(img.pixels[:], np.float32).reshape(size, size, 4)[::-1][:, :, 0]
    obj.data.materials.clear()
    for m in old:
        obj.data.materials.append(m)
    bpy.data.images.remove(img)
    # Soften: skin scatters light into its creases.
    return 0.35 + 0.65 * px


def _uv_mask(obj, size):
    import bmesh

    bm = bmesh.new()
    bm.from_mesh(obj.data)
    uv = bm.loops.layers.uv.active
    mask = np.zeros((size, size), bool)
    for f in bm.faces:
        pts = np.array([[l[uv].uv.x * size, (1 - l[uv].uv.y) * size] for l in f.loops])
        x0, y0 = np.floor(pts.min(0)).astype(int)
        x1, y1 = np.ceil(pts.max(0)).astype(int)
        x0, y0 = max(x0 - 1, 0), max(y0 - 1, 0)
        x1, y1 = min(x1 + 1, size), min(y1 + 1, size)
        if x1 <= x0 or y1 <= y0:
            continue
        mask[y0:y1, x0:x1] = True  # coarse: the bake's margin fills the rest
    bm.free()
    return mask


def save(name, rgba, out, colorspace="sRGB"):
    """Writes an (H, W, 4) float array, top row first, as a PNG image."""
    h, w, _ = rgba.shape
    img = bpy.data.images.new(name, w, h, alpha=True)
    img.pixels.foreach_set(np.ascontiguousarray(rgba[::-1]).astype(np.float32).ravel())
    path = os.path.join(out, "textures", f"{name}.png")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    img.filepath_raw = path
    img.file_format = "PNG"
    img.save()
    img.colorspace_settings.name = colorspace
    img.filepath = path
    return img


def to_srgb(c):
    c = np.clip(c, 0, 1)
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * np.power(c, 1 / 2.4) - 0.055)


# ------------------------------------------------------------------- skin ---


def paint_skin(skin, marks, size, out):
    pos = _bake_attribute(skin, size, "position")
    nor = _bake_attribute(skin, size, "normal")
    cover = pos[:, :, 3] > 0.5
    P = pos[cover][:, :3]
    N = nor[cover][:, :3]
    el, er = np.array(marks.eye_l, np.float32), np.array(marks.eye_r, np.float32)
    mid = (el + er) / 2
    mouth = np.array(marks.mouth, np.float32)
    # Face-local coordinates: x her left, z up, y toward the back of the head.
    X = P[:, 0] - mid[0]
    Z = P[:, 2] - mid[2]
    Y = P[:, 1] - mid[1]
    front = smooth(0.015, -0.03, Y)  # 1 on the face's front, 0 behind the ears
    nose_tip = np.array(marks.nose_tip, np.float32) if hasattr(marks, "nose_tip") else mid + np.array([0, -0.035, -0.045], np.float32)
    cheek_l = mid + np.array([0.042, -0.01, -0.03], np.float32)
    cheek_r = mid + np.array([-0.042, -0.01, -0.03], np.float32)

    # Base: fair skin, linear albedo; low-frequency mottling in hue/value.
    base = np.array(SKIN_BASE, np.float32)
    mott = fbm(P, 0.03, 4, seed=1) - 0.5
    mott2 = fbm(P, 0.006, 3, seed=2) - 0.5
    mott3 = fbm(P, 0.0035, 3, seed=3) - 0.5
    col = base[None, :] * (1 + 0.14 * mott[:, None] + 0.08 * mott2[:, None] + 0.06 * mott3[:, None])
    # Blotchy warmth at a few millimetres: skin is never one tone.
    blotch = np.clip((fbm(P, 0.005, 3, seed=6) - 0.5) * 2.5, 0, 1)
    col = mix(col, col * np.array([1.04, 0.93, 0.92], np.float32), blotch * 0.35)
    # Warm vs cool zones: the neck and temples cooler and paler.
    cool = smooth(-0.07, -0.12, Z)
    col = mix(col, col * np.array([0.97, 0.97, 1.0], np.float32), cool * 0.5)

    # Redness: cheeks, nose (tip and wings), chin, ears, eyelids; broken
    # up by noise so it reads as capillaries, not paint.
    red_t = np.array([0.62, 0.30, 0.27], np.float32)
    capill = 0.55 + 0.9 * (fbm(P, 0.004, 3, seed=5) - 0.5)
    red = (
        0.55 * gauss(P, cheek_l, (0.022, 0.03, 0.02))
        + 0.55 * gauss(P, cheek_r, (0.022, 0.03, 0.02))
        + 0.75 * gauss(P, nose_tip, (0.012, 0.03, 0.012))
        + 0.35 * gauss(P, mid + np.array([0, -0.025, -0.02], np.float32), (0.009, 0.03, 0.02))
        + 0.30 * gauss(P, mouth + np.array([0, 0, -0.035], np.float32), (0.018, 0.03, 0.012))
        + 0.10 * (gauss(P, el + np.array([0, -0.005, 0.006], np.float32), (0.014, 0.02, 0.006)) + gauss(P, er + np.array([0, -0.005, 0.006], np.float32), (0.014, 0.02, 0.006)))
        + 0.6 * smooth(0.06, 0.075, np.abs(X)) * smooth(0.04, -0.02, np.abs(Z + 0.02))  # ears
    )
    red = np.clip(red * capill, 0, 1) * 0.32
    col = mix(col, red_t[None, :] * (col.mean(1, keepdims=True) / 0.45), red)

    # Under-eye: a faint blue-purple, slightly darker.
    under = sum(gauss(P, e + np.array([0, -0.004, -0.012], np.float32), (0.013, 0.02, 0.006)) for e in (el, er))
    col = mix(col, col * np.array([0.92, 0.89, 0.92], np.float32), np.clip(under, 0, 1) * UNDER_EYE)
    global LAST_UNDER
    LAST_UNDER = np.zeros((size, size), np.float32)
    LAST_UNDER[cover] = np.clip(under, 0, 1)
    # Eyelid crease and lid: slightly darker, warmer.
    lid = sum(gauss(P, e + np.array([0, 0.0, 0.009], np.float32), (0.013, 0.02, 0.004)) for e in (el, er))
    col = mix(col, col * np.array([0.94, 0.92, 0.92], np.float32), np.clip(lid, 0, 1) * 0.35)

    # The upper lash line: a dark, soft band where the lashes root, thicker
    # toward the outer corner.
    for e, sign in ((el, 1.0), (er, -1.0)):
        dx = P[:, 0] - e[0]
        outer = np.clip(dx * sign / 0.016, 0, 1)
        lid_z = e[2] + 0.0042 * np.sqrt(np.clip(1 - (dx / 0.0155) ** 2, 0, 1)) - 0.0012
        band = np.exp(-(((P[:, 2] - lid_z) / (0.0006 + 0.0006 * outer)) ** 2)) * (np.abs(dx) < 0.017) * (P[:, 1] < e[1])
        col = mix(col, np.array([0.03, 0.022, 0.018], np.float32)[None, :], np.clip(band, 0, 1) * 0.6)

    # Lips: an analytic Cupid's bow and lower lip in the face plane.
    mx = (P[:, 0] - mouth[0]) / 0.026  # half mouth width 26 mm
    mz = P[:, 2] - mouth[2]
    bow = 0.0016 * np.exp(-((np.abs(mx) - 0.22) ** 2) / 0.02) - 0.0009 * np.exp(-(mx**2) / 0.006)
    upper_top = (0.0085 + bow) * np.sqrt(np.clip(1 - mx**2, 0, 1)) ** 0.7
    lower_bot = -0.0105 * np.sqrt(np.clip(1 - mx**2, 0, 1)) ** 0.8
    lip_in = (mz < upper_top) & (mz > lower_bot) & (np.abs(mx) < 1.0) & (np.abs(P[:, 1] - mouth[1]) < 0.012) & (front > 0.5)
    edge = np.minimum(upper_top - mz, mz - lower_bot)
    lips = np.where(lip_in, smooth(0.0, 0.0012, edge), 0.0) * smooth(1.0, 0.85, np.abs(mx))
    if "lipmask" in skin.data.attributes:
        # The vermilion from the mesh itself (the CC0 lip-volume targets'
        # reach, stored by the build), so colour follows the lips' shape.
        lm = _bake_attribute(skin, size, "lipmask")[:, :, 0][cover]
        lips = smooth(0.35, 0.7, lm)
    global LAST_LIPS
    LAST_LIPS = np.zeros((size, size), np.float32)
    LAST_LIPS[cover] = lips
    lip_c = np.array([0.42, 0.22, 0.20], np.float32) * (1 + 0.08 * (fbm(P, 0.002, 2, seed=9)[:, None] - 0.5))
    # The lips' natural rose-brown, a little darker where they meet.
    col = mix(col, lip_c, lips * 0.78)
    meet = np.exp(-((P[:, 2] - mouth[2]) / 0.0008) ** 2) * (np.abs(mx) < 1.0) * front
    col = mix(col, col * 0.55, np.clip(meet, 0, 1) * 0.6)

    # Freckles: jittered points (2.2 mm cells) kept where a mask over the
    # nose bridge and cheeks allows, each a soft tan disc of its own size
    # and strength; a sparser, fainter dusting on the forehead.
    fmask = np.clip(
        0.95 * gauss(P, mid + np.array([0, -0.03, -0.02], np.float32), (0.018, 0.04, 0.016))
        + 0.85 * gauss(P, cheek_l + np.array([-0.006, 0, 0.004], np.float32), (0.02, 0.03, 0.016))
        + 0.85 * gauss(P, cheek_r + np.array([0.006, 0, 0.004], np.float32), (0.02, 0.03, 0.016))
        + 0.25 * gauss(P, mid + np.array([0, -0.01, 0.05], np.float32), (0.04, 0.05, 0.03)),
        0,
        1,
    ) * front
    d, ident = cells(P, 0.0022, seed=21)
    alive = ident < fmask * FRECKLES[0]
    radius = 0.18 + 0.25 * ((ident * 7.13) % 1.0)
    frk = np.where(alive, smooth(radius, radius * 0.45, d), 0.0) * (0.35 + 0.5 * ((ident * 3.71) % 1.0))
    frk_c = np.array([0.40, 0.24, 0.15], np.float32)
    col = mix(col, frk_c * (col.mean(1, keepdims=True) / 0.45), np.clip(frk, 0, 1) * FRECKLES[1])

    # Brows: strands from the head (inner) rising then lying along the arch,
    # medium ash brown, darker than the hair.
    brow = np.zeros(len(P), np.float32)
    rng = np.random.default_rng(11)
    for e, sign in ((el, 1.0), (er, -1.0)):
        inner = e + np.array([-sign * 0.016, -0.012, 0.016 + BROW_DZ], np.float32)
        sel = np.where((np.abs(P[:, 0] - (e[0] + sign * 0.002)) < 0.034) & (np.abs(P[:, 2] - (e[2] + 0.018 + BROW_DZ)) < 0.016) & (P[:, 1] < e[1]))[0]
        if len(sel) == 0:
            continue
        Q = P[sel]
        acc = np.zeros(len(sel), np.float32)
        fill = np.zeros(len(sel), np.float32)
        for _ in range(950):
            s = rng.uniform(0, 1)
            # The brow's arch: from the inner head, peaking at 2/3, tapering.
            bx = inner[0] + sign * 0.044 * s
            bz = inner[2] + 0.009 * math.sin(min(s / 0.68, 1.0) * math.pi / 2) - 0.010 * max(s - 0.68, 0) / 0.32 * 0.7
            thick = (0.0062 * (1 - 0.6 * s) + 0.0016) * (0.6 if BROWS == "pale" else 1.0)
            bz += rng.uniform(-0.5, 0.5) * thick
            ang = math.radians(70 - 60 * min(s / 0.5, 1)) if s < 0.5 else math.radians(10 - 25 * (s - 0.5))
            length = 0.0075 * rng.uniform(0.7, 1.2)
            dx, dz = sign * math.cos(ang), math.sin(ang)
            ax, az = bx - dx * length * 0.5, bz - dz * length * 0.5
            vx, vz = Q[:, 0] - ax, Q[:, 2] - az
            t = np.clip((vx * dx + vz * dz) / length, 0, 1)
            dist = np.sqrt((vx - t * dx * length) ** 2 + (vz - t * dz * length) ** 2)
            w = 0.00011 * (1.2 - t)
            fill = np.maximum(fill, np.exp(-((Q[:, 0] - bx) ** 2 + (Q[:, 2] - bz) ** 2) / (2 * (thick * 0.6) ** 2)) * 0.5)
            acc = np.maximum(acc, smooth(w * 1.8, w * 0.5, dist) * rng.uniform(0.55, 1.0))
        # A soft fill under the strands: skin seen through a full brow.
        brow[sel] = np.maximum(brow[sel], np.maximum(acc, fill * BROW_FILL))
    # The brow's coverage as an image (size x size, texels the skin covers),
    # for laying our painted brow over another albedo (MetaHuman's).
    global LAST_BROW
    LAST_BROW = np.zeros((size, size), np.float32)
    LAST_BROW[cover] = brow
    brow_c = np.array([0.30, 0.26, 0.19], np.float32) if BROWS == "pale" else np.array([0.17, 0.13, 0.09], np.float32)
    col = mix(col, brow_c, np.clip(brow, 0, 1) * (0.55 if BROWS == "pale" else BROW_STRENGTH))

    # Battle decals (character.md): soot smudged over the cheeks and jaw,
    # and a thin line of blood from the corner of the mouth.
    blood = np.zeros(len(P), np.float32)
    if BATTLE:
        smudge = fbm(P, 0.012, 4, seed=71)
        grain = fbm(P, 0.0015, 3, seed=72)
        zone = np.clip(
            0.9 * gauss(P, cheek_l + np.array([0.012, 0.0, -0.02], np.float32), (0.02, 0.04, 0.025))
            + 0.6 * gauss(P, cheek_r + np.array([-0.01, 0.0, -0.03], np.float32), (0.018, 0.04, 0.02))
            + 0.7 * gauss(P, mouth + np.array([0.025, 0.0, -0.03], np.float32), (0.025, 0.04, 0.015))
            + 0.3 * gauss(P, mid + np.array([0.0, 0.0, 0.05], np.float32), (0.04, 0.05, 0.02)),
            0,
            1,
        )
        soot = np.clip((smudge - 0.42) * 3.0, 0, 1) * zone * (0.6 + 0.6 * grain)
        col = mix(col, np.array([0.06, 0.05, 0.045], np.float32)[None, :], np.clip(soot, 0, 0.85))
        # Blood: from her right mouth corner, down and a little out.
        c0 = mouth + np.array([-0.0235, -0.002, -0.001], np.float32)
        for k in range(24):
            t = k / 23
            c = c0 + np.array([-0.003 * t, 0.002 * t, -0.024 * t], np.float32)
            w = 0.0007 * (1 - 0.5 * t) + (0.0006 if k == 23 else 0.0)
            d = np.sqrt((P[:, 0] - c[0]) ** 2 + (P[:, 2] - c[2]) ** 2)
            blood = np.maximum(blood, smooth(w, w * 0.4, d) * (np.abs(P[:, 1] - c[1]) < 0.02))
        col = mix(col, np.array([0.22, 0.012, 0.01], np.float32)[None, :], blood * 0.92)

    # ---- height: pores, lines, lip lines -> normal map
    pore_d, pore_id = cells(P, 0.00034, seed=31)
    pore_mask = 0.45 + 0.55 * np.clip(
        gauss(P, nose_tip, (0.015, 0.04, 0.02)) + 0.8 * gauss(P, cheek_l, (0.03, 0.04, 0.03)) + 0.8 * gauss(P, cheek_r, (0.03, 0.04, 0.03)) + 0.4 * gauss(P, mid + np.array([0, 0, 0.05], np.float32), (0.05, 0.05, 0.03)), 0, 1
    )
    pores = -smooth(0.42, 0.0, pore_d) * (0.6 + 0.4 * pore_id) * pore_mask
    # Fine skin lines: two crossing sets of stretched noise.
    lines = (value_noise(P * np.array([1, 1, 6], np.float32), 0.0016, seed=41) - 0.5) + (value_noise(P * np.array([6, 1, 1], np.float32), 0.0016, seed=42) - 0.5)
    micro = fbm(P, 0.0009, 2, seed=43) - 0.5
    height = 0.9 * pores + 0.35 * lines + 0.25 * micro
    # Vertical lip lines.
    lipl = (value_noise(P * np.array([1, 1, 0.08], np.float32), 0.0007, seed=44) - 0.5) * lips
    height = height * (1 - lips * 0.7) + 0.8 * lipl
    # Soft creases under the eyes.
    for e in (el, er):
        dz = P[:, 2] - (e[2] - 0.0115 - 0.12 * (P[:, 0] - e[0]) ** 2 / 0.01)
        height -= 0.6 * np.exp(-(dz / 0.0009) ** 2) * gauss(P, e + np.array([0, 0, -0.011], np.float32), (0.012, 0.03, 0.01))
    # Glare: vertical furrows between the brows (the corrugator's creases)
    # and a horizontal one over the nose bridge.
    if EXPRESSION == "furrowed":
        for xo in (-0.0045, 0.0045):
            dz = P[:, 2] - (mid[2] + 0.012)
            dx = P[:, 0] - (mid[0] + xo)
            height -= 2.2 * np.exp(-(dx / 0.0007) ** 2) * np.exp(-(dz / 0.009) ** 2) * front
        height -= 1.2 * np.exp(-((P[:, 2] - (mid[2] + 0.002)) / 0.0008) ** 2) * np.exp(-((P[:, 0] - mid[0]) / 0.006) ** 2) * front
    # The nasolabial folds and the corners of the mouth: soft grooves.
    for sgn in (1.0, -1.0):
        nx = nose_tip[0] + sgn * 0.017
        t = np.clip((nose_tip[2] - 0.005 - P[:, 2]) / 0.035, 0, 1)
        line_x = nx + sgn * (0.008 * t)
        fold = np.exp(-((P[:, 0] - line_x) / 0.0018) ** 2) * (t > 0) * (t < 1) * front
        height -= FOLD_DEPTH * fold
    height *= 0.00009  # metres: pores ~0.08 mm deep
    rough = 0.58 + 0.12 * (fbm(P, 0.006, 3, seed=51) - 0.5)
    tzone = np.clip(gauss(P, mid + np.array([0, -0.02, 0.045], np.float32), (0.03, 0.05, 0.03)) + gauss(P, nose_tip, (0.012, 0.04, 0.03)), 0, 1)
    rough = rough - TZONE_GLOSS * tzone - 0.04 * gauss(P, cheek_l, (0.02, 0.03, 0.02)) - 0.04 * gauss(P, cheek_r, (0.02, 0.03, 0.02))
    rough = rough + 0.12 * np.clip(-pores, 0, 1)
    # Moist lips: glossier, varied by fine lip lines.
    rough = rough * (1 - lips) + (0.34 + 0.12 * (fbm(P * np.array([1, 1, 0.2], np.float32), 0.0006, 2, seed=45) - 0.5)) * lips
    rough = np.where(brow > 0.3, 0.6, rough)
    rough = rough * (1 - blood) + 0.18 * blood

    def unpack(values, fill):
        img = np.full((size, size) + values.shape[1:], fill, np.float32)
        img[cover] = values
        return img

    col_img = unpack(col, 0.0)
    hgt = unpack(height, 0.0)
    rough_img = unpack(rough, 0.5)
    # Fill uncovered texels with the nearest covered colour (simple dilation)
    # so mips and filtering at seams don't pull in black.
    col_img, hgt, rough_img = _dilate([col_img, hgt, rough_img], cover, 8)
    # Tangent-space normal from height and the texel's world size.
    pimg = pos[:, :, :3]
    du = np.linalg.norm(np.roll(pimg, -1, 1) - np.roll(pimg, 1, 1), axis=2) / 2
    dv = np.linalg.norm(np.roll(pimg, -1, 0) - np.roll(pimg, 1, 0), axis=2) / 2
    du = np.where(cover, np.clip(du, 1e-6, 1e-3), 1e-4)
    dv = np.where(cover, np.clip(dv, 1e-6, 1e-3), 1e-4)
    dhdu = (np.roll(hgt, -1, 1) - np.roll(hgt, 1, 1)) / (2 * du)
    dhdv = (np.roll(hgt, -1, 0) - np.roll(hgt, 1, 0)) / (2 * dv)
    nrm = np.stack([-dhdu, -dhdv, np.ones_like(hgt)], axis=2)
    nrm /= np.linalg.norm(nrm, axis=2, keepdims=True)
    if DETAIL_TILE:
        # A tiled micro-detail normal (MetaHuman's skin micro normal when
        # building from the MetaHuman pipeline), blended in whiteout style.
        tile_path, repeats, strength = DETAIL_TILE
        timg = bpy.data.images.load(tile_path)
        tw, th = timg.size
        t = np.array(timg.pixels[:], np.float32).reshape(th, tw, timg.channels)[::-1][:, :, :3] * 2 - 1
        bpy.data.images.remove(timg)
        ys = (np.arange(size) * repeats * th // size) % th
        xs = (np.arange(size) * repeats * tw // size) % tw
        d = t[ys][:, xs]
        d[..., :2] *= strength
        nrm = np.stack([nrm[..., 0] + d[..., 0], nrm[..., 1] + d[..., 1], nrm[..., 2] * np.clip(d[..., 2], 0.2, 1)], axis=2)
        nrm /= np.linalg.norm(nrm, axis=2, keepdims=True)
    nrm_img = np.concatenate([nrm * 0.5 + 0.5, np.ones((size, size, 1), np.float32)], axis=2)
    if CAVITY_TILE:
        # MetaHuman's cavity tile darkens pores and fine creases in the
        # albedo, as its skin material multiplies cavity in.
        cpath, crep = CAVITY_TILE
        cimg = bpy.data.images.load(cpath)
        cw, ch = cimg.size
        cav = np.array(cimg.pixels[:], np.float32).reshape(ch, cw, cimg.channels)[::-1][:, :, 0]
        bpy.data.images.remove(cimg)
        ys = (np.arange(size) * crep * ch // size) % ch
        xs = (np.arange(size) * crep * cw // size) % cw
        c = cav[ys][:, xs]
        col_img = col_img * (0.72 + 0.28 * c)[..., None]
    base_img = np.concatenate([to_srgb(col_img), np.ones((size, size, 1), np.float32)], axis=2)
    ao_img = _bake_ao(skin, size)
    (ao_img,) = _dilate([ao_img], ao_img > 0.0, 6)
    orm_img = np.stack([np.clip(ao_img, 0.0, 1.0), np.clip(rough_img, 0.05, 1), np.zeros((size, size), np.float32), np.ones((size, size), np.float32)], axis=2)
    return {
        "base": save("draconia_skin_base", base_img, out),
        "normal": save("draconia_skin_normal", nrm_img, out, "Non-Color"),
        "orm": save("draconia_skin_orm", orm_img, out, "Non-Color"),
    }


def _dilate(images, cover, steps):
    filled = cover.copy()
    imgs = [i.copy() for i in images]
    for _ in range(steps):
        grow = np.zeros_like(filled)
        acc = [np.zeros_like(i) for i in imgs]
        cnt = np.zeros(filled.shape, np.float32)
        for dy, dx in ((0, 1), (0, -1), (1, 0), (-1, 0)):
            nf = np.roll(np.roll(filled, dy, 0), dx, 1)
            cnt += nf
            for k, i in enumerate(imgs):
                shifted = np.roll(np.roll(i, dy, 0), dx, 1)
                acc[k] += shifted * (nf[..., None] if i.ndim == 3 else nf)
        new = (~filled) & (cnt > 0)
        for k, i in enumerate(imgs):
            avg = acc[k] / (np.maximum(cnt, 1)[..., None] if i.ndim == 3 else np.maximum(cnt, 1))
            i[new] = avg[new]
        filled |= new
    return imgs


# -------------------------------------------------------------------- eye ---


def paint_eye(size, out):
    """The eyeball in its azimuthal UV: the iris is the disc of UV radius
    0.0826 about (0.5, 0.5) (a 5.9 mm limbus on a 11.9 mm ball), the pupil
    0.022."""
    n = size
    yy, xx = np.mgrid[0:n, 0:n].astype(np.float32)
    u = (xx + 0.5) / n - 0.5
    v = 0.5 - (yy + 0.5) / n
    r = np.sqrt(u * u + v * v)
    a = np.arctan2(v, u)
    R_IRIS, R_PUPIL = 0.0826, 0.0205
    t = np.clip((r - R_PUPIL) / (R_IRIS - R_PUPIL), 0, 1)  # 0 pupil edge, 1 limbus
    P = np.stack([np.cos(a) * 3, np.sin(a) * 3, r * 40], axis=-1).reshape(-1, 3).astype(np.float32)
    # Radial fibres: noise stretched along the radius.
    # Radial fibres: noise stretched along the radius at several angular
    # scales, so they read as fine, irregular stroma, not clock ticks.
    def radial(freq, rf, seed):
        return value_noise(np.stack([np.cos(a) * freq, np.sin(a) * freq, r * rf], -1).reshape(-1, 3).astype(np.float32), 1.0, seed=seed).reshape(n, n)

    fib = 0.5 * radial(90, 18, 3) + 0.3 * radial(220, 40, 13) + 0.2 * radial(40, 8, 23)
    fib2 = 0.6 * radial(160, 30, 4) + 0.4 * radial(330, 60, 14)
    crypt = fbm(P * 2, 1.0, 3, seed=5).reshape(n, n)
    # Grey-green: a stroma of pale sage over a darker blue-grey base.
    deep = np.array([0.075, 0.105, 0.095], np.float32)
    stroma = np.array([0.20, 0.34, 0.24], np.float32)
    pale = np.array([0.33, 0.48, 0.34], np.float32)
    k = np.clip(0.35 + 0.55 * fib + 0.35 * (fib2 - 0.5), 0, 1)
    iris = deep * (1 - k[..., None]) + stroma * k[..., None]
    iris = iris + (pale - stroma) * np.clip(fib2 - 0.62, 0, 1)[..., None] * 1.5
    # Crypts: darker gaps.
    iris *= (1 - 0.45 * smooth(0.55, 0.75, crypt) * smooth(0.15, 0.4, t) * smooth(0.95, 0.7, t))[..., None]
    # Collarette: a ruffled ring at ~1/3, warm hazel flecks inside it.
    coll_r = 0.33 + 0.04 * (value_noise(np.stack([np.cos(a) * 9, np.sin(a) * 9, np.zeros_like(a)], -1).reshape(-1, 3).astype(np.float32), 1.0, seed=7).reshape(n, n) - 0.5)
    inner = smooth(coll_r + 0.03, coll_r - 0.03, t)
    warm = np.array([0.30, 0.25, 0.12], np.float32)
    iris = iris * (1 - 0.45 * inner[..., None]) + warm * (0.45 * inner * fib)[..., None]
    iris *= (1 - 0.35 * np.exp(-((t - coll_r) / 0.02) ** 2))[..., None]
    # Limbal ring: dark, soft-edged, 0.12 of the iris radius wide.
    limbal = smooth(0.80, 0.97, t)
    iris = iris * (1 - 0.82 * limbal[..., None]) + np.array([0.03, 0.04, 0.04], np.float32) * (0.82 * limbal)[..., None]
    # Pupil edge: a thin dark ruff.
    iris *= (1 - 0.6 * smooth(0.06, 0.0, t))[..., None]
    pupil = np.array([0.008, 0.008, 0.009], np.float32)
    # Sclera: no brighter than her skin; pinker and veined toward the edge.
    sc = np.array([0.62, 0.58, 0.55], np.float32)
    edge = smooth(0.12, 0.36, r)
    sclera = sc * (1 - 0.10 * edge[..., None]) + np.array([0.62, 0.40, 0.38], np.float32) * (0.25 * edge)[..., None]
    vein_n = value_noise(np.stack([np.cos(a) * 25, np.sin(a) * 25, r * 2.5], -1).reshape(-1, 3).astype(np.float32), 1.0, seed=8).reshape(n, n)
    veins = smooth(0.08, 0.0, np.abs(vein_n - 0.5)) * smooth(0.11, 0.3, r)
    sclera = sclera * (1 - 0.25 * veins[..., None]) + np.array([0.55, 0.12, 0.10], np.float32) * (0.25 * veins)[..., None]
    # Blend the zones; the limbus fades into the sclera over 0.4 mm.
    blend_limbus = smooth(R_IRIS - 0.002, R_IRIS + 0.004, r)
    col = iris * (1 - blend_limbus[..., None]) + sclera * blend_limbus[..., None]
    col = np.where((r < R_PUPIL)[..., None], pupil, col)
    soft = smooth(R_PUPIL - 0.0015, R_PUPIL + 0.0015, r)
    col = pupil * (1 - soft[..., None]) + col * soft[..., None]
    img = np.concatenate([to_srgb(col), np.ones((n, n, 1), np.float32)], axis=2)
    rough = np.where(r < R_IRIS, 0.5, 0.18).astype(np.float32)
    orm = np.stack([socket_occlusion(u, v), rough, np.zeros((n, n), np.float32), np.ones((n, n), np.float32)], 2)
    # The cornea's map, in its own UV (the limbus is the unit circle's
    # half): the same lid shadow, wet roughness.
    m = 256
    cy, cx = np.mgrid[0:m, 0:m].astype(np.float32)
    cu = ((cx + 0.5) / m - 0.5) * 2 * R_IRIS
    cv = (0.5 - (cy + 0.5) / m) * 2 * R_IRIS
    corm = np.stack([socket_occlusion(cu, cv), np.full((m, m), 0.08, np.float32), np.zeros((m, m), np.float32), np.ones((m, m), np.float32)], 2)
    return {
        "base": save("draconia_eye_base", img, out),
        "orm": save("draconia_eye_orm", orm, out, "Non-Color"),
        "cornea_orm": save("draconia_cornea_orm", corm, out, "Non-Color"),
    }


def socket_occlusion(u, v):
    """The socket's shadow on the eye in its UV (offsets from the front
    pole; +v up): the upper lid's shadow over the top of the iris, the
    recessed corners, a little under the lower lid. MetaHuman draws this
    with an eye-occlusion shell; a fixed eye can carry it in a map."""
    up = smooth(0.012, 0.085, v)
    corner = smooth(0.11, 0.24, np.abs(u))
    low = smooth(-0.075, -0.14, v)
    ao = (1 - 0.55 * up) * (1 - 0.45 * corner) * (1 - 0.3 * low)
    return ao.astype(np.float32)


def paint(skin, eyes, marks, tex, out):
    return {"skin": paint_skin(skin, marks, tex, out), "eye": paint_eye(min(tex, 2048), out)}
