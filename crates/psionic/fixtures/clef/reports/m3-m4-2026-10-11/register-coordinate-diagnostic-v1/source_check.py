#!/usr/bin/env python3
"""Check the frozen diagnostic's source and tiny fixture bounds without a workload."""
import ast
import json
import struct
from common import ROOT, verify

frozen, manifest = verify()
for name in frozen['source_sha256']:
    if name.endswith('.py'):
        ast.parse((ROOT / name).read_text(), filename=name)
swift = (ROOT / 'exporter.swift').read_text()
build = (ROOT / 'build.py').read_text()
assert swift.count('dispatchThreadgroups(') == 1
assert 'for side in [1, 32]' in swift
assert 'makeFunction(name: "register_input_roundtrip")' in swift
assert 'op.run' not in swift and 'gemm(' not in swift
assert 'gpuStartTime' not in swift and 'gpuEndTime' not in swift
assert 'coordinate_shape' not in swift
assert swift.index('try save(raw,') < swift.index('var counts = ')
assert swift.index('try save(dump,') < swift.index('var counts = ')
assert "'swiftc', '-O', '-num-threads', '2'" in build
assert "'metal'" not in build and "'metallib'" not in build
assert manifest['cases'] == [
    {'name':'tail-1','dimensions':[1,1,1],'base':[0,0,0]},
    {'name':'interior-32','dimensions':[32,32,32],'base':[0,0,0]}]
assert manifest['library_sha256'] in swift
assert 8 * 32 * (1 + 64 * 4) * 4 == 263168
assert 32 * 32 * 8 + 263168 + 4 * 512 == 273408 < 1024**2
for side in [1, 32]:
    x = set(range(0x2000, 0x2000 + side * side))
    w = set(range(0x3000, 0x3000 + side * side))
    assert len(x) == len(w) == side * side and not x.intersection(w)
    assert all(0 < b < 0x7c00 for b in x | w)
    assert all(0 < struct.unpack('<f', struct.pack('<I', 0x3f000000 + i))[0] < 1 for i in range(side * side))
print(json.dumps({'status':'source checks passed','cases':2,'commands':2,'dispatches':2,
                  'maximum_explicit_buffer_bytes':273408,'compile_or_gpu_execution':False,
                  'unchanged_library_sha256':manifest['library_sha256'],
                  'source_sha256':frozen['source_sha256']},indent=2))
