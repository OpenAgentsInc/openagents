"""Original crypt-lab models, in the arrangement of a studied private laboratory.

Run headless:
    Blender -b --factory-startup --python scripts/blender/chamber_lab.py -- \
        [OUT_DIR] [NAME ...]

Reference mode only. The private 1.12.1 Scholomance view was studied for what
the room contains and where those kinds of objects sit: a vaulted stone hall,
slab tables, three cauldron colors, candelabra, floor candles, rugs, specimen
jars, alchemy benches, bookshelves, bones, chains, and cobwebs. Nothing here
is that geometry, those textures, or those silhouettes. Every solid is a
primitive built in this file.

Textures come from two places, both recorded in PROVENANCE.md:

- the admitted CC0 Quaternius Medieval Village kit under
  `assets/verse/everglade/village/` (`T_Brick_BaseColor` for the walls,
  vault, and pillar shafts, and `T_RockTrim_BaseColor` for the dressed
  stone), downscaled and tinted through the material's base color factor;
- small images this file authors with NumPy from seeded value noise (wood
  grain, iron, brass, flagstone, linen, parchment, a written page, a jar
  label, bone, and a rug pattern), packed into each model as PNG.

Frame: 1 unit = 1 m. Fronts face -Y in Blender, which the glTF export turns
into +Z. A prop's origin is on the ground at its base center; a wall-hung
prop's origin is on the floor below the wall face it hangs on. The hall's
origin is the center of its floor; glTF +Y is up and glTF +Z is the
entrance end. The hall is closed: candles, braziers, and cauldrons light it,
and one window in the far gable lets in a shaft of moonlight.

Budgets: the hall stays under 30,000 triangles and every prop under 5,000.
Emission strengths are luminance in cd/m², which Verse reads from the glTF
emissive factor and `KHR_materials_emissive_strength`.
"""

import math
import os
import sys

import bmesh
import bpy
import numpy as np
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

KITS = os.path.join(kit.REPO, "assets", "verse", "everglade")

# The hall, in Blender space: X across, Y along (entrance at -Y), Z up.
HALF_X = 6.0
HALF_Y = 8.6
SPRING = 4.4  # where the vault springs from the walls
RISE = 2.7  # the vault's rise above the springing line
PANEL = 0.55  # depth of the inner wall facing, which the alcoves cut
SHELL = 0.6  # the outer wall behind the alcoves
BAYS = (-5.6, -1.9, 1.9, 5.6)  # pillar stations along the long walls
NICHES = (-3.75, 0.0, 3.75)  # alcove centers between the pillars
NICHE_W = 1.9
NICHE_H = 2.9

NAMES = [
    "crypt_hall",
    "slab_table",
    "cauldron_green",
    "cauldron_red",
    "cauldron_amber",
    "candelabrum_tall",
    "candelabrum_short",
    "floor_candles",
    "ritual_rug",
    "specimen_jar",
    "specimen_jar_bones",
    "alchemy_bench",
    "bone_scatter",
    "cobweb",
    "bookshelf",
    "jar_shelf",
    "writing_desk",
    "lectern",
    "chained_skeleton",
    "hanging_chains",
    "brazier",
    "crate",
    "barrel",
    "iron_cage",
    "sarcophagus",
]
BUDGET = {"crypt_hall": 30000}
PROP_BUDGET = 5000
BLOCKERS = ("wall", "pillar", "dais", "door")


def out_dir_and_names():
    argv = kit.args()
    default = os.path.join(kit.REPO, "assets", "verse", "generated", "chamber")
    if argv and argv[0] not in NAMES:
        return argv[0], (argv[1:] or NAMES)
    return default, (argv or NAMES)


# --- Authored textures --------------------------------------------------------


def rng_for(text):
    value = 2166136261
    for char in text:
        value ^= ord(char)
        value = (value * 16777619) & 0xFFFFFFFF
    return np.random.default_rng(value)


def value_noise(n, cells, rng):
    """Tileable value noise on an n x n grid with `cells` lattice cells."""
    grid = rng.random((cells, cells))
    x = np.arange(n) * cells / n
    i0 = np.floor(x).astype(int) % cells
    i1 = (i0 + 1) % cells
    f = x - np.floor(x)
    f = f * f * (3 - 2 * f)
    rows0 = grid[i0][:, i0] * (1 - f)[None, :] + grid[i0][:, i1] * f[None, :]
    rows1 = grid[i1][:, i0] * (1 - f)[None, :] + grid[i1][:, i1] * f[None, :]
    return rows0 * (1 - f)[:, None] + rows1 * f[:, None]


def fbm(n, cells, rng, octaves=4):
    total = np.zeros((n, n))
    amp = 1.0
    norm = 0.0
    for k in range(octaves):
        c = cells * (2**k)
        if c > n:
            break
        total += value_noise(n, c, rng) * amp
        norm += amp
        amp *= 0.5
    return total / norm


def to_image(name, rgb):
    """Pack an (h, w, 3) array of display colors, 0 to 1, as a PNG image."""
    h, w, _ = rgb.shape
    img = bpy.data.images.new(name, w, h, alpha=False)
    rgba = np.ones((h, w, 4), dtype=np.float32)
    rgba[:, :, :3] = np.clip(rgb, 0.0, 1.0)
    img.pixels.foreach_set(rgba.ravel())
    img.file_format = "PNG"
    img.pack()
    return img


def mix(a, b, t):
    t = t[..., None] if np.ndim(t) == 2 else t
    return np.asarray(a) * (1 - t) + np.asarray(b) * t


def tex_wood(n=256):
    """Dark oak planks: grain along U, four boards per repeat."""
    rng = rng_for("wood")
    v = np.arange(n)[:, None] / n
    u = np.arange(n)[None, :] / n
    warp = fbm(n, 4, rng) * 0.6
    grain = np.sin((v * 34 + warp * 9 + fbm(n, 8, rng) * 1.5) * math.pi) * 0.5 + 0.5
    board = np.floor(v * 4)
    shade = 0.85 + 0.3 * rng.random(4)[board.astype(int) % 4]
    seam = np.clip(np.abs((v * 4) % 1 - 0.5) * 2 - 0.94, 0, 1) / 0.06
    knots = np.clip(fbm(n, 16, rng) - 0.72, 0, 1) * 3
    t = np.clip(grain * 0.55 + fbm(n, 32, rng) * 0.45, 0, 1)
    rgb = mix((0.20, 0.12, 0.07), (0.38, 0.24, 0.13), t)
    rgb = rgb * (shade * (1 - seam * 0.75) * (1 - knots * 0.5) + 0 * u)[..., None]
    return rgb


def tex_iron(n=128):
    rng = rng_for("iron")
    base = fbm(n, 4, rng)
    rust = np.clip((fbm(n, 8, rng) - 0.58) * 4, 0, 1)
    rgb = mix((0.10, 0.10, 0.10), (0.20, 0.19, 0.18), base)
    return mix(rgb, (0.30, 0.15, 0.07), rust * 0.8)


def tex_brass(n=128):
    rng = rng_for("brass")
    base = fbm(n, 4, rng)
    tarnish = np.clip((fbm(n, 8, rng) - 0.5) * 3, 0, 1)
    rgb = mix((0.55, 0.40, 0.17), (0.78, 0.60, 0.28), base)
    return mix(rgb, (0.22, 0.24, 0.16), tarnish * 0.7)


def tex_flag(n=256):
    """Worn flagstone: mottled gray-brown with pits and a darker rim."""
    rng = rng_for("flag")
    base = fbm(n, 4, rng, 5)
    speck = (rng.random((n, n)) > 0.985).astype(float)
    pits = np.clip((fbm(n, 16, rng) - 0.66) * 5, 0, 1)
    rgb = mix((0.22, 0.20, 0.18), (0.40, 0.37, 0.33), base)
    rgb = rgb * (1 - pits * 0.35)[..., None] * (1 - speck * 0.4)[..., None]
    x = np.abs(np.arange(n) / n - 0.5) * 2
    rim = np.maximum(x[None, :], x[:, None])
    rim = np.clip((rim - 0.86) / 0.14, 0, 1)
    return rgb * (1 - rim * 0.45)[..., None]


def tex_parchment(n=128, lines=False, name="parchment"):
    rng = rng_for(name)
    base = fbm(n, 4, rng)
    stain = np.clip((fbm(n, 8, rng) - 0.6) * 3, 0, 1)
    rgb = mix((0.72, 0.62, 0.44), (0.86, 0.79, 0.62), base)
    rgb = mix(rgb, (0.45, 0.32, 0.18), stain * 0.6)
    if lines:
        ink = np.zeros((n, n))
        row = n // 14
        for r in range(2, 13):
            y = r * row
            start = int(n * 0.1)
            end = int(n * (0.6 + 0.3 * rng.random()))
            for x in range(start, end):
                if rng.random() < 0.82:
                    h = int(1 + 2 * rng.random())
                    ink[y - h : y + 1, x] = 1
        rgb = mix(rgb, (0.12, 0.08, 0.05), ink * 0.85)
    return rgb


def tex_label(n=64):
    rgb = tex_parchment(n, name="label")
    ink = np.zeros((n, n))
    for y in (n // 3, n // 2, 2 * n // 3):
        ink[y - 2 : y + 1, n // 6 : 5 * n // 6] = 1
    ink[n // 2 - 1 : n // 2 + 1, n // 6 : 5 * n // 6] = 0
    return mix(rgb, (0.15, 0.08, 0.05), ink * 0.8)


def tex_rug(w=192, h=288):
    """A wool rug: a dark red field of small diamonds, a guard stripe, a
    gold-and-indigo border of steps, and a central medallion."""
    rng = rng_for("rug")
    u = (np.arange(w)[None, :] + 0.5) / w
    v = (np.arange(h)[:, None] + 0.5) / h
    # Distance to the edge in rug units, so the border is even all round.
    ex = np.minimum(u, 1 - u) * 2.4
    ey = np.minimum(v, 1 - v) * 3.6
    edge = np.minimum(ex, ey) + 0 * u * v
    red = np.array((0.40, 0.06, 0.05))
    dark = np.array((0.16, 0.03, 0.03))
    indigo = np.array((0.07, 0.09, 0.2))
    gold = np.array((0.72, 0.52, 0.2))
    cream = np.array((0.78, 0.7, 0.52))
    rgb = np.zeros((h, w, 3)) + red
    # The field: a lattice of small diamonds.
    gx = (u * 2.4 / 0.2) % 1 - 0.5
    gy = (v * 3.6 / 0.2) % 1 - 0.5
    diamond = np.abs(gx) + np.abs(gy)
    rgb = np.where((diamond < 0.22)[..., None], dark, rgb)
    rgb = np.where(((diamond > 0.40) & (diamond < 0.46))[..., None], gold * 0.7, rgb)
    # The medallion.
    cx, cy = (u - 0.5) * 2.4, (v - 0.5) * 3.6
    med = np.abs(cx) / 0.75 + np.abs(cy) / 1.05
    rgb = np.where((med < 1.0)[..., None], indigo, rgb)
    rgb = np.where(((med > 0.94) & (med < 1.0))[..., None], gold, rgb)
    rgb = np.where(((med > 0.55) & (med < 0.6))[..., None], cream, rgb)
    rgb = np.where((med < 0.3)[..., None], red, rgb)
    rgb = np.where((med < 0.12)[..., None], gold, rgb)
    # The border: an indigo band of gold steps between guard stripes.
    band = (edge > 0.08) & (edge < 0.26)
    steps = ((np.floor(u * 2.4 / 0.09) + np.floor(v * 3.6 / 0.09)) % 2 == 0) & (np.abs(edge - 0.17) < 0.045)
    rgb = np.where(band[..., None], indigo, rgb)
    rgb = np.where((band & steps)[..., None], gold, rgb)
    for a, b, c in ((0.0, 0.08, dark), (0.26, 0.29, cream), (0.29, 0.31, dark)):
        rgb = np.where(((edge >= a) & (edge < b))[..., None], c, rgb)
    wear = fbm(max(w, h), 6, rng)[:h, :w]
    worn = np.clip((wear - 0.62) * 3, 0, 1)
    rgb = rgb * (0.82 + 0.3 * fbm(max(w, h), 24, rng)[:h, :w])[..., None]
    return mix(rgb, np.array((0.3, 0.24, 0.18)), worn * 0.5)


def tex_bone(n=64):
    rng = rng_for("bone")
    base = fbm(n, 4, rng)
    return mix((0.62, 0.56, 0.44), (0.84, 0.80, 0.68), base)


# --- Materials ------------------------------------------------------------------


def img_mat(name, img, rough=0.85, metal=0.0, tint=None, emit=None, strength=0.0):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nodes = m.node_tree.nodes
    bsdf = nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = rough
    bsdf.inputs["Metallic"].default_value = metal
    tex = nodes.new("ShaderNodeTexImage")
    tex.image = img
    out = tex.outputs["Color"]
    if tint is not None:
        node = nodes.new("ShaderNodeMix")
        node.data_type = "RGBA"
        node.blend_type = "MULTIPLY"
        node.inputs["Factor"].default_value = 1.0
        m.node_tree.links.new(out, node.inputs["A"])
        node.inputs["B"].default_value = (*tint, 1.0)
        out = node.outputs["Result"]
    m.node_tree.links.new(out, bsdf.inputs["Base Color"])
    if emit is not None:
        bsdf.inputs["Emission Color"].default_value = (*emit, 1.0)
        bsdf.inputs["Emission Strength"].default_value = strength
    m.diffuse_color = (*(tint or (0.5, 0.5, 0.5)), 1.0)
    return m


def kit_image(rel, size):
    img = bpy.data.images.load(os.path.join(KITS, rel))
    img.scale(size, size)
    img.pack()
    return img


def fade(material, alpha):
    """Mark a material as blended and double-sided. Verse reads that alpha."""
    material.node_tree.nodes["Principled BSDF"].inputs["Alpha"].default_value = alpha
    material.blend_method = "BLEND"
    if hasattr(material, "surface_render_method"):
        material.surface_render_method = "BLENDED"
    material.use_backface_culling = False
    color = material.diffuse_color
    material.diffuse_color = (color[0], color[1], color[2], alpha)
    return material


class Mats:
    """The shared material set, built lazily for each model."""

    def __init__(self):
        self._cache = {}

    def get(self, key):
        if key not in self._cache:
            self._cache[key] = getattr(self, "_" + key)()
        return self._cache[key]

    def __getattr__(self, key):
        if key.startswith("_"):
            raise AttributeError(key)
        return self.get(key)

    def _wood(self):
        return img_mat("Wood_Oak", to_image("T_Lab_Wood", tex_wood()), rough=0.78)

    def _wood_dark(self):
        return img_mat("Wood_Dark", to_image("T_Lab_WoodDark", tex_wood()), rough=0.8, tint=(0.55, 0.5, 0.48))

    def _iron(self):
        return img_mat("Iron", to_image("T_Lab_Iron", tex_iron()), rough=0.55, metal=0.85)

    def _brass(self):
        return img_mat("Brass", to_image("T_Lab_Brass", tex_brass()), rough=0.4, metal=1.0)

    def _bone(self):
        return img_mat("Bone", to_image("T_Lab_Bone", tex_bone()), rough=0.7)

    def _bone_dark(self):
        return kit.mat("Bone_Socket", (0.05, 0.04, 0.03), 0.9)

    def _parchment(self):
        return img_mat("Parchment", to_image("T_Lab_Parchment", tex_parchment()), rough=0.9)

    def _page(self):
        return img_mat("Page_Written", to_image("T_Lab_Page", tex_parchment(128, True, "page")), rough=0.9)

    def _label(self):
        return img_mat("Jar_Label", to_image("T_Lab_Label", tex_label()), rough=0.9)

    def _wax(self):
        return kit.mat("Candle_Wax", (0.86, 0.80, 0.64), 0.55)

    def _wax_old(self):
        return kit.mat("Candle_WaxOld", (0.74, 0.66, 0.48), 0.6)

    def _wick(self):
        return kit.mat("Candle_Wick", (0.03, 0.03, 0.03), 0.9)

    def _flame(self):
        return kit.mat("Candle_Flame", (1.0, 0.72, 0.32), 0.5, emit=(1.0, 0.62, 0.22), strength=1800.0)

    def _ember(self):
        return kit.mat("Ember", (1.0, 0.36, 0.08), 0.8, emit=(1.0, 0.32, 0.06), strength=900.0)

    def _coal(self):
        return kit.mat("Coal", (0.04, 0.035, 0.03), 0.95)

    def _stone(self):
        return img_mat("Stone_Block", kit_image("village/T_Brick_BaseColor.png", 256), rough=0.9, tint=(0.62, 0.58, 0.54))

    def _stone_trim(self):
        return img_mat("Stone_Trim", kit_image("village/T_RockTrim_BaseColor.png", 256), rough=0.88, tint=(0.66, 0.62, 0.58))

    def _glass(self):
        return fade(kit.mat("Glass", (0.62, 0.74, 0.70), 0.06), 0.22)

    def _cloth(self):
        linen = tex_parchment(128, name="linen")
        gray = linen.mean(axis=2, keepdims=True) * 0.6 + linen * 0.4
        return img_mat("Shroud", to_image("T_Lab_Linen", gray), rough=0.95, tint=(0.78, 0.77, 0.74))

    def _blood(self):
        return kit.mat("Stain", (0.07, 0.008, 0.006), 0.5)

    def _leather_red(self):
        return kit.mat("Leather_Red", (0.30, 0.06, 0.05), 0.7)

    def _leather_green(self):
        return kit.mat("Leather_Green", (0.08, 0.18, 0.10), 0.7)

    def _leather_brown(self):
        return kit.mat("Leather_Brown", (0.25, 0.14, 0.07), 0.7)

    def _leather_blue(self):
        return kit.mat("Leather_Blue", (0.07, 0.10, 0.22), 0.7)

    def _leather_black(self):
        return kit.mat("Leather_Black", (0.05, 0.045, 0.04), 0.65)

    def _clay(self):
        return kit.mat("Clay", (0.42, 0.24, 0.13), 0.85)

    def _ink(self):
        return kit.mat("Ink", (0.02, 0.02, 0.03), 0.2)

    def potion(self, key, rgb, strength=0.0):
        """A colored liquid; `strength` makes it glow."""
        name = "potion_" + key
        if name not in self._cache:
            self._cache[name] = kit.mat(
                "Potion_" + key, rgb, 0.15, emit=rgb if strength else None, strength=strength
            )
        return self._cache[name]


# --- Geometry helpers -------------------------------------------------------------


def uv_box(obj, tile=1.0, offset=(0.0, 0.0), swap=False):
    """World-space box projection, `tile` meters per repeat; `swap` turns
    the texture a quarter turn (vertical wood grain)."""
    kit.apply_modifiers(obj)
    me = obj.data
    if not me.uv_layers:
        me.uv_layers.new(name="UVMap")
    uv = me.uv_layers.active.data
    mw = obj.matrix_world
    for poly in me.polygons:
        n = mw.to_3x3() @ poly.normal
        ax = max(range(3), key=lambda i: abs(n[i]))
        for li in poly.loop_indices:
            p = mw @ me.vertices[me.loops[li].vertex_index].co
            u, v = [(p.y, p.z), (p.x, p.z), (p.x, p.y)][ax]
            if swap:
                u, v = v, u
            uv[li].uv = (u / tile + offset[0], v / tile + offset[1])
    return obj


def mesh_obj(name, verts, faces, material=None):
    me = bpy.data.meshes.new(name)
    me.from_pydata(verts, [], faces)
    me.update()
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    if material is not None:
        o.data.materials.append(material)
    return o


def prism(name, outline, depth, material, frame):
    """Extrude a 2D outline (counterclockwise in its plane) by `depth`.

    `frame` is (origin, u_axis, v_axis, normal) in Blender space: outline
    point (a, b) lands at origin + a*u + b*v, and the solid runs from the
    plane along `normal` for `depth`.
    """
    o, u, v, n = (Vector(x) for x in frame)
    front = [o + u * a + v * b for a, b in outline]
    back = [p + n * depth for p in front]
    verts = [tuple(p) for p in front + back]
    k = len(outline)
    faces = [list(reversed(range(k))), [k + i for i in range(k)]]
    for i in range(k):
        j = (i + 1) % k
        faces.append([i, j, k + j, k + i])
    obj = mesh_obj(name, verts, faces, material)
    kit.fix_normals(obj)
    return obj


def arch_points(cx, spring, half, rise, segs, start=0.0, end=math.pi):
    """Points along an elliptical arch from angle `start` to `end`."""
    pts = []
    for i in range(segs + 1):
        t = start + (end - start) * i / segs
        pts.append((cx + half * math.cos(t), spring + rise * math.sin(t)))
    return pts


def vault_z(x):
    """The vault's inner surface height above x (an elliptical barrel)."""
    c = max(-1.0, min(1.0, x / HALF_X))
    return SPRING + RISE * math.sqrt(max(0.0, 1 - c * c))


def align(obj, a, b):
    """Point an object's local +Z from a to b, centered between them."""
    a, b = Vector(a), Vector(b)
    d = b - a
    obj.location = (a + b) / 2
    obj.rotation_mode = "QUATERNION"
    obj.rotation_quaternion = d.to_track_quat("Z", "Y")
    return obj


def rod(name, a, b, r, material, verts=8, r2=None):
    length = (Vector(b) - Vector(a)).length
    o = kit.cyl(name, r, length, (0, 0, 0), material, verts=verts, r2=r2)
    return align(o, a, b)


def chain(name, points, material, link=0.075, thick=0.011):
    """Iron links along a polyline, each turned 90 degrees from the last."""
    parts = []
    count = 0
    for a, b in zip(points, points[1:]):
        a, b = Vector(a), Vector(b)
        d = b - a
        n = max(1, int(d.length / (link * 0.78)))
        for i in range(n):
            p = a + d * ((i + 0.5) / n)
            o = kit.ring("%s_%s" % (name, count), link * 0.38, thick, (0, 0, 0), material, segs=8, minor_segs=4)
            o.scale = (1.0, 1.55, 1.0)
            q = d.to_track_quat("Y", "Z")
            twist = Matrix.Rotation(math.pi / 2 * (count % 2), 4, "Y")
            o.matrix_world = Matrix.Translation(p) @ q.to_matrix().to_4x4() @ twist @ Matrix.Diagonal((1.0, 1.55, 1.0, 1.0))
            parts.append(o)
            count += 1
    return parts


def sag(a, b, drop, segs=8):
    a, b = Vector(a), Vector(b)
    pts = []
    for i in range(segs + 1):
        t = i / segs
        p = a.lerp(b, t)
        p.z -= drop * 4 * t * (1 - t)
        pts.append(tuple(p))
    return pts


def blob(name, r, h, loc, material, seed, segs=14, wobble=0.25):
    """A flattened, irregular puddle (wax, blood, liquid), base on loc."""
    rng = rng_for(name + str(seed))
    bm = bmesh.new()
    radii = [r * (1 + wobble * (rng.random() * 2 - 1)) for _ in range(segs)]
    top = bm.verts.new((0, 0, h))
    ring_top = []
    ring_bot = []
    for i in range(segs):
        t = 2 * math.pi * i / segs
        ring_top.append(bm.verts.new((radii[i] * 0.86 * math.cos(t), radii[i] * 0.86 * math.sin(t), h * 0.8)))
        ring_bot.append(bm.verts.new((radii[i] * math.cos(t), radii[i] * math.sin(t), 0)))
    for i in range(segs):
        j = (i + 1) % segs
        bm.faces.new((top, ring_top[i], ring_top[j]))
        bm.faces.new((ring_top[i], ring_bot[i], ring_bot[j], ring_top[j]))
    bm.faces.new(list(reversed(ring_bot)))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    o.location = loc
    o.data.materials.append(material)
    return o


def rock(name, s, loc, material, rng):
    """A rough fieldstone, flattened and half sunk at loc."""
    o = kit.ball(name, s * 0.6, loc, material, segs=7, rings=5, scale=(1.2 + rng.random() * 0.4, 0.9 + rng.random() * 0.3, 0.6))
    for v in o.data.vertices:
        v.co *= 0.85 + rng.random() * 0.3
    o.rotation_euler = (rng.random() * 0.3, rng.random() * 0.3, rng.random() * math.tau)
    return o


def flame(name, loc, m, size=1.0):
    """A teardrop flame standing on loc."""
    s = size
    profile = [(0, 0), (0.012 * s, 0.006 * s), (0.02 * s, 0.022 * s), (0.016 * s, 0.045 * s), (0.007 * s, 0.07 * s), (0, 0.085 * s)]
    return kit.lathe(name, profile, loc, m.flame, segs=6)


def candle(name, x, y, z, h, r, m, seed, wax=None, drips=3, lit=True):
    """A candle standing at (x, y, z): body, melted lip, drips, wick, flame."""
    rng = rng_for(name + str(seed))
    wax = wax or m.wax
    kit.cyl(name + "_body", r, h, (x, y, z + h / 2), wax, verts=10)
    kit.ring(name + "_lip", r * 0.92, r * 0.22, (x, y, z + h - r * 0.08), wax, segs=10, minor_segs=4)
    for i in range(drips):
        a = rng.random() * math.tau
        length = h * (0.2 + 0.55 * rng.random())
        dz = z + h - length / 2 - r * 0.1
        kit.ball(
            "%s_drip%s" % (name, i),
            r * 0.24,
            (x + math.cos(a) * r * 0.98, y + math.sin(a) * r * 0.98, dz),
            wax,
            segs=6,
            rings=4,
            scale=(1, 1, length / (r * 0.48)),
        )
    kit.cyl(name + "_wick", r * 0.08, 0.02, (x, y, z + h + 0.008), m.wick, verts=4)
    if lit:
        flame(name + "_flame", (x, y, z + h + 0.012), m, size=max(0.75, r / 0.03))


def skull(name, loc, m, s=1.0, yaw=0.0, tilt=0.0, jaw=True):
    """A human skull about 0.2 m long at scale 1, facing -Y."""
    parts = [
        kit.ball(name + "_cranium", 0.09 * s, (0, 0.015 * s, 0.1 * s), m.bone, segs=12, rings=8, scale=(0.82, 1.0, 0.92)),
        kit.ball(name + "_face", 0.06 * s, (0, -0.05 * s, 0.05 * s), m.bone, segs=10, rings=6, scale=(0.95, 0.8, 1.0)),
        kit.ball(name + "_eye_l", 0.019 * s, (-0.028 * s, -0.093 * s, 0.072 * s), m.bone_dark, segs=6, rings=4),
        kit.ball(name + "_eye_r", 0.019 * s, (0.028 * s, -0.093 * s, 0.072 * s), m.bone_dark, segs=6, rings=4),
        kit.ball(name + "_nose", 0.011 * s, (0, -0.103 * s, 0.045 * s), m.bone_dark, segs=5, rings=3, scale=(1, 1, 1.5)),
        kit.box(name + "_teeth", (0.05 * s, 0.012 * s, 0.014 * s), (0, -0.097 * s, 0.022 * s), m.bone),
    ]
    if jaw:
        parts.append(kit.ring(name + "_jaw", 0.042 * s, 0.011 * s, (0, -0.045 * s, 0.006 * s), m.bone, segs=10, minor_segs=4))
    pivot = Matrix.Translation(Vector(loc)) @ Matrix.Rotation(yaw, 4, "Z") @ Matrix.Rotation(tilt, 4, "X")
    return move(parts, pivot)


def move(parts, pivot):
    """Apply `pivot` on top of each part's own placement."""
    bpy.context.view_layer.update()
    for p in parts:
        p.matrix_world = pivot @ p.matrix_world
    return parts


def bone(name, a, b, r, m):
    """A long bone between a and b with knobbed ends."""
    a, b = Vector(a), Vector(b)
    d = (b - a).normalized()
    side = d.cross(Vector((0, 0, 1)))
    if side.length < 1e-3:
        side = Vector((1, 0, 0))
    side = side.normalized() * r * 0.9
    parts = [rod(name + "_shaft", a, b, r, m.bone, verts=6)]
    for k, end in enumerate((a, b)):
        parts.append(kit.ball("%s_k%s_a" % (name, k), r * 1.55, tuple(end + side), m.bone, segs=6, rings=4))
        parts.append(kit.ball("%s_k%s_b" % (name, k), r * 1.55, tuple(end - side), m.bone, segs=6, rings=4))
    return parts


def book(name, loc, size, cover, m, yaw=0.0, roll=0.0):
    """A closed book: cover boards round a page block. size = (w, depth, h)."""
    w, d, h = size
    parts = [
        kit.box(name + "_cover", (w, d, h), (0, 0, 0), cover),
        kit.box(name + "_pages", (w * 0.86, d * 0.94, h * 0.93), (0, -d * 0.04, 0), m.parchment),
    ]
    pivot = Matrix.Translation(Vector(loc)) @ Matrix.Rotation(yaw, 4, "Z") @ Matrix.Rotation(roll, 4, "Y")
    return move(parts, pivot)


def flask(name, loc, m, r=0.05, neck=0.08, liquid=None, kind="round"):
    """Glass: a round-bottom or conical flask with liquid inside."""
    x, y, z = loc
    if kind == "round":
        kit.ball(name + "_bulb", r, (x, y, z + r), m.glass, segs=10, rings=6)
        kit.cyl(name + "_neck", r * 0.28, neck, (x, y, z + r * 1.8 + neck / 2), m.glass, verts=8)
        if liquid:
            kit.ball(name + "_liquid", r * 0.86, (x, y, z + r * 0.92), liquid, segs=8, rings=5, scale=(1, 1, 0.7))
    else:
        kit.cyl(name + "_body", r, r * 1.6, (x, y, z + r * 0.8), m.glass, verts=10, r2=r * 0.3)
        kit.cyl(name + "_neck", r * 0.28, neck, (x, y, z + r * 1.6 + neck / 2), m.glass, verts=8)
        if liquid:
            kit.cyl(name + "_liquid", r * 0.86, r * 0.7, (x, y, z + r * 0.36), liquid, verts=8, r2=r * 0.55)


def bottle(name, loc, m, r=0.035, h=0.14, color=None, cork=True):
    x, y, z = loc
    kit.cyl(name + "_body", r, h * 0.7, (x, y, z + h * 0.35), color or m.glass, verts=8)
    kit.cyl(name + "_shoulder", r, h * 0.12, (x, y, z + h * 0.76), color or m.glass, verts=8, r2=r * 0.35)
    kit.cyl(name + "_neck", r * 0.33, h * 0.18, (x, y, z + h * 0.91), color or m.glass, verts=6)
    if cork:
        kit.cyl(name + "_cork", r * 0.36, h * 0.07, (x, y, z + h * 1.02), m.wood, verts=6)


# --- Finishing ----------------------------------------------------------------------


def shade(objs):
    """Smooth curved surfaces and keep edges sharper than 50 degrees hard."""
    for o in objs:
        for poly in o.data.polygons:
            poly.use_smooth = True
        o.data.set_sharp_from_angle(angle=math.radians(50))


def finish(name, ground=True, blockers=BLOCKERS, extra=None):
    shade(kit.meshes())
    sources = list(kit.meshes())
    boxes = None
    if not ground:
        boxes = [aabb_box(obj, obj.name) for obj in sources if obj.name.startswith(blockers)]
    obj = kit.join(name) if len(sources) > 1 else sources[0]
    if ground:
        kit.ground(obj)
        boxes = [aabb_box(obj, name)]
    elif not boxes:
        boxes = [aabb_box(obj, name)]
    write_footprint(name, boxes, extra)
    folder = out_dir_and_names()[0]
    info = kit.export(os.path.join(folder, name + ".glb"))
    limit = BUDGET.get(name, PROP_BUDGET)
    if info["triangles"] > limit:
        sys.exit("%s is %s triangles; the budget is %s" % (name, info["triangles"], limit))
    return info


def write_footprint(name, boxes, extra=None):
    """Write glTF-space collision boxes beside the model."""
    path = os.path.join(out_dir_and_names()[0], name + ".footprint.json")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    data = {
        "model": name + ".glb",
        "frame": "glTF: 1 unit = 1 m, +Y up, +Z toward the entrance, origin at the base center",
        "triangles": kit.triangles(),
        "boxes": boxes,
    }
    if extra:
        data.update(extra)
    with open(path, "w") as handle:
        kit.json.dump(data, handle, indent=2)
        handle.write("\n")


def aabb_box(obj, name):
    pts = [obj.matrix_world @ v.co for v in obj.data.vertices]
    # Blender (x, y, z) becomes glTF (x, z, -y).
    glo = [min(p.x for p in pts), min(p.z for p in pts), min(-p.y for p in pts)]
    ghi = [max(p.x for p in pts), max(p.z for p in pts), max(-p.y for p in pts)]
    return {
        "name": name,
        "center": [round((a + b) / 2, 3) for a, b in zip(glo, ghi)],
        "half_extents": [round((b - a) / 2, 3) for a, b in zip(glo, ghi)],
    }


# --- The hall -----------------------------------------------------------------------


def notched_outline(x0, x1, z0, z1, holes, segs=10):
    """A wall panel's outline in its plane with arched holes open at z0.

    `holes` is a list of (center, half_width, spring) in panel coordinates.
    The outline runs counterclockwise from (x0, z0).
    """
    pts = [(x0, z0)]
    for cx, half, spring in sorted(holes):
        pts.append((cx - half, z0))
        if spring > z0:
            pts.append((cx - half, spring))
        arc = arch_points(cx, spring, half, half, segs, math.pi, 0.0)
        pts.extend(arc[1:-1])
        if spring > z0:
            pts.append((cx + half, spring))
        pts.append((cx + half, z0))
    pts += [(x1, z0), (x1, z1), (x0, z1)]
    clean = []
    for p in pts:
        if not clean or (abs(p[0] - clean[-1][0]) > 1e-6 or abs(p[1] - clean[-1][1]) > 1e-6):
            clean.append(p)
    if abs(clean[0][0] - clean[-1][0]) < 1e-6 and abs(clean[0][1] - clean[-1][1]) < 1e-6:
        clean.pop()
    return clean


def voussoirs(name, cx, spring, half, frame, material, count=9, depth=0.06, width=0.24):
    """Raised arch stones around an arched opening, `frame` as for prism."""
    o, u, v, n = (Vector(x) for x in frame)
    parts = []
    for i in range(count):
        t = math.pi * (i + 0.5) / count
        a = cx + (half + width / 2) * math.cos(t)
        b = spring + (half + width / 2) * math.sin(t)
        p = o + u * a + v * b - n * depth / 2
        blk = kit.box("%s_%s" % (name, i), (width * 1.05, (half + width / 2) * math.pi / count * 0.94, depth), (0, 0, 0), material)
        # Local X runs radially, local Y along the arc, local Z off the wall.
        radial = (u * math.cos(t) + v * math.sin(t)).normalized()
        tangent = (u * -math.sin(t) + v * math.cos(t)).normalized()
        basis = Matrix((radial, tangent, -n)).transposed().to_4x4()
        blk.matrix_world = Matrix.Translation(p) @ basis
        parts.append(blk)
    return parts


def build_crypt_hall():
    """A closed, vaulted crypt hall. The floor center is the origin; the
    entrance is at Blender -Y, which the export turns into glTF +Z."""
    kit.reset()
    m = Mats()
    rng = rng_for("crypt_hall")
    flag_img = to_image("T_Lab_Flag", tex_flag())
    flags = [
        img_mat("Flag_A", flag_img, 0.88, tint=(1.0, 1.0, 1.0)),
        img_mat("Flag_B", flag_img, 0.9, tint=(0.82, 0.8, 0.76)),
        img_mat("Flag_C", flag_img, 0.86, tint=(0.95, 0.88, 0.8)),
    ]
    grout = kit.mat("Grout", (0.05, 0.045, 0.04), 0.95)
    stone = m.stone
    trim = m.stone_trim
    vault_mat = img_mat("Vault_Stone", kit_image("village/T_Brick_BaseColor.png", 256), 0.92, tint=(0.46, 0.43, 0.40))

    # Floor: a grout bed and individual flags, each a little off level, in
    # rows offset by half a flag as laid stone runs.
    kit.box("floor_bed", (HALF_X * 2 + 1.2, HALF_Y * 2 + 1.2, 0.2), (0, 0, -0.12), grout)
    ny = int(HALF_Y * 2)
    for j in range(ny):
        y = -HALF_Y + 0.5 + j
        cuts = [-HALF_X + k for k in range(int(HALF_X * 2) + 1)]
        if j % 2:
            cuts = [-HALF_X] + [c + 0.5 for c in cuts[:-1]] + [HALF_X]
        for i, (a, b) in enumerate(zip(cuts, cuts[1:])):
            h = 0.08 + rng.random() * 0.015
            f = kit.box(
                "flag_%s_%s" % (i, j),
                (b - a - 0.05, 0.95, h),
                ((a + b) / 2, y, -h / 2 + 0.004 + rng.random() * 0.01),
                flags[int(rng.random() * 3)],
                rot=(rng.normal() * 0.006, rng.normal() * 0.006, rng.normal() * 0.012),
            )
            uv_box(f, 1.0, (rng.random(), rng.random()))

    # Long walls: an inner facing cut by three arched alcoves each, and a
    # solid shell behind it that backs the alcoves.
    niche_spring = NICHE_H - NICHE_W / 2
    for side, sx in (("east", 1), ("west", -1)):
        x = sx * HALF_X
        frame = ((x, 0, 0), (0, 1, 0), (0, 0, 1), (sx, 0, 0))
        outline = notched_outline(-HALF_Y, HALF_Y, 0.0, SPRING, [(c, NICHE_W / 2, niche_spring) for c in NICHES])
        panel = prism("wall_face_%s" % side, outline, PANEL, stone, frame)
        uv_box(panel, 2.0)
        shell = kit.box(
            "wall_shell_%s" % side,
            (SHELL, HALF_Y * 2 + 2 * (PANEL + SHELL), SPRING + RISE + 1.0),
            (x + sx * (PANEL + SHELL / 2), 0, (SPRING + RISE + 1.0) / 2 - 0.1),
            stone,
        )
        uv_box(shell, 2.0)
        for c in NICHES:
            sill = kit.box("niche_sill_%s_%s" % (side, c), (PANEL, NICHE_W, 0.06), (x + sx * PANEL / 2, c, -0.02), trim)
            uv_box(sill, 1.0)
            for blk in voussoirs("arch_%s_%s" % (side, c), c, niche_spring, NICHE_W / 2, frame, trim):
                uv_box(blk, 1.0)
        # The string course where the vault springs.
        course = kit.box("course_%s" % side, (0.22, HALF_Y * 2, 0.24), (x - sx * 0.1, 0, SPRING - 0.12), trim)
        uv_box(course, 1.0)
        # A low plinth on each pier between the alcoves.
        edges = [-HALF_Y] + [e for c in NICHES for e in (c - NICHE_W / 2, c + NICHE_W / 2)] + [HALF_Y]
        for k in range(0, len(edges), 2):
            a, b = edges[k], edges[k + 1]
            plinth = kit.box("plinth_%s_%s" % (side, k), (0.14, b - a, 0.32), (x - sx * 0.07, (a + b) / 2, 0.16), trim)
            uv_box(plinth, 1.0)

    # End walls: the far wall's arched recess behind the dais, and the
    # entrance wall's doorway.
    door_half, door_spring = 1.2, 2.2
    recess_half, recess_spring = 1.6, 2.4
    for end, sy, hole in (("far", 1, (0.0, recess_half, recess_spring)), ("entrance", -1, (0.0, door_half, door_spring))):
        y = sy * HALF_Y
        frame = ((0, y, 0), (1, 0, 0), (0, 0, 1), (0, sy, 0))
        outline = notched_outline(-HALF_X, HALF_X, 0.0, SPRING, [hole])
        panel = prism("wall_face_%s" % end, outline, PANEL, stone, frame)
        uv_box(panel, 2.0)
        for blk in voussoirs("arch_%s" % end, hole[0], hole[2], hole[1], frame, trim, count=11, width=0.3):
            uv_box(blk, 1.0)
        course = kit.box("course_%s" % end, (HALF_X * 2, 0.22, 0.24), (0, y - sy * 0.1, SPRING - 0.12), trim)
        uv_box(course, 1.0)
        back = kit.box(
            "wall_shell_%s" % end,
            (HALF_X * 2 + 2 * (PANEL + SHELL), SHELL, SPRING),
            (0, y + sy * (PANEL + SHELL / 2), SPRING / 2),
            stone,
        )
        uv_box(back, 2.0)
        # The gable above the springing line, in strips under the vault. The
        # far gable leaves a lancet window open at its center.
        window = end == "far"
        breaks = [-HALF_X + (HALF_X - 0.45) * k / 12 for k in range(13)]
        breaks += [0.45 + (HALF_X - 0.45) * k / 12 for k in range(13)]
        for a, b in zip(breaks, breaks[1:]):
            top = max(vault_z(a), vault_z(b)) + 0.5
            if window and a >= -0.46 and b <= 0.46:
                sill = kit.box(
                    "wall_gable_%s_sill" % end,
                    (b - a + 0.01, PANEL + SHELL, 5.2 - SPRING),
                    ((a + b) / 2, y + sy * (PANEL + SHELL) / 2, (SPRING + 5.2) / 2),
                    stone,
                )
                uv_box(sill, 2.0)
                head = prism(
                    "wall_gable_%s_head" % end,
                    notched_outline(a, b, 6.35, top, [(0.0, 0.45, 6.35)], segs=8),
                    PANEL + SHELL,
                    stone,
                    ((0, y, 0), (1, 0, 0), (0, 0, 1), (0, sy, 0)),
                )
                uv_box(head, 2.0)
                continue
            strip = kit.box(
                "wall_gable_%s_%.2f" % (end, a),
                (b - a + 0.01, PANEL + SHELL, top - SPRING),
                ((a + b) / 2, y + sy * (PANEL + SHELL) / 2, (SPRING + top) / 2),
                stone,
            )
            uv_box(strip, 2.0)
        if window:
            sill_stone = kit.box("window_sill", (1.1, PANEL + 0.2, 0.12), (0, y + sy * 0.15, 5.2), trim)
            uv_box(sill_stone, 1.0)
            for bx in (-0.22, 0.0, 0.22):
                rod("window_bar_%s" % bx, (bx, y + sy * 0.3, 5.2), (bx, y + sy * 0.3, 6.75), 0.025, m.iron, verts=6)
            rod("window_rail", (-0.45, y + sy * 0.3, 5.8), (0.45, y + sy * 0.3, 5.8), 0.022, m.iron, verts=6)

    # The far recess's back.
    recess_back = kit.box(
        "wall_recess_back",
        (recess_half * 2 + 0.2, 0.2, recess_spring + recess_half + 0.2),
        (0, HALF_Y + PANEL + 0.1, (recess_spring + recess_half) / 2),
        stone,
    )
    uv_box(recess_back, 2.0)

    # Pillars between the alcoves: a plinth, a torus base, a shaft, a
    # flared capital, and an abacus the ribs spring from.
    for side, sx in (("e", 1), ("w", -1)):
        for y in BAYS:
            x = sx * (HALF_X - 0.3)
            base = kit.box("pillar_plinth_%s_%s" % (side, y), (0.95, 0.95, 0.34), (x, y, 0.17), trim)
            uv_box(base, 1.0)
            kit.ring("pillar_torus_%s_%s" % (side, y), 0.4, 0.07, (x, y, 0.4), trim, segs=16, minor_segs=5)
            shaft = kit.cyl("pillar_shaft_%s_%s" % (side, y), 0.36, SPRING - 0.9, (x, y, 0.38 + (SPRING - 0.9) / 2), stone, verts=14)
            kit.cyl_uv(shaft, 1.5)
            kit.ring("pillar_neck_%s_%s" % (side, y), 0.37, 0.04, (x, y, SPRING - 0.52), trim, segs=14, minor_segs=4)
            cap = kit.cyl("pillar_capital_%s_%s" % (side, y), 0.36, 0.34, (x, y, SPRING - 0.33), trim, verts=14, r2=0.56)
            kit.cyl_uv(cap, 1.0)
            abacus = kit.box("pillar_abacus_%s_%s" % (side, y), (1.12, 1.12, 0.18), (x, y, SPRING - 0.08), trim)
            uv_box(abacus, 1.0)

    # The vault: an elliptical barrel, solid, its faces toward the room.
    segs = 24
    verts = []
    for yy in (-HALF_Y - PANEL, HALF_Y + PANEL):
        for i in range(segs + 1):
            t = math.pi * i / segs
            verts.append((HALF_X * math.cos(t), yy, SPRING + RISE * math.sin(t)))
    faces = [(i, segs + 1 + i, segs + 2 + i, i + 1) for i in range(segs)]
    vault = mesh_obj("vault", verts, faces, vault_mat)
    me = vault.data
    me.update()
    for poly in me.polygons:
        c = poly.center
        inward = Vector((-c.x / HALF_X ** 2, 0, -(c.z - SPRING) / RISE ** 2))
        if poly.normal.dot(inward) < 0:
            poly.flip()
    me.update()
    arc = [0.0]
    for i in range(1, segs + 1):
        arc.append(arc[-1] + (Vector(verts[i]) - Vector(verts[i - 1])).length)
    layer = me.uv_layers.new(name="UVMap")
    for poly in me.polygons:
        for li in poly.loop_indices:
            vi = me.loops[li].vertex_index
            layer.data[li].uv = (verts[vi][1] / 2.0, arc[vi % (segs + 1)] / 2.0)
    sol = vault.modifiers.new("Solidify", "SOLIDIFY")
    sol.thickness = 0.35
    sol.offset = -1.0

    # Ribs: transverse arches at each pillar pair and a ridge along the crown.
    for y in BAYS:
        pts = [(HALF_X * math.cos(math.pi * i / 16), SPRING + RISE * math.sin(math.pi * i / 16)) for i in range(17)]
        for i, (a, b) in enumerate(zip(pts, pts[1:])):
            p0 = Vector((a[0], y, a[1]))
            p1 = Vector((b[0], y, b[1]))
            mid = (p0 + p1) / 2
            inward = Vector((-mid.x / HALF_X ** 2, 0, -(mid.z - SPRING) / RISE ** 2)).normalized()
            seg = kit.box("rib_%s_%s" % (y, i), ((p1 - p0).length + 0.05, 0.34, 0.28), (0, 0, 0), trim)
            along = (p1 - p0).normalized()
            basis = Matrix((along, Vector((0, 1, 0)), -inward)).transposed().to_4x4()
            seg.matrix_world = Matrix.Translation(mid + inward * 0.1) @ basis
            uv_box(seg, 1.0)
        boss = kit.ball("boss_%s" % y, 0.2, (0, y, SPRING + RISE - 0.3), trim, segs=10, rings=6, scale=(1, 1, 0.6))
        uv_box(boss, 0.5)
    ridge = kit.box("ridge", (0.26, HALF_Y * 2, 0.22), (0, 0, SPRING + RISE - 0.08), trim)
    uv_box(ridge, 1.0)

    # The far dais, two steps up, where the sarcophagus lies.
    for k, (w, d) in enumerate(((4.4, 2.6), (3.6, 1.9))):
        step = kit.box("dais_%s" % k, (w, d, 0.18), (0, HALF_Y - d / 2, 0.09 + k * 0.18), trim)
        uv_box(step, 1.0)

    # The entrance door: two heavy oak leaves under iron straps and studs.
    wood = m.wood
    iron = m.iron
    y = -HALF_Y - PANEL * 0.55
    door_outline = [(door_half, 0.0)] + arch_points(0.0, door_spring, door_half, door_half, 12) + [(-door_half, 0.0)]
    door = prism("door_leaves", door_outline, 0.14, wood, ((0, y, 0), (1, 0, 0), (0, 0, 1), (0, 1, 0)))
    uv_box(door, 1.6, swap=True)
    kit.box("door_seam", (0.03, 0.02, door_spring + door_half - 0.05), (0, y - 0.01, (door_spring + door_half) / 2), kit.mat("Door_Seam", (0.02, 0.015, 0.01), 0.9))
    for z in (0.45, 1.5, 2.55):
        half = door_half if z < door_spring else math.sqrt(max(0.0, door_half ** 2 - (z - door_spring) ** 2))
        kit.box("door_strap_%s" % z, (half * 2 - 0.1, 0.03, 0.11), (0, y - 0.02, z), iron)
        for k in range(-5, 6):
            sx = k * (half - 0.15) / 5
            kit.ball("door_stud_%s_%s" % (z, k), 0.028, (sx, y - 0.04, z), iron, segs=6, rings=3)
    for sx in (-0.28, 0.28):
        kit.cyl("door_plate_%s" % sx, 0.09, 0.02, (sx, y - 0.03, 1.15), iron, verts=8, rot=(math.pi / 2, 0, 0))
        kit.ring("door_ring_%s" % sx, 0.1, 0.016, (sx, y - 0.06, 1.05), iron, segs=12, minor_segs=4, rot=(math.pi / 2, 0, 0))
    threshold = kit.box("door_threshold", (door_half * 2 + 0.3, PANEL + 0.1, 0.06), (0, -HALF_Y - PANEL / 2, 0.01), trim)
    uv_box(threshold, 1.0)

    # Rubble against the walls, clear of the pillars and alcoves.
    for k in range(14):
        sx = 1 if k % 2 else -1
        y = -HALF_Y + 0.6 + rng.random() * (HALF_Y * 2 - 1.2)
        if any(abs(y - b) < 0.8 for b in BAYS) or any(abs(y - c) < NICHE_W / 2 + 0.1 for c in NICHES):
            continue
        s = 0.1 + rng.random() * 0.14
        r = kit.box(
            "rubble_%s" % k,
            (s * 1.4, s, s * 0.8),
            (sx * (HALF_X - 0.3 - rng.random() * 0.2), y, s * 0.3),
            stone,
            rot=(rng.random() * 0.4, rng.random() * 0.4, rng.random() * 3),
        )
        uv_box(r, 1.0)

    window = {"window": {"center": [0.0, 5.78, -(HALF_Y + (PANEL + SHELL) / 2)], "half_width": 0.45}}
    return finish("crypt_hall", ground=False, extra=window)


def wrap(obj, tile=1.0, swap=False):
    """Cylindrical UVs about the object's own vertical axis."""
    bpy.context.view_layer.update()
    loc = obj.matrix_world.translation
    kit.cyl_uv(obj, tile, (loc.x, loc.y))
    if swap:
        for d in obj.data.uv_layers.active.data:
            d.uv = (d.uv[1], d.uv[0])
    return obj


def cyl_patch(name, r, a0, a1, z0, z1, loc, material, segs=6):
    """A curved label: part of a cylinder's side, UVs 0 to 1."""
    verts = []
    for z in (z0, z1):
        for i in range(segs + 1):
            a = a0 + (a1 - a0) * i / segs
            verts.append((loc[0] + r * math.cos(a), loc[1] + r * math.sin(a), loc[2] + z))
    faces = [(i, i + 1, segs + 2 + i, segs + 1 + i) for i in range(segs)]
    o = mesh_obj(name, verts, faces, material)
    layer = o.data.uv_layers.new(name="UVMap")
    for poly in o.data.polygons:
        for li in poly.loop_indices:
            vi = o.data.loops[li].vertex_index
            layer.data[li].uv = ((vi % (segs + 1)) / segs, vi // (segs + 1))
    for poly in o.data.polygons:
        c = poly.center
        if (c.x - loc[0]) * poly.normal.x + (c.y - loc[1]) * poly.normal.y < 0:
            poly.flip()
    return o


# --- Brewing ------------------------------------------------------------------------


def build_cauldron(name, rgb):
    """An iron cauldron on claw feet over a ring of stones and embers, its
    liquid glowing and bubbling."""
    kit.reset()
    m = Mats()
    rng = rng_for(name)
    iron = m.iron
    liquid = kit.mat("Liquid_" + name, rgb, 0.12, emit=rgb, strength=140.0)
    froth = kit.mat("Froth_" + name, tuple(min(1.0, c * 0.6 + 0.4) for c in rgb), 0.3, emit=rgb, strength=220.0)
    bowl = kit.lathe(
        "Bowl",
        [(0, 0.3), (0.28, 0.31), (0.5, 0.38), (0.64, 0.52), (0.68, 0.66), (0.64, 0.8), (0.56, 0.9), (0.55, 0.96),
         (0.49, 0.96), (0.5, 0.86), (0.0, 0.84)],
        material=iron,
        segs=24,
    )
    wrap(bowl, 1.0)
    kit.ring("Rim", 0.53, 0.045, (0, 0, 0.96), iron, segs=24, minor_segs=6)
    kit.ring("Band", 0.665, 0.02, (0, 0, 0.66), iron, segs=24, minor_segs=4)
    # Lugs and a bail handle resting back over one side.
    for sx in (-1, 1):
        kit.ring("Lug%s" % sx, 0.06, 0.018, (sx * 0.6, 0, 0.9), iron, segs=8, minor_segs=4, rot=(math.pi / 2, 0, 0))
    pts = []
    for i in range(13):
        t = math.pi * i / 12
        pts.append(Vector((0.6 * math.cos(t), 0.36 * math.sin(t), 0.9 + 0.22 * math.sin(t))))
    for i, (a, b) in enumerate(zip(pts, pts[1:])):
        rod("Bail%s" % i, a, b, 0.016, iron, verts=6)
    # Three claw feet.
    for i in range(3):
        a = i * math.tau / 3 + 0.4
        top = Vector((0.36 * math.cos(a), 0.36 * math.sin(a), 0.36))
        knee = Vector((0.5 * math.cos(a), 0.5 * math.sin(a), 0.18))
        foot = Vector((0.52 * math.cos(a), 0.52 * math.sin(a), 0.04))
        rod("Leg%s_a" % i, top, knee, 0.045, iron, verts=6, r2=0.035)
        rod("Leg%s_b" % i, knee, foot, 0.035, iron, verts=6)
        kit.ball("Foot%s" % i, 0.06, tuple(foot), iron, segs=6, rings=4, scale=(1.3, 1.3, 0.7))
    # The liquid, a ring of froth, and bubbles breaking its surface.
    kit.cyl("Liquid", 0.5, 0.04, (0, 0, 0.86), liquid, verts=24)
    kit.ring("Froth", 0.47, 0.025, (0, 0, 0.885), froth, segs=24, minor_segs=4)
    for i in range(9):
        a = rng.random() * math.tau
        d = rng.random() * 0.38
        r = 0.03 + rng.random() * 0.05
        kit.ball("Bubble%s" % i, r, (d * math.cos(a), d * math.sin(a), 0.88), froth if i % 3 == 0 else liquid, segs=8, rings=4, scale=(1, 1, 0.6))
    # The fire beneath: stones, crossed logs, coals, and embers.
    stone = m.stone_trim
    for i in range(11):
        a = i * math.tau / 11 + rng.random() * 0.2
        s = 0.13 + rng.random() * 0.05
        blk = rock("Stone%s" % i, s, (0.78 * math.cos(a), 0.78 * math.sin(a), s * 0.12), stone, rng)
        uv_box(blk, 0.6, (rng.random(), rng.random()))
    for i in range(3):
        a = i * math.tau / 3 + 1.0
        rod("Log%s" % i, (0.5 * math.cos(a), 0.5 * math.sin(a), 0.05), (-0.15 * math.cos(a), -0.15 * math.sin(a), 0.12), 0.05, m.wood_dark, verts=7)
    blob("Coals", 0.42, 0.07, (0, 0, 0), m.coal, 1, segs=12)
    for i in range(7):
        a = rng.random() * math.tau
        d = rng.random() * 0.3
        kit.ball("Ember%s" % i, 0.04 + rng.random() * 0.03, (d * math.cos(a), d * math.sin(a), 0.06), m.ember, segs=6, rings=4, scale=(1.4, 1.1, 0.6))
    return finish(name)


def build_brazier():
    """A wrought-iron brazier: a spiked bowl of glowing coals on a tripod."""
    kit.reset()
    m = Mats()
    rng = rng_for("brazier")
    iron = m.iron
    bowl = kit.lathe(
        "Bowl",
        [(0, 0.78), (0.18, 0.79), (0.36, 0.86), (0.46, 0.98), (0.48, 1.04), (0.43, 1.04), (0.41, 0.98), (0.0, 0.92)],
        material=iron,
        segs=20,
    )
    wrap(bowl, 1.0)
    kit.ring("Rim", 0.46, 0.025, (0, 0, 1.04), iron, segs=20, minor_segs=5)
    for i in range(8):
        a = i * math.tau / 8
        kit.cyl("Spike%s" % i, 0.025, 0.14, (0.46 * math.cos(a), 0.46 * math.sin(a), 1.11), iron, verts=5, r2=0.0)
    for i in range(3):
        a = i * math.tau / 3
        p0 = Vector((0.12 * math.cos(a), 0.12 * math.sin(a), 0.8))
        p1 = Vector((0.3 * math.cos(a), 0.3 * math.sin(a), 0.45))
        p2 = Vector((0.42 * math.cos(a), 0.42 * math.sin(a), 0.04))
        rod("Leg%s_a" % i, p0, p1, 0.03, iron, verts=6)
        rod("Leg%s_b" % i, p1, p2, 0.03, iron, verts=6)
        kit.ball("Foot%s" % i, 0.05, tuple(p2), iron, segs=6, rings=4, scale=(1.4, 1.4, 0.6))
    kit.ring("Brace", 0.29, 0.015, (0, 0, 0.45), iron, segs=16, minor_segs=4)
    blob("Coals", 0.42, 0.16, (0, 0, 0.92), m.coal, 2, segs=14, wobble=0.15)
    for i in range(16):
        a = rng.random() * math.tau
        d = rng.random() * 0.32
        lump = rock("Ember%s" % i, 0.09 + rng.random() * 0.07, (d * math.cos(a), d * math.sin(a), 1.04 + 0.08 * (1 - d / 0.32)), m.ember if i % 3 else m.coal, rng)
        lump.scale = (1.0, 1.0, 1.4)
    return finish("brazier")


def build_alchemy_bench():
    """A heavy oak workbench crowded with glassware, a burner, a mortar and
    pestle, books, a scroll, and a skull with a candle on it."""
    kit.reset()
    m = Mats()
    wood = m.wood
    top_z = 0.86
    top = kit.box("Top", (1.8, 0.76, 0.07), (0, 0, top_z - 0.035), wood, bevel=0.01)
    uv_box(top, 1.2)
    for x in (-0.82, 0.82):
        for y in (-0.3, 0.3):
            leg = kit.box("Leg_%s_%s" % (x, y), (0.09, 0.09, top_z - 0.07), (x, y, (top_z - 0.07) / 2), wood)
            uv_box(leg, 1.2, swap=True)
    for y in (-0.3, 0.3):
        s = kit.box("Rail_%s" % y, (1.6, 0.05, 0.08), (0, y, 0.2), wood)
        uv_box(s, 1.2)
    shelf = kit.box("Shelf", (1.66, 0.62, 0.04), (0, 0, 0.26), m.wood_dark)
    uv_box(shelf, 1.2)
    # Under the bench: clay pots and a bottle.
    for i, (x, r, h) in enumerate(((-0.55, 0.12, 0.26), (-0.25, 0.1, 0.2), (0.45, 0.13, 0.3))):
        pot = kit.lathe("Pot%s" % i, [(0, 0), (r * 0.7, 0), (r, h * 0.4), (r * 0.8, h * 0.85), (r * 0.5, h), (0, h * 0.98)], (x, 0.05, 0.28), m.clay, segs=10)
        wrap(pot, 0.5)
    bottle("UnderBottle", (0.1, -0.1, 0.28), m, 0.05, 0.24, m.potion("brown", (0.2, 0.1, 0.05)))
    z = top_z
    # A retort on an iron ring stand over a brass burner.
    kit.cyl("Burner", 0.06, 0.06, (-0.6, 0.05, z + 0.03), m.brass, verts=10)
    flame("BurnerFlame", (-0.6, 0.05, z + 0.06), m, size=0.8)
    kit.ring("StandRing", 0.09, 0.008, (-0.6, 0.05, z + 0.2), m.iron, segs=10, minor_segs=3)
    for i in range(3):
        a = i * math.tau / 3
        rod("StandLeg%s" % i, (-0.6 + 0.09 * math.cos(a), 0.05 + 0.09 * math.sin(a), z + 0.2), (-0.6 + 0.12 * math.cos(a), 0.05 + 0.12 * math.sin(a), z), 0.006, m.iron, verts=4)
    kit.ball("Retort", 0.1, (-0.6, 0.05, z + 0.29), m.glass, segs=12, rings=8)
    kit.ball("RetortLiquid", 0.085, (-0.6, 0.05, z + 0.27), m.potion("violet", (0.45, 0.1, 0.6), 40.0), segs=8, rings=5, scale=(1, 1, 0.75))
    rod("RetortNeck", (-0.55, 0.05, z + 0.37), (-0.3, 0.05, z + 0.24), 0.018, m.glass, verts=6, r2=0.01)
    flask("Receiver", (-0.27, 0.05, z), m, 0.055, 0.06, m.potion("violet_drop", (0.45, 0.1, 0.6), 30.0))
    # Flasks and bottles.
    flask("FlaskGreen", (-0.05, -0.15, z), m, 0.07, 0.1, m.potion("green", (0.15, 0.8, 0.25), 70.0))
    flask("FlaskRed", (0.1, 0.18, z), m, 0.06, 0.12, m.potion("red", (0.7, 0.06, 0.05), 25.0), kind="cone")
    bottle("BottleA", (0.2, -0.22, z), m, 0.035, 0.16, m.potion("amber_glass", (0.45, 0.25, 0.05)))
    bottle("BottleB", (0.27, -0.12, z), m, 0.03, 0.2, m.potion("blue_glass", (0.08, 0.18, 0.4)))
    # A test-tube rack.
    rack = kit.box("Rack", (0.26, 0.07, 0.03), (0.0, 0.25, z + 0.06), m.wood_dark)
    uv_box(rack, 0.5)
    kit.box("RackFoot", (0.26, 0.07, 0.02), (0.0, 0.25, z + 0.01), m.wood_dark)
    for i in range(5):
        x = -0.1 + i * 0.05
        kit.cyl("Tube%s" % i, 0.011, 0.14, (x, 0.25, z + 0.07), m.glass, verts=6)
        colors = [(0.2, 0.7, 0.3), (0.7, 0.1, 0.1), (0.8, 0.5, 0.1), (0.3, 0.2, 0.7), (0.6, 0.6, 0.2)]
        kit.cyl("TubeLiquid%s" % i, 0.009, 0.06, (x, 0.25, z + 0.03 + 0.012 * i), m.potion("tube%s" % i, colors[i], 20.0), verts=6)
    # Mortar and pestle.
    mortar = kit.lathe("Mortar", [(0, 0), (0.07, 0), (0.09, 0.05), (0.085, 0.09), (0.065, 0.09), (0.06, 0.04), (0, 0.03)], (0.45, -0.12, z), m.stone_trim, segs=12)
    wrap(mortar, 0.3)
    rod("Pestle", (0.45, -0.12, z + 0.04), (0.53, -0.08, z + 0.2), 0.016, m.stone_trim, verts=6, r2=0.022)
    # Books and an unrolled scroll.
    covers = [m.leather_red, m.leather_brown, m.leather_green]
    for i in range(3):
        book("Book%s" % i, (0.72, 0.15, z + 0.03 + i * 0.06), (0.24, 0.17, 0.055), covers[i], m, yaw=0.2 * (i - 1))
    sheet = kit.box("Scroll", (0.32, 0.22, 0.004), (0.4, 0.18, z + 0.002), m.page, rot=(0, 0, 0.15))
    uv_box(sheet, 0.32)
    for k, dy in enumerate((-0.12, 0.12)):
        kit.cyl("ScrollRoll%s" % k, 0.018, 0.34, (0.4 - dy * 0.15, 0.18 + dy, z + 0.018), m.parchment, verts=8, rot=(0, math.pi / 2, 0.15))
    # A skull with a candle burned down on its crown.
    skull("Skull", (0.72, -0.18, z), m, 1.0, yaw=0.5)
    blob("SkullWax", 0.05, 0.02, (0.72, -0.175, z + 0.18), m.wax_old, 3, segs=8)
    candle("SkullCandle", 0.72, -0.175, z + 0.19, 0.08, 0.022, m, 1, wax=m.wax_old, drips=3)
    candle("BenchCandle", -0.82, -0.28, z, 0.16, 0.025, m, 2, drips=2)
    blob("BenchWax", 0.05, 0.008, (-0.82, -0.28, z), m.wax, 4, segs=8)
    return finish("alchemy_bench")


def jar(prefix, loc, r, h, m, contents, label=True, lid=None, seed=0, glow=0.0):
    """A specimen jar: glass, murky liquid, a lid, a label, and whatever
    `contents(prefix, (x, y, z), r, h)` puts inside."""
    x, y, z = loc
    lid = lid or m.brass
    kit.cyl(prefix + "_base", r * 1.05, h * 0.05, (x, y, z + h * 0.025), lid, verts=12)
    kit.cyl(prefix + "_glass", r, h * 0.82, (x, y, z + h * 0.05 + h * 0.41), m.glass, verts=14)
    kit.cyl(prefix + "_shoulder", r, h * 0.06, (x, y, z + h * 0.9), m.glass, verts=14, r2=r * 0.7)
    kit.cyl(prefix + "_lid", r * 0.78, h * 0.06, (x, y, z + h * 0.96), lid, verts=12)
    kit.ball(prefix + "_knob", r * 0.16, (x, y, z + h * 1.0), lid, segs=6, rings=4)
    murk = kit.mat(
        "Murk_%s" % prefix,
        (0.24, 0.27, 0.12) if not glow else (0.16, 0.32, 0.16),
        0.2,
        emit=(0.16, 0.4, 0.2) if glow else None,
        strength=glow,
    )
    fade(murk, 0.62)
    kit.cyl(prefix + "_liquid", r * 0.93, h * 0.7, (x, y, z + h * 0.06 + h * 0.35), murk, verts=12)
    if label:
        cyl_patch(prefix + "_label", r * 1.01, -math.pi / 2 - 0.55, -math.pi / 2 + 0.55, h * 0.35, h * 0.6, (x, y, z), m.label)
    contents(prefix, (x, y, z), r, h)


def creature(prefix, loc, r, h, m):
    """A pale curled specimen: a head and a tapering spine."""
    flesh = kit.mat("Specimen_Flesh", (0.66, 0.6, 0.46), 0.6)
    x, y, z = loc
    for i in range(9):
        t = i / 8
        a = t * 4.2
        rr = r * 0.42 * (1 - t * 0.55)
        kit.ball("%s_seg%s" % (prefix, i), r * (0.28 - 0.18 * t), (x + rr * math.cos(a), y + rr * math.sin(a) * 0.5, z + h * (0.42 - 0.18 * t + 0.08 * math.sin(a))), flesh, segs=8, rings=5)
    kit.ball(prefix + "_head", r * 0.36, (x + r * 0.4, y, z + h * 0.48), flesh, segs=10, rings=6)
    kit.ball(prefix + "_eye", r * 0.1, (x + r * 0.55, y - r * 0.28, z + h * 0.5), m.bone_dark, segs=6, rings=4)


def remains(prefix, loc, r, h, m):
    x, y, z = loc
    skull(prefix + "_skull", (x, y, z + h * 0.08), m, r / 0.14, yaw=0.4)
    for i in range(3):
        a = i * 1.7
        bone(prefix + "_bone%s" % i, (x + r * 0.5 * math.cos(a), y + r * 0.5 * math.sin(a), z + h * 0.08), (x - r * 0.4 * math.cos(a), y - r * 0.4 * math.sin(a), z + h * 0.45), r * 0.07, m)


def build_specimen_jar(name, bones):
    kit.reset()
    m = Mats()
    jar("Jar", (0, 0, 0), 0.2, 0.62, m, (lambda p, l, r, h: remains(p, l, r, h, m)) if bones else (lambda p, l, r, h: creature(p, l, r, h, m)), glow=0.0 if bones else 3.0)
    return finish(name)


def build_jar_shelf():
    """A storage shelf of jars, bottles, pots, a skull, and a candle."""
    kit.reset()
    m = Mats()
    rng = rng_for("jar_shelf")
    wood = m.wood_dark
    w, d, h = 1.5, 0.38, 1.9
    for sx in (-1, 1):
        side = kit.box("Side%s" % sx, (0.05, d, h), (sx * (w / 2 - 0.025), 0, h / 2), wood)
        uv_box(side, 1.2, swap=True)
    back = kit.box("Back", (w, 0.02, h), (0, d / 2 - 0.01, h / 2), wood)
    uv_box(back, 1.2, swap=True)
    levels = (0.08, 0.55, 1.0, 1.45, 1.88)
    for k, z in enumerate(levels):
        board = kit.box("Board%s" % k, (w - 0.1, d - 0.02, 0.04), (0, 0, z - 0.02), wood)
        uv_box(board, 1.2)
    kit.box("Cornice", (w + 0.08, d + 0.06, 0.06), (0, 0, h + 0.03), wood)
    colors = [((0.3, 0.45, 0.15), 0.0), ((0.5, 0.12, 0.08), 0.0), ((0.2, 0.6, 0.35), 9.0), ((0.55, 0.4, 0.12), 0.0), ((0.25, 0.2, 0.5), 6.0)]
    for k, z in enumerate(levels[:-1]):
        x = -w / 2 + 0.12
        i = 0
        while x < w / 2 - 0.12:
            kind = int(rng.random() * 5)
            if k == 2 and i == 2:
                skull("Skull%s" % k, (x + 0.06, -0.02, z), m, 0.9, yaw=0.3)
                x += 0.2
            elif kind <= 1:
                r = 0.05 + rng.random() * 0.03
                hh = 0.16 + rng.random() * 0.12
                rgb, glow = colors[int(rng.random() * len(colors))]
                kit.cyl("Jar%s_%s" % (k, i), r, hh, (x + r, 0, z + hh / 2), m.glass, verts=10)
                kit.cyl("JarFill%s_%s" % (k, i), r * 0.9, hh * 0.7, (x + r, 0, z + hh * 0.36), m.potion("shelf%s" % int(rgb[0] * 100), rgb, glow), verts=8)
                kit.cyl("JarLid%s_%s" % (k, i), r * 0.95, 0.025, (x + r, 0, z + hh + 0.012), m.brass if kind else m.wood, verts=8)
                cyl_patch("JarLabel%s_%s" % (k, i), r * 1.01, -math.pi / 2 - 0.5, -math.pi / 2 + 0.5, hh * 0.3, hh * 0.6, (x + r, 0, z), m.label, segs=4)
                x += r * 2 + 0.03
            elif kind == 2:
                r = 0.03 + rng.random() * 0.02
                rgb, glow = colors[int(rng.random() * len(colors))]
                bottle("Bottle%s_%s" % (k, i), (x + r, -0.03, z), m, r, 0.16 + rng.random() * 0.1, m.potion("bottle%s" % int(rgb[1] * 100), rgb, glow * 0.5))
                x += r * 2 + 0.03
            elif kind == 3:
                r = 0.07 + rng.random() * 0.03
                hh = 0.14 + rng.random() * 0.08
                pot = kit.lathe("Pot%s_%s" % (k, i), [(0, 0), (r * 0.7, 0), (r, hh * 0.45), (r * 0.7, hh * 0.9), (r * 0.5, hh), (0, hh * 0.97)], (x + r, 0, z), m.clay, segs=10)
                wrap(pot, 0.4)
                x += r * 2 + 0.03
            else:
                for b in range(2):
                    book("Book%s_%s_%s" % (k, i, b), (x + 0.12, 0, z + 0.025 + b * 0.05), (0.2, 0.15, 0.045), [m.leather_brown, m.leather_black][b], m, yaw=0.15 * b)
                x += 0.27
            i += 1
    candle("Candle", w / 2 - 0.2, -0.08, levels[-1] + 0.0, 0.12, 0.025, m, 3, drips=3)
    blob("CandleWax", 0.05, 0.01, (w / 2 - 0.2, -0.08, levels[-1]), m.wax, 5, segs=8)
    return finish("jar_shelf")


# --- Dissection ---------------------------------------------------------------------


def build_slab_table():
    """A stone dissection slab on stone trestles: a shrouded body, a bare
    arm, straps, stains, a tray of instruments, and a bucket beneath."""
    kit.reset()
    m = Mats()
    stone = m.stone_trim
    top_z = 0.92
    slab = kit.box("Slab", (2.2, 0.95, 0.14), (0, 0, top_z - 0.07), stone, bevel=0.02)
    uv_box(slab, 1.0)
    for sy in (-1, 1):
        lip = kit.box("Lip%s" % sy, (2.2, 0.05, 0.04), (0, sy * 0.45, top_z + 0.02), stone)
        uv_box(lip, 1.0)
    for x in (-0.75, 0.75):
        leg = kit.box("Trestle%s" % x, (0.3, 0.75, top_z - 0.14), (x, 0, (top_z - 0.14) / 2), stone)
        uv_box(leg, 1.0)
        foot = kit.box("TrestleFoot%s" % x, (0.42, 0.85, 0.1), (x, 0, 0.05), stone)
        uv_box(foot, 1.0)
    z = top_z + 0.0
    cloth = m.cloth
    flesh = kit.mat("Body_Flesh", (0.46, 0.44, 0.38), 0.65)
    # The body under a draped shroud, head toward -X: a sheet laid over
    # ellipsoids for the head, chest, belly, hips, legs, and feet, hanging
    # over the slab's edges.
    body = [(-0.86, 0.0, 0.0, 0.12, 0.1, 0.16), (-0.46, 0.0, 0.0, 0.3, 0.22, 0.19), (-0.12, 0.0, 0.0, 0.24, 0.18, 0.12),
            (0.14, 0.0, 0.0, 0.16, 0.2, 0.11), (0.45, 0.1, 0.0, 0.3, 0.08, 0.085), (0.45, -0.1, 0.0, 0.3, 0.08, 0.085),
            (0.76, 0.1, 0.0, 0.18, 0.065, 0.07), (0.76, -0.1, 0.0, 0.18, 0.065, 0.07), (0.92, 0.1, 0.0, 0.05, 0.06, 0.13), (0.92, -0.1, 0.0, 0.05, 0.06, 0.13)]

    def rise(x, y):
        best = 0.0
        for cx, cy, _cz, rx, ry, rz in body:
            q = 1 - ((x - cx) / rx) ** 2 - ((y - cy) / ry) ** 2
            if q > 0:
                best = max(best, rz * math.sqrt(q))
        return best

    nx, ny = 34, 16
    hx, hy = 1.2, 0.72
    grid = []
    for j in range(ny + 1):
        row = []
        for i in range(nx + 1):
            x = -hx + 2 * hx * i / nx
            y = -hy + 2 * hy * j / ny
            over = max(abs(x) - 1.05, abs(y) - 0.47, 0.0)
            zz = z + 0.025 + rise(x, y) * 1.05 if over <= 0 else z + 0.02 - over * 1.6
            row.append([x, y, zz])
        grid.append(row)
    for _ in range(2):
        smooth = [[list(p) for p in row] for row in grid]
        for j in range(1, ny):
            for i in range(1, nx):
                smooth[j][i][2] = max(grid[j][i][2], (grid[j][i][2] * 2 + grid[j][i - 1][2] + grid[j][i + 1][2] + grid[j - 1][i][2] + grid[j + 1][i][2]) / 6)
        grid = smooth
    verts = [tuple(p) for row in grid for p in row]
    faces = [(j * (nx + 1) + i, j * (nx + 1) + i + 1, (j + 1) * (nx + 1) + i + 1, (j + 1) * (nx + 1) + i) for j in range(ny) for i in range(nx)]
    shroud = mesh_obj("Shroud", verts, faces, cloth)
    for poly in shroud.data.polygons:
        if poly.normal.z < 0:
            poly.flip()
    # One bare arm hangs off the near side.
    rod("ArmUpper", (-0.52, -0.3, z + 0.08), (-0.42, -0.53, z + 0.0), 0.04, flesh, verts=8)
    kit.ball("Elbow", 0.038, (-0.42, -0.53, z + 0.0), flesh, segs=8, rings=5)
    rod("ArmLower", (-0.42, -0.53, z + 0.0), (-0.38, -0.56, z - 0.28), 0.032, flesh, verts=8, r2=0.026)
    kit.ball("Hand", 0.045, (-0.38, -0.56, z - 0.33), flesh, segs=8, rings=5, scale=(0.6, 1, 1.5))
    for f in range(4):
        rod("Finger%s" % f, (-0.38 + (f - 1.5) * 0.012, -0.56, z - 0.36), (-0.38 + (f - 1.5) * 0.016, -0.565, z - 0.43), 0.007, flesh, verts=4)
    for x in (-0.3, 0.42):
        strap = [(x, -0.5, z + 0.0)] + [(x, yy, z + 0.035 + rise(x, yy) * 1.05) for yy in (-0.3, -0.15, 0.0, 0.15, 0.3)] + [(x, 0.5, z + 0.0)]
        for k, (a, b) in enumerate(zip(strap, strap[1:])):
            kit.box("Strap%s_%s" % (x, k), (0.06, (Vector(b) - Vector(a)).length + 0.01, 0.012), (0, 0, 0), m.leather_black)
            align_box = bpy.context.object
            mid = (Vector(a) + Vector(b)) / 2
            dirv = (Vector(b) - Vector(a)).normalized()
            align_box.matrix_world = Matrix.Translation(mid) @ dirv.to_track_quat("Y", "Z").to_matrix().to_4x4()
    # Stains on the slab and the floor.
    blob("StainSlab", 0.16, 0.006, (-0.95, 0.38, z), m.blood, 1, segs=10, wobble=0.45)
    blob("StainFloor", 0.32, 0.006, (0.1, -0.6, 0.0), m.blood, 2, segs=12, wobble=0.5)
    blob("StainDrip", 0.12, 0.005, (-0.3, -0.55, 0.0), m.blood, 3, segs=8, wobble=0.5)
    # A tray of instruments at the foot.
    tray = kit.box("Tray", (0.42, 0.26, 0.015), (0.85, -0.22, z + 0.008), m.iron)
    for k in range(4):
        kit.box("TrayRim%s" % k, (0.42 if k < 2 else 0.015, 0.015 if k < 2 else 0.26, 0.03), (0.85 + (0 if k < 2 else (k * 2 - 5) * 0.21), -0.22 + (0 if k >= 2 else (k * 2 - 1) * 0.13), z + 0.02), m.iron)
    for k in range(4):
        x = 0.7 + k * 0.08
        kit.box("Blade%s" % k, (0.012, 0.12, 0.003), (x, -0.25, z + 0.018), m.iron, rot=(0, 0, 0.2))
        kit.box("Handle%s" % k, (0.018, 0.08, 0.016), (x - 0.02, -0.15, z + 0.022), m.wood_dark, rot=(0, 0, 0.2))
    kit.box("Saw", (0.18, 0.05, 0.003), (1.1, -0.42, z + 0.002), m.iron, rot=(0, 0, 0.5))
    # A bucket beneath.
    bucket = kit.lathe("Bucket", [(0, 0), (0.16, 0), (0.19, 0.32), (0.17, 0.32), (0.145, 0.04), (0, 0.04)], (0.0, 0.1, 0.0), m.wood, segs=14)
    wrap(bucket, 0.6, swap=True)
    for zz in (0.06, 0.26):
        kit.ring("BucketHoop%s" % zz, 0.165 + zz * 0.08, 0.01, (0.0, 0.1, zz), m.iron, segs=14, minor_segs=4)
    kit.cyl("BucketBlood", 0.17, 0.01, (0.0, 0.1, 0.27), m.blood, verts=12)
    return finish("slab_table")


def build_chained_skeleton():
    """A skeleton slumped against the wall, its wrists shackled to rings
    above it and one ankle chained to the floor. The wall face is the
    Blender plane y = 0; the origin is on the floor below it."""
    kit.reset()
    m = Mats()
    iron = m.iron
    wy = 0.0
    for sx in (-1, 1):
        kit.box("Plate%s" % sx, (0.16, 0.03, 0.2), (sx * 0.5, wy - 0.015, 1.7), iron)
        kit.ring("Ring%s" % sx, 0.06, 0.014, (sx * 0.5, wy - 0.05, 1.64), iron, segs=10, minor_segs=4, rot=(math.pi / 2, 0, 0))
    pelvis = Vector((0.0, -0.22, 0.14))
    # Spine and ribcage, leaning against the wall.
    neck = Vector((0.0, -0.12, 0.72))
    for i in range(9):
        p = pelvis.lerp(neck, i / 8)
        kit.ball("Vert%s" % i, 0.028, tuple(p), m.bone, segs=6, rings=4, scale=(1.2, 1.0, 0.8))
    for i in range(6):
        p = pelvis.lerp(neck, 0.45 + i * 0.09)
        rib = kit.ring("Rib%s" % i, 0.13 - abs(i - 2.5) * 0.012, 0.009, tuple(p + Vector((0, -0.08, 0))), m.bone, segs=12, minor_segs=3, rot=(0.25, 0, 0))
        rib.scale = (1.0, 0.75, 1.0)
    kit.ball("Sternum", 0.02, tuple(neck.lerp(pelvis, 0.3) + Vector((0, -0.18, 0))), m.bone, segs=6, rings=4, scale=(1, 0.6, 3.5))
    for sx in (-1, 1):
        kit.ball("Hip%s" % sx, 0.08, (sx * 0.08, pelvis.y - 0.02, pelvis.z + 0.02), m.bone, segs=8, rings=5, scale=(1.0, 0.5, 0.9))
    skull("Skull", tuple(neck + Vector((0.02, -0.06, 0.02))), m, 1.0, yaw=0.25, tilt=-0.5)
    # Arms raised to the shackles.
    for sx in (-1, 1):
        shoulder = neck + Vector((sx * 0.17, -0.02, -0.04))
        wrist = Vector((sx * 0.46, -0.1, 1.32))
        elbow = shoulder.lerp(wrist, 0.5) + Vector((sx * 0.12, -0.06, -0.08))
        bone("Humerus%s" % sx, shoulder, elbow, 0.018, m)
        bone("Forearm%s" % sx, elbow, wrist, 0.015, m)
        for f in range(4):
            rod("Finger%s_%s" % (sx, f), tuple(wrist + Vector((0, 0, 0.02))), tuple(wrist + Vector((sx * 0.02 * f - sx * 0.03, -0.02, 0.09))), 0.006, m.bone, verts=4)
        kit.ring("Cuff%s" % sx, 0.045, 0.014, tuple(wrist + Vector((0, 0, 0.03))), iron, segs=10, minor_segs=4)
        chain("Chain%s" % sx, sag((sx * 0.5, wy - 0.06, 1.58), tuple(wrist + Vector((0, 0, 0.07))), 0.05, 4), iron)
    # Legs: thighs forward along the floor, shins down, feet.
    for sx in (-1, 1):
        hip = Vector((sx * 0.1, pelvis.y - 0.04, pelvis.z - 0.02))
        knee = Vector((sx * 0.2, -0.62, 0.24))
        ankle = Vector((sx * 0.24, -0.86, 0.05))
        bone("Femur%s" % sx, hip, knee, 0.022, m)
        bone("Tibia%s" % sx, knee, ankle, 0.018, m)
        kit.ball("Foot%s" % sx, 0.04, tuple(ankle + Vector((sx * 0.02, -0.07, -0.02))), m.bone, segs=6, rings=4, scale=(0.8, 2.0, 0.5))
    kit.ring("AnkleCuff", 0.05, 0.014, (0.24, -0.86, 0.07), iron, segs=10, minor_segs=4, rot=(0, math.pi / 2, 0))
    kit.ring("FloorRing", 0.06, 0.014, (0.55, -0.4, 0.015), iron, segs=10, minor_segs=4)
    kit.box("FloorPlate", (0.12, 0.12, 0.02), (0.55, -0.4, 0.01), iron)
    chain("FloorChain", [(0.55, -0.4, 0.02), (0.45, -0.6, 0.02), (0.3, -0.82, 0.05)], iron)
    obj = finish_wall("chained_skeleton")
    return obj


def finish_wall(name):
    """Finish a wall-hung prop: its origin stays on the floor at the wall."""
    return finish(name, ground=False, blockers=("__none__",))


def build_hanging_chains():
    """An iron bracket from the wall, with three chains: a hook, a
    manacle, and a broken end. The wall face is the Blender plane y = 0."""
    kit.reset()
    m = Mats()
    iron = m.iron
    kit.box("Plate", (0.22, 0.03, 0.3), (0, -0.015, 3.0), iron)
    rod("Arm", (0, -0.03, 3.05), (0, -0.6, 3.05), 0.025, iron, verts=6)
    rod("Strut", (0, -0.03, 2.75), (0, -0.45, 3.04), 0.018, iron, verts=6)
    rod("Bar", (-0.25, -0.6, 3.04), (0.25, -0.6, 3.04), 0.02, iron, verts=6)
    ends = ((-0.22, 1.35), (0.0, 1.7), (0.22, 2.05))
    for k, (x, bottom) in enumerate(ends):
        chain("Chain%s" % k, [(x, -0.6, 3.0), (x, -0.6, bottom)], iron)
        if k == 0:
            pts = [(x, -0.6, bottom), (x, -0.6, bottom - 0.1), (x + 0.05, -0.6, bottom - 0.16), (x + 0.09, -0.6, bottom - 0.1)]
            for i, (a, b) in enumerate(zip(pts, pts[1:])):
                rod("Hook%s" % i, a, b, 0.012, iron, verts=6)
        elif k == 1:
            kit.ring("Manacle", 0.055, 0.014, (x, -0.6, bottom - 0.07), iron, segs=10, minor_segs=4, rot=(0, math.pi / 2, 0))
    return finish_wall("hanging_chains")


def build_iron_cage():
    """A standing iron cage with a domed top and a hanging ring, bones on
    its floor."""
    kit.reset()
    m = Mats()
    iron = m.iron
    r = 0.55
    floor = kit.cyl("Floor", r + 0.02, 0.05, (0, 0, 0.025), iron, verts=16)
    kit.ring("BaseRing", r, 0.025, (0, 0, 0.06), iron, segs=20, minor_segs=4)
    kit.ring("MidRing", r, 0.02, (0, 0, 0.95), iron, segs=20, minor_segs=4)
    kit.ring("TopRing", r, 0.025, (0, 0, 1.75), iron, segs=20, minor_segs=4)
    for i in range(16):
        a = i * math.tau / 16
        kit.cyl("Bar%s" % i, 0.014, 1.7, (r * math.cos(a), r * math.sin(a), 0.9), iron, verts=5)
    for i in range(4):
        a = i * math.pi / 4
        pts = [(r * math.cos(t) * math.cos(a), r * math.cos(t) * math.sin(a), 1.75 + 0.4 * math.sin(t)) for t in [math.pi * k / 10 for k in range(11)]]
        for k, (p, q) in enumerate(zip(pts, pts[1:])):
            rod("Dome%s_%s" % (i, k), p, q, 0.016, iron, verts=5)
    kit.ring("Hang", 0.08, 0.018, (0, 0, 2.22), iron, segs=10, minor_segs=4, rot=(math.pi / 2, 0, 0))
    for zz in (0.35, 1.45):
        kit.box("Hinge%s" % zz, (0.05, 0.03, 0.08), (r * math.cos(-1.2), r * math.sin(-1.2), zz), iron)
    kit.box("Lock", (0.07, 0.04, 0.1), (r * math.cos(-1.95), r * math.sin(-1.95), 0.95), iron)
    skull("Skull", (0.12, 0.1, 0.05), m, 1.0, yaw=2.4)
    bone("BoneA", (-0.25, -0.1, 0.07), (0.15, -0.25, 0.07), 0.02, m)
    bone("BoneB", (-0.3, 0.15, 0.07), (-0.05, 0.35, 0.08), 0.018, m)
    return finish("iron_cage")


# --- Candles --------------------------------------------------------------------------


def build_candelabrum_tall():
    """A wrought-iron floor candelabrum: tripod feet, a turned stem, four
    curling arms and a crown candle, wax running off every cup."""
    kit.reset()
    m = Mats()
    rng = rng_for("candelabrum_tall")
    iron = m.iron
    stem = kit.lathe(
        "Stem",
        [(0, 0.12), (0.05, 0.12), (0.07, 0.18), (0.035, 0.26), (0.06, 0.33), (0.028, 0.4), (0.024, 1.1), (0.055, 1.16), (0.024, 1.22),
         (0.024, 1.46), (0.07, 1.5), (0.0, 1.52)],
        material=iron,
        segs=10,
    )
    wrap(stem, 0.5)
    for i in range(3):
        a = i * math.tau / 3 + 0.3
        p0 = Vector((0.03 * math.cos(a), 0.03 * math.sin(a), 0.2))
        p1 = Vector((0.2 * math.cos(a), 0.2 * math.sin(a), 0.1))
        p2 = Vector((0.3 * math.cos(a), 0.3 * math.sin(a), 0.03))
        rod("Foot%s_a" % i, p0, p1, 0.018, iron, verts=6)
        rod("Foot%s_b" % i, p1, p2, 0.016, iron, verts=6)
        kit.ball("Toe%s" % i, 0.03, tuple(p2), iron, segs=6, rings=4, scale=(1.3, 1.3, 0.8))
    tips = [(0.0, 0.0, 1.52)]
    for i in range(4):
        a = i * math.tau / 4 + 0.785
        c, s = math.cos(a), math.sin(a)
        pts = [Vector((0.02 * c, 0.02 * s, 1.36)), Vector((0.14 * c, 0.14 * s, 1.31)), Vector((0.25 * c, 0.25 * s, 1.36)), Vector((0.3 * c, 0.3 * s, 1.47)), Vector((0.3 * c, 0.3 * s, 1.52))]
        for k, (p, q) in enumerate(zip(pts, pts[1:])):
            rod("Arm%s_%s" % (i, k), p, q, 0.014, iron, verts=6)
        tips.append((0.3 * c, 0.3 * s, 1.52))
    for i, (x, y, z) in enumerate(tips):
        kit.cyl("Cup%s" % i, 0.032, 0.035, (x, y, z + 0.0175), iron, verts=10)
        kit.cyl("Pan%s" % i, 0.06, 0.008, (x, y, z), iron, verts=10)
        h = 0.14 + rng.random() * 0.14 + (0.06 if i == 0 else 0.0)
        blob("PanWax%s" % i, 0.05, 0.012, (x, y, z + 0.004), m.wax_old, i, segs=8)
        candle("Candle%s" % i, x, y, z + 0.03, h, 0.024, m, i, drips=3)
        for k in range(2):
            a = rng.random() * math.tau
            length = 0.03 + rng.random() * 0.05
            kit.cyl("Stalactite%s_%s" % (i, k), 0.008, length, (x + 0.055 * math.cos(a), y + 0.055 * math.sin(a), z - length / 2), m.wax_old, verts=5, r2=0.002)
    blob("FloorWax", 0.12, 0.01, (0.06, -0.08, 0.0), m.wax_old, 9, segs=10, wobble=0.4)
    return finish("candelabrum_tall")


def build_candelabrum_short():
    """A brass dish of three candles burned to different heights."""
    kit.reset()
    m = Mats()
    dish = kit.lathe("Dish", [(0, 0), (0.12, 0), (0.15, 0.02), (0.15, 0.035), (0.13, 0.03), (0.0, 0.015)], material=m.brass, segs=14)
    wrap(dish, 0.4)
    kit.ring("Handle", 0.035, 0.008, (0.17, 0, 0.025), m.brass, segs=8, minor_segs=4, rot=(math.pi / 2, 0, 0))
    blob("DishWax", 0.11, 0.012, (0, 0, 0.016), m.wax_old, 1, segs=10)
    for i, (x, y, h, r) in enumerate(((0.0, 0.02, 0.26, 0.026), (0.065, -0.04, 0.17, 0.022), (-0.06, -0.035, 0.11, 0.024))):
        candle("Candle%s" % i, x, y, 0.025, h, r, m, i, drips=3)
    return finish("candelabrum_short")


def build_floor_candles():
    """A cluster of floor candles standing in their own melted wax."""
    kit.reset()
    m = Mats()
    rng = rng_for("floor_candles")
    blob("Pool", 0.34, 0.025, (0, 0, 0), m.wax_old, 1, segs=16, wobble=0.3)
    blob("PoolB", 0.18, 0.02, (0.22, 0.12, 0), m.wax_old, 2, segs=10, wobble=0.3)
    spots = [(0.0, 0.0, 0.42, 0.045), (0.13, 0.06, 0.28, 0.038), (-0.12, 0.09, 0.33, 0.04), (0.03, -0.15, 0.2, 0.035),
             (-0.08, -0.11, 0.38, 0.042), (0.17, -0.09, 0.14, 0.032), (-0.18, -0.02, 0.24, 0.036), (0.27, 0.15, 0.11, 0.03),
             (0.09, 0.19, 0.18, 0.03)]
    for i, (x, y, h, r) in enumerate(spots):
        candle("Candle%s" % i, x, y, 0.015, h, r, m, i, wax=m.wax if i % 3 else m.wax_old, drips=4, lit=i != 7)
    for i in range(3):
        a = rng.random() * math.tau
        rod("Fallen%s" % i, (0.38 * math.cos(a), 0.38 * math.sin(a), 0.02), (0.38 * math.cos(a) + 0.1 * math.cos(a + 1.3), 0.38 * math.sin(a) + 0.1 * math.sin(a + 1.3), 0.02), 0.018, m.wax_old, verts=6)
    return finish("floor_candles")


# --- Study ------------------------------------------------------------------------------


def build_ritual_rug():
    """A worn wool rug, lying a little rumpled, with fringed ends."""
    kit.reset()
    m = Mats()
    rug_mat = img_mat("Rug_Wool", to_image("T_Lab_Rug", tex_rug()), rough=0.95)
    w, l = 2.4, 3.6
    nx, ny = 6, 10
    verts = []
    for j in range(ny + 1):
        for i in range(nx + 1):
            x = -w / 2 + w * i / nx
            y = -l / 2 + l * j / ny
            z = 0.012 + 0.012 * math.sin(i * 1.7 + j * 0.9) * math.sin(j * 0.6) ** 2
            verts.append((x, y, z))
    faces = [(j * (nx + 1) + i, j * (nx + 1) + i + 1, (j + 1) * (nx + 1) + i + 1, (j + 1) * (nx + 1) + i) for j in range(ny) for i in range(nx)]
    rug = mesh_obj("Rug", verts, faces, rug_mat)
    layer = rug.data.uv_layers.new(name="UVMap")
    for poly in rug.data.polygons:
        for li in poly.loop_indices:
            vi = rug.data.loops[li].vertex_index
            layer.data[li].uv = ((vi % (nx + 1)) / nx, (vi // (nx + 1)) / ny)
    under = kit.box("Underside", (w, l, 0.01), (0, 0, 0.006), kit.mat("Rug_Back", (0.16, 0.05, 0.04), 0.95))
    fringe = kit.mat("Rug_Fringe", (0.62, 0.52, 0.34), 0.95)
    for sy in (-1, 1):
        for k in range(24):
            x = -w / 2 + 0.05 + k * (w - 0.1) / 23
            kit.box("Fringe%s_%s" % (sy, k), (0.025, 0.12, 0.006), (x, sy * (l / 2 + 0.05), 0.006), fringe, rot=(0, 0, 0.15 * math.sin(k * 2.3)))
    return finish("ritual_rug")


def build_bookshelf():
    """A tall oak bookcase, its shelves crowded with books, scrolls, a skull,
    and a candle stub."""
    kit.reset()
    m = Mats()
    rng = rng_for("bookshelf")
    wood = m.wood_dark
    w, d, h = 1.6, 0.42, 2.5
    for sx in (-1, 1):
        side = kit.box("Side%s" % sx, (0.06, d, h), (sx * (w / 2 - 0.03), 0, h / 2), wood)
        uv_box(side, 1.4, swap=True)
    back = kit.box("Back", (w, 0.025, h), (0, d / 2 - 0.0125, h / 2), wood)
    uv_box(back, 1.4, swap=True)
    kit.box("Plinth", (w + 0.04, d + 0.03, 0.1), (0, 0, 0.05), wood)
    kit.box("Cornice", (w + 0.1, d + 0.08, 0.08), (0, 0, h + 0.04), wood)
    kit.box("CorniceTop", (w + 0.16, d + 0.12, 0.04), (0, 0, h + 0.1), wood)
    levels = [0.1 + k * 0.48 for k in range(5)]
    covers = [m.leather_red, m.leather_brown, m.leather_green, m.leather_blue, m.leather_black]
    for k, z in enumerate(levels):
        board = kit.box("Board%s" % k, (w - 0.12, d - 0.03, 0.035), (0, -0.01, z + 0.0175), wood)
        uv_box(board, 1.4)
        z0 = z + 0.035
        x = -w / 2 + 0.07
        i = 0
        while x < w / 2 - 0.1:
            room = w / 2 - 0.07 - x
            pick = rng.random()
            if pick < 0.08 and room > 0.25 and k in (1, 3):
                skull("Skull%s_%s" % (k, i), (x + 0.1, -0.03, z0), m, 0.9, yaw=0.4 - rng.random() * 0.8)
                x += 0.22
            elif pick < 0.16 and room > 0.3:
                for s in range(3):
                    book("Flat%s_%s_%s" % (k, i, s), (x + 0.14, -0.02, z0 + 0.025 + s * 0.05), (0.26, 0.2, 0.045), covers[int(rng.random() * 5)], m, yaw=rng.random() * 0.3 - 0.15)
                x += 0.3
            elif pick < 0.22 and room > 0.2:
                for s in range(3):
                    kit.cyl("Scroll%s_%s_%s" % (k, i, s), 0.025, 0.3, (x + 0.08, -0.02, z0 + 0.025 + s * 0.045 + (0.02 if s == 2 else 0)), m.parchment, verts=8, rot=(math.pi / 2, 0, 0.1 * s))
                x += 0.18
            else:
                bw = 0.035 + rng.random() * 0.035
                bh = 0.22 + rng.random() * 0.14
                bd = 0.22 + rng.random() * 0.1
                lean = 0.0
                if rng.random() < 0.12:
                    lean = 0.25
                book("Book%s_%s" % (k, i), (x + bw / 2 + lean * bh * 0.5, -0.02, z0 + bh / 2 * math.cos(lean)), (bw, bd, bh), covers[int(rng.random() * 5)], m, roll=lean)
                if rng.random() < 0.5:
                    kit.box("Band%s_%s" % (k, i), (bw * 1.02, bd * 0.6, 0.012), (x + bw / 2, -0.02 - bd * 0.2, z0 + bh * 0.8), m.brass)
                x += bw + 0.004 + (lean * bh)
            i += 1
    candle("Stub", w / 2 - 0.22, -0.1, h + 0.12, 0.07, 0.028, m, 4, wax=m.wax_old, drips=3)
    blob("StubWax", 0.06, 0.012, (w / 2 - 0.22, -0.1, h + 0.12), m.wax_old, 6, segs=8)
    return finish("bookshelf")


def open_tome(prefix, loc, m, size=(0.5, 0.36), tilt=0.0, yaw=0.0):
    """An open book: a leather cover and two page blocks whose top pages
    rise from the spine and curl down at the fore-edge."""
    x, y, z = loc
    w, d = size
    half = w / 2
    parts = [kit.box(prefix + "_cover", (w + 0.03, d + 0.03, 0.01), (0, 0, 0.005), m.leather_red)]
    segs = 8

    def lift(u):
        # Page height above the cover from the spine (u = 0) to the edge.
        return 0.01 + 0.032 * math.sin(math.pi * min(1.0, 0.18 + u * 0.95)) + 0.012 * (1 - u)

    for sx in (-1, 1):
        verts = []
        for j, v in enumerate((-d / 2, d / 2)):
            for i in range(segs + 1):
                u = i / segs
                verts.append((sx * half * u, v, lift(u)))
        base = len(verts)
        for j, v in enumerate((-d / 2, d / 2)):
            for i in range(segs + 1):
                u = i / segs
                verts.append((sx * half * u, v, 0.01))
        n = segs + 1
        faces = [(i, i + 1, n + i + 1, n + i) for i in range(segs)]
        faces += [(base + i, base + i + 1, i + 1, i) for i in range(segs)]
        faces += [(n + i, n + i + 1, base + n + i + 1, base + n + i) for i in range(segs)]
        faces += [(segs, base + segs, base + n + segs, n + segs)]
        page = mesh_obj("%s_pages_%s" % (prefix, sx), verts, faces, m.page)
        layer = page.data.uv_layers.new(name="UVMap")
        for poly in page.data.polygons:
            for li in poly.loop_indices:
                co = page.data.vertices[page.data.loops[li].vertex_index].co
                layer.data[li].uv = (abs(co.x) / half, co.y / d + 0.5)
        kit.fix_normals(page)
        parts.append(page)
    parts.append(kit.box(prefix + "_ribbon", (0.012, 0.004, 0.14), (0.02, -d / 2 - 0.004, -0.05), kit.mat("Ribbon", (0.5, 0.05, 0.06), 0.6)))
    pivot = Matrix.Translation(Vector((x, y, z))) @ Matrix.Rotation(yaw, 4, "Z") @ Matrix.Rotation(tilt, 4, "X")
    return move(parts, pivot)


def build_lectern():
    """A carved oak lectern holding a great open tome, a candle on its arm."""
    kit.reset()
    m = Mats()
    wood = m.wood
    for i in range(2):
        foot = kit.box("Foot%s" % i, (0.62, 0.1, 0.08), (0, 0, 0.04), wood, rot=(0, 0, i * math.pi / 2 + math.pi / 4))
        uv_box(foot, 1.0)
    post = kit.lathe("Post", [(0, 0.08), (0.08, 0.08), (0.06, 0.16), (0.045, 0.2), (0.045, 0.9), (0.07, 0.95), (0.05, 1.0), (0.0, 1.0)], material=wood, segs=10)
    wrap(post, 1.0, swap=True)
    tilt = math.radians(24)
    desk = kit.box("Desk", (0.62, 0.46, 0.04), (0, 0, 0), wood, bevel=0.008)
    ledge = kit.box("Ledge", (0.62, 0.03, 0.05), (0, -0.23, 0.035), wood)
    block = kit.box("Block", (0.2, 0.2, 0.12), (0, 0.04, -0.07), wood)
    tome = open_tome("Tome", (0, 0.0, 0.02), m, (0.5, 0.36))
    pivot = Matrix.Translation(Vector((0, 0, 1.08))) @ Matrix.Rotation(tilt, 4, "X")
    move([desk, ledge, block] + tome, pivot)
    for o in (desk, ledge, block):
        uv_box(o, 1.0)
    rod("CandleArm", (0.04, 0.0, 0.92), (0.36, -0.05, 0.98), 0.012, m.iron, verts=5)
    kit.cyl("CandleCup", 0.03, 0.03, (0.36, -0.05, 0.995), m.iron, verts=8)
    blob("CupWax", 0.04, 0.008, (0.36, -0.05, 1.01), m.wax_old, 7, segs=8)
    candle("Candle", 0.36, -0.05, 1.01, 0.12, 0.022, m, 5, drips=3)
    return finish("lectern")


def build_writing_desk():
    """A scholar's desk: drawers, an open book, papers, ink and quill, a
    crystal on a brass stand, and a cluster of candles."""
    kit.reset()
    m = Mats()
    rng = rng_for("writing_desk")
    wood = m.wood
    top_z = 0.78
    top = kit.box("Top", (1.4, 0.72, 0.05), (0, 0, top_z - 0.025), wood, bevel=0.008)
    uv_box(top, 1.2)
    for sx in (-1, 1):
        ped = kit.box("Pedestal%s" % sx, (0.38, 0.62, top_z - 0.05), (sx * 0.48, 0, (top_z - 0.05) / 2), m.wood_dark)
        uv_box(ped, 1.2, swap=True)
        for k in range(3):
            zz = 0.14 + k * 0.22
            kit.box("Drawer%s_%s" % (sx, k), (0.32, 0.01, 0.17), (sx * 0.48, -0.315, zz), wood)
            kit.ball("Knob%s_%s" % (sx, k), 0.016, (sx * 0.48, -0.325, zz), m.brass, segs=6, rings=4)
    z = top_z
    open_tome("Book", (-0.05, -0.05, z), m, (0.42, 0.3), yaw=0.08)
    for k in range(5):
        paper = kit.box("Paper%s" % k, (0.2, 0.27, 0.002), (0.38 + rng.random() * 0.15, -0.05 + rng.random() * 0.2, z + 0.001 + k * 0.0015), m.page, rot=(0, 0, rng.random() * 1.2 - 0.6))
        uv_box(paper, 0.27)
    kit.cyl("Inkwell", 0.04, 0.06, (0.3, 0.22, z + 0.03), m.ink, verts=10)
    kit.cyl("InkNeck", 0.022, 0.02, (0.3, 0.22, z + 0.07), m.ink, verts=8)
    quill = rod("Quill", (0.3, 0.22, z + 0.05), (0.38, 0.32, z + 0.28), 0.004, kit.mat("Quill", (0.85, 0.82, 0.75), 0.7), verts=4)
    kit.ball("Vane", 0.05, (0.355, 0.29, z + 0.2), kit.mat("Feather", (0.8, 0.78, 0.72), 0.8), segs=6, rings=4, scale=(0.12, 0.35, 1.3), rot=(0.4, -0.3, 0.6))
    for i in range(2):
        book("Stack%s" % i, (-0.5, 0.18, z + 0.03 + i * 0.06), (0.26, 0.2, 0.055), [m.leather_blue, m.leather_brown][i], m, yaw=0.25 * i)
    kit.cyl("CrystalStand", 0.06, 0.04, (0.5, 0.25, z + 0.02), m.brass, verts=10)
    for i in range(3):
        a = i * math.tau / 3
        rod("CrystalClaw%s" % i, (0.5 + 0.045 * math.cos(a), 0.25 + 0.045 * math.sin(a), z + 0.04), (0.5 + 0.06 * math.cos(a), 0.25 + 0.06 * math.sin(a), z + 0.1), 0.006, m.brass, verts=4)
    kit.ball("Crystal", 0.075, (0.5, 0.25, z + 0.12), kit.mat("Crystal", (0.35, 0.55, 0.8), 0.05, emit=(0.3, 0.5, 0.9), strength=35.0), segs=12, rings=8)
    blob("CandleWax", 0.09, 0.012, (-0.55, -0.2, z), m.wax_old, 8, segs=10)
    for i, (x, y, h) in enumerate(((-0.56, -0.2, 0.2), (-0.5, -0.24, 0.13), (-0.6, -0.25, 0.09))):
        candle("Candle%s" % i, x, y, z + 0.008, h, 0.022, m, i + 10, drips=3)
    return finish("writing_desk")


# --- Bones and webs -------------------------------------------------------------------------


def build_bone_scatter():
    """A heap of skulls and bones on a mound of dust and fragments."""
    kit.reset()
    m = Mats()
    rng = rng_for("bone_scatter")
    dust = kit.mat("Bone_Dust", (0.2, 0.18, 0.15), 0.95)
    blob("Mound", 0.5, 0.1, (0, 0, 0), dust, 1, segs=14, wobble=0.35)
    for i in range(9):
        a = rng.random() * math.tau
        d = 0.15 + rng.random() * 0.35
        rock("Lump%s" % i, 0.12 + rng.random() * 0.1, (d * math.cos(a), d * math.sin(a), 0.0), dust, rng)

    def height(x, y):
        d = math.hypot(x, y) / 0.55
        return max(0.0, 0.12 * (1 - d * d))

    for i in range(4):
        a = i * 1.7 + rng.random()
        d = 0.15 + rng.random() * 0.3
        x, y = d * math.cos(a), d * math.sin(a)
        skull("Skull%s" % i, (x, y, height(x, y) - 0.015), m, 0.9 + rng.random() * 0.2, yaw=rng.random() * math.tau, tilt=rng.random() * 0.4 - 0.2)
    for i in range(10):
        a = rng.random() * math.tau
        d = rng.random() * 0.5
        x, y = d * math.cos(a), d * math.sin(a)
        dx, dy = math.cos(a + 1.5 + rng.random()), math.sin(a + 1.5 + rng.random())
        length = 0.25 + rng.random() * 0.2
        p = Vector((x - dx * length / 2, y - dy * length / 2, 0))
        q = Vector((x + dx * length / 2, y + dy * length / 2, 0))
        p.z = height(p.x, p.y) + 0.02
        q.z = height(q.x, q.y) + 0.02
        bone("Bone%s" % i, p, q, 0.018 + rng.random() * 0.006, m)
    for i in range(5):
        a = rng.random() * math.tau
        d = 0.2 + rng.random() * 0.35
        rib = kit.ring("Rib%s" % i, 0.12, 0.008, (d * math.cos(a), d * math.sin(a), height(d * math.cos(a), d * math.sin(a)) + 0.03), m.bone, segs=12, minor_segs=3, rot=(1.2, 0, a))
        rib.scale = (1.0, 0.6, 1.0)
    for i in range(14):
        a = rng.random() * math.tau
        d = rng.random() * 0.75
        x, y = d * math.cos(a), d * math.sin(a)
        kit.ball("Shard%s" % i, 0.018 + rng.random() * 0.02, (x, y, height(x, y) + 0.005), m.bone, segs=5, rings=3, scale=(1.6, 0.8, 0.6), rot=(0, 0, rng.random() * 3))
    return finish("bone_scatter")


def build_cobweb():
    """A web strung across a corner: radial threads, a spiral, a faint
    sheet, and loose strands. The corner's walls are the Blender planes
    x = 0 and y = 0, and the web hangs below the origin."""
    kit.reset()
    silk = fade(kit.mat("Web_Silk", (0.82, 0.83, 0.8), 0.6), 0.55)
    sheet = fade(kit.mat("Web_Sheet", (0.8, 0.8, 0.78), 0.7), 0.12)
    a = Vector((1.1, 0.0, 0.0))
    b = Vector((0.0, 1.1, 0.0))
    c = Vector((0.0, 0.0, -1.1))
    hub = (a + b + c) / 3 * 0.9
    anchors = [a, a.lerp(b, 0.5), b, b.lerp(c, 0.5), c, c.lerp(a, 0.5), a.lerp(b, 0.25), b.lerp(c, 0.75)]
    for i, p in enumerate(anchors):
        rod("Radial%s" % i, hub, p, 0.004, silk, verts=3)
    ring_pts = [a, a.lerp(b, 0.25), a.lerp(b, 0.5), b, b.lerp(c, 0.5), b.lerp(c, 0.75), c, c.lerp(a, 0.5)]
    for loop in range(1, 7):
        t = loop / 7
        pts = [hub.lerp(p, t) for p in ring_pts]
        for i in range(len(pts)):
            p, q = pts[i], pts[(i + 1) % len(pts)]
            mid = (p + q) / 2 + (hub - (p + q) / 2) * 0.08
            rod("Spiral%s_%s_a" % (loop, i), p, mid, 0.003, silk, verts=3)
            rod("Spiral%s_%s_b" % (loop, i), mid, q, 0.003, silk, verts=3)
    mesh_obj("Sheet", [tuple(a), tuple(b), tuple(c), tuple(hub)], [(0, 1, 3), (1, 2, 3), (2, 0, 3)], sheet)
    for i, p in enumerate((a.lerp(b, 0.6), hub.lerp(c, 0.4))):
        rod("Strand%s" % i, p, p + Vector((0.05, -0.03, -0.4 - 0.2 * i)), 0.003, silk, verts=3)
    return finish("cobweb", ground=False, blockers=("__none__",))


# --- Storage ------------------------------------------------------------------------------


def build_crate():
    """A plank crate with battened edges and corner irons."""
    kit.reset()
    m = Mats()
    s = 0.7
    body = kit.box("Body", (s - 0.04, s - 0.04, s - 0.04), (0, 0, s / 2), m.wood)
    uv_box(body, 0.7)
    for k in range(3):
        for sign in (-1, 1):
            # Edge battens along each axis.
            for sign2 in (-1, 1):
                if k == 0:
                    size, loc = (s, 0.07, 0.07), (0, sign * (s / 2 - 0.035), s / 2 + sign2 * (s / 2 - 0.035))
                elif k == 1:
                    size, loc = (0.07, s, 0.07), (sign * (s / 2 - 0.035), 0, s / 2 + sign2 * (s / 2 - 0.035))
                else:
                    size, loc = (0.07, 0.07, s), (sign * (s / 2 - 0.035), sign2 * (s / 2 - 0.035), s / 2)
                batten = kit.box("Batten%s_%s_%s" % (k, sign, sign2), size, loc, m.wood_dark)
                uv_box(batten, 0.7, swap=k == 2)
    for sx in (-1, 1):
        for sy in (-1, 1):
            for sz in (0.035, s - 0.035):
                kit.box("Corner%s_%s_%s" % (sx, sy, sz), (0.08, 0.08, 0.08), (sx * (s / 2 - 0.035), sy * (s / 2 - 0.035), sz), m.iron)
    brace = kit.box("Brace", (0.06, 0.02, s * 1.2), (0, -s / 2 + 0.005, s / 2), m.wood_dark, rot=(0, math.pi / 4, 0))
    uv_box(brace, 0.7, swap=True)
    return finish("crate")


def build_barrel():
    """An oak barrel of bulging staves under three iron hoops."""
    kit.reset()
    m = Mats()
    staves = kit.lathe(
        "Staves",
        [(0, 0.02), (0.27, 0.0), (0.31, 0.2), (0.33, 0.45), (0.31, 0.7), (0.27, 0.9), (0.25, 0.9), (0.25, 0.86), (0.0, 0.86)],
        material=m.wood,
        segs=18,
    )
    wrap(staves, 0.8, swap=True)
    for z, r in ((0.08, 0.282), (0.3, 0.322), (0.6, 0.322), (0.82, 0.275)):
        hoop = kit.ring("Hoop%s" % z, r, 0.012, (0, 0, z), m.iron, segs=18, minor_segs=4)
        hoop.scale = (1, 1, 2.5)
    lid = kit.cyl("Lid", 0.25, 0.015, (0, 0, 0.865), m.wood_dark, verts=16)
    uv_box(lid, 0.5)
    kit.box("LidSeam", (0.5, 0.006, 0.004), (0, 0.06, 0.875), m.coal)
    kit.box("LidSeamB", (0.5, 0.006, 0.004), (0, -0.07, 0.875), m.coal)
    return finish("barrel")


def tapered(name, length, head, foot, z0, height, material):
    """A box narrowing from `head` wide at -X to `foot` wide at +X."""
    outline = [(-length / 2, -head / 2), (length / 2, -foot / 2), (length / 2, foot / 2), (-length / 2, head / 2)]
    return prism(name, outline, height, material, ((0, 0, z0), (1, 0, 0), (0, 1, 0), (0, 0, 1)))


def build_sarcophagus():
    """A carved stone sarcophagus: a stepped plinth, a chest that narrows
    toward the feet with framed panels and rosettes, and a lid carved with
    a recumbent effigy holding a sword, pushed a hand's width askew."""
    kit.reset()
    m = Mats()
    rng = rng_for("sarcophagus")
    carved = img_mat("Carved_Stone", to_image("T_Lab_Effigy", tex_flag(128) * 1.2), rough=0.85, tint=(0.86, 0.82, 0.76))
    dark = kit.mat("Sarcophagus_Gap", (0.02, 0.018, 0.016), 0.95)
    for k, (grow, z0, h) in enumerate(((0.16, 0.0, 0.1), (0.08, 0.1, 0.08))):
        step = tapered("Plinth%s" % k, 2.3 + grow * 2, 1.08 + grow * 2, 0.88 + grow * 2, z0, h, carved)
        uv_box(step, 1.0, (rng.random(), rng.random()))
    chest = tapered("Chest", 2.3, 1.08, 0.88, 0.18, 0.66, carved)
    uv_box(chest, 1.2)
    # Framed panels along each side, each with a carved rosette.
    for sy in (-1, 1):
        for k in range(3):
            x = -0.72 + k * 0.72
            half = (1.08 + (0.88 - 1.08) * ((x + 1.15) / 2.3)) / 2
            y = sy * (half + 0.012)
            slope = math.atan2(0.1, 2.3) * -sy
            for dz, size in ((0.26, (0.6, 0.03, 0.035)), (0.74, (0.6, 0.03, 0.035))):
                rail = kit.box("Rail%s_%s_%s" % (sy, k, dz), size, (x, y, dz), carved, rot=(0, 0, slope))
                uv_box(rail, 0.6)
            for dx in (-0.3, 0.3):
                stile = kit.box("Stile%s_%s_%s" % (sy, k, dx), (0.035, 0.03, 0.5), (x + dx, y + sy * dx * 0.0, 0.5), carved, rot=(0, 0, slope))
                uv_box(stile, 0.6)
            kit.ring("Rosette%s_%s" % (sy, k), 0.09, 0.022, (x, y, 0.5), carved, segs=12, minor_segs=4, rot=(math.pi / 2, 0, slope))
            kit.ball("Boss%s_%s" % (sy, k), 0.04, (x, y, 0.5), carved, segs=8, rings=4, scale=(1, 0.5, 1))
    for sx, w in ((-1, 1.08), (1, 0.88)):
        end_panel = kit.box("End%s" % sx, (0.03, w * 0.7, 0.42), (sx * 1.162, 0, 0.5), carved)
        uv_box(end_panel, 0.6)
    rim = tapered("Rim", 2.36, 1.14, 0.94, 0.84, 0.06, carved)
    uv_box(rim, 1.0)
    # The dark gap the askew lid leaves.
    tapered("Gap", 2.2, 0.98, 0.8, 0.86, 0.04, dark)
    # The lid, with a chamfered top and the effigy.
    lid = tapered("Lid", 2.42, 1.2, 1.0, 0.0, 0.12, carved)
    top = tapered("LidTop", 2.2, 1.0, 0.82, 0.12, 0.06, carved)
    pillow = kit.box("Pillow", (0.3, 0.42, 0.09), (-0.86, 0, 0.21), carved)
    effigy = [
        pillow,
        kit.ball("Head", 0.1, (-0.8, 0, 0.3), carved, segs=12, rings=8, scale=(1.0, 0.9, 0.85)),
        kit.ball("Hood", 0.12, (-0.84, 0, 0.29), carved, segs=12, rings=6, scale=(0.9, 1.05, 0.7)),
        kit.ball("Shoulders", 0.2, (-0.5, 0, 0.26), carved, segs=12, rings=6, scale=(0.8, 1.35, 0.45)),
        kit.ball("Torso", 0.2, (-0.15, 0, 0.25), carved, segs=12, rings=6, scale=(1.6, 1.05, 0.4)),
        kit.ball("Robe", 0.2, (0.42, 0, 0.23), carved, segs=12, rings=6, scale=(2.0, 0.95, 0.33)),
        kit.ball("Hands", 0.055, (-0.3, 0, 0.36), carved, segs=8, rings=5, scale=(1.2, 1.3, 0.8)),
    ]
    for sy in (-1, 1):
        effigy.append(kit.ball("Foot%s" % sy, 0.06, (0.86, sy * 0.08, 0.27), carved, segs=8, rings=5, scale=(0.8, 0.9, 1.4)))
        effigy.append(rod("Arm%s" % sy, (-0.5, sy * 0.24, 0.27), (-0.3, sy * 0.06, 0.34), 0.04, carved, verts=8))
    sword = [
        kit.box("Blade", (0.95, 0.045, 0.02), (0.25, 0, 0.33), m.iron),
        kit.box("Guard", (0.03, 0.22, 0.03), (-0.24, 0, 0.35), m.iron),
        kit.box("Grip", (0.12, 0.03, 0.03), (-0.32, 0, 0.36), m.iron),
    ]
    lid_frame = Matrix.Translation(Vector((0.11, -0.06, 0.9))) @ Matrix.Rotation(0.07, 4, "Z")
    move([lid, top] + effigy + sword, lid_frame)
    for o in (lid, top, pillow):
        uv_box(o, 1.0)
    return finish("sarcophagus")


BUILDERS = {
    "crypt_hall": build_crypt_hall,
    "slab_table": build_slab_table,
    "cauldron_green": lambda: build_cauldron("cauldron_green", (0.15, 0.85, 0.25)),
    "cauldron_red": lambda: build_cauldron("cauldron_red", (0.85, 0.12, 0.06)),
    "cauldron_amber": lambda: build_cauldron("cauldron_amber", (0.95, 0.55, 0.08)),
    "candelabrum_tall": build_candelabrum_tall,
    "candelabrum_short": build_candelabrum_short,
    "floor_candles": build_floor_candles,
    "ritual_rug": build_ritual_rug,
    "specimen_jar": lambda: build_specimen_jar("specimen_jar", False),
    "specimen_jar_bones": lambda: build_specimen_jar("specimen_jar_bones", True),
    "alchemy_bench": build_alchemy_bench,
    "bone_scatter": build_bone_scatter,
    "cobweb": build_cobweb,
    "bookshelf": build_bookshelf,
    "jar_shelf": build_jar_shelf,
    "writing_desk": build_writing_desk,
    "lectern": build_lectern,
    "chained_skeleton": build_chained_skeleton,
    "hanging_chains": build_hanging_chains,
    "brazier": build_brazier,
    "crate": build_crate,
    "barrel": build_barrel,
    "iron_cage": build_iron_cage,
    "sarcophagus": build_sarcophagus,
}


def write_provenance(built):
    folder = out_dir_and_names()[0]
    lines = [
        "# Crypt lab models",
        "",
        "Mode: **Reference**. These models are original solids built from",
        "primitives. A private 1.12.1 Scholomance laboratory view was studied",
        "for the kinds of objects in the room and where they sit. No mesh,",
        "texture, font, or UI file from that view is in this folder, and the",
        "models are not copies of those silhouettes.",
        "",
        "Script: `scripts/blender/chamber_lab.py`",
        "",
        "Command:",
        "",
        "```sh",
        "Blender -b --factory-startup --python scripts/blender/chamber_lab.py -- \\",
        "    assets/verse/generated/chamber",
        "```",
        "",
        "Blender %s." % built[0]["blender"],
        "",
        "## Textures",
        "",
        "Each model packs its images into its glb as PNG.",
        "",
        "From the admitted CC0 1.0 Quaternius kits (Credit: Quaternius),",
        "downscaled to 256 pixels and tinted through the base color factor:",
        "",
        "| Image | Kit | Source file | License file SHA-256 |",
        "| --- | --- | --- | --- |",
    ]
    for rel in ("village/T_Brick_BaseColor.png", "village/T_RockTrim_BaseColor.png"):
        kit_dir = rel.split("/")[0]
        lic = os.path.join(KITS, kit_dir, "license.txt")
        import hashlib

        digest = hashlib.sha256(open(lic, "rb").read()).hexdigest()
        lines.append("| `%s` | Medieval Village MegaKit (Standard) | `assets/verse/everglade/%s` | `%s` |" % (os.path.basename(rel), rel, digest))
    lines += [
        "",
        "Authored in this script with NumPy from seeded value noise, so a",
        "rebuild gives the same pixels: `T_Lab_Wood` (oak boards),",
        "`T_Lab_Iron` (iron with rust), `T_Lab_Brass` (tarnished brass),",
        "`T_Lab_Flag` (worn flagstone), `T_Lab_Effigy` (pale carved stone),",
        "`T_Lab_Linen` (a shroud), `T_Lab_Parchment`, `T_Lab_Page` (a written",
        "page), `T_Lab_Label` (a jar label), `T_Lab_Bone`, and `T_Lab_Rug` (a",
        "wool rug's field, border, and medallion).",
        "",
        "## Light",
        "",
        "The hall is closed. Flames, embers, glowing liquids, and the",
        "crystal carry glTF emission in cd/m² (`KHR_materials_emissive_strength`),",
        "and the capture places a point light at each source. One barred",
        "window in the far gable lets in a shaft of moonlight.",
        "",
        "| Model | Triangles |",
        "| --- | --- |",
    ]
    for info in built:
        lines.append("| `%s` | %s |" % (os.path.basename(info["out"]), info["triangles"]))
    lines.append("")
    path = os.path.join(folder, "PROVENANCE.md")
    with open(path, "w") as handle:
        handle.write("\n".join(lines))


def main():
    folder, names = out_dir_and_names()
    os.makedirs(folder, exist_ok=True)
    unknown = [name for name in names if name not in BUILDERS]
    if unknown:
        sys.exit("Unknown model %s" % ", ".join(unknown))
    built = [BUILDERS[name]() for name in names]
    if names == NAMES:
        write_provenance(built)


if __name__ == "__main__":
    main()
