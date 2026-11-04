"""Turnarounds of the Cultist Raid's robes (verse/tools/humans/cult_robe.py --render DIR), in Blender.

Each look (Cultist, Fanatic, Warlock, Darian) is drawn the way the pack draws it: its robe, the
male head and the hands, read through verse/tools/people.py (rebound onto the people rig's male
skeleton as the pack has them) and posed in a clip of the Universal Animation Library as
aethyr/skin.coax poses them (linear blend). The robe's texture is dyed the way the realm's shader
dyes a person's pale cloth (verse/everglade/gpu.coax eg_person_dye, rule 1). Front, three-quarter
and back, then one frame of each clip; a PNG each into DIR.
"""
import json, math, os
from pathlib import Path

import bpy
import numpy as np

import people
import cult_robe as cr

MANIFEST = cr.REPO / "verse" / "packs" / "everglade.json"
HANDS = [f"{n}_{sd}" for sd in ("l", "r") for n in ["hand"] + [f"{f}_0{k}" for f in ("index", "middle", "ring", "pinky", "thumb") for k in (1, 2, 3)]]
LOOKS = [
    # name, robe, dye (linear rgb), variant, skin tone
    ("cultist", "cult-robe", (0.42, 0.045, 0.05), 0, 0.86),
    ("fanatic", "cult-robe", (0.20, 0.018, 0.028), 1, 0.76),
    ("warlock", "cult-robe", (0.13, 0.035, 0.24), 0, 0.66),
    ("darian", "darian-robe", (0.03, 0.025, 0.035), 1, 0.52),
]
POSES = [("idle", 0.3), ("walk", 0.35), ("run", 0.2), ("spell", 0.5), ("slash", 0.45), ("die", 0.97)]


def srgb_to_lin(c):
    c = c / 255.0
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def lin_to_srgb(c):
    c = np.clip(c, 0, 1)
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * c ** (1 / 2.4) - 0.055)


def dyed(path, dye):
    """The texture as the realm's shader dyes it (eg_person_dye, rule 1, share 1)."""
    im = bpy.data.images.load(str(path))
    w, h = im.size
    px = np.array(im.pixels[:], dtype=np.float32).reshape(h, w, 4)
    bpy.data.images.remove(im)
    t = srgb_to_lin(px[..., :3] * 255.0)
    hi = t.max(axis=2); lo = t.min(axis=2)
    sat = (hi - lo) / np.maximum(hi, 1e-4)
    light = t @ np.array([0.2126, 0.7152, 0.0722])
    ss = lambda a, b, x: np.clip((x - a) / (b - a), 0, 1) ** 2 * (3 - 2 * np.clip((x - a) / (b - a), 0, 1))
    mask = ss(0.12, 0.3, hi) * (1 - ss(0.22, 0.42, sat))
    d = np.array(dye)[None, None, :] * np.clip(light / 0.42, 0, 1.6)[..., None]
    out = t + (d - t) * mask[..., None]
    px[..., :3] = lin_to_srgb(out)
    im = bpy.data.images.new(f"dyed-{path.stem}-{dye}", w, h)
    im.pixels.foreach_set(px.ravel())
    return im


def pose_mats(sk, clip, t):
    """Each joint's skinning matrix (rows) at t seconds into clip."""
    def pair(keys, t):
        i = max(sum(1 for k in keys if k[0] <= t) - 1, 0)
        a = keys[i]; b = keys[i + 1] if i + 1 < len(keys) else a
        f = min(max((t - a[0]) / (b[0] - a[0]), 0.0), 1.0) if b[0] > a[0] else 0.0
        return a[1], b[1], f
    t = min(t, clip["duration"] - 1e-4)
    pel = sk.index("pelvis")
    gq, gt = [], []
    for j in range(len(sk.names)):
        lt = sk.t[j]
        if j == pel and clip["move"]["male"]:
            a, b, f = pair(clip["move"]["male"], t); lt = tuple(x + (y - x) * f for x, y in zip(a, b))
        lq = sk.q[j]
        if clip["rot"][j]:
            a, b, f = pair(clip["rot"][j], t); lq = people.slerp(people.qunit(a), people.qunit(b), f)
        p = sk.parent[j]
        if p < 0:
            gq.append(lq); gt.append(lt)
        else:
            gq.append(people.qunit(people.qmul(gq[p], lq)))
            r = people.apply(people.mat_tr((0, 0, 0), gq[p]), lt)
            gt.append(tuple(x + y for x, y in zip(gt[p], r)))
    return np.array([people.mmul(people.mat_tr(gt[j], gq[j]), sk.ib[j]) for j in range(len(sk.names))])


def skinned(prims, mats):
    out = []
    for pr in prims:
        P = np.array([v["p"] for v in pr["verts"]]); N = np.array([v["n"] for v in pr["verts"]])
        J = np.array([v["j"] for v in pr["verts"]]); W = np.array([v["w"] for v in pr["verts"]], dtype=float) / 255.0
        M = (mats[J] * W[..., None, None]).sum(axis=1)
        p = np.einsum("nij,nj->ni", M[:, :3, :3], P) + M[:, :3, 3]
        n = np.einsum("nij,nj->ni", M[:, :3, :3], N)
        out.append((pr, p, n))
    return out


def material(name, image, tint=1.0):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    tex = nt.nodes.new("ShaderNodeTexImage"); tex.image = image; tex.interpolation = "Linear"
    if tint != 1.0:
        mix = nt.nodes.new("ShaderNodeMix"); mix.data_type = "RGBA"; mix.blend_type = "MULTIPLY"
        mix.inputs["Factor"].default_value = 1.0
        mix.inputs["B"].default_value = (tint, tint * 0.97, tint * 0.95, 1)
        nt.links.new(tex.outputs["Color"], mix.inputs["A"]); nt.links.new(mix.outputs["Result"], bsdf.inputs["Base Color"])
    else:
        nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    bsdf.inputs["Roughness"].default_value = 0.85
    return m


def add_mesh(name, posed, mat_for):
    verts, faces, uvs, mats, slots = [], [], [], [], {}
    for pr, p, _n in posed:
        base = len(verts)
        verts += [(x, -z, y) for x, y, z in p]           # glTF to Blender
        key = (str(pr["image"]), pr["material"])
        if key not in slots:
            slots[key] = len(slots)
        for t in pr["tris"]:
            faces.append(tuple(base + i for i in t)); mats.append(slots[key])
            uvs.append([pr["verts"][i]["uv"] for i in t])
    me = bpy.data.meshes.new(name)
    me.from_pydata(verts, [], faces)
    uvl = me.uv_layers.new()
    for f, poly in enumerate(me.polygons):
        poly.material_index = mats[f]
        poly.use_smooth = True
        for k, li in enumerate(poly.loop_indices):
            u, v = uvs[f][k]; uvl.data[li].uv = (u, 1 - v)
    for key in sorted(slots, key=slots.get):
        me.materials.append(mat_for(key))
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    return ob


def scene_setup():
    sc = bpy.context.scene
    sc.render.engine = "BLENDER_EEVEE"
    sc.render.resolution_x, sc.render.resolution_y = 520, 760
    sc.render.film_transparent = False
    sc.view_settings.view_transform = "Standard"
    w = bpy.data.worlds.new("w"); sc.world = w; w.use_nodes = True
    w.node_tree.nodes["Background"].inputs["Color"].default_value = (0.32, 0.36, 0.42, 1)
    w.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.9
    sun = bpy.data.lights.new("sun", "SUN"); sun.energy = 3.2; sun.angle = 0.2
    so = bpy.data.objects.new("sun", sun); sc.collection.objects.link(so)
    so.rotation_euler = (math.radians(50), 0, math.radians(-30))
    fill = bpy.data.lights.new("fill", "SUN"); fill.energy = 0.8
    fo = bpy.data.objects.new("fill", fill); sc.collection.objects.link(fo)
    fo.rotation_euler = (math.radians(70), 0, math.radians(150))
    gm = bpy.data.meshes.new("ground")
    gm.from_pydata([(-3, -3, 0), (3, -3, 0), (3, 3, 0), (-3, 3, 0)], [], [(0, 1, 2, 3)])
    g = bpy.data.objects.new("ground", gm); sc.collection.objects.link(g)
    gmat = bpy.data.materials.new("ground"); gmat.use_nodes = True
    gmat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.20, 0.26, 0.14, 1)
    gm.materials.append(gmat)
    cam = bpy.data.cameras.new("cam"); cam.lens = 60
    co = bpy.data.objects.new("cam", cam); sc.collection.objects.link(co); sc.camera = co
    return co


def aim(co, yaw_deg, dist=3.6, height=1.15, at=(0, 0, 0.98)):
    from mathutils import Vector
    a = math.radians(yaw_deg)
    co.location = (at[0] + dist * math.sin(a), at[1] - dist * math.cos(a), height)
    d = Vector(at) - co.location
    co.rotation_euler = d.to_track_quat("-Z", "Y").to_euler()


def render(out, dest, argv):
    dest.mkdir(parents=True, exist_ok=True)
    m = json.loads(MANIFEST.read_text())["people"]
    home = os.path.expanduser
    skeletons = {k: people.Skeleton(people.load(v)) for k, v in m["skeletons"].items()}
    sk = skeletons["male"]
    want = {p for p, _ in POSES}
    spec = dict(m, clips=[c for c in m["clips"] if c["name"] in want])
    library_rest = people.Skeleton(people.load(next(iter(spec["libraries"].values())))).t
    clips = {c["name"]: c for c in people.clips(spec, skeletons, library_rest)}
    part = {p["name"]: p for p in m["parts"]}
    head = people.part_mesh({k: v for k, v in part["male-head"].items() if k != "keep"}, skeletons)
    hands = people.part_mesh({"name": "male-hands", "body": "male", "file": part["male-peasant"]["file"],
                              "meshes": ["Male_Peasant_Arms"], "cut": HANDS}, skeletons)
    looks = [l for l in LOOKS if "--looks" not in argv or l[0] in argv[argv.index("--looks") + 1].split(",")]
    far = "--far" in argv
    robes = {r: people.part_mesh({"name": r, "body": "male", "file": str(out / f"{r}{'-far' if far else ''}.gltf")}, skeletons)
             for r in {l[1] for l in looks}}
    poses = [p for p in POSES if "--poses" not in argv or p[0] in argv[argv.index("--poses") + 1].split(",")]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    co = scene_setup()
    for name, robe, dye, variant, tone in looks:
        img = dyed(out / (cr.IMAGE_2 if variant else cr.IMAGE), dye)
        mats = {}
        def mat_for(key, tone=tone):
            if key not in mats:
                path, matname = key
                if matname == cr.MATERIAL:
                    mats[key] = material(f"{name}-robe", img)
                else:
                    mats[key] = material(f"{name}-{matname}", bpy.data.images.load(path, check_existing=True), tone)
            return mats[key]
        for pose, frac in poses:
            clip = clips[pose]
            M = pose_mats(sk, clip, clip["duration"] * frac)
            obs = [add_mesh(f"{name}-{n}", skinned(prims, M), mat_for) for n, prims in (("robe", robes[robe]), ("head", head), ("hands", hands))]
            views = [("front", 0), ("three-quarter", 35), ("back", 180)] if pose == "idle" else [("three-quarter", 35)]
            if pose == "die":
                views = [("side", 90)]
            for view, yaw in views:
                aim(co, yaw, dist=4.2 if pose == "die" else 3.6, height=1.6 if pose == "die" else 1.15,
                    at=(0, 0, 0.4) if pose == "die" else (0, 0, 0.98))
                bpy.context.scene.render.filepath = str(dest / f"{name}-{pose}-{view}.png")
                bpy.ops.render.render(write_still=True)
                print(f"cult_robe: rendered {name} {pose} {view}")
            for o in obs:
                bpy.data.objects.remove(o, do_unlink=True)
