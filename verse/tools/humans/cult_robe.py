"""The Cultist Raid's robes (Everglade, #12740): hooded robes for Darian Odiodei's cult, made in
Blender from code only, so the same command makes the same files.

    blender -b --factory-startup --python verse/tools/humans/cult_robe.py -- [--out DIR] [--render DIR]

It writes, into --out (default assets/verse/cult-robe/), a glTF per robe, skinned to the Quaternius
Modular Character Outfits' male skeleton (the people rig's male body, verse/tools/people.py):

  cult-robe.gltf        a Cultist's robe: a deep hood over the head, a cowl on the shoulders, bell
                        sleeves, a hem near the ankles with folds, a rope belt with hanging ends,
                        boots, a sigil on the chest, trim at the hem, cuffs, hood and down the front
  cult-robe-far.gltf    the same, lighter, for the crowd
  darian-robe.gltf      Darian Odiodei's: longer and wider, a broad mantle and a high standing
                        collar, a deeper hood, ram's horns through it, gilt clasps, a sigil front
                        and back
  darian-robe-far.gltf  the same, lighter
  T_Cult_Robe_BaseColor.png, T_Cult_Robe_2_BaseColor.png
                        the texture they share, and its other colourway (gold trim and sigil)

The robes are one material, MI_Cult_Robe (people.py kind "pale"): its pale cloth takes the
person's dye (verse/everglade/gpu.coax eg_person_dye, rule 1), so one robe is a Cultist's crimson,
a Fanatic's darker crimson or a Warlock's purple, and Darian's black; the trim, rope, leather,
horn, gilt and the dark inside are dark or saturated, so the dye leaves them. The variant layer
(T_Cult_Robe_2) has gold trim and a gold sigil: the Fanatics' and Darian's.

How a robe is made:
  1. its shape is a signed distance (metres, Blender's space: +z up, the figure faces -y, its
     left +x) over a 5 mm grid: the robe's body (an ellipse a height, fitted round the
     peasant's body and flaring to an A line below the hips, with folds deepest at the hem), the
     sleeves (cones along the T pose's arms, belled from the elbow), the hood (an ellipsoid round
     the base body's head with a drooping point) and the cowl (or Darian's mantle); OpenVDB
     turns it into triangles;
  2. the openings are cut (the faces of the hem's cap, the cuffs' caps and the face's opening),
     it is thinned to its budget (Blender's decimate, symmetric across x), then the hood is lined
     (solidify, 9 mm, kept only in the hood: the realm draws both faces of a triangle, so the hem's
     and the sleeves' inside is the cloth's back), its dark lining shading the face;
  3. the belt, the boots, Darian's collar and horns are added as their own pieces;
  4. each face's region (cloth, inside, rope, leather, horn, sigil, gilt, or the band: cloth near a
     trimmed edge) picks its swatch of the texture; in the band a vertex's u is its distance from the
     edge (the hem, a cuff, the face's opening, the front's seam, the cowl's edge), so the trim's
     line is straight whatever the triangles;
  5. skin weights come from the body under it: each vertex takes the weights of the nearest point
     on the Quaternius peasant's shirt, trousers, sleeves and boots and the base body's head
     (barycentric), then they are smoothed along the cloth; the skirt hangs from the pelvis and
     more and more from the thighs down to the knee (four fifths at most), and below it partly from
     the calves; the
     horns are the head's. Four joints a vertex at most.

The glTF it writes keeps the source's own skeleton: its 65 joint nodes, their rest and inverse
binds are copied from Male_Peasant.gltf, so people.py rebinds it exactly.

--render DIR draws turnarounds into DIR (front, three-quarter, back) of each look (Cultist,
Fanatic, Warlock, Darian), each in the people rig's idle and a few clip frames (walk, run, spell,
slash, die), with the male head and hands the pack draws, posed as aethyr/skin.coax poses them.

Sources: the Quaternius Modular Character Outfits - Fantasy and Universal Base Characters (CC0
1.0), under ~/Downloads as verse/packs/everglade.json names them; the robes themselves are
OpenAgents' own, CC0 1.0.
"""
import json, math, os, struct, sys
from pathlib import Path

import bpy, bmesh
import numpy as np
from mathutils import Vector
from mathutils.bvhtree import BVHTree

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "verse" / "tools"))
import people  # noqa: E402

DL = "~/Downloads"
PEASANT = f"{DL}/Modular Character Outfits - Fantasy[Standard]/Exports/glTF (Godot-Unreal)/Outfits/Male_Peasant.gltf"
BASE = f"{DL}/Universal Base Characters[Standard]/Base Characters/Godot - UE/Superhero_Male_FullBody.gltf"
VOXEL = 0.005
TEX = 256
IMAGE, IMAGE_2 = "T_Cult_Robe_BaseColor.png", "T_Cult_Robe_2_BaseColor.png"
MATERIAL = "MI_Cult_Robe"


# ---- the looks

COMMON = dict(
    # the body's profile: (z, half width, front y, back y), margins in; below the last the A line
    profile=[(1.53, 0.150, -0.060, 0.125), (1.46, 0.205, -0.098, 0.150), (1.36, 0.192, -0.132, 0.158),
             (1.24, 0.180, -0.140, 0.148), (1.10, 0.182, -0.146, 0.156), (1.00, 0.200, -0.138, 0.185),
             (0.94, 0.212, -0.140, 0.198)],
    hem=0.085, hem_x=0.345, hem_front=-0.285, hem_back=0.300, folds=12, fold=0.016,
    sleeve_r=(0.084, 0.076), bell=0.052, cuff=0.640,
    hood_c=(0.0, -0.012, 1.700), hood_r=(0.148, 0.172, 0.168), peak=((0.0, 0.05, 1.77), 0.12, (0.0, 0.215, 1.735), 0.016),
    opening=(0.086, 1.688, 0.110),
    cowl_c=(0.0, 0.020, 1.485), cowl_r=(0.245, 0.190, 0.078),
    belt_z=1.00, sigil=(1.285, 0.075), back_sigil=None,
    outer=1600, far=520, mantle=False, collar=False, horns=False, clasps=False,
)
LOOKS = {
    "cult-robe": dict(COMMON),
    "darian-robe": dict(COMMON, hem=0.045, hem_x=0.405, hem_front=-0.320, hem_back=0.345, folds=14, fold=0.022,
                        sleeve_r=(0.088, 0.080), bell=0.075, cuff=0.635,
                        hood_c=(0.0, -0.022, 1.705), hood_r=(0.156, 0.192, 0.176),
                        peak=((0.0, 0.05, 1.79), 0.12, (0.0, 0.20, 1.86), 0.014),
                        opening=(0.084, 1.690, 0.106),
                        cowl_c=(0.0, 0.035, 1.462), cowl_r=(0.305, 0.200, 0.095),
                        sigil=(1.205, 0.075), back_sigil=(1.16, 0.12),
                        outer=2200, far=700, mantle=True, collar=True, horns=True, clasps=True),
}


def smoothstep(a, b, t):
    t = np.clip((t - a) / (b - a), 0.0, 1.0)
    return t * t * (3 - 2 * t)


def smin(a, b, k):
    h = np.clip(0.5 + 0.5 * (b - a) / k, 0.0, 1.0)
    return b * (1 - h) + a * h - k * h * (1 - h)


def profile_at(L, z):
    """The body's half width, centre y and half depth at heights z (arrays)."""
    P = L["profile"]
    zs = np.array([p[0] for p in P])[::-1]
    xs = np.array([p[1] for p in P])[::-1]
    fr = np.array([p[2] for p in P])[::-1]
    bk = np.array([p[3] for p in P])[::-1]
    top = P[-1][0]
    rx = np.interp(z, zs, xs); f = np.interp(z, zs, fr); b = np.interp(z, zs, bk)
    # the A line: from the hips to the hem, a little round at first
    t = np.clip((top - z) / (top - L["hem"]), 0.0, 1.0) ** 1.15
    rx = np.where(z < top, P[-1][1] + (L["hem_x"] - P[-1][1]) * t, rx)
    f = np.where(z < top, P[-1][2] + (L["hem_front"] - P[-1][2]) * t, f)
    b = np.where(z < top, P[-1][3] + (L["hem_back"] - P[-1][3]) * t, b)
    return rx, (f + b) / 2, (b - f) / 2


def ellipse_d(dx, dy, rx, ry):
    """About the distance to an ellipse (its first order: the implicit over its gradient)."""
    q = np.sqrt((dx / rx) ** 2 + (dy / ry) ** 2) + 1e-9
    g = np.sqrt((dx / rx ** 2) ** 2 + (dy / ry ** 2) ** 2) + 1e-9
    return (q - 1) * q * q / g


def ellipsoid_d(p, c, r):
    k0 = np.sqrt(sum(((p[i] - c[i]) / r[i]) ** 2 for i in range(3))) + 1e-9
    k1 = np.sqrt(sum(((p[i] - c[i]) / (r[i] * r[i])) ** 2 for i in range(3))) + 1e-9
    return k0 * (k0 - 1) / k1


def cone_d(p, a, ra, b, rb):
    ab = [b[i] - a[i] for i in range(3)]; L2 = sum(x * x for x in ab)
    t = np.clip(sum((p[i] - a[i]) * ab[i] for i in range(3)) / L2, 0.0, 1.0)
    d = np.sqrt(sum((p[i] - (a[i] + t * ab[i])) ** 2 for i in range(3)))
    return d - (ra + (rb - ra) * t)


def hem_at(L, theta):
    return L["hem"] + 0.010 * np.cos(L["folds"] * theta) + 0.012 * np.clip(-np.cos(theta), 0, 1)


def cuff_at(L, z):
    return L["cuff"] + 0.035 * np.clip((1.455 - z) / 0.12, -1.0, 1.0)


def sleeve_r(L, x):
    r0, r1 = L["sleeve_r"]
    ax = np.abs(x)
    base = np.interp(ax, [0.10, 0.443], [r0, r1])
    return base + L["bell"] * smoothstep(0.45, L["cuff"], ax) ** 1.5


ARM_Y, ARM_Z = 0.068, 1.455


def parts_sdf(L, x, y, z):
    """Each primitive's distance: body, sleeves, cowl (or mantle), hood."""
    rx, cy, ry = profile_at(L, z)
    theta = np.arctan2(x, -(y - cy))          # 0 at the front centre
    fold = L["fold"] * smoothstep(0.93, L["hem"], z) * np.cos(L["folds"] * theta)
    body = ellipse_d(x, y - cy, rx, ry) - fold
    body = np.maximum(body, z - 1.56)
    body = np.maximum(body, hem_at(L, theta) - z)
    ax = np.abs(x)
    rho = np.sqrt((y - ARM_Y) ** 2 + (z - ARM_Z) ** 2)
    sleeve = (rho - sleeve_r(L, x)) * 0.97
    sleeve = np.maximum(sleeve, 0.10 - ax)
    sleeve = np.maximum(sleeve, ax - cuff_at(L, z))
    cowl = ellipsoid_d((x, y, z), L["cowl_c"], L["cowl_r"])
    hood = ellipsoid_d((x, y, z), L["hood_c"], L["hood_r"])
    a, ra, b, rb = L["peak"]
    hood = smin(hood, cone_d((x, y, z), a, ra, b, rb), 0.045)
    return body, sleeve, cowl, hood


def robe_sdf(L, x, y, z):
    body, sleeve, cowl, hood = parts_sdf(L, x, y, z)
    d = np.minimum(np.minimum(body, sleeve), cowl)
    return smin(d, hood, 0.035)


def boots_sdf(L, x, y, z):
    ax = np.abs(x)
    ankle = ellipsoid_d((ax, y, z), (0.091, 0.072, 0.095), (0.058, 0.078, 0.105))
    toe = ellipsoid_d((ax, y, z), (0.091, -0.065, 0.040), (0.052, 0.088, 0.046))
    d = smin(ankle, toe, 0.03)
    d = np.maximum(d, -0.002 - z)
    return np.maximum(d, z - 0.175)


def collar_sdf(L, x, y, z):
    # a standing collar behind the hood, flaring as it rises, open at the front, its top higher behind
    s = 1 + 0.22 * np.clip((z - 1.50) / 0.30, 0, 1) ** 1.5
    wall = np.abs(ellipse_d(x, y - 0.035, 0.205 * s, 0.180 * s)) - 0.008
    top = 1.76 + 0.08 * np.clip((y + 0.05) / 0.25, 0, 1)
    d = np.maximum(wall, 1.47 - z)
    d = np.maximum(d, z - top)
    return np.maximum(d, -0.055 - y)


def polygonize(fn, lo, hi, adaptivity=0.15):
    import openvdb
    n = [int(math.ceil((hi[i] - lo[i]) / VOXEL)) + 1 for i in range(3)]
    arr = np.empty(n, dtype=np.float32)
    xs = lo[0] + VOXEL * np.arange(n[0]); ys = lo[1] + VOXEL * np.arange(n[1]); zs = lo[2] + VOXEL * np.arange(n[2])
    X, Y = np.meshgrid(xs, ys, indexing="ij")
    for k in range(n[2]):
        arr[:, :, k] = fn(X, Y, np.full_like(X, zs[k]))
    g = openvdb.FloatGrid(4 * VOXEL)        # outside, so the grid's edge is no surface
    g.copyFromArray(np.clip(arr, -4 * VOXEL, 4 * VOXEL), tolerance=0.0)
    pts, tris, quads = g.convertToPolygons(isovalue=0.0, adaptivity=adaptivity)
    pts = np.asarray(pts) * VOXEL + np.array(lo)
    faces = [tuple(t) for t in np.asarray(tris).tolist()] + [tuple(q) for q in np.asarray(quads).tolist()]
    return pts, faces


def mesh_object(name, pts, faces):
    me = bpy.data.meshes.new(name)
    me.from_pydata([tuple(p) for p in pts], [], faces)
    me.validate()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    return ob


def select_only(ob):
    for o in bpy.context.scene.objects:
        o.select_set(False)
    ob.select_set(True)
    bpy.context.view_layer.objects.active = ob


def apply_mods(ob):
    select_only(ob)
    for m in list(ob.modifiers):
        bpy.ops.object.modifier_apply(modifier=m.name)


def decimate_to(ob, target):
    tris = sum(len(p.vertices) - 2 for p in ob.data.polygons)
    if tris <= target:
        return
    m = ob.modifiers.new("dec", "DECIMATE")
    m.decimate_type = "COLLAPSE"; m.ratio = target / tris
    m.use_symmetry = True; m.symmetry_axis = "X"
    apply_mods(ob)
    triangulate(ob, normals=True)      # a collapse can turn a face over; solidify would push the inside out


def triangulate(ob, normals=False):
    bm = bmesh.new(); bm.from_mesh(ob.data)
    if normals:
        bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
    bmesh.ops.triangulate(bm, faces=bm.faces[:])
    bm.to_mesh(ob.data); bm.free()


def delete_faces(ob, keep):
    """Keep the faces keep(centre, normal, material) says to."""
    bm = bmesh.new(); bm.from_mesh(ob.data)
    gone = [f for f in bm.faces if not keep(f.calc_center_median(), f.normal, f.material_index)]
    bmesh.ops.delete(bm, geom=gone, context="FACES")
    loose = [v for v in bm.verts if not v.link_faces]
    bmesh.ops.delete(bm, geom=loose, context="VERTS")
    bm.to_mesh(ob.data); bm.free()


def largest_only(ob):
    """Without the crumbs a cut leaves: only the biggest piece."""
    bm = bmesh.new(); bm.from_mesh(ob.data)
    seen, best = set(), []
    for f in bm.faces:
        if f.index in seen:
            continue
        comp, st = [], [f]
        seen.add(f.index)
        while st:
            x = st.pop(); comp.append(x)
            for e in x.edges:
                for y in e.link_faces:
                    if y.index not in seen:
                        seen.add(y.index); st.append(y)
        if len(comp) > len(best):
            best = comp
    keep = {f.index for f in best}
    bmesh.ops.delete(bm, geom=[f for f in bm.faces if f.index not in keep], context="FACES")
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    bm.to_mesh(ob.data); bm.free()


def opening_inside(L, c):
    ax, zc, az = L["opening"]
    return (c.x / ax) ** 2 + ((c.z - zc) / az) ** 2


def robe_shell(L, outer):
    lo = (-0.80, -0.46, max(0.0, L["hem"] - 0.03)); hi = (0.80, 0.46, 2.02)
    pts, faces = polygonize(lambda x, y, z: robe_sdf(L, x, y, z), lo, hi)
    ob = mesh_object("shell", pts, faces)
    triangulate(ob, normals=True)

    def keep(c, n, _m):
        th = math.atan2(c.x, -(c.y - float(profile_at(L, np.array(c.z))[1])))
        if c.z < float(hem_at(L, np.array(th))) + 0.4 * VOXEL:
            return False                                   # the hem's cap (on its cut, the walls above)
        if abs(c.x) > float(cuff_at(L, np.array(c.z))) - 0.4 * VOXEL:
            return False                                   # a cuff's cap
        if c.y < -0.06 and c.z > 1.5 and opening_inside(L, c) < 1.0:
            return False                                   # the face's opening
        return True
    delete_faces(ob, keep)
    largest_only(ob)
    decimate_to(ob, outer)
    # the inside: inner shell (material 1) and rims (material 2)
    ob.data.materials.append(None); ob.data.materials.append(None); ob.data.materials.append(None)
    m = ob.modifiers.new("sol", "SOLIDIFY")
    m.thickness = 0.009; m.offset = -1.0; m.use_rim = False; m.use_even_offset = False; m.use_quality_normals = True
    m.material_offset = 1; m.material_offset_rim = 2
    apply_mods(ob)
    triangulate(ob)

    def seen(c, _n, mat):
        if mat != 1:
            return True
        # the hood's lining only (the realm draws both faces of a triangle, so the hem's and the sleeves'
        # inside is their cloth's back), and none of it that the thinning left near the outside
        return c.z > 1.52 and float(robe_sdf(L, np.array(c.x), np.array(c.y), np.array(c.z))) < -0.0045
    delete_faces(ob, seen)
    return ob


def piece(name, fn, lo, hi, target):
    pts, faces = polygonize(fn, lo, hi)
    ob = mesh_object(name, pts, faces)
    triangulate(ob, normals=True)
    decimate_to(ob, target)
    triangulate(ob)
    return ob


def tube(name, path, radii, sides, closed=False, uv_along=False):
    """A tube along a path of points (Blender space) with a radius at each."""
    bm = bmesh.new()
    rings = []
    n = len(path)
    for i, (p, r) in enumerate(zip(path, radii)):
        p = Vector(p)
        a = Vector(path[(i + 1) % n] if closed or i + 1 < n else path[i]) - Vector(path[i - 1] if closed or i > 0 else path[i])
        t = a.normalized()
        ref = Vector((0, 0, 1)) if abs(t.z) < 0.9 else Vector((1, 0, 0))
        u = t.cross(ref).normalized(); v = t.cross(u)
        rings.append([bm.verts.new(p + r * (math.cos(2 * math.pi * k / sides) * u + math.sin(2 * math.pi * k / sides) * v))
                      for k in range(sides)])
    for i in range(n if closed else n - 1):
        A, B = rings[i], rings[(i + 1) % n]
        for k in range(sides):
            bm.faces.new((A[k], A[(k + 1) % sides], B[(k + 1) % sides], B[k]))
    if not closed:
        if radii[-1] > 1e-4:
            bm.faces.new(list(reversed(rings[-1])))
        if radii[0] > 1e-4:
            bm.faces.new(rings[0])
    bmesh.ops.remove_doubles(bm, verts=bm.verts[:], dist=1e-5)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces[:])
    bmesh.ops.triangulate(bm, faces=bm.faces[:])
    me = bpy.data.meshes.new(name); bm.to_mesh(me); bm.free()
    ob = bpy.data.objects.new(name, me); bpy.context.scene.collection.objects.link(ob)
    if uv_along:
        # the along-the-tube parameter, for a horn's light tip
        along = [i / (n - 1) for i in range(n) for _ in range(sides)]
        attr = me.attributes.new("along", "FLOAT", "POINT")
        for i, a in enumerate(along[:len(me.vertices)]):
            attr.data[i].value = a
    return ob


def belt(L, sides=6, segs=28):
    z = L["belt_z"]
    rx, cy, ry = (float(v) for v in profile_at(L, np.array(z)))
    rx += 0.012; ry += 0.012
    ring = [(rx * math.sin(2 * math.pi * k / segs), cy - ry * math.cos(2 * math.pi * k / segs), z + 0.004 * math.cos(4 * math.pi * k / segs))
            for k in range(segs)]
    obs = [tube("belt", ring, [0.0135] * segs, sides, closed=True)]
    # the knot's two ends, hanging down the front a little left of the centre
    for dx, l in ((0.055, 0.25), (0.085, 0.20)):
        path, radii = [], []
        for i in range(5):
            t = i / 4
            zz = z - 0.01 - l * t
            fr = float(profile_at(L, np.array(zz))[1] - profile_at(L, np.array(zz))[2])
            path.append((dx + 0.01 * t, fr - 0.016, zz)); radii.append(0.011 if i < 4 else 0.016)
        path.append((dx + 0.012, path[-1][1], path[-1][2] - 0.03)); radii.append(0.0)
        obs.append(tube("rope", path, radii, sides))
    return obs


def horns():
    obs = []
    for sd in (1, -1):
        pts = [(0.095, -0.035, 1.805), (0.150, -0.030, 1.870), (0.195, 0.000, 1.950), (0.212, 0.055, 2.010),
               (0.198, 0.110, 2.040), (0.172, 0.150, 2.035)]
        radii = [0.034, 0.030, 0.024, 0.017, 0.010, 0.0]
        obs.append(tube("horn", [(sd * p[0], p[1], p[2]) for p in pts], radii, 8, uv_along=True))
    return obs


def clasps():
    obs = []
    for sd in (1, -1):
        bm = bmesh.new()
        bmesh.ops.create_uvsphere(bm, u_segments=8, v_segments=4, radius=0.026)
        bmesh.ops.scale(bm, vec=(1.0, 0.45, 1.0), verts=bm.verts)
        bmesh.ops.translate(bm, vec=(sd * 0.085, -0.170, 1.445), verts=bm.verts)
        bmesh.ops.triangulate(bm, faces=bm.faces[:])
        me = bpy.data.meshes.new("clasp"); bm.to_mesh(me); bm.free()
        ob = bpy.data.objects.new("clasp", me); bpy.context.scene.collection.objects.link(ob)
        obs.append(ob)
    # the chain between them
    obs.append(tube("chain", [(-0.075, -0.176, 1.440), (0.0, -0.188, 1.415), (0.075, -0.176, 1.440)], [0.006] * 3, 5))
    return obs


# ---- the texture

# swatches (pixels, top-left origin): x0, y0, x1, y1
SW = {"cloth": (0, 0, 128, 64), "inner": (128, 0, 192, 64), "leather": (192, 0, 256, 64),
      "trim": (0, 64, 128, 128), "rope": (128, 64, 192, 128), "horn": (192, 64, 256, 128),
      "sigil": (0, 128, 128, 256), "band": (128, 128, 256, 192), "gilt": (128, 192, 256, 256)}
# the band swatch: across it (u) the distance from an edge (the hem, a cuff, the face's opening, the
# front's seam, the cowl's edge), so a trim's inner edge falls where it should whatever the triangles
BAND_SPAN, BAND_TRIM = 0.12, 0.048     # metres across the swatch, and of them the trim's
CLOTH = np.array([174, 170, 166])


def noise(seed, cells, size=TEX):
    rng = np.random.default_rng(seed)
    g = rng.random((cells + 1, cells + 1))
    t = np.linspace(0, cells, size, endpoint=False)
    i = t.astype(int); f = t - i; f = f * f * (3 - 2 * f)
    a = g[i][:, i] * (1 - f)[None, :] + g[i][:, i + 1] * f[None, :]
    b = g[i + 1][:, i] * (1 - f)[None, :] + g[i + 1][:, i + 1] * f[None, :]
    return a * (1 - f)[:, None] + b * f[:, None]


def texture(gold):
    img = np.zeros((TEX, TEX, 3))
    n1 = noise(3, 12) - 0.5; n2 = noise(5, 28) - 0.5
    yy, xx = np.mgrid[0:TEX, 0:TEX]

    def fill(name, rgb, var=0.0):
        x0, y0, x1, y1 = SW[name]
        img[y0:y1, x0:x1] = np.array(rgb)[None, None, :] * (1 + var * (n1 + 0.5 * n2))[y0:y1, x0:x1, None]
    fill("cloth", CLOTH, 0.10)
    fill("inner", (30, 22, 26), 0.05)
    fill("leather", (76, 52, 36), 0.12)
    trim, line = ((206, 158, 62), (150, 104, 36)) if gold else ((46, 12, 16), (82, 26, 30))
    fill("trim", trim, 0.06)
    x0, y0, x1, y1 = SW["trim"]
    sub = (yy[y0:y1, x0:x1] - y0, xx[y0:y1, x0:x1] - x0)
    diamond = (np.abs(((sub[1] + sub[0]) % 16) - 8) < 1.6) | (np.abs(((sub[1] - sub[0]) % 16) - 8) < 1.6)
    img[y0:y1, x0:x1][diamond] = line
    fill("rope", (148, 108, 66), 0.05)
    x0, y0, x1, y1 = SW["rope"]
    twist = ((xx[y0:y1, x0:x1] + yy[y0:y1, x0:x1]) % 10) < 3
    img[y0:y1, x0:x1][twist] = (112, 78, 46)
    x0, y0, x1, y1 = SW["horn"]
    t = (xx[y0:y1, x0:x1] - x0) / (x1 - x0)
    img[y0:y1, x0:x1] = (np.array((40, 33, 30))[None, None, :] * (1 - t[..., None]) + np.array((96, 82, 66))[None, None, :] * t[..., None])
    # the sigil on pale cloth: a ring, a downward triangle in it, an eye at its heart and two horns
    fill("sigil", CLOTH, 0.10)
    x0, y0, x1, y1 = SW["sigil"]
    u = (xx[y0:y1, x0:x1] - (x0 + 64)) / 64.0; v = (yy[y0:y1, x0:x1] - (y0 + 64)) / 64.0
    r = np.sqrt(u * u + v * v)
    ink = np.abs(r - 0.80) < 0.085
    # the triangle, point down, its corners on a circle of 0.68
    for k in range(3):
        a0 = math.pi / 2 + 2 * math.pi * k / 3 + math.pi; a1 = a0 + 2 * math.pi / 3
        p0 = (0.68 * math.cos(a0), -0.68 * math.sin(a0)); p1 = (0.68 * math.cos(a1), -0.68 * math.sin(a1))
        ex, ey = p1[0] - p0[0], p1[1] - p0[1]; L2 = ex * ex + ey * ey
        tt = np.clip(((u - p0[0]) * ex + (v - p0[1]) * ey) / L2, 0, 1)
        ink |= np.sqrt((u - p0[0] - tt * ex) ** 2 + (v - p0[1] - tt * ey) ** 2) < 0.07
    ink |= np.sqrt(u * u + (v + 0.06) ** 2) < 0.15
    for sd in (1, -1):
        ink |= (np.abs(np.sqrt((u - sd * 0.48) ** 2 + (v + 0.95) ** 2) - 0.36) < 0.07) & (v < -0.62) & (sd * u < sd * 0.48 + 0.1)
    img[y0:y1, x0:x1][ink] = (206, 158, 62) if gold else (70, 14, 18)
    fill("band", CLOTH, 0.10)
    x0, y0, x1, y1 = SW["band"]
    d = (xx[y0:y1, x0:x1] - x0 - 4) / (x1 - x0 - 8) * BAND_SPAN
    sub = (yy[y0:y1, x0:x1] - y0, xx[y0:y1, x0:x1] - x0)
    diamond = (np.abs(((sub[1] + sub[0]) % 16) - 8) < 1.6) | (np.abs(((sub[1] - sub[0]) % 16) - 8) < 1.6)
    band = img[y0:y1, x0:x1]
    band[d < BAND_TRIM] = trim
    band[(d < BAND_TRIM) & diamond] = line
    band[(d >= BAND_TRIM - 0.006) & (d < BAND_TRIM)] = line
    x0, y0, x1, y1 = SW["gilt"]
    fill("gilt", (212, 164, 70), 0.0)
    shine = 0.80 + 0.35 * np.sin((xx[y0:y1, x0:x1] - x0) / (x1 - x0) * math.pi)
    img[y0:y1, x0:x1] *= shine[..., None]
    return np.clip(img, 0, 255).astype(np.uint8)


def save_png(arr, path):
    h, w, _ = arr.shape
    im = bpy.data.images.new(path.stem, w, h, alpha=False)
    px = np.concatenate([arr[::-1].astype(np.float32) / 255.0, np.ones((h, w, 1), np.float32)], axis=2)
    im.pixels.foreach_set(px.ravel())
    im.filepath_raw = str(path); im.file_format = "PNG"
    im.save()
    bpy.data.images.remove(im)


# ---- regions and UVs

def band_d(L, kind, p):
    """How far (metres) a point of the robe's outside is from the nearest trimmed edge."""
    if kind == "collar":
        top = 1.76 + 0.08 * min(max((p.y + 0.05) / 0.25, 0), 1)
        return top - p.z
    if kind != "robe":
        return 9.0
    x, y, z = p.x, p.y, p.z
    rx, cy, ry = (float(v) for v in profile_at(L, np.array(z)))
    th = math.atan2(x, -(y - cy))
    d = z - float(hem_at(L, np.array(th)))
    d = min(d, float(cuff_at(L, np.array(z))) - abs(x) + (0.0 if abs(x) > 0.35 else 9.0))
    ax, zc, az = L["opening"]
    q = math.sqrt((x / ax) ** 2 + ((z - zc) / az) ** 2)
    d = min(d, (q - 1) * min(ax, az) + max(0.0, y + 0.06) * 6 + max(0.0, 1.5 - z) * 6)
    body, sleeve, cowl, hood = (float(v) for v in parts_sdf(L, np.array(x), np.array(y), np.array(z)))
    if body <= min(sleeve, cowl) + 0.003:
        d = min(d, abs(x) + 0.028 + max(0.0, y - (cy - 0.05)) * 6 + max(0.0, z - 1.40) * 6)
    if cowl <= min(body, sleeve, hood) + 0.003 and z < L["cowl_c"][2]:
        d = min(d, max(0.0, min(body, sleeve)) * 1.1 + 0.012)
    return d


def region_of(L, kind, c, n, mat, sd, bands=None):
    """A face's swatch by its piece, centre, normal, solidify material and its pieces' distances."""
    if kind in ("rope", "leather", "horn", "gilt"):
        return kind
    if kind == "collar":
        inward = n.x * c.x + n.y * (c.y - 0.035) < 0
        return "gilt" if inward or n.z > 0.6 else ("band" if min(bands) < BAND_SPAN else "cloth")
    rx, cy, ry = (float(v) for v in profile_at(L, np.array(c.z)))
    th = math.atan2(c.x, -(c.y - cy))
    near_hem = c.z < float(hem_at(L, np.array(th))) + 0.052
    near_cuff = abs(c.x) > float(cuff_at(L, np.array(c.z))) - 0.045
    near_face = c.z > 1.52 and c.y < -0.05 and opening_inside(L, c) < 1.45
    if mat == 2:
        return "trim" if (near_hem or near_cuff or near_face) else "inner"
    if mat == 1:
        return "inner"
    body, sleeve, cowl, hood = sd
    z0, r = L["sigil"]
    if c.y < cy - 0.05 and abs(c.x) < r and abs(c.z - z0) < r and body <= min(sleeve, cowl) + 0.002:
        return "sigil"
    if L["back_sigil"]:
        z0, r = L["back_sigil"]
        if c.y > cy + 0.05 and abs(c.x) < r and abs(c.z - z0) < r and body <= min(sleeve, cowl) + 0.002:
            return "sigil_back"
    return "band" if min(bands) < BAND_SPAN else "cloth"


def uv_of(L, region, p, along, band=9.0):
    if region in ("sigil", "sigil_back"):
        x0, y0, x1, y1 = SW["sigil"]
        z0, r = L["sigil"] if region == "sigil" else L["back_sigil"]
        su = p.x if region == "sigil" else -p.x
        u = (x0 + 64 + 64 * su / r) / TEX; v = (y0 + 64 + 64 * (z0 - p.z) / r) / TEX
        return (min(max(u, (x0 + 1) / TEX), (x1 - 1) / TEX), min(max(v, (y0 + 1) / TEX), (y1 - 1) / TEX))
    x0, y0, x1, y1 = SW[region]
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
    hx, hy = (x1 - x0) * 0.30, (y1 - y0) * 0.30
    if region == "band":
        u = x0 + 4 + (x1 - x0 - 8) * min(max(band / BAND_SPAN, 0.0), 1.0)
        return (u / TEX, (cy + hy * math.cos(11.7 * p.y + 5.1 * p.z - 3.3 * p.x)) / TEX)
    if region == "horn":
        return ((x0 + 10 + (x1 - x0 - 20) * along) / TEX, cy / TEX)
    jx = math.sin(13.1 * p.x + 7.3 * p.z + 2.1 * p.y); jy = math.cos(11.7 * p.y + 5.1 * p.z - 3.3 * p.x)
    return ((cx + hx * jx) / TEX, (cy + hy * jy) / TEX)


# ---- skin weights

class Source:
    """The body under the robe: triangles in Blender space and each vertex's joint weights."""
    def __init__(self, names):
        self.names = names
        verts, tris, W = [], [], []
        def add(ob, keep_vert=None):
            me = ob.evaluated_get(bpy.context.evaluated_depsgraph_get()).to_mesh()
            gi = {g.index: g.name for g in ob.vertex_groups}
            base = len(verts)
            ok = []
            for v in me.vertices:
                w = np.zeros(len(names))
                for g in v.groups:
                    nm = gi.get(g.group)
                    if nm in names:
                        w[names.index(nm)] += g.weight
                s = w.sum()
                verts.append(tuple(ob.matrix_world @ v.co)); W.append(w / s if s > 0 else w)
                ok.append(keep_vert is None or keep_vert(w))
            me.calc_loop_triangles()
            for t in me.loop_triangles:
                if all(ok[i] for i in t.vertices):
                    tris.append(tuple(base + i for i in t.vertices))
            ob.to_mesh_clear()
        self.add = add
        self.verts, self.tris, self.W = verts, tris, W

    def build(self):
        self.Wa = np.array(self.W)
        self.bvh = BVHTree.FromPolygons(self.verts, self.tris)

    def at(self, p):
        loc, _n, f, _d = self.bvh.find_nearest(Vector(p))
        a, b, c = (Vector(self.verts[i]) for i in self.tris[f])
        # barycentric of loc in (a, b, c)
        v0, v1, v2 = b - a, c - a, loc - a
        d00, d01, d11 = v0.dot(v0), v0.dot(v1), v1.dot(v1)
        d20, d21 = v2.dot(v0), v2.dot(v1)
        den = d00 * d11 - d01 * d01 or 1e-12
        v = (d11 * d20 - d01 * d21) / den; w = (d00 * d21 - d01 * d20) / den; u = 1 - v - w
        bc = np.clip(np.array([u, v, w]), 0, 1); bc /= bc.sum()
        i, j, k = self.tris[f]
        return bc[0] * self.Wa[i] + bc[1] * self.Wa[j] + bc[2] * self.Wa[k]


def skin_weights(L, ob, src, names, horn_verts):
    me = ob.data
    N = len(me.vertices)
    P = np.array([tuple(v.co) for v in me.vertices])
    W = np.array([src.at(p) for p in P])
    J = {n: names.index(n) for n in names}
    # along the cloth: smoothed, much in the skirt so a stride pulls it as one piece
    nb = [[] for _ in range(N)]
    for e in me.edges:
        a, b = e.vertices; nb[a].append(b); nb[b].append(a)
    # the pieces that touch on the surface but share no edge (inside and outside meet at the rims)
    z = P[:, 2]
    iters = np.where(z < 1.0, 40, 4)
    for it in range(40):
        live = iters > it
        mean = np.array([W[x].mean(axis=0) if x else W[i] for i, x in enumerate(nb)])
        W = np.where(live[:, None], 0.5 * W + 0.5 * mean, W)
    # the skirt hangs from the pelvis, then the thighs; below the knee part from the calves
    hip, knee = 0.97, 0.54
    # (a fifth stays the pelvis's at the hem, so a stride or a run swings it less than the leg)
    a = 0.8 * np.clip((hip + 0.05 - z) / (hip + 0.05 - knee), 0, 1)[:, None]
    pel = np.zeros(len(names)); pel[J["pelvis"]] = 1
    skirt = (z < hip + 0.05)[:, None]
    W = np.where(skirt, (1 - a) * pel[None, :] + a * W, W)
    for sd in ("_l", "_r"):
        for low in ("foot" + sd, "ball" + sd, "ball_leaf" + sd):
            W[:, J["calf" + sd]] += W[:, J[low]]; W[:, J[low]] = 0
        share = 0.40 * np.clip((knee - z) / (knee - L["hem"]), 0, 1)
        moved = W[:, J["calf" + sd]] * (1 - share)
        W[:, J["thigh" + sd]] += np.where(z < hip, moved, 0); W[:, J["calf" + sd]] -= np.where(z < hip, moved, 0)
    # horns are the head's
    for i in horn_verts:
        W[i] = 0; W[i, J["Head"]] = 1
    # four at most, summing to one
    out = []
    for w in W:
        idx = np.argsort(-w)[:4]
        ww = np.clip(w[idx], 0, None)
        if ww.sum() <= 0:
            idx = np.array([J["pelvis"], 0, 0, 0]); ww = np.array([1.0, 0, 0, 0])
        ww = ww / ww.sum()
        ww = np.where(ww < 0.01, 0, ww); ww = ww / ww.sum()
        out.append((idx, ww))
    return out


# ---- glTF

def bl_to_gl(p):
    return (p[0], p[2], -p[1])


def write_gltf(path, name, verts, tris, source):
    """One mesh, one primitive, MI_Cult_Robe, on the source's own skeleton (its joint nodes, rest
    and inverse binds copied)."""
    src = people.load(source)
    joints = src["skins"][0]["joints"]
    nodes = []
    remap = {}
    for k, j in enumerate(sorted(joints)):
        remap[j] = k
    for j in sorted(joints):
        n = dict(src["nodes"][j])
        if "children" in n:
            n["children"] = [remap[c] for c in n["children"]]
        nodes.append(n)
    roots = [remap[j] for j in joints if not any(j in src["nodes"][p].get("children", []) for p in joints)]
    mesh_node = len(nodes); arm_node = mesh_node + 1
    nodes.append({"name": name, "mesh": 0, "skin": 0})
    nodes.append({"name": "Armature", "children": roots + [mesh_node]})
    ib = src["acc"](src["skins"][0]["inverseBindMatrices"])
    pos = [bl_to_gl(v["p"]) for v in verts]
    nor = [bl_to_gl(v["n"]) for v in verts]
    blobs = []
    def blob(data):
        off = sum(len(b) for b in blobs); pad = (-len(data)) % 4
        blobs.append(data + b"\0" * pad); return off, len(data)
    views, accs = [], []
    def acc(data, ctype, typ, count, target=None, mn=None, mx=None):
        off, ln = blob(data)
        v = {"buffer": 0, "byteOffset": off, "byteLength": ln}
        if target: v["target"] = target
        views.append(v)
        a = {"bufferView": len(views) - 1, "componentType": ctype, "count": count, "type": typ}
        if mn is not None: a["min"], a["max"] = mn, mx
        accs.append(a); return len(accs) - 1
    f3 = lambda rows: b"".join(struct.pack("<3f", *r) for r in rows)
    a_pos = acc(f3(pos), 5126, "VEC3", len(pos), 34962, [min(p[i] for p in pos) for i in range(3)], [max(p[i] for p in pos) for i in range(3)])
    a_nor = acc(f3(nor), 5126, "VEC3", len(nor), 34962)
    a_uv = acc(b"".join(struct.pack("<2f", *v["uv"]) for v in verts), 5126, "VEC2", len(verts), 34962)
    a_j = acc(b"".join(struct.pack("<4B", *v["j"]) for v in verts), 5121, "VEC4", len(verts), 34962)
    a_w = acc(b"".join(struct.pack("<4f", *v["w"]) for v in verts), 5126, "VEC4", len(verts), 34962)
    a_ib = acc(b"".join(struct.pack("<16f", *m) for m in ib), 5126, "MAT4", len(ib))
    a_idx = acc(struct.pack(f"<{len(tris) * 3}I", *[i for t in tris for i in t]), 5125, "SCALAR", len(tris) * 3, 34963)
    bin_name = path.with_suffix(".bin").name
    g = {
        "asset": {"version": "2.0", "generator": "verse/tools/humans/cult_robe.py",
                  "copyright": "OpenAgents' own, CC0 1.0; skeleton from the Quaternius Modular Character Outfits - Fantasy (CC0 1.0)"},
        "scene": 0, "scenes": [{"name": "Scene", "nodes": [arm_node]}], "nodes": nodes,
        "skins": [{"name": "Armature", "joints": [remap[j] for j in joints], "inverseBindMatrices": a_ib}],
        "meshes": [{"name": name, "primitives": [{"attributes": {"POSITION": a_pos, "NORMAL": a_nor, "TEXCOORD_0": a_uv,
                                                                 "JOINTS_0": a_j, "WEIGHTS_0": a_w},
                                                  "indices": a_idx, "material": 0}]}],
        "materials": [{"name": MATERIAL, "pbrMetallicRoughness": {"baseColorTexture": {"index": 0}, "metallicFactor": 0.0, "roughnessFactor": 1.0}}],
        "textures": [{"sampler": 0, "source": 0}], "images": [{"name": IMAGE[:-4], "uri": IMAGE, "mimeType": "image/png"}],
        "samplers": [{"magFilter": 9729, "minFilter": 9987}],
        "accessors": accs, "bufferViews": views,
        "buffers": [{"byteLength": sum(len(b) for b in blobs), "uri": bin_name}],
    }
    path.with_suffix(".bin").write_bytes(b"".join(blobs))
    path.write_text(json.dumps(g, indent=1) + "\n")


def export_vertices(L, obs, src, names):
    """The pieces joined: vertices split where their faces' swatches differ, each with its uv, smooth
    normal and four joints; triangles."""
    verts, tris = [], []
    horn_count = 0
    for kind, ob in obs:
        me = ob.data
        me.calc_loop_triangles()
        horn_verts = list(range(len(me.vertices))) if kind == "horn" else []
        sw = skin_weights(L, ob, src, names, horn_verts)
        along = me.attributes.get("along")
        key = {}
        bd = [band_d(L, kind, v.co) for v in me.vertices]
        for poly in me.polygons:
            c = poly.center; n = poly.normal
            sd = [float(v) for v in parts_sdf(L, np.array(c.x), np.array(c.y), np.array(c.z))] if kind == "robe" else None
            reg = region_of(L, kind, c, n, poly.material_index, sd, [bd[i] for i in poly.vertices])
            face = []
            for vi in poly.vertices:
                k = (vi, reg)
                if k not in key:
                    v = me.vertices[vi]
                    a = along.data[vi].value if along else 0.0
                    j, w = sw[vi]
                    nn = v.normal
                    key[k] = len(verts)
                    verts.append({"p": tuple(v.co), "n": tuple(nn), "uv": uv_of(L, reg, v.co, a, bd[vi]),
                                  "j": [int(x) for x in j], "w": [float(x) for x in w]})
                face.append(key[k])
            tris.append(tuple(face))
    return verts, tris


def build_look(name, L, src, names, far=False):
    for o in [o for o in bpy.context.scene.objects if o.name.split(".")[0] in ("shell", "boots", "collar", "belt", "rope", "horn", "clasp", "chain")]:
        bpy.data.objects.remove(o, do_unlink=True)
    obs = [("robe", robe_shell(L, L["far"] if far else L["outer"]))]
    obs.append(("leather", piece("boots", lambda x, y, z: boots_sdf(L, x, y, z), (-0.17, -0.18, -0.01), (0.17, 0.18, 0.19), 70 if far else 220)))
    if L["collar"]:
        obs.append(("collar", piece("collar", lambda x, y, z: collar_sdf(L, x, y, z), (-0.32, -0.12, 1.45), (0.32, 0.32, 1.92), 180 if far else 640)))
    for ob in belt(L, sides=4 if far else 6, segs=16 if far else 28):
        obs.append(("rope", ob))
    if L["horns"]:
        for ob in horns():
            obs.append(("horn", ob))
    if L["clasps"]:
        for ob in clasps():
            obs.append(("gilt", ob))
    return export_vertices(L, obs, src, names)


def body_source():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=os.path.expanduser(PEASANT))
    sk = people.Skeleton(people.load(PEASANT))
    names = sk.names
    src = Source(names)
    for n in ("Male_Peasant_Body", "Male_Peasant_Legs", "Male_Peasant_Arms", "Male_Peasant_Feet"):
        src.add(bpy.data.objects[n])
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=os.path.expanduser(BASE))
    head = next(o for o in set(bpy.data.objects) - before if o.type == "MESH" and o.name.startswith("SuperHero"))
    hi, ni = names.index("Head"), names.index("neck_01")
    src.add(head, keep_vert=lambda w: w[hi] + w[ni] > 0.5)
    for o in set(bpy.data.objects) - before:
        bpy.data.objects.remove(o, do_unlink=True)
    for o in [o for o in bpy.data.objects if o.type == "MESH"]:
        bpy.data.objects.remove(o, do_unlink=True)
    src.build()
    return src, names


def main(argv):
    out = REPO / "assets" / "verse" / "cult-robe"
    render = None
    if "--out" in argv: out = Path(argv[argv.index("--out") + 1])
    if "--render" in argv: render = Path(argv[argv.index("--render") + 1])
    only = argv[argv.index("--only") + 1].split(",") if "--only" in argv else None
    out.mkdir(parents=True, exist_ok=True)
    if not render or "--build" in argv:
        save_png(texture(False), out / IMAGE)
        save_png(texture(True), out / IMAGE_2)
        src, names = body_source()
        for name, L in LOOKS.items():
            for far in (False, True):
                stem = name + ("-far" if far else "")
                if only and stem not in only:
                    continue
                verts, tris = build_look(name, L, src, names, far)
                write_gltf(out / f"{stem}.gltf", stem, verts, tris, os.path.expanduser(PEASANT))
                print(f"cult_robe: {stem}: {len(tris)} triangles, {len(verts)} vertices")
    if render:
        import cult_robe_render
        cult_robe_render.render(out, render, argv)


if __name__ == "__main__":
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    main(argv)
