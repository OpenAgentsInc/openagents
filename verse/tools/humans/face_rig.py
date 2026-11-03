"""A MetaHuman face rig for MetaHuman Lab, from the DNA's rig behaviour.

The tables come from dna_behavior (its behavior.txt and targets.bin, made from the head DNA by
OpenRigLogic's reader). This module evaluates them the way OpenRigLogic does, as a reference,
and packs them into one block the lab's Coax port evaluates every frame (metahumanlab.coax,
face_*). The evaluation follows OpenRigLogic (MIT, Copyright (c) 2026 Epic Games, Inc.):

  controls   raw controls set by an expression; each pose-space corrective (PSD) is its weight
             times the product of its inputs clamped to 0..1, capped at 1
  joints     each joint group adds values x inputs (row-major, outputs x inputs, the LOD 0 rows)
             to its output attributes (joint * 9 + tx ty tz rx ry rz sx sy sz); a joint's local
             transform is its neutral translation and Euler rotation (degrees, x then y then z)
             plus those deltas, scale 1 plus its delta
  blendshapes each channel's weight is its input control; a target adds its deltas times that
             weight to its mesh's points, before the joints skin them

The data is private (a MetaHuman's): never committed, never used to train AI models.
"""
import math
import struct
from pathlib import Path

MAGIC = b"FRG1"
# The meshes the pack draws that the face rig moves, by DNA mesh name.
FACE_MESHES = ("head_lod0_mesh", "teeth_lod0_mesh", "eyeLeft_lod0_mesh", "eyeRight_lod0_mesh", "eyelashes_lod0_mesh")
DELTA_MIN = 0.01        # cm: blend shape deltas below this (a tenth of a millimetre) are dropped
DELTA_UNIT = 0.001      # cm per step of a packed delta (i16)
INFLUENCES = 6          # joints kept per point
# Expressions: raw controls (CTRL_expressions.NAME) and their values. "talk" also opens and closes
# the jaw over time in the lab; "blink" is the lab's blink, on top of any expression.
EXPRESSIONS = {
    "neutral": {},
    "smile": {"mouthCornerPullL": 0.85, "mouthCornerPullR": 0.85, "eyeCheekRaiseL": 0.45, "eyeCheekRaiseR": 0.45,
              "mouthStretchL": 0.15, "mouthStretchR": 0.15, "noseNasolabialDeepenL": 0.3, "noseNasolabialDeepenR": 0.3},
    "frown": {"browDownL": 0.8, "browDownR": 0.8, "browLateralL": 0.5, "browLateralR": 0.5, "mouthCornerDepressL": 0.6,
              "mouthCornerDepressR": 0.6, "noseWrinkleL": 0.25, "noseWrinkleR": 0.25, "eyeSquintInnerL": 0.3, "eyeSquintInnerR": 0.3},
    "surprise": {"browRaiseInL": 0.9, "browRaiseInR": 0.9, "browRaiseOuterL": 0.9, "browRaiseOuterR": 0.9,
                 "eyeWidenL": 0.6, "eyeWidenR": 0.6, "jawOpen": 0.35},
    "talk": {"mouthLipsTogetherUL": 0.0},
    "blink": {"eyeBlinkL": 1.0, "eyeBlinkR": 1.0},
    "jaw": {"jawOpen": 1.0},
}


def euler(rx, ry, rz):
    """Rz Ry Rx (degrees), as metahuman.euler_xyz: the DNA's joint rotations."""
    d = math.pi / 180
    cx, sx, cy, sy, cz, sz = math.cos(rx * d), math.sin(rx * d), math.cos(ry * d), math.sin(ry * d), math.cos(rz * d), math.sin(rz * d)
    return [[cy * cz, cz * sx * sy - cx * sz, cx * cz * sy + sx * sz],
            [cy * sz, cx * cz + sx * sy * sz, -cz * sx + cx * sy * sz],
            [-sy, cy * sx, cx * cy]]


class Behavior:
    """dna_behavior's behavior.txt."""
    def __init__(self, path):
        lines = Path(path).read_text().splitlines()
        self.raw = lines[0].split()[2:]
        self.groups, self.joints, self.meshes = [], [], []
        nums = lambda l, f=float: [f(x) for x in l.split()[2:]]
        i = 1
        while i < len(lines):
            w = lines[i].split()
            k = w[0] if w else ""
            if k == "psdrows": self.psd_rows = nums(lines[i], int)
            elif k == "psdcols": self.psd_cols = nums(lines[i], int)
            elif k == "psdvalues": self.psd_values = nums(lines[i])
            elif k == "lods":
                lods = nums(lines[i], int); ins = nums(lines[i + 1], int); outs = nums(lines[i + 2], int)
                vals = nums(lines[i + 3]); js = nums(lines[i + 4], int)
                rows = lods[0]
                self.groups.append({"in": ins, "out": outs[:rows], "values": vals[:rows * len(ins)], "joints": js})
                i += 5; continue
            elif k == "bscin": self.bsc_in = nums(lines[i], int)
            elif k == "bscout": self.bsc_out = nums(lines[i], int)
            elif k == "mesh":
                self.meshes.append(w[1])
            elif k == "joint":
                self.joints.append((w[1], int(w[2]), tuple(float(x) for x in w[3:6]), tuple(float(x) for x in w[6:9])))
            i += 1
        self.controls = 1 + max([len(self.raw) - 1] + self.psd_rows + self.psd_cols + self.bsc_in +
                                [c for g in self.groups for c in g["in"]])
        # each PSD: its output, weight (the product of its entries' values) and inputs
        psd = {}
        for r, c, v in zip(self.psd_rows, self.psd_cols, self.psd_values):
            e = psd.setdefault(r, [1.0, []]); e[0] *= v; e[1].append(c)
        self.psds = [(r, w, ins) for r, (w, ins) in sorted(psd.items())]
        self.channel_control = {o: c for c, o in zip(self.bsc_in, self.bsc_out)}

    def control(self, name):
        return self.raw.index("CTRL_expressions." + name)


def targets(path, meshes):
    """targets.bin: per DNA mesh index, its targets (channel, [(vertex, (dx, dy, dz))])."""
    b = Path(path).read_bytes(); at = 0; out = {}
    while at < len(b):
        m, n = struct.unpack_from("<HI", b, at); at += 6
        ts = []
        for _ in range(n):
            c, cnt = struct.unpack_from("<HI", b, at); at += 6
            idx = struct.unpack_from(f"<{cnt}I", b, at); at += 4 * cnt
            d = struct.unpack_from(f"<{3 * cnt}f", b, at); at += 12 * cnt
            ts.append((c, [(idx[i], d[3 * i:3 * i + 3]) for i in range(cnt)
                           if abs(d[3 * i]) + abs(d[3 * i + 1]) + abs(d[3 * i + 2]) >= DELTA_MIN]))
        out[m] = ts
    return out


def evaluate(bh, raw):
    """OpenRigLogic's controls and joints: the control vector and each joint's local transform
    (3x3, translation cm) for raw control values {index: value}."""
    c = [0.0] * bh.controls
    for k, v in raw.items():
        c[k] = v
    for out, w, ins in bh.psds:
        v = w
        for i in ins:
            v *= min(max(c[i], 0.0), 1.0)
        c[out] = min(1.0, v)
    attr = [0.0] * (len(bh.joints) * 9)
    for g in bh.groups:
        ins = g["in"]; xs = [c[i] for i in ins]
        if not any(xs):
            continue
        n = len(ins); vals = g["values"]
        for r, o in enumerate(g["out"]):
            row = vals[r * n:(r + 1) * n]
            attr[o] += sum(a * x for a, x in zip(row, xs))
    local = []
    for j, (_, _, t, r) in enumerate(bh.joints):
        a = attr[9 * j:9 * j + 9]
        R = euler(r[0] + a[3], r[1] + a[4], r[2] + a[5])
        s = (1 + a[6], 1 + a[7], 1 + a[8])
        R = [[R[i][k] * s[k] for k in range(3)] for i in range(3)]
        local.append((R, (t[0] + a[0], t[1] + a[1], t[2] + a[2])))
    return c, local


def world(bh, local):
    out = []
    for j, (_, par, _, _) in enumerate(bh.joints):
        R, t = local[j]
        if par < 0:
            out.append((R, t))
        else:
            PR, pt = out[par]
            out.append(([[sum(PR[i][k] * R[k][m] for k in range(3)) for m in range(3)] for i in range(3)],
                        tuple(pt[i] + sum(PR[i][k] * t[k] for k in range(3)) for i in range(3))))
    return out


def skinned(bh, tg, raw, points, skins, base):
    """Where each point goes (cm): its mesh's targets, then its joints (pose world x neutral world
    inverse). points: rest positions; skins: [(joint, weight)]; base: the targets' meshes and
    their first point."""
    c, local = evaluate(bh, raw)
    W = world(bh, local); W0 = world(bh, evaluate(bh, {})[1])
    p = [list(x) for x in points]
    for m, first in base.items():
        for ch, ds in tg.get(m, []):
            w = c[bh.channel_control[ch]]
            if w == 0:
                continue
            for v, d in ds:
                for i in range(3):
                    p[first + v][i] += w * d[i]
    out = []
    for x, sk in zip(p, skins):
        q = [0.0, 0.0, 0.0]
        for j, w in sk:
            R, t = W[j]; R0, t0 = W0[j]
            # neutral world inverse: R0^T (x - t0)
            y = [sum(R0[k][i] * (x[k] - t0[k]) for k in range(3)) for i in range(3)]
            for i in range(3):
                q[i] += w * (t[i] + sum(R[i][k] * y[k] for k in range(3)))
        out.append(q)
    return out


def pack(bh, tg, meshes, mesh_index, vertex_points):
    """The block the lab reads (little-endian, 4-byte aligned):

      FRG1, u32 controls, raws, psds, groups, joints, points, vertices, targets, expressions
      psds        out, weight (f32), n, n inputs
      groups      rows, cols, cols inputs, rows outputs, rows x cols values (f32)
      joints      parent (i32, -1 none), t xyz (cm), r xyz (degrees)
      points      rest xyz (cm), normal xyz, INFLUENCES x (joint u16, pad u16, weight f32)
      vertices    per face vertex of the pack (in order), its point (u32)
      targets     control, first point, n, n x (point u16, dx dy dz i16 in DELTA_UNIT cm)
      expressions name (16 bytes), n, n x (control u32, value f32)

    meshes: [(name, points, normals, skins)]; mesh_index: DNA mesh index by name;
    vertex_points: per face vertex of the pack, its point (in the meshes' concatenation)."""
    b = bytearray(MAGIC)
    first, at = {}, 0
    for name, pts, _, _ in meshes:
        first[mesh_index[name]] = at; at += len(pts)
    tlist = []
    for name, pts, _, _ in meshes:
        m = mesh_index[name]
        for ch, ds in tg.get(m, []):
            if ds:
                tlist.append((bh.channel_control[ch], first[m], ds))
    exprs = [(n, {bh.control(k): v for k, v in e.items()}) for n, e in EXPRESSIONS.items()]
    b += struct.pack("<9I", bh.controls, len(bh.raw), len(bh.psds), len(bh.groups), len(bh.joints), at,
                     len(vertex_points), len(tlist), len(exprs))
    for out, w, ins in bh.psds:
        b += struct.pack(f"<IfI{len(ins)}I", out, w, len(ins), *ins)
    for g in bh.groups:
        b += struct.pack(f"<II{len(g['in'])}I{len(g['out'])}I{len(g['values'])}f", len(g["out"]), len(g["in"]),
                         *g["in"], *g["out"], *g["values"])
    for _, par, t, r in bh.joints:
        b += struct.pack("<i6f", par, *t, *r)
    for _, pts, nrm, sks in meshes:
        for x, n, sk in zip(pts, nrm, sks):
            top = sorted(sk, key=lambda jw: -jw[1])[:INFLUENCES]
            tot = sum(w for _, w in top) or 1.0
            top = [(j, w / tot) for j, w in top] + [(0, 0.0)] * (INFLUENCES - len(top))
            b += struct.pack("<6f", *x, *n)
            for j, w in top:
                b += struct.pack("<HHf", j, 0, w)
    b += struct.pack(f"<{len(vertex_points)}I", *vertex_points)
    for ctl, f0, ds in tlist:
        b += struct.pack("<III", ctl, f0, len(ds))
        for v, d in ds:
            q = [max(-32767, min(32767, round(x / DELTA_UNIT))) for x in d]
            b += struct.pack("<H3h", v, *q)
    for name, e in exprs:
        b += struct.pack(f"<16sI", name.encode()[:16], len(e))
        for k, v in sorted(e.items()):
            b += struct.pack("<If", k, v)
    return bytes(b)
