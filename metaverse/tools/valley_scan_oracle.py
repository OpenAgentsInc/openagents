# Firm reason: 2 (independent glTF matrix/Pillow oracle for the Coax scan importer).
"""Usage: python3 valley_scan_oracle.py PRIVATE_SOURCE_ROOT. Never writes asset bytes."""
import hashlib
import json
import math
from pathlib import Path
import struct
import sys
from PIL import Image

for source_id in ('uk4cdch', 'ubhvccfda'):
    root = Path(sys.argv[1]) / source_id
    doc = json.loads((root / f'{source_id}_tier_3.gltf').read_text())
    binary = (root / f'{source_id}_tier_3.bin').read_bytes()
    payload = (root / 'mesh.vsm').read_bytes()
    magic, vertices, index_count, version = struct.unpack_from('<4I', payload)
    assert (magic, version) == (0x314d5356, 1)
    assert len(payload) == 16 + vertices * 36 + index_count * 4
    primitive = doc['meshes'][0]['primitives'][0]
    def accessor(index):
        a = doc['accessors'][index]
        v = doc['bufferViews'][a['bufferView']]
        components = {'VEC2': 2, 'VEC3': 3, 'SCALAR': 1}[a['type']]
        code = {5126: 'f', 5123: 'H'}[a['componentType']]
        start = v.get('byteOffset', 0) + a.get('byteOffset', 0)
        return list(struct.iter_unpack('<' + code * components,
                    binary[start:start + a['count'] * components * struct.calcsize(code)]))
    positions = accessor(primitive['attributes']['POSITION'])
    normals = accessor(primitive['attributes']['NORMAL'])
    uv = accessor(primitive['attributes']['TEXCOORD_0'])
    indices = [r[0] for r in accessor(primitive['indices'])]
    assert len(positions) == vertices and len(indices) == index_count
    node = doc['nodes'][0]
    x, y, z, w = node['rotation']
    norm = math.sqrt(x*x+y*y+z*z+w*w)
    x, y, z, w = [v / norm for v in (x, y, z, w)]
    matrix = ((1-2*(y*y+z*z), 2*(x*y-z*w), 2*(x*z+y*w)),
              (2*(x*y+z*w), 1-2*(x*x+z*z), 2*(y*z-x*w)),
              (2*(x*z-y*w), 2*(y*z+x*w), 1-2*(x*x+y*y)))
    def rotate(p):
        return tuple(sum(a*b for a, b in zip(row, p)) for row in matrix)
    scaled = [tuple(v*s for v, s in zip(rotate(p), node['scale'])) for p in positions]
    low = [min(p[k] for p in scaled) for k in range(3)]
    high = [max(p[k] for p in scaled) for k in range(3)]
    offset = [(low[0]+high[0])/2, low[1], (low[2]+high[2])/2]
    image = Image.open(root / 'basecolor.jpg').convert('RGB')
    max_position_error = max_normal_error = max_color_error = 0.0
    for k, p in enumerate(scaled):
        actual = struct.unpack_from('<9f', payload, 16+k*36)
        expected = [v-o for v, o in zip(p, offset)]
        max_position_error = max(max_position_error, *(abs(a-b) for a, b in zip(actual[:3], expected)))
        n = rotate(normals[k]); length = math.sqrt(sum(v*v for v in n))
        max_normal_error = max(max_normal_error, *(abs(a-b/length) for a, b in zip(actual[3:6], n)))
        u, v = [t-math.floor(t) for t in uv[k]]
        color = image.getpixel((int(u*image.width), int(v*image.height)))
        max_color_error = max(max_color_error, *(abs(a-b/255) for a, b in zip(actual[6:], color)))
    actual_indices = struct.unpack_from('<' + str(index_count) + 'I', payload, 16+vertices*36)
    assert tuple(indices) == actual_indices
    assert max_position_error < 0.00001, max_position_error
    assert max_normal_error < 0.000001, max_normal_error
    assert max_color_error < 0.000001, max_color_error
    print(json.dumps(dict(id=source_id, vertices=vertices, triangles=index_count//3,
          sha256=hashlib.sha256(payload).hexdigest(), position_error_m=max_position_error,
          normal_error=max_normal_error, color_error=max_color_error,
          dimensions_m=[high[k]-low[k] for k in range(3)], indices='all exact',
          basecolor='all vertex samples match independent Pillow decoding')))
