# Firm reason: 2 (independent complete source-cluster ownership and Q16/F32 bounds oracle).
import hashlib
from pathlib import Path
import struct
import sys
import statistics

root = Path(sys.argv[1])
q = (root / 'valley-detail.page').read_bytes()
r = (root / 'valley-detail-reference.page').read_bytes()
t = (root / 'valley-detail.clusters').read_bytes()
qi = (root / 'valley-detail-spatial.page').read_bytes()
ri = (root / 'valley-detail-spatial-reference.page').read_bytes()
order = (root / 'valley-detail.order').read_bytes()
assert hashlib.sha256(q).hexdigest() == '8f7f401b6acf1dbf7a2a4e69ffff75bd03a4ca1e7340f164f2ef11008c45c1bd'
assert hashlib.sha256(r).hexdigest() == '652031658f97cc103a590f8c5aafa4cfa838f3626fe85517d09d1c5743576b17'
u32 = lambda b, p: struct.unpack_from('<I', b, p)[0]
magic, version, models, nn, instances, mo, no, io = struct.unpack_from('<8I', t)
assert (magic, version, models, instances, mo, no) == (0x31485756, 1, 1, 1, 32, 56)
assert len(t) == io + 52 == 108 + nn * 48 and len(t) <= 8388608
first, count, start, nodes, root_node, leaves = struct.unpack_from('<6I', t, 32)
assert (first, count, start, nodes, root_node) == (0, 5992848, 0, nn, nn-1)
assert nn == 2*leaves-1
seen = bytearray(count//3)
assert len(order) == len(seen)*4
for k in range(len(seen)):
    triangle = u32(order, k*4)
    assert triangle < len(seen) and not seen[triangle]
    seen[triangle] = 1
assert all(seen)
for profile, page, source_page, base in ((1, qi, q, 8000000), (2, ri, r, 12000000)):
    assert struct.unpack_from('<8I', page) == (0x31494456, 1, profile, base, count, 128, 128+count*4, count//3)
    assert len(page) == 128+count*4
    assert page[32:64] == hashlib.sha256(source_page).digest()
    assert page[64:96] == source_page[96:128] and page[96:128] == hashlib.sha256(order).digest()
assert struct.unpack_from('<I12f', t, io) == (0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0)
qr, rr = u32(q, 24), u32(r, 24)
low, high = struct.unpack_from('<3f', q, 32), struct.unpack_from('<3f', q, 44)
records = [struct.unpack_from('<4I6f2I', t, no+i*48) for i in range(nn)]
owners = [0] * nn
cursor = 0
max_vertices = max_triangles = 0
for i, node in enumerate(records):
    f, n, left, right, *rest = node
    lo, hi, height, verts = rest[:3], rest[3:6], rest[6], rest[7]
    assert n > 0 and n % 3 == 0 and 0 <= f <= count-n
    assert all(a <= b for a, b in zip(lo, hi))
    if i < leaves:
        assert f == cursor and n <= 372 and left == right == 0xffffffff and height == 0
        unique = set()
        for corner in range(f, f+n):
            old = u32(order, corner//3*4)*3+corner%3
            vertex = u32(ri, 128+corner*4)-12000000
            assert vertex == u32(r, rr+old*4)-12000000
            assert vertex == u32(qi, 128+corner*4)-8000000 == u32(q, qr+old*4)-8000000
            assert 0 <= vertex < 1012930
            unique.add(vertex)
            p = struct.unpack_from('<3f', r, 128+vertex*32)
            a, b = struct.unpack_from('<2I', q, 128+vertex*16)
            quant = (a & 65535, a >> 16, b & 65535)
            decoded = tuple(x+(y-x)*code/65535 for x, y, code in zip(low, high, quant))
            assert all(l <= x <= h and l <= y <= h for l, h, x, y in zip(lo, hi, p, decoded)), (i, corner)
        assert verts == len(unique) <= 64
        max_vertices = max(max_vertices, verts)
        max_triangles = max(max_triangles, n//3)
        cursor += n
    else:
        assert 0 <= left < i and 0 <= right < i and verts == 0
        a, b = records[left], records[right]
        assert a[0] == f and b[0] == f+a[1] and n == a[1]+b[1]
        assert height == 1+max(a[-2], b[-2])
        for child in (a, b):
            assert all(x <= y for x, y in zip(lo, child[4:7]))
            assert all(x >= y for x, y in zip(hi, child[7:10]))
        owners[left] += 1
        owners[right] += 1
assert cursor == count and records[-1][0:2] == (0, count)
assert owners[:-1] == [1] * (nn-1) and owners[-1] == 0
print(f'PASS exact ownership of all {count} original corners in {leaves} leaves / {nn} nodes; '
      f'max {max_vertices} vertices / {max_triangles} triangles; height {records[-1][-2]}')
print('PASS every leaf encloses every referenced F32 and Q16 position; every ancestor encloses both children')
print('PASS spatial indices permute each original triangle exactly once with original winding and corner attributes')
print(f'bytes={len(t)} sha256={hashlib.sha256(t).hexdigest()}')
for name, data in [('Q16 spatial index', qi), ('F32 spatial index', ri)]:
    print(f'{name} bytes={len(data)} sha256={hashlib.sha256(data).hexdigest()}')
root_box = records[-1]
volume = lambda row: (row[7]-row[4])*(row[8]-row[5])*(row[9]-row[6])
volumes = sorted(volume(row)/volume(root_box) for row in records[:leaves])
print(f'leaf/root AABB volume fraction: median={statistics.median(volumes):.6f} p90={volumes[int(leaves*.9)]:.6f}')
