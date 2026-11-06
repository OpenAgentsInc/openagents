# Firm reason: 2 (independent glTF/SDK/binary oracle, never used by the product).
"""Compare every exported face sample and sparse morph against original SDK/glTF data."""
import json
import mmap
from pathlib import Path
import struct
import sys

source, sdk, packed = map(Path, sys.argv[1:4])
u32 = lambda b, at: struct.unpack_from('<I', b, at)[0]
f32 = lambda x: struct.unpack('<f', struct.pack('<f', x))[0]
name = lambda b, at: bytes(b[at:at+64]).split(b'\0', 1)[0].decode().casefold()
face = (packed / 'echo.face').read_bytes()
_, version, total, nt, nv, nd, nc, nb, nw, nj, tt, rn, bt, wt, ct, dt, data = struct.unpack_from('<17I', face)
assert version == 1 and total == len(face)
g = json.loads((source / 'Echo.gltf').read_text())
stream = (source / g['buffers'][0]['uri']).open('rb')
binary = mmap.mmap(stream.fileno(), 0, access=mmap.ACCESS_READ)

def accessor(index):
    a = g['accessors'][index]
    v = g['bufferViews'][a['bufferView']]
    assert a['componentType'] == 5126 and a['type'] == 'VEC3' and 'sparse' not in a
    assert v.get('byteStride', 12) == 12
    start = v.get('byteOffset', 0) + a.get('byteOffset', 0)
    return memoryview(binary)[start:start+a['count']*12]

cursor = 0
for target in range(nt):
    assert u32(face, tt+target*80+64) == cursor
    base = 0
    for primitive in g['meshes'][0]['primitives']:
        p = accessor(primitive['targets'][target]['POSITION'])
        n = accessor(primitive['targets'][target]['NORMAL'])
        assert len(p) == len(n)
        for vertex, (position, normal) in enumerate(zip(struct.iter_unpack('<3I', p), struct.iter_unpack('<3I', n))):
            words = position + normal
            if any(word & 0x7fffffff for word in words):
                assert struct.unpack_from('<8I', face, dt+cursor*32) == (base+vertex, target) + words
                cursor += 1
        base += len(p)//12
    assert base <= nv and cursor == u32(face, tt+target*80+64)+u32(face, tt+target*80+68)
assert cursor == nd

ranges = (packed / 'echo.morph-ranges').read_bytes()
morph = (packed / 'echo.morph').read_bytes()
seen = [0]*nv
for vertex, target, *words in struct.iter_unpack('<8I', face[dt:data]):
    first, count = struct.unpack_from('<2I', ranges, vertex*8)
    assert seen[vertex] < count
    assert struct.unpack_from('<7I', morph, (first+seen[vertex])*28) == (target, *words)
    seen[vertex] += 1
assert len(morph) == nd*28 and sum(seen) == nd
for vertex in range(nv):
    assert seen[vertex] == u32(ranges, vertex*8+4)

files = {p.name.casefold(): p for p in sdk.glob('*.samples.json')}
curve_names = [name(face, wt+i*68) for i in range(nw)]
bone_names = [name(face, bt+i*68) for i in range(nb)]
curve_samples = bone_samples = 0

def local(values):
    v = [0.0 if x == 0 else f32(x) for x in values]
    assert v[7:] == [1, 1, 1]
    out = [f32(v[0]*f32(.01)), f32(v[2]*f32(.01)), f32(v[1]*f32(.01)), -v[3], -v[5], -v[4], v[6]]
    # Decimal input canonicalizes signed zero before the coordinate reflection.
    return out

for clip in range(nc):
    at = ct+clip*80
    raw = json.loads(files[name(face, at)+'.samples.json'].read_text())
    frames, rate, weights, bones = struct.unpack_from('<IfII', face, at+64)
    assert frames == raw['frames'] and rate == raw['fps_numerator']/raw['fps_denominator']
    curves_by_name = {c['name'].casefold(): c['values'] for c in raw['curves']}
    bones_by_name = {b['name'].casefold(): b['values'] for b in raw['bones']}
    for frame in range(frames):
        for channel, key in enumerate(curve_names):
            assert face[weights+(frame*nw+channel)*4:weights+(frame*nw+channel+1)*4] == struct.pack('<f', curves_by_name[key][frame])
            curve_samples += 1
        for bone, key in enumerate(bone_names):
            expected = struct.pack('<7f', *local(bones_by_name[key][frame*10:(frame+1)*10]))
            at_bone = bones+(frame*nb+bone)*28
            assert struct.unpack_from("<7f", face, at_bone) == struct.unpack("<7f", expected), (clip, frame, key, struct.unpack_from("<7f",face,at_bone), struct.unpack("<7f",expected))
            bone_samples += 1

shot = (packed / 'echo.shot').read_bytes()
_, v, size, frames, bones, rate, names, start, stride, first, clip, face_first, face_last = struct.unpack_from('<5If7I', shot)
assert v == 1 and size == len(shot) and first == 1001 and clip == 16 and face_first == 1121 and face_last == 1159
raw = json.loads((sdk / 'TF0290_Echo.samples.json').read_text())
tracks = {b['name'].casefold(): b['values'] for b in raw['bones']}
for bone in range(bones):
    key = name(shot, names+bone*68).removeprefix('groom.')
    for frame in range(frames):
        at = start+frame*stride+bone*28
        assert struct.unpack_from('<7f', shot, at) == tuple(local(tracks[key][frame*10:(frame+1)*10]))

print(json.dumps({'source_morph_deltas_bit_exact': nd, 'gpu_csr_records_bit_exact': nd,
                  'source_face_clips': nc, 'curve_samples_bit_exact': curve_samples,
                  'facial_local_samples_exact_values': bone_samples,
                  'shot_body_local_samples_exact_values': frames*bones}))

# Optional actual Slay render extension: original UVs, sword and every source texture.
if len(sys.argv) > 4:
    import hashlib
    slay = Path(sys.argv[4])
    sg = json.loads((slay / 'SK_Echo.gltf').read_text())
    sb = (slay / sg['buffers'][0]['uri']).read_bytes()
    def dense(document, buffer, index):
        a = document['accessors'][index]
        v = document['bufferViews'][a['bufferView']]
        width = {'SCALAR': 1, 'VEC2': 2, 'VEC3': 3, 'VEC4': 4, 'MAT4': 16}[a['type']]
        size = {5121: 1, 5123: 2, 5125: 4, 5126: 4}[a['componentType']]
        start = v.get('byteOffset', 0) + a.get('byteOffset', 0)
        return buffer[start:start+a['count']*width*size]
    expected_uv = bytearray()
    for original, actual in zip(g['meshes'][0]['primitives'], sg['meshes'][0]['primitives']):
        for field in ['POSITION', 'NORMAL', 'TANGENT', 'JOINTS_0', 'WEIGHTS_0']:
            assert dense(g, binary, original['attributes'][field]) == dense(sg, sb, actual['attributes'][field])
        for before, after in zip(original['targets'], actual['targets']):
            for field in ['POSITION', 'NORMAL']:
                assert dense(g, binary, before[field]) == dense(sg, sb, after[field])
        expected_uv.extend(dense(sg, sb, actual['attributes']['TEXCOORD_0']))
    assert (packed / 'echo.slay-uv').read_bytes() == expected_uv
    sw = json.loads((slay / 'SK_Echo_Sword.gltf').read_text())
    swb = (slay / sw['buffers'][0]['uri']).read_bytes()
    sword = (packed / 'echo.sword-vertices').read_bytes()
    pr = sw['meshes'][0]['primitives'][0]
    for field, offset, width in [('POSITION',0,12), ('NORMAL',12,12), ('TEXCOORD_0',24,8), ('TANGENT',60,16)]:
        expected = dense(sw, swb, pr['attributes'][field])
        for vertex in range(25547):
            assert sword[vertex*96+offset:vertex*96+offset+width] == expected[vertex*width:(vertex+1)*width]
    assert all(struct.unpack_from('<I', sword, vertex*96+36)[0] == 135 and struct.unpack_from('<I', sword, vertex*96+52)[0] == 255 for vertex in range(25547))
    source_indices = dense(sw, swb, pr['indices'])
    assert (packed / 'echo.sword-indices').read_bytes() == b''.join(struct.pack('<I', x+900000) for (x,) in struct.iter_unpack('<H', source_indices))
    stream = (packed / 'valley-slay.pack').open('rb')
    pack = mmap.mmap(stream.fileno(), 0, access=mmap.ACCESS_READ)
    entries = [struct.unpack_from('<3I', pack, 32+i*48) for i in range(u32(pack,4))]
    _, start, count = entries[1]
    vb = memoryview(pack)[start:start+count]
    for vertex in range(101672):
        assert vb[24211664+vertex*96+24:24211664+vertex*96+32] == expected_uv[vertex*8:(vertex+1)*8]
    sword_base = u32(vb, 24209616+31*64)*4
    assert vb[sword_base:] == sword
    _, start, count = entries[2]
    assert pack[start+29710980:start+count] == (packed / 'echo.sword-indices').read_bytes()
    for index, (_, start, count) in enumerate(entries[3:]):
        label = bytes(pack[32+(index+3)*48+16:32+(index+3)*48+48]).split(b'\0',1)[0].decode()
        file = (source.parent / 'section-source' if index < 2 else slay) / label
        assert hashlib.sha256(pack[start:start+count]).digest() == hashlib.sha256(file.read_bytes()).digest()
    print(json.dumps({'slay_uv_pairs_bit_exact': 101672, 'sword_vertices_bit_exact': 25547,
                      'sword_indices_bit_exact': 134418, 'bundled_source_textures_bit_exact': len(entries)-3,
                      'slay_mesh_and_morph_geometry_equals_valley': True}))
