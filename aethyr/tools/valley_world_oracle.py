# Firm reason: 2 (independent stdlib/Pillow oracle for SDK geometry, placements and resized source pixels).
"""SOURCE_DIR PACK_DIR ORIGINAL_ECHO_PACK OUTPUT_JSON; no source project code is executed."""
import hashlib
import io
import json
import math
import mmap
import struct
import sys
from pathlib import Path
from PIL import Image, ImageChops

source, output, original, receipt = map(Path, sys.argv[1:])
world = json.loads((source / 'world.json').read_text())
materials = json.loads((source / 'materials.json').read_text())
component_flags = {(r['actor'], r['component']): r for r in json.loads((source / 'component-flags.json').read_text())['components']}
material_ids = {m['path']: k+1 for k, m in enumerate(materials['materials'])}
opened = []

def mapped(path):
    f = path.open('rb')
    b = mmap.mmap(f.fileno(), 0, access=mmap.ACCESS_READ)
    opened.extend([f, b])
    return b

def table(b):
    magic, count, size = struct.unpack_from('<III', b)
    assert magic == 0x314B5056 and size == len(b)
    rows = [struct.unpack_from('<III', b, 32 + k * 48) for k in range(count)]
    assert all(at >= 32 + count * 48 and at + n <= size for _, at, n in rows)
    return rows

pack = mapped(output / 'valley-world.pack')
old = mapped(original)
entries, previous = table(pack), table(old)
assert [e[0] for e in entries[:3]] == [1, 2, 3]
cat, vertices, indices = [memoryview(pack)[at:at+n] for _, at, n in entries[:3]]
assert cat[:10142528] == memoryview(old)[previous[0][1]:previous[0][1]+10142528]
assert vertices[:77954848] == memoryview(old)[previous[1][1]:previous[1][1]+77954848]
assert indices[:29710980] == memoryview(old)[previous[2][1]:previous[2][1]+29710980]
header = struct.unpack_from('<8I', cat, 10142528)
assert header[:3] == (0x31445756, 1, 27501)
assert header[3:8] == (len(vertices), len(indices), len(entries)-3, 257, 66049)
vertex_word, matrix_word, material_word = struct.unpack_from('<III', cat, 10142528+44)
assert vertex_word == 77954848 // 4

def accessor(g, number, binary):
    a = g['accessors'][number]
    v = g['bufferViews'][a['bufferView']]
    assert 'byteStride' not in v
    width = {'SCALAR': 1, 'VEC2': 2, 'VEC3': 3, 'VEC4': 4}[a['type']]
    size = {5121: 1, 5123: 2, 5125: 4, 5126: 4}[a['componentType']]
    offset = v.get('byteOffset', 0) + a.get('byteOffset', 0)
    return memoryview(binary)[offset:offset+a['count']*size*width], a['count'], a['componentType']

mesh_ranges = {}
vertex_cursor = 0
index_cursor = 29710980 // 4
unique_triangles = 0
for mesh in world['meshes']:
    number = len(mesh_ranges)
    gltf_path = source / mesh.get('gltf', f'mesh{number:03d}.gltf')
    g = json.loads(gltf_path.read_text())
    assert g['nodes'] == [{'mesh': 0}], 'Unapplied SDK node transform'
    binary = mapped(gltf_path.parent / g['buffers'][0]['uri'])
    low, high = [math.inf]*3, [-math.inf]*3
    mesh_first = index_cursor
    for pr in g['meshes'][0]['primitives']:
        p, nv, _ = accessor(g, pr['attributes']['POSITION'], binary)
        n, nn, _ = accessor(g, pr['attributes']['NORMAL'], binary)
        uv, nuv, _ = accessor(g, pr['attributes']['TEXCOORD_0'], binary)
        ix, ni, kind = accessor(g, pr['indices'], binary)
        assert nv == nn == nuv and ni % 3 == 0
        slot = pr['material']
        material = material_ids.get(mesh['materials'][slot], 0)
        for k in range(nv):
            at = vertex_word*4 + (vertex_cursor+k)*36
            assert vertices[at:at+12] == p[k*12:k*12+12]
            assert vertices[at+12:at+24] == n[k*12:k*12+12]
            assert vertices[at+24:at+32] == uv[k*8:k*8+8]
            assert struct.unpack_from('<I', vertices, at+32)[0] == material | (slot << 16)
            point = struct.unpack_from('<3f', p, k*12)
            for axis in range(3):
                low[axis], high[axis] = min(low[axis], point[axis]), max(high[axis], point[axis])
        for k, (value,) in enumerate(struct.iter_unpack({5121: '<B', 5123: '<H', 5125: '<I'}[kind], ix)):
            assert struct.unpack_from('<I', indices, (index_cursor+k)*4)[0] == 2000000+vertex_cursor+value
        vertex_cursor += nv
        index_cursor += ni
        unique_triangles += ni//3
    mesh_ranges[mesh['path']] = (mesh_first, index_cursor-mesh_first, low, high)
assert (vertex_word*4 + vertex_cursor*36 + 15)//16*16 == matrix_word*4
assert index_cursor*4 == len(indices)

def native_matrix(instance):
    x, y, z, w = instance['quaternion_xyzw']
    r = [[1-2*y*y-2*z*z, 2*x*y-2*z*w, 2*x*z+2*y*w],
         [2*x*y+2*z*w, 1-2*x*x-2*z*z, 2*y*z-2*x*w],
         [2*x*z-2*y*w, 2*y*z+2*x*w, 1-2*x*x-2*y*y]]
    axes = [0, 2, 1]
    origin = [-2341.011254092498, -453.31197018573005, -1130.1458705213108]
    scales = instance['scale']
    t = [(instance['translation_cm'][a]-origin[a])/100 for a in axes]
    m = [[r[a][b]*scales[b] for b in axes] + [t[k]] for k, a in enumerate(axes)]
    normal = [[r[a][b]/scales[b] for b in axes] + [0] for a in axes]
    return m, normal

matrix_error = 0
point_bound = 0
submitted_triangles = 0
draw_start = 10142528 + 64 + 66049*4
assert len(world['instances']) == 27501
for k, instance in enumerate(world['instances']):
    first, count, low, high = mesh_ranges[instance['mesh']]
    assert struct.unpack_from('<4I', cat, draw_start+k*32) == (first, count, k+3, 1)
    m, normal = native_matrix(instance)
    overrides = component_flags[(instance['actor'], instance['component'])]['overrides']
    expected_override = material_ids.get(overrides[0], 0) if overrides else 0
    assert struct.unpack_from('<I', vertices, matrix_word*4+k*96+60)[0] == expected_override
    got = [list(struct.unpack_from('<4f', vertices, matrix_word*4+k*96+row*16)) for row in range(6)]
    for row, expected in enumerate(m + normal):
        matrix_error = max(matrix_error, *(abs(got[row][c]-expected[c]) for c in range(4)))
    for corner in range(8):
        p = [high[a] if corner & (1 << a) else low[a] for a in range(3)]
        error = [sum((got[row][c]-m[row][c])*p[c] for c in range(3)) + got[row][3]-m[row][3] for row in range(3)]
        point_bound = max(point_bound, math.sqrt(sum(e*e for e in error)))
    submitted_triangles += count//3
assert matrix_error < 0.0001 and point_bound < 0.0002

heights = struct.unpack_from('<66049f', cat, 10142528+64)
assert all(math.isfinite(v) and -128 <= v <= 256 for v in heights)
assert len(entries)-3 == 32 + len(materials['textures'])
image_checks = 0
for layer, (_, at, size) in enumerate(entries[3:]):
    if layer < 32:
        _, original_at, original_size = previous[3+layer]
        encoded = old[original_at:original_at+original_size]
    else:
        encoded = (source / materials['textures'][layer-32]['file']).read_bytes()
    with Image.open(io.BytesIO(encoded)) as im:
        expected = im.convert('RGBA').resize((2048, 2048), Image.Resampling.LANCZOS)
    with Image.open(io.BytesIO(pack[at:at+size])) as im:
        actual = im.convert('RGBA')
    assert actual.size == (2048, 2048)
    assert ImageChops.difference(expected, actual).getbbox(alpha_only=False) is None, f'Layer {layer} differs from independent Pillow resize'
    image_checks += 1
    expected.close()
    actual.close()

report = {'schema': 'valley.world.independent-oracle.v1', 'source_actors': len(world['actors']),
          'meshes': len(mesh_ranges), 'instances': len(world['instances']), 'unique_vertices': vertex_cursor,
          'unique_fallback_triangles': unique_triangles, 'whole_scene_fallback_triangles': submitted_triangles,
          'geometry': 'every position/normal/UV/index matches SDK LOD0 fallback bytes',
          'original_echo_prefix': 'rig, geometry, indices and groom byte-exact',
          'matrix_max_absolute_error': matrix_error, 'affine_vertex_error_bound_metres': point_bound,
          'component_material_bindings': 'all original overrides checked independently',
          'derived_texture_layers_exact_against_pillow': image_checks,
          'ground_samples': len(heights), 'uncovered_samples': sum(v == -128 for v in heights),
          'pack_bytes': len(pack), 'pack_sha256': hashlib.sha256(pack).hexdigest(),
          'scope': 'source-data parity; does not establish Unreal image, Nanite, material-graph or triangle-collision parity'}
receipt.write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps(report, indent=2))
