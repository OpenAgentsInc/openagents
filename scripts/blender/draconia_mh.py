"""Read the CC0 MakeHuman base mesh and its targets, for Draconia's build.

MakeHuman's assets (the hm08 base mesh, its modelling and expression targets)
are CC0 1.0 (makehuman/LICENSE.md, section C). Its program code is AGPL and
none of it is used here: this module is our own reader for two plain text
formats.

- `base.obj`: an OBJ whose vertex order is what every target indexes. Faces
  belong to groups: `body` is the skin; `helper-*` groups are fitting
  geometry (eyeballs, teeth, tongue, lash shells, a hair cap, tights and a
  skirt); `joint-*` groups are tiny cubes that mark joint centres.
- `*.target`: lines of `index dx dy dz`, offsets in the base mesh's units
  (decimetres), Y up, Z forward.

The source lives outside the repository; `draconia_fetch.sh` fetches the
pinned commit into `~/work/film-out/draconia/src/mh` (or set `MH_DATA`).
"""

import os

MH_COMMIT = "a8bc2d54ff0ac92e78ff71431b1023eda42bf482"
MH_DATA = os.environ.get(
    "MH_DATA",
    os.path.expanduser("~/work/film-out/draconia/src/mh/makehuman/data"),
)


class Base:
    """The base mesh: positions (dm), UVs, and faces with their group."""

    def __init__(self, path=None):
        path = path or os.path.join(MH_DATA, "3dobjs", "base.obj")
        self.co = []
        self.uv = []
        self.faces = []  # (vertex indices, uv indices, group)
        group = None
        with open(path) as f:
            for line in f:
                if line.startswith("v "):
                    _, x, y, z = line.split()[:4]
                    self.co.append([float(x), float(y), float(z)])
                elif line.startswith("vt "):
                    _, u, v = line.split()[:3]
                    self.uv.append((float(u), float(v)))
                elif line.startswith("g "):
                    group = line.split()[1]
                elif line.startswith("f "):
                    vs, ts = [], []
                    for token in line.split()[1:]:
                        parts = token.split("/")
                        vs.append(int(parts[0]) - 1)
                        ts.append(int(parts[1]) - 1 if len(parts) > 1 and parts[1] else -1)
                    self.faces.append((vs, ts, group))

    def apply(self, rel, weight=1.0):
        """Add a target (path relative to targets/) at `weight`."""
        if abs(weight) < 1e-6:
            return
        for i, d in read_target(rel):
            c = self.co[i]
            c[0] += d[0] * weight
            c[1] += d[1] * weight
            c[2] += d[2] * weight

    def group_vertices(self, prefix):
        out = set()
        for vs, _, g in self.faces:
            if g and g.startswith(prefix):
                out.update(vs)
        return out

    def centre(self, group):
        vs = self.group_vertices(group)
        n = len(vs)
        return [sum(self.co[i][k] for i in vs) / n for k in range(3)]


def read_target(rel):
    path = os.path.join(MH_DATA, "targets", rel)
    if not path.endswith(".target"):
        path += ".target"
    out = []
    with open(path) as f:
        for line in f:
            if not line.strip() or line.startswith("#"):
                continue
            p = line.split()
            out.append((int(p[0]), (float(p[1]), float(p[2]), float(p[3]))))
    return out


def target_delta(rel):
    """A target as {index: (dx, dy, dz)}."""
    return dict(read_target(rel))
