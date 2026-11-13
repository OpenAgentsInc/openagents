# Firm reason: 2 (independent f64 topology, boundary and surface-distance oracle for Coax grid parents)
"""Run with SOURCE_VRO_DIRECTORY PAGE_DIRECTORY [FIELD_RECORD]; emits JSON, imports no project code."""
import hashlib
import json
from pathlib import Path
import struct
import sys
import math
from collections import Counter


def sub(a, b):
    return tuple(x - y for x, y in zip(a, b))


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def cross(a, b):
    return (a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0])


def distance(p, tri):
    a, b, c = tri
    ab, ac = sub(b, a), sub(c, a)
    n = cross(ab, ac)
    h = dot(sub(p, a), n) / dot(n, n)
    q = tuple(x-h*y for x, y in zip(p, n))
    v = sub(q, a)
    aa, bb, cc = dot(ab, ab), dot(ab, ac), dot(ac, ac)
    det = aa*cc-bb*bb
    u = (dot(v, ab)*cc-dot(v, ac)*bb)/det
    w = (dot(v, ac)*aa-dot(v, ab)*bb)/det
    if u >= 0 and w >= 0 and u+w <= 1:
        return math.sqrt(dot(sub(p, q), sub(p, q)))
    result = math.inf
    for x, y in ((a, b), (b, c), (c, a)):
        e = sub(y, x)
        t = max(0, min(1, dot(sub(p, x), e)/dot(e, e)))
        d = tuple(pv-xv-t*ev for pv, xv, ev in zip(p, x, e))
        result = min(result, math.sqrt(dot(d, d)))
    return result


def boundary(triangles):
    edges = Counter((a, b) for tri in triangles for a, b in zip(tri, tri[1:]+tri[:1]))
    for (a, b), count in edges.items():
        assert count == 1 and a != b
    result = {edge for edge in edges if edge[::-1] not in edges}
    vertices = {p for tri in triangles for p in tri}
    undirected = {frozenset(e) for e in edges}
    assert len(vertices)-len(undirected)+len(triangles) == 1
    return result


HASHES = {
    8: 'c39ef513e76cfb373abbb2185e35120cf4a02e13db13f80f92c82e05449bbf64',
    16: '3669c50bede0c4da9f611f4c658620ffc906ad6839984107838ab73aba37550b',
    32: '4e2053348a34e5a363ee5451b54412daba41bd41813c40d0cd6d8559f2ddd56c',
}
rows = []
for div, expected_hash in HASHES.items():
    raw = (Path(sys.argv[1])/f'rock-{div}.vro').read_bytes()
    assert hashlib.sha256(raw).hexdigest() == expected_hash
    magic, version, d, seed, nv, ni = struct.unpack_from('<6I', raw)
    assert (magic, version, d, seed) == (0x314F5256, 1, div, 12609)
    points = [struct.unpack_from('<3f', raw, 24+k*24) for k in range(nv)]
    indices = struct.unpack_from(f'<{ni}I', raw, 24+nv*24)
    chains = []
    for group in (0, 1):
        x0, y0 = div//2-4+group*4, div//2-2
        local = [points[(y0+y)*(div+1)+x0+x] for y in range(5) for x in range(5)]
        source_ids = [((y0+y)*div+x0+x)*2+t for y in range(4) for x in range(4) for t in range(2)]
        fine = [tuple(points[indices[t*3+c]] for c in range(3)) for t in source_ids]
        page = bytes.fromhex((Path(sys.argv[2])/f'grid-{div}-{group}.vgp.hex').read_text())
        magic, n, count, error = struct.unpack_from('<3If', page)
        assert (magic, n, count, len(page)) == (0x31504756, 21, 72, 556)
        p = [struct.unpack_from('<3f', page, 16+k*12) for k in range(n)]
        ids = struct.unpack_from('<72I', page, 16+n*12)
        assert all(i < n for i in ids)
        coarse = [tuple(p[ids[t*3+c]] for c in range(3)) for t in range(24)]
        assert set(p) == {v for k, v in enumerate(local) if k not in (6, 8, 16, 18)}
        assert boundary(fine) == boundary(coarse) and len(boundary(coarse)) == 16
        # Compare raw f32 words, not epsilon equality, at every retained point.
        for point in p:
            assert struct.pack('<3f', *point) in [struct.pack('<3f', *v) for v in local]
        exact = max(sum(abs(a-b) for a, b in zip(local[k], local[k-1])) for k in (6, 8, 16, 18))
        previous = struct.unpack('<f', struct.pack('<I', struct.unpack('<I', struct.pack('<f', error))[0]-1))[0]
        assert previous < exact <= error
        worst = []
        for src, dst in ((fine, coarse), (coarse, fine)):
            value = 0
            for tri in src:
                for u in range(9):
                    for v in range(9-u):
                        q = tuple(a*(1-(u+v)/8)+b*u/8+c*v/8 for a, b, c in zip(*tri))
                        value = max(value, min(distance(q, t) for t in dst))
            assert value <= error
            worst.append(value)
        chains.append([local[y*5+(4 if group == 0 else 0)] for y in range(5)])
        rows.append(dict(divisions=div, group=group, source_sha256=expected_hash,
                         page_sha256=hashlib.sha256(page).hexdigest(), triangles=[32, 24],
                         vertices=[25, 21], exact_boundary_segments=16, encoded_bytes=[700, 556],
                         resident_bytes=[1384, 1096], certified_error_metres=error,
                         sampled_distances_metres=worst, samples=[1440, 1080]))
    assert chains[0] == chains[1]
field_checks = []
if len(sys.argv) > 3:
    record = json.loads(Path(sys.argv[3]).read_text())
    assert record['version'] == 1 and record['seed'] == 12609 and len(record['words']) == 30
    def f32(x):
        return struct.unpack('<f', struct.pack('<f', x))[0]
    words = [struct.unpack('<f', struct.pack('<I', x))[0] for x in record['words']]
    for div in (8, 16):
        for group in (0, 1):
            page = bytes.fromhex((Path(sys.argv[2])/f'grid-{div}-{group}.vgp.hex').read_text())
            _, n, _, error = struct.unpack_from('<3If', page)
            coarse = [struct.unpack_from('<3f', page, 16+k*12) for k in range(n)]
            raw = (Path(sys.argv[1])/f'rock-{div}.vro').read_bytes()
            x0, y0 = div//2-4+group*4, div//2-2
            fine = [struct.unpack_from('<3f', raw, 24+((y0+y)*(div+1)+x0+x)*24) for y in range(5) for x in range(5)]
            for instance in range(6):
                x, y, z, yaw, scale = words[instance*5:instance*5+5]
                assert scale in (0.5, 1.0)
                c, sn = f32(math.cos(yaw)), f32(math.sin(yaw))
                operator = max(1, math.sqrt(c*c+sn*sn))
                rounding = 0
                for px, py, pz in fine+coarse:
                    ideal = ((px*c+pz*sn)*scale+x, py*scale+y, (pz*c-px*sn)*scale+z)
                    rounded = (f32(f32(f32(f32(px*c)+f32(pz*sn))*scale)+x), f32(f32(py*scale)+y), f32(f32(f32(f32(pz*c)-f32(px*sn))*scale)+z))
                    rounding = max(rounding, math.sqrt(dot(sub(ideal, rounded), sub(ideal, rounded))))
                bound = error*scale*operator+2*rounding
                padded = f32(f32(error*scale)+f32(0.00001))
                assert bound <= padded
                field_checks.append(dict(divisions=div, group=group, instance=instance, rounded_vertex_distance=rounding, affine_surface_bound=bound, padded_runtime_bound=padded))
print(json.dumps(dict(scope='independent f64 samples supplement the analytic barycentric certificate; all four mixed cuts preserve the same shared four-edge chain; field enclosure models f32 rigid transforms with exact binary scales', profiles=rows, field_checks=field_checks), indent=2))
