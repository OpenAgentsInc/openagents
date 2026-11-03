"""Her figure: a soft sculpt of the MetaHuman body's trunk before anything is fitted to it.

The export's body is Creator's medium build: a waist nearly as wide as the ribs, a belly that
stands 15 cm in front of the spine and flat glutes, which read heavy and older under fitted
cloth. figure() moves the trunk's vertices (both the DNA's bind and the arms-down pose the
clothes were cut on, vertex for vertex, so the joint fits between them don't change) by smooth
bands along the height, each measured from the spine:

  - the belly drawn back (its front's distance from the spine), most at the navel;
  - the waist drawn in at its narrowest, the hips let out a little below it;
  - the glutes let out behind.

The arms, hands, neck, head and legs below the hips don't move (a vertex moves by its share of
trunk and hip weight). The portrait's garments (cut on the old figure) follow the body points
near them (moved()), so they keep their distance from the skin. Numbers are centimetres in the
DNA's space: y up from the floor, z forward, x the figure's left.
"""
import math
from pathlib import Path

# Bands along the height: (centre y, half-width) and how much. Measured on the export's body:
# navel 100, natural waist 107, widest hips 77, glutes 84.
BELLY = (100.0, 16.0, 0.45)    # the front's distance from the spine drawn in by up to 45%
WAIST = (106.0, 11.0, 0.13)    # the half-width drawn in by up to 13%
HIPS = (78.0, 8.0, 0.035)      # and let out by up to 3.5% at the widest hips
GLUTES = (84.0, 10.0, 0.16)    # the back's distance behind the spine let out by up to 16%
TRUNK = ("pelvis", "spine", "thigh")
OUT_OF = ("upperarm", "lowerarm", "hand", "index", "middle", "ring", "pinky", "thumb", "clavicle", "neck", "head")


def bump(y, c, h):
    """A smooth band: 1 at c, 0 past c +- h (a raised cosine)."""
    t = abs(y - c) / h
    return 0.5 + 0.5 * math.cos(math.pi * t) if t < 1.0 else 0.0


def share(w, names, lower):
    return sum(v for k, v in w.items() if lower[k].split("_")[0] in names)


def offset(x, spine_z):
    """The move of a trunk point at x (cm) by the bands."""
    px, py, pz = x
    dz, dx = 0.0, 0.0
    # in front of the spine: the belly drawn back, most along the middle (a point near the side
    # is the waist's, which the width band shapes)
    front = pz - spine_z
    if front > 0.0:
        mid = max(0.0, 1.0 - (px / 16.0) ** 2)
        dz -= front * BELLY[2] * bump(py, BELLY[0], BELLY[1]) * mid
    else:
        dz -= front * GLUTES[2] * bump(py, GLUTES[0], GLUTES[1])
    dx -= px * WAIST[2] * bump(py, WAIST[0], WAIST[1])
    dx += px * HIPS[2] * bump(py, HIPS[0], HIPS[1])
    return (dx, 0.0, dz)


def deltas(xs, ws, lower, spine_z):
    out = []
    for x, w in zip(xs, ws):
        k = share(w, TRUNK, lower) * (1.0 - min(1.0, share(w, OUT_OF, lower) * 2.0))
        d = offset(x, spine_z)
        out.append(tuple(c * k for c in d))
    return out


def normals(vs, vns, faces):
    """Each obj normal index's normal from its positions' faces (area weighted)."""
    acc = [[0.0, 0.0, 0.0] for _ in vns]
    for f in faces:
        for k in range(1, len(f) - 1):
            a, b, c = vs[f[0][0]], vs[f[k][0]], vs[f[k + 1][0]]
            e1 = [b[i] - a[i] for i in range(3)]; e2 = [c[i] - a[i] for i in range(3)]
            n = (e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0])
            for (_, _, ni) in (f[0], f[k], f[k + 1]):
                for i in range(3):
                    acc[ni][i] += n[i]
    out = []
    for a, old in zip(acc, vns):
        l = math.sqrt(sum(c * c for c in a))
        out.append(tuple(c / l for c in a) if l > 1e-12 else old)
    return out


def write_obj(src, dst, vs, vns):
    """src's obj with its positions and normals replaced (faces and uvs as they were)."""
    lines, vi, ni = [], 0, 0
    for line in Path(src).read_text().splitlines():
        if line.startswith("v "):
            lines.append("v %.5f %.5f %.5f" % vs[vi]); vi += 1
        elif line.startswith("vn "):
            lines.append("vn %.5f %.5f %.5f" % vns[ni]); ni += 1
        else:
            lines.append(line)
    Path(dst).write_text("\n".join(lines) + "\n")


def figure(body, down_path, weights, lower, spine_z, obj, out):
    """Writes the sculpted body (its bind mesh, skin and joints beside it, and the arms-down mesh)
    into out; returns out, the arms-down mesh's path there and each vertex's move."""
    out.mkdir(parents=True, exist_ok=True)
    vb, _, vnb, fb = obj(body / "body_lod0_mesh.obj")
    d = deltas(vb, weights, lower, spine_z)
    moved_b = [tuple(a + b for a, b in zip(x, dd)) for x, dd in zip(vb, d)]
    write_obj(body / "body_lod0_mesh.obj", out / "body_lod0_mesh.obj", moved_b, normals(moved_b, vnb, fb))
    for name in ("body_lod0_mesh.skin.txt", "joints.txt"):
        (out / name).write_text((body / name).read_text())
    down_out = out / "body_down_lod0_mesh.obj"
    if down_path.exists():
        vd, _, vnd, fd = obj(down_path)
        moved_d = [tuple(a + b for a, b in zip(x, dd)) for x, dd in zip(vd, d)]
        write_obj(down_path, down_out, moved_d, normals(moved_d, vnd, fd))
    far = max(math.sqrt(sum(c * c for c in dd)) for dd in d)
    print(f"figure: the trunk sculpted, {sum(1 for dd in d if any(abs(c) > 0.05 for c in dd))} of {len(d)} body points moved, up to {far:.1f} cm")
    return out, down_out, d


def moved(points, body_x, d, near, reach=8.0):
    """Garment points (cm, the arms-down pose) moved as the body points near them moved: a
    Gaussian over those within reach, so cloth keeps its distance from the skin."""
    out = []
    for x in points:
        found = near.within(x, reach)
        if not found:
            out.append(x); continue
        found.sort()
        s = max(1.0, found[0][0])
        acc, tot = [0.0, 0.0, 0.0], 0.0
        for dist, i in found[:32]:
            f = math.exp(-(dist / (1.5 * s)) ** 2)
            tot += f
            for c in range(3):
                acc[c] += f * d[i][c]
        out.append(tuple(x[c] + acc[c] / tot for c in range(3)) if tot > 1e-9 else x)
    return out
