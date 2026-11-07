# Firm reason: 2 (independent triangle rasterization, capsule coverage and premultiplied UNORM composition for the actual Coax HUD, plus browser lifetime observations).
"""Compare every capture pixel with an independent HUD raster.

The prior world-only GPU image is a frozen base for the native composite;
world shading is not independently qualified by this HUD oracle. Browser
startup instead has an authored clear base and no area resources at all.
No masks or alignment shifts are used; the RGBA8 channel bound is two.
"""
import base64
import hashlib
import json
import math
from pathlib import Path
import struct
import sys


def capture(path):
    r = json.loads(path.read_bytes())
    assert r['format'] in ('rgba8unorm', 'bgra8unorm')
    width, height, row = r['width'], r['height'], r['rowBytes']
    data = base64.b64decode(r['data'], validate=True)
    assert row >= width * 4 and len(data) == row * height
    result = bytearray()
    order = (2, 1, 0, 3) if r['format'] == 'bgra8unorm' else (0, 1, 2, 3)
    for y in range(height):
        for x in range(width):
            result.extend(data[y * row + x * 4 + i] for i in order)
    return width, height, bytes(result)


def edge(a, b, p):
    return (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])


def top_left(a, b):
    return b[1] < a[1] or (b[1] == a[1] and b[0] > a[0])


def raster(base, width, height, vertices):
    assert len(base) == width * height * 4 and len(vertices) % 144 == 0
    data = list(struct.iter_unpack('<12f', vertices))
    half = lambda value: struct.unpack('<e', struct.pack('<e', value))[0]
    # The product composes in linear FP16, then converts to display UNORM
    # once. Each simulated attachment store rounds to IEEE binary16.
    result = [half(value / 255) for value in base]
    for start in range(0, len(data), 3):
        a, b, c = data[start:start + 3]
        area = edge(a, b, c)
        if area < 0:
            b, c, area = c, b, -area
        if area == 0:
            continue
        for y in range(max(0, math.floor(min(a[1], b[1], c[1]))), min(height, math.ceil(max(a[1], b[1], c[1])))):
            for x in range(max(0, math.floor(min(a[0], b[0], c[0]))), min(width, math.ceil(max(a[0], b[0], c[0])))):
                point = x + .5, y + .5
                sides = edge(b, c, point), edge(c, a, point), edge(a, b, point)
                edges = ((b, c), (c, a), (a, b))
                if not all(s > 0 or (s == 0 and top_left(*e)) for s, e in zip(sides, edges)):
                    continue
                weights = [s / area for s in sides]
                v = [sum(w * corner[k] for w, corner in zip(weights, (a, b, c))) for k in range(12)]
                alpha = v[6]
                if v[11] < 5.5:
                    along = max(-v[2], v[2] - v[4], 0)
                    distance = math.hypot(along, v[3])
                    t = min(1, max(0, (distance - (v[5] - .5)) / 1.1))
                    alpha *= 1 - t * t * (3 - 2 * t)
                at = (y * width + x) * 4
                for lane in range(4):
                    source = v[8 + lane] * alpha if lane < 3 else alpha
                    mixed = source + result[at + lane] * (1 - alpha)
                    result[at + lane] = half(mixed)
    return bytes(max(0, min(255, round(value * 255))) for value in result)


def compare(actual, expected):
    assert len(actual) == len(expected)
    differences = [abs(a - b) for a, b in zip(actual, expected)]
    assert max(differences, default=0) <= 2, (max(differences), sum(d > 2 for d in differences))
    return {'channels': len(actual), 'max_channel_error': max(differences), 'channels_different': sum(d > 0 for d in differences)}


def self_checks():
    # Hand-worked 2x2 quads: the diagonal belongs to exactly one triangle,
    # reversed winding gives the same coverage, and two translucent layers
    # retain the first layer under the second rather than overwriting it.
    def quad(color):
        points = ((0, 0), (2, 0), (2, 2), (0, 0), (2, 2), (0, 2))
        return b''.join(struct.pack('<12f', x, y, 0, 0, 0, 0, .5, 0, *color, 6) for x, y in points)
    black = bytes((0, 0, 0, 255)) * 4
    red, green = quad((1, 0, 0)), quad((0, 1, 0))
    assert raster(black, 2, 2, red) == bytes((128, 0, 0, 255)) * 4
    reversed_red = b''.join(red[i:i + 48] for i in (96, 48, 0, 240, 192, 144))
    assert raster(black, 2, 2, reversed_red) == bytes((128, 0, 0, 255)) * 4
    assert raster(black, 2, 2, red + green) == bytes((64, 128, 0, 255)) * 4
    assert raster(black, 2, 2, red[:48] * 3) == black
    rejected = 0
    for at in (0, 7, 15):
        changed = bytearray(black)
        changed[at] ^= 16
        try:
            compare(changed, black)
        except AssertionError:
            rejected += 1
    assert rejected == 3
    return {'hand_worked_raster_cases': 4, 'unmasked_channel_corruptions_rejected': rejected}


def inspect_native(output, prior):
    report = json.loads((output / 'receipt.json').read_bytes())
    assert report['schema'] == 'aethyr.stream-walk-native-gpu.v2'
    vertices = (output / 'before-hud.f32').read_bytes()
    assert vertices == (output / 'after-hud.f32').read_bytes()
    original = (prior / 'before.rgba').read_bytes()
    expected = raster(original, 320, 180, vertices)
    before, after = (output / 'before.rgba').read_bytes(), (output / 'after.rgba').read_bytes()
    assert before == after
    result = compare(before, expected)
    assert any(a != b for a, b in zip(before, original))
    return {'native_composite': result, 'vertices': len(vertices) // 48, 'full_recovery_equal': True,
            'image_sha256': hashlib.sha256(before).hexdigest(), 'independent_world_shader_reference': False}


def inspect_browser(output, native):
    report = json.loads((output / 'receipt.json').read_bytes())
    assert report['schema'] == 'aethyr.stream-walk-hud.v1'
    vertices = (native / 'before-hud.f32').read_bytes()
    width, height, initial = capture(output / 'initial.json')
    assert (width, height) in ((320, 180), (640, 360))
    expected = raster(bytes((8, 13, 20, 255)) * (width * height), width, height, vertices)
    initial_result = compare(initial, expected)
    before = capture(output / 'before.json')
    after = capture(output / 'after.json')
    end = capture(output / 'end.json')
    assert before == after and before[:2] == end[:2] == (width, height)
    assert before[2] != end[2]
    start = report['initial']
    assert start['stats']['drawnCommands'] == 0 and start['stats']['info'][8] == 0
    assert start['stats']['info'][14] == 0 and start['hud']['draws'] == 1
    for hud in (start['hud'], report['beforeHUD'], report['afterHUD'], report['endHUD']):
        assert 600 < hud['vertices'] < 2048 and hud['info'][2:] == [98304, 1, 1]
    assert report['afterHUD']['draws'] == report['beforeHUD']['draws'] + 1
    assert report['recovery']['requests'] == 0 and report['recovery']['epoch'] == 2
    assert len(report['frames']) == 3233 and report['frames'][0][0] == 0
    assert report['endHUD']['draws'] == 3234
    assert report['legs'][0]['last'][5] > 704 and report['legs'][1]['last'][5] < 64
    assert report['legs'][0]['cells'][0][0] == 0
    assert report['legs'][1]['cells'][0][2] > report['legs'][0]['cells'][0][2] > 0
    for leg in report['legs']:
        assert [c[3] for c in leg['cells']] == list(range(1, 13))
        assert leg['hud']['info'][3:] == [1, 1]
    cleanup = report['cleanup']
    assert all(cleanup['info'][i] == 0 for i in (4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 22, 23))
    assert cleanup['hud']['info'][3:] == [0, 0] and not cleanup['gpuErrors']
    assert cleanup['hud']['draws'] == report['endHUD']['draws'] + len(cleanup['closingFrames'])
    return {'browser_startup_hud': initial_result, 'width': width, 'height': height, 'tier': report['profile']['tier'],
            'full_recovery_equal': True, 'route_frames': len(report['frames']), 'hud_draws': cleanup['hud']['draws'],
            'first_area_unloaded_with_hud_live': True, 'final_zero_owners': True}


def compare_hosts(browser, native):
    b = json.loads((browser / 'receipt.json').read_bytes())
    n = json.loads((native / 'receipt.json').read_bytes())
    assert b['profile']['tier'] == n['tier']
    assert b['profile']['sceneSHA'] == n['sceneSHA256']
    assert b['profile']['presentationSchema'] == n['presentationSchema'] == 'aethyr.stream-walk-display.v1'
    width, height, pixels = capture(browser / 'before.json')
    assert (width, height) == (320, 180)
    other = (native / 'before.rgba').read_bytes()
    result = compare(pixels, other)
    return {'full_native_browser_recovery_view': result, 'tier': n['tier'],
            'browser_sha256': hashlib.sha256(pixels).hexdigest(), 'native_sha256': hashlib.sha256(other).hexdigest()}


if __name__ == '__main__':
    if sys.argv[1] == '--self-check':
        result = self_checks()
    elif sys.argv[1] == '--native':
        result = inspect_native(Path(sys.argv[2]), Path(sys.argv[3]))
    elif sys.argv[1] == '--compare-hosts':
        result = compare_hosts(Path(sys.argv[2]), Path(sys.argv[3]))
    else:
        assert sys.argv[1] == '--browser'
        result = inspect_browser(Path(sys.argv[2]), Path(sys.argv[3]))
    print(json.dumps(result, sort_keys=True))
