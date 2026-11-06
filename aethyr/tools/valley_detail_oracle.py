# Firm reason: 2 (independent struct/math source-corner and compression oracle).
import hashlib
import json
import math
from pathlib import Path
import struct
import sys

root = Path(sys.argv[1])
g = json.loads((root / 'mesh003.gltf').read_text())
source = (root / g['buffers'][0]['uri']).read_bytes()
q = (root / 'valley-detail.page').read_bytes()
r = (root / 'valley-detail-reference.page').read_bytes()
u32 = lambda b, p: struct.unpack_from('<I', b, p)[0]
qh, rh = struct.unpack_from('<8I', q), struct.unpack_from('<8I', r)
assert qh[:3] == (0x31504456, 1, 1) and rh[:3] == (0x31504456, 1, 2)
assert qh[3:6] == rh[3:6] and qh[5] == 128
nv, ni = qh[3:5]
assert qh[6] == 128 + nv * 16 and rh[6] == 128 + nv * 32
assert qh[7] == len(q) <= 67108864 and rh[7] == len(r) <= 67108864
assert len(q[qh[6]:]) == len(r[rh[6]:]) == ni * 4
assert q[96:128] == r[96:128] == hashlib.sha256(source).digest()
assert q[32:72] == r[32:72] and q[84:96] == r[84:96]
assert r[72:84] == bytes(12)
pr = g['meshes'][0]['primitives'][0]
assert len(g['meshes'][0]['primitives']) == 1

def accessor(a):
    row = g['accessors'][a]
    view = g['bufferViews'][row['bufferView']]
    assert 'byteStride' not in view
    return view.get('byteOffset', 0) + row.get('byteOffset', 0), row['count']

po, source_nv = accessor(pr['attributes']['POSITION'])
no, _ = accessor(pr['attributes']['NORMAL'])
uv, _ = accessor(pr['attributes']['TEXCOORD_0'])
io, source_ni = accessor(pr['indices'])
assert source_ni == ni
view_s, view_r = memoryview(source), memoryview(r)
for corner in range(ni):
    vertex = u32(source, io + corner * 4)
    assert vertex < source_nv
    index = u32(r, rh[6] + corner * 4) - 12000000
    assert index == u32(q, qh[6] + corner * 4) - 8000000
    assert 0 <= index < nv
    at = 128 + index * 32
    assert view_r[at:at+12] == view_s[po+vertex*12:po+vertex*12+12]
    assert view_r[at+12:at+24] == view_s[no+vertex*12:no+vertex*12+12]
    assert view_r[at+24:at+32] == view_s[uv+vertex*8:uv+vertex*8+8]
print(f'PASS all {ni} original corners retain exact F32 position/normal/UV and source triangle order', flush=True)
low = struct.unpack_from('<3f', q, 32)
high = struct.unpack_from('<3f', q, 44)
ulo, uhi = struct.unpack_from('<2f', q, 56), struct.unpack_from('<2f', q, 64)
error, normal_bound, uv_bound = struct.unpack_from('<3f', q, 72)
max_p = max_n = max_uv = 0.0
for vertex in range(nv):
    a, b, normal, uvword = struct.unpack_from('<4I', q, 128 + vertex * 16)
    assert b >> 16 == 0
    original = struct.unpack_from('<8f', r, 128 + vertex * 32)
    quantized = (a & 65535, a >> 16, b & 65535)
    p = tuple(lo + (hi-lo) * code / 65535 for code, lo, hi in zip(quantized, low, high))
    distance = math.sqrt(sum((x-y)**2 for x, y in zip(p, original[:3])))
    assert distance <= error
    max_p = max(max_p, distance)
    x, y = (normal & 65535) / 65535 * 2 - 1, (normal >> 16) / 65535 * 2 - 1
    z = 1 - abs(x) - abs(y)
    t = max(0, -z)
    x += -t if x >= 0 else t
    y += -t if y >= 0 else t
    length = math.sqrt(x*x+y*y+z*z)
    old_length = math.sqrt(sum(n*n for n in original[3:6]))
    nerror = max(abs(n/length - old/old_length) for n, old in zip((x, y, z), original[3:6]))
    assert nerror <= normal_bound
    max_n = max(max_n, nerror)
    tex = tuple(lo+(hi-lo)*code/65535 for code, lo, hi in zip((uvword & 65535, uvword >> 16), ulo, uhi))
    uerror = max(abs(x-y) for x, y in zip(tex, original[6:]))
    assert uerror <= uv_bound
    max_uv = max(max_uv, uerror)
print(f'PASS {nv} compressed vertices: max position error={max_p:.9g}m <= {error:.9g}m; '
      f'normal component={max_n:.9g}; UV={max_uv:.9g}', flush=True)
for name, data in [('Q16', q), ('F32', r)]:
    print(f'{name} bytes={len(data)} sha256={hashlib.sha256(data).hexdigest()}')
