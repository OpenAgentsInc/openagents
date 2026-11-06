# Firm reason: 2 (independent stdlib/Pillow oracle for source triangle bytes, VPK layout and Unreal transforms).
"""Usage: SOURCE_DIR PACK_DIR COAX_POSE_JSON. Reads private bytes; emits metadata only."""
import hashlib
import json
from pathlib import Path
import struct
import sys
import math
from PIL import Image

source, packed, pose_file = map(Path, sys.argv[1:])
doc = json.loads((source/'canyon.gltf').read_text())
raw = (source/'canyon.bin').read_bytes()
vertex_raw = (packed/'canyon.vertices.bin').read_bytes()
index_raw = (packed/'canyon.indices.bin').read_bytes()
vertex_count = len(vertex_raw)//48
index_count = len(index_raw)//4
primitive = doc['meshes'][0]['primitives'][0]
def accessor(index):
    a = doc['accessors'][index]
    v = doc['bufferViews'][a['bufferView']]
    return v.get('byteOffset', 0)+a.get('byteOffset', 0)
source_index_offset = accessor(primitive['indices'])
position_offset = accessor(primitive['attributes']['POSITION'])
normal_offset = accessor(primitive['attributes']['NORMAL'])
uv_offset = accessor(primitive['attributes']['TEXCOORD_0'])
paint_offset = accessor(primitive['attributes']['COLOR_0'])
assert index_count == 2999985 and vertex_count == 504367
for k in range(index_count):
    source_index = struct.unpack_from('<I', raw, source_index_offset+k*4)[0]
    index = struct.unpack_from('<I', index_raw, k*4)[0]
    assert index < vertex_count
    expected = (raw[position_offset+source_index*12:position_offset+source_index*12+12]
              + raw[normal_offset+source_index*12:normal_offset+source_index*12+12]
              + raw[uv_offset+source_index*8:uv_offset+source_index*8+8]
              + raw[paint_offset+source_index*4:paint_offset+source_index*4+4])
    assert vertex_raw[index*48:index*48+36] == expected, k
pack = (packed/'canyon.pack').read_bytes()
assert pack[:4] == b'VPK1' and struct.unpack_from('<I', pack, 8)[0] == len(pack)
entries = []
for k in range(struct.unpack_from('<I', pack, 4)[0]):
    at = 32+k*48
    kind, offset, size = struct.unpack_from('<3I', pack, at)
    name = pack[at+16:at+48].split(b'\0')[0].decode()
    payload = pack[offset:offset+size]
    assert len(payload) == size
    if kind == 2: assert payload == vertex_raw
    if kind == 3: assert payload == index_raw
    if kind == 4:
        filename = {'albedo': 'Albedo.png', 'roughness': 'Roughness.png'}[name]
        assert payload == (source/filename).read_bytes()
        assert Image.open(source/filename).size == (4096, 4096)
    entries.append(dict(kind=kind, name=name, bytes=size, sha256=hashlib.sha256(payload).hexdigest()))
assert [e['kind'] for e in entries] == [1, 2, 3, 4, 4]
receipt = json.loads((source/'placement.json').read_text())
words = json.loads(pose_file.read_text())['pose_words']
values = struct.unpack('<48f', struct.pack('<48I', *words))
origin = receipt['instances'][0]['translation_cm']
max_sample_error = max_geometry_pose_error = max_normal_error = 0.0
def apply(rows, p, translate=False):
    return tuple(row[0]*p[0]+row[1]*p[1]+row[2]*p[2]+(row[3] if translate else 0) for row in rows)
def normalized(n):
    length = math.sqrt(sum(x*x for x in n))
    return tuple(v/length for v in n)
for i, instance in enumerate(receipt['instances']):
    matrix = [values[i*24+j*4:i*24+j*4+4] for j in range(3)]
    normal_matrix = [values[i*24+12+j*4:i*24+12+j*4+4] for j in range(3)]
    for sample in instance['sdk_samples']:
        x,y,z = sample['local_cm']; local = (x/100,z/100,y/100)
        w = sample['world_cm']; expected = ((w[0]-origin[0])/100,(w[2]-origin[2])/100,(w[1]-origin[1])/100)
        actual = apply(matrix, local, True)
        max_sample_error = max(max_sample_error, *(abs(a-b) for a,b in zip(actual,expected)))
    qx,qy,qz,qw = instance['quaternion_xyzw']
    rotation = ((1-2*(qy*qy+qz*qz),2*(qx*qy-qz*qw),2*(qx*qz+qy*qw),0),
                (2*(qx*qy+qz*qw),1-2*(qx*qx+qz*qz),2*(qy*qz-qx*qw),0),
                (2*(qx*qz-qy*qw),2*(qy*qz+qx*qw),1-2*(qx*qx+qy*qy),0))
    scale = instance['scale']; translation = instance['translation_cm']
    for k in range(vertex_count):
        x,y,z,nx,ny,nz,u,v = struct.unpack_from('<8f',vertex_raw,k*48)
        assert all(math.isfinite(t) for t in (x,y,z,nx,ny,nz,u,v))
        unreal = apply(rotation,(x*100*scale[0],z*100*scale[1],y*100*scale[2]))
        delta = tuple((unreal[j]+translation[j]-origin[j])/100 for j in range(3))
        expected = (delta[0],delta[2],delta[1]); actual = apply(matrix,(x,y,z),True)
        max_geometry_pose_error = max(max_geometry_pose_error, *(abs(a-b) for a,b in zip(actual,expected)))
        un = apply(rotation,(nx/scale[0],nz/scale[1],ny/scale[2]))
        expected_n = normalized((un[0],un[2],un[1])); actual_n = normalized(apply(normal_matrix,(nx,ny,nz)))
        max_normal_error = max(max_normal_error, *(abs(a-b) for a,b in zip(actual_n,expected_n)))
assert max_sample_error < 0.000002, max_sample_error
assert max_geometry_pose_error < 0.000002, max_geometry_pose_error
assert max_normal_error < 0.000001, max_normal_error
print(json.dumps(dict(source_triangles_per_instance=999995, section_triangles=1999990,
    gpu_vertices=504367, source_corner_attributes='all 2,999,985 indexed corner records bit-exact',
    winding='all original source triangle orders preserved', texture_maps='both original 4096x4096 PNGs byte-exact',
    sdk_sample_error_m=max_sample_error, all_vertex_pose_error_m=max_geometry_pose_error,
    all_vertex_normal_error=max_normal_error, pack_bytes=len(pack), pack_sha256=hashlib.sha256(pack).hexdigest(),
    entries=entries), indent=2))
