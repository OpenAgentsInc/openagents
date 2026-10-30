#!/usr/bin/env python3
"""Test tooling: independent integer counter and struct binary16/f32 oracle."""
import json, pathlib, struct, sys
ROOT = pathlib.Path(__file__).resolve().parents[1]
def word(counter):
    mask = (1 << 32) - 1
    x = ((123 + 0x9e3779b9 * (counter + 1)) & mask) ^ ((7 * 0x85ebca6b) & mask)
    x ^= x >> 16
    x = (x * 0x7feb352d) & mask
    x ^= x >> 15
    x = (x * 0x846ca68b) & mask
    return x ^ (x >> 16)
def encoded(value, dtype):
    if dtype == 0: return struct.pack('<f', value)
    if dtype == 1: return struct.pack('<e', value)
    # All generated values lie on the exact bfloat16 grid; no rounding oracle here.
    return struct.pack('<H', struct.unpack('<I', struct.pack('<f', value))[0] >> 16)
profile = json.loads((ROOT / 'fixtures/tensor-device/profile.json').read_text())
assert [f'{word(i):08x}' for i in range(8)] == profile['counter_vectors']
if '--header' in sys.argv:
    print('static const uint32_t oracle_words[] = {' + ','.join(f'0x{word(i):08x}u' for i in range(300)) + '};')
    for dtype, shift, scale in [(0,8,2**24),(1,21,2048),(2,24,256)]:
        raw = b''.join(encoded((word(i) >> shift) / scale, dtype) for i in range(17,274))
        print(f'static const uint8_t oracle_random_{dtype}[] = {{' + ','.join(map(str,raw)) + '};')
else:
    print('Counter32-v1: frozen integer vectors and exact dtype-grid fixtures pass')
if '--header' in sys.argv:
    for dtype, maximum in [(1,0x7bff),(2,0x7f7f)]:
        selected = sorted(set(range(0,maximum,37)) | set(range(32)) | {maximum-1,1022,1023,1024,127,128})
        inputs, outputs = [], []
        for low in selected:
            def decoded(w):
                if dtype == 1: return struct.unpack('<e',struct.pack('<H',w))[0]
                return struct.unpack('<f',struct.pack('<I',w<<16))[0]
            middle = (decoded(low)+decoded(low+1))/2
            u = struct.unpack('<I',struct.pack('<f',middle))[0]
            for delta in [-1,0,1]:
                chosen = low if delta<0 or (delta==0 and low%2==0) else low+1
                for sign in [0,0x80000000]:
                    inputs.append((u+delta)|sign)
                    outputs.append(chosen|(sign>>16))
        print(f'static const uint32_t oracle_round_input_{dtype}[] = {{'+','.join(f'0x{u:08x}u' for u in inputs)+'};')
        print(f'static const uint16_t oracle_round_output_{dtype}[] = {{'+','.join(map(str,outputs))+'};')
