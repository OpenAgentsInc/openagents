"""Packs the Grid's cast from aethyr-cpp's assets into one file for Coax.

Tooling only: the app never runs this, it ships the file it writes. It reads
aethyr-cpp's assets (read only) and writes:

  Cast.bin           the cast: buffers behind a table (verse/cast.coax)
  cast-layerN.jpg    the character's texture layers, copied as they are

The cast is the default character aethyr's player wears (LookSpec{}: the
male body, dressed as a peasant, in the second hair colour) with short
parted hair in place of aethyr's default buns, at
its nearest level of detail, with the six clips the player plays, and the
Grid's near robot (its faces and line art) with its clips and the recorded
patrols. The characters are Quaternius's CC0 kits (Universal Base Characters,
Modular Character Outfits, Universal Animation Library 1 and 2); the robot
is OpenAgents' own Grid art.

    python3 -I verse/tools/pack_cast.py ../aethyr-cpp/assets \
        apps/openagents-ios/OpenAgents/Resources

The table (MRC1): magic, version 1, buffer count, 0; then per buffer a
16-byte semantic, byte offset, byte length, byte stride and element count
(engine/v1 BufferDescriptor's fields), every u32 little endian. Buffers
start on 16-byte boundaries.
"""
import hashlib, math, os, shutil, struct, sys

CLIPS = ["Idle_Loop", "Walk_Loop", "Sprint_Loop", "Jump_Start", "Jump_Loop", "Jump_Land", "Dance_Loop"]
PARTS = ["Male_Peasant_Arms", "Male_Peasant_Body", "Male_Peasant_Legs", "Male_Peasant_Feet", "Male_head", "Hair_SimpleParted_M"]
LAYERS = [5, 9, 10, 11, 12, 16]          # texture slot i is assets/layer{LAYERS[i]}.jpg
HAIR = (0.22, 0.11, 0.05)                 # kHairColors[1], the default; layers 10 and 11 take it
UNLIT = 255                               # the slot of a flat, unlit colour


def half(h):
    return struct.unpack("<e", struct.pack("<H", h))[0]


def display(linear):
    c = min(max(linear, 0.0), 1.0)
    s = c * 12.92 if c <= 0.0031308 else 1.055 * c ** (1 / 2.4) - 0.055
    return int(round(s * 255))


def vertex(p, n, uv, weights, joints, tint, slot):
    return struct.pack("<3f3f2f4f4B4B8x", *p, *n, *uv, *weights, *joints, *tint, slot)


def character(assets):
    scene = open(os.path.join(assets, "scene.bin"), "rb").read()
    assert scene[:4] == b"SCN4"
    meshes, vcount, icount = struct.unpack_from("<3I", scene, 4)
    vbase = 16 + meshes * 100
    ibase = vbase + vcount * 44
    assert len(scene) == ibase + icount * 4
    found = {}
    for i in range(meshes):
        at = 16 + i * 100
        name = scene[at:at + 48].split(b"\0")[0].decode()
        kind, body = struct.unpack_from("<II", scene, at + 48)
        lod0 = struct.unpack_from("<2I", scene, at + 68)
        if name in PARTS and body == 0:
            found[name] = lod0
    assert sorted(found) == sorted(PARTS), found
    verts, index, remap = [], [], {}
    for name in PARTS:
        first, count = found[name]
        for k in range(count):
            v = struct.unpack_from("<I", scene, ibase + (first + k) * 4)[0]
            if v not in remap:
                at = vbase + v * 44
                p = struct.unpack_from("<3f", scene, at)
                n = struct.unpack_from("<3f", scene, at + 12)
                uv = struct.unpack_from("<2f", scene, at + 24)
                color = scene[at + 32:at + 35]
                layer = scene[at + 35]
                joints = scene[at + 36:at + 40]
                weights = [w / 255 for w in scene[at + 40:at + 44]]
                total = sum(weights) or 1
                weights = [w / total for w in weights]
                assert layer in LAYERS, (name, layer)
                hair = layer in (10, 11)
                tint = [display(c / 255 * (HAIR[i] if hair else 1)) for i, c in enumerate(color)]
                remap[v] = len(verts)
                verts.append(vertex(p, n, uv, weights, joints, tint, LAYERS.index(layer)))
            index.append(remap[v])
    return verts, index


def skeleton_and_clips(assets):
    a = open(os.path.join(assets, "anims.bin"), "rb").read()
    s = open(os.path.join(assets, "skeleton.bin"), "rb").read()
    assert a[:4] == b"ANM1" and s[:4] == b"SKL1"
    joints, bodies, frames, clips, fps = struct.unpack_from("<5I", a, 4)
    assert fps == 30, "cast.coax samples the character clips at 30 fps"
    assert struct.unpack_from("<2I", s, 4) == (joints, bodies)
    texels = 24 + clips * 40
    table = {}
    for i in range(clips):
        at = 24 + i * 40
        table[a[at:at + 32].split(b"\0")[0].decode()] = struct.unpack_from("<2I", a, at + 32)
    bones = []
    for j in range(joints):
        at = 12 + j * 36
        parent = struct.unpack_from("<i", s, at + 32)[0]
        assert parent < j
        bind = struct.unpack_from("<3f", s, 12 + joints * 36 + j * 12)   # body 0
        bones.append(struct.pack("<i3f", parent, *bind))
    clip_rows, poses = [], []
    for name in CLIPS:
        first, count = table[name]
        clip_rows.append(struct.pack("<2I", len(poses) // joints, count))
        for f in range(first, first + count):
            for j in range(joints):
                h = struct.unpack_from("<8H", a, texels + ((0 * frames + f) * joints + j) * 16)
                q = [half(x) for x in h[:4]]
                l = math.sqrt(sum(x * x for x in q)) or 1
                poses.append(struct.pack("<4f3f4x", *[x / l for x in q], *[half(x) for x in h[4:7]]))
    return joints, fps, bones, clip_rows, poses


class Reader:
    def __init__(self, data):
        self.d, self.at = data, 0

    def u(self):
        v = struct.unpack_from("<I", self.d, self.at)[0]; self.at += 4; return v

    def f(self):
        v = struct.unpack_from("<f", self.d, self.at)[0]; self.at += 4; return v

    def arr(self):
        n = self.u(); v = self.d[self.at:self.at + n]; self.at += n; return v


def quat(x):
    # A column-major rotation to a quaternion (aethyr's addRobotModel).
    m00, m11, m22 = x[0], x[5], x[10]
    tr = m00 + m11 + m22
    if tr > 0:
        s = math.sqrt(tr + 1) * 2
        q = ((x[6] - x[9]) / s, (x[8] - x[2]) / s, (x[1] - x[4]) / s, s / 4)
    elif m00 > m11 and m00 > m22:
        s = math.sqrt(1 + m00 - m11 - m22) * 2
        q = (s / 4, (x[4] + x[1]) / s, (x[8] + x[2]) / s, (x[6] - x[9]) / s)
    elif m11 > m22:
        s = math.sqrt(1 + m11 - m00 - m22) * 2
        q = ((x[4] + x[1]) / s, s / 4, (x[9] + x[6]) / s, (x[8] - x[2]) / s)
    else:
        s = math.sqrt(1 + m22 - m00 - m11) * 2
        q = ((x[8] + x[2]) / s, (x[9] + x[6]) / s, s / 4, (x[1] - x[4]) / s)
    l = math.sqrt(sum(c * c for c in q)) or 1
    return [c / l for c in q]


def robot(assets):
    r = Reader(open(os.path.join(assets, "grid.bin"), "rb").read())
    assert r.u() == 0x31524741
    models = []
    for _ in range(r.u()):
        bones, surfaces = r.u(), r.u()
        surf = []
        for _ in range(surfaces):
            lines, _texture = r.u(), r.u()
            tint = (r.f(), r.f(), r.f())
            pos, _second, idx, joints, weights = r.arr(), r.arr(), r.arr(), r.arr(), r.arr()
            surf.append((lines, tint, pos, idx, joints, weights))
        clips = []
        for _ in range(r.u()):
            clips.append((r.f(), r.arr()))
        models.append((bones, surf, clips))
    placements = r.u()
    r.at += placements * 68                # the buildings' placements: not yet
    near, _far = r.u(), r.u()
    _far_distance, scale, rate = r.f(), r.f(), r.f()
    patrols = []
    for _ in range(r.u()):
        r.f()
        patrols.append(r.arr())
    assert r.at == len(r.d)
    bones, surf, clips = models[near]
    verts, faces, lines = [], [], []
    for is_lines, tint, pos, idx, joints, weights in surf:
        base = len(verts)
        n = len(pos) // 12
        assert len(joints) == n * 8 and len(weights) == n * 16
        for i in range(n):
            p = struct.unpack_from("<3f", pos, i * 12)
            j = struct.unpack_from("<4H", joints, i * 8)
            w = struct.unpack_from("<4f", weights, i * 16)
            total = sum(w)
            w = [x / total for x in w] if total > 0 else [1, 0, 0, 0]
            assert max(j) < bones
            verts.append(vertex(p, (0, 1, 0), (0, 0), w, j, [display(c) for c in tint], UNLIT))
        out = lines if is_lines else faces
        out.extend(base + ix for ix in struct.unpack(f"<{len(idx) // 4}I", idx))
    clip_rows, frames = [], []
    for duration, data in clips:
        stride = bones * 16
        count = len(data) // 4 // stride
        assert count * stride * 4 == len(data) and count > 0
        clip_rows.append(struct.pack("<2If4x", len(frames) // bones, count, duration))
        m = struct.unpack(f"<{count * stride}f", data)
        for f in range(count):
            for j in range(bones):
                x = m[(f * bones + j) * 16:(f * bones + j + 1) * 16]
                frames.append(struct.pack("<4f3f4x", *quat(x), x[12], x[13], x[14]))
    patrol_rows, samples = [], []
    for data in patrols:
        n = len(data) // 20
        assert n * 20 == len(data) and n > 0
        patrol_rows.append(struct.pack("<2I", len(samples), n))
        samples.extend(data[i * 20:(i + 1) * 20] for i in range(n))
    setup = struct.pack("<If2f", bones, rate, scale, 0)
    return verts, faces, lines, clip_rows, frames, patrol_rows, samples, setup, bodies(verts, faces)


def bodies(verts, faces):
    """One capsule per robot bone round the faces that bone moves, in the
    model's bind space: along the points' longest axis, wide enough to hold
    every point. Bones thinner than 3 cm (the fingers) sit inside the hand's
    and get none."""
    points = {}
    for i in set(faces):
        p = struct.unpack_from("<3f", verts[i], 0)
        j = struct.unpack_from("<4B", verts[i], 48)
        w = struct.unpack_from("<4f", verts[i], 32)
        points.setdefault(j[w.index(max(w))], []).append(p)
    rows = []
    for bone in sorted(points):
        ps = points[bone]
        n = len(ps)
        c = [sum(p[k] for p in ps) / n for k in range(3)]
        d = [[p[k] - c[k] for k in range(3)] for p in ps]
        cov = [[sum(v[a] * v[b] for v in d) for b in range(3)] for a in range(3)]
        axis = [1.0, 0.7, 0.3]
        for _ in range(64):
            axis = [sum(cov[a][b] * axis[b] for b in range(3)) for a in range(3)]
            l = math.sqrt(sum(x * x for x in axis))
            if l == 0:
                axis = [0.0, 1.0, 0.0]
                break
            axis = [x / l for x in axis]
        t = [sum(v[k] * axis[k] for k in range(3)) for v in d]
        r = max(math.sqrt(max(sum(x * x for x in v) - tv * tv, 0)) for v, tv in zip(d, t))
        if r < 0.03:
            continue
        lo, hi = min(t), max(t)
        # The caps hold the ends: the segment stops a radius short of each.
        a, b = lo + r, hi - r
        if a > b:
            a = b = (lo + hi) / 2
        ends = [[c[k] + axis[k] * e for k in range(3)] for e in (a, b)]
        rows.append(struct.pack("<3ff3fI", *ends[0], r, *ends[1], bone))
    return rows


def write(out, buffers):
    head = 16 + len(buffers) * 32
    at = (head + 15) // 16 * 16
    table, blobs = b"", []
    for semantic, stride, rows in buffers:
        data = b"".join(rows)
        assert rows and all(len(r) == stride for r in rows), semantic
        table += struct.pack("<16s4I", semantic.encode(), at, len(data), stride, len(rows))
        pad = (-len(data)) % 16
        blobs.append(data + b"\0" * pad)
        at += len(data) + pad
    body = struct.pack("<4I", 0x3143524D, 1, len(buffers), 0) + table
    body += b"\0" * ((-len(body)) % 16)
    open(out, "wb").write(body + b"".join(blobs))


def main():
    assets, out = sys.argv[1], sys.argv[2]
    verts, index = character(assets)
    joints, fps, bones, clip_rows, poses = skeleton_and_clips(assets)
    rv, rf, rl, rc, rframes, rp, rs, setup, rb = robot(assets)
    u32 = lambda xs: [struct.pack("<I", x) for x in xs]
    write(os.path.join(out, "Cast.bin"), [
        ("skeleton", 16, bones),
        ("clips", 8, clip_rows),
        ("poses", 32, poses),
        ("body_vertices", 64, verts),
        ("body_indices", 4, u32(index)),
        ("robot_setup", 16, [setup]),
        ("robot_vertices", 64, rv),
        ("robot_faces", 4, u32(rf)),
        ("robot_lines", 4, u32(rl)),
        ("robot_clips", 16, rc),
        ("robot_frames", 32, rframes),
        ("patrols", 8, rp),
        ("patrol_samples", 20, rs),
        ("robot_bodies", 32, rb),
    ])
    for layer in LAYERS:
        shutil.copyfile(os.path.join(assets, f"layer{layer}.jpg"), os.path.join(out, f"cast-layer{layer}.jpg"))
    print("joints", joints, "fps", fps, "body vertices", len(verts), "indices", len(index),
          "poses", len(poses), "robot vertices", len(rv), "faces", len(rf), "lines", len(rl),
          "robot frames", len(rframes), "patrols", len(rp), "bodies", len(rb))
    for name in ("scene.bin", "anims.bin", "skeleton.bin", "grid.bin"):
        print(name, hashlib.sha256(open(os.path.join(assets, name), "rb").read()).hexdigest())


main()
