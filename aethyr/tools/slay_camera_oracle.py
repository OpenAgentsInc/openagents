# Firm reason: 2 (independent SDK-channel and matrix oracle for the packed source camera).
"""Read private SDK exports; compare every SHS1 camera without importing the packer."""
import json
import math
from pathlib import Path
import struct
import sys

sdk, template_path, shot_path = map(Path, sys.argv[1:4])
outer = json.loads((sdk / 'tf0290-shot-samples.json').read_text())
template = json.loads(template_path.read_text())
assert outer['frames'] == template['frames'] == 200
assert outer['start'] == template['source_first'] == 1001
assert template['blend_type'] == 8  # SDK AdditiveFromBase
assert len(template['inner_ticks']) == 200
assert all(a < b for a, b in zip(template['inner_ticks'], template['inner_ticks'][1:]))
channels = next(t for b in outer['bindings'] if b['name'] == 'TF0290_01_Camera'
                for t in b['tracks'] if t['name'] == 'Transform')['sections'][0]['channels']
base = {c['name']: c['values'] for c in channels}
shake = {c['name']: c for c in template['channels']}
scales = {s['type']: s['values'] for s in template['scales'] if s['property'] == 'Transform'}
assert len(base) == len(shake) == 9 and len(scales) == 2
assert all(len(c['values']) == 200 for c in template['channels'])
assert all(len(s) == 200 for s in scales.values())
f32 = lambda x: struct.unpack('<f', struct.pack('<f', x))[0]
shot = shot_path.read_bytes()
header = struct.unpack_from('<5If7I', shot)
assert header[0] == 0x31534853 and header[1] == 2 and header[2] == len(shot)
assert header[3] == 200 and header[4] == 135 and header[5] == 24
data, stride = header[7:9]


def multiply(a, b):
    return [[sum(a[r][k] * b[k][c] for k in range(3)) for c in range(3)] for r in range(3)]


def basis(roll, pitch, yaw):
    r, p, y = map(math.radians, (-roll, -pitch, yaw))
    rx = [[1, 0, 0], [0, math.cos(r), -math.sin(r)], [0, math.sin(r), math.cos(r)]]
    ry = [[math.cos(p), 0, math.sin(p)], [0, 1, 0], [-math.sin(p), 0, math.cos(p)]]
    rz = [[math.cos(y), -math.sin(y), 0], [math.sin(y), math.cos(y), 0], [0, 0, 1]]
    matrix = multiply(multiply(rz, ry), rx)
    # SHS1 stores right, up, forward, reflecting native Z-up into Y-up.
    return [matrix[row][col] for col in (1, 2, 0) for row in (0, 2, 1)]


max_position = max_basis = max_offset = 0.0
for frame in range(200):
    composed = []
    for group in ('Location', 'Rotation'):
        scale = scales['TransformProperty' + group + 'Only'][frame]
        for axis in 'XYZ':
            key = group + '.' + axis
            channel = shake[key]
            # Native property scaling is relative to the channel's base value;
            # the piecewise additive blender then removes that base value.
            origin = channel['base']
            scaled = (channel['values'][frame] - f32(origin)) * scale + f32(origin)
            delta = scaled - origin
            max_offset = max(max_offset, abs(delta))
            composed.append(f32(f32(base[key][frame]) + delta))
    eye = [f32(composed[k] * f32(.01)) for k in (0, 2, 1)]
    expected = basis(*composed[3:6])
    actual = struct.unpack_from('<12f', shot, data + frame * stride + 135 * 28)
    position_error = max(abs(a-b) for a, b in zip(actual[:3], eye))
    basis_error = max(abs(a-b) for a, b in zip(actual[3:12], expected))
    assert position_error <= 1e-6, ('camera position mismatch', frame, position_error)
    assert basis_error <= 2e-6, ('camera basis mismatch', frame, basis_error)
    max_position = max(max_position, position_error)
    max_basis = max(max_basis, basis_error)
assert max_offset > 0, 'fixture must contain authored motion'
print(json.dumps({'source_camera_frames': 200, 'sdk_template_channels': 9,
                  'sdk_property_scales': 2, 'max_position_error_m': max_position,
                  'max_basis_component_error': max_basis,
                  'basis_tolerance': 2e-6, 'position_tolerance_m': 1e-6,
                  'exact_sdk_time_transform_samples': len(template['inner_ticks'])}))
