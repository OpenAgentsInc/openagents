#!/usr/bin/env python3
"""Check copied production math, source identities, counts, and buffer accounting."""
import ast
import hashlib
import json
from common import ROOT, REPO, SOURCE_NAMES, verify

frozen, manifest = verify()
for name in SOURCE_NAMES:
    if name.endswith('.py'):
        ast.parse((ROOT / name).read_text(), filename=name)
source = json.loads((ROOT / 'production-source.json').read_text())
for name, info in source['source_files'].items():
    data = (REPO / name).read_bytes()
    assert len(data) == info['bytes'] and hashlib.sha256(data).hexdigest() == info['sha256'], 'production source changed'
shape_path = 'crates/psionic/crates/psionic-backend-metal/src/kernels/clef_gemm_shapes.metal'
prefill_path = 'crates/psionic/crates/psionic-backend-metal/src/kernels/clef_prefill.metal'
assert (ROOT / 'shapes.metal').read_bytes() == (REPO / shape_path).read_bytes()
text = (REPO / prefill_path).read_text()
parts = []
for name in ['group_sum', 'silu', 'rms', 'silu_mul']:
    item = source['extracted_functions'][name]
    start = text.index(item['production_start']); opening = text.index('{', start)
    depth = 1; end = opening + 1
    while depth:
        depth += (text[end] == '{') - (text[end] == '}');end += 1
    function = text[start:end] + "\n"
    assert len(function.encode()) == item['bytes'] and hashlib.sha256(function.encode()).hexdigest() == item['sha256']
    parts.append(function)
assert (ROOT / 'pointwise.metal').read_text() == source['pointwise_prelude'] + '\n'.join(parts)
assert manifest['command_buffers'] == 8*2 + 3*5*2*4 == 136
assert manifest['dispatches'] == 8*2*5 + 3*5*4*(3+5) == 560
assert manifest['timing_rows'] == 3*5*2*4 == 120
n = 2048
weight_bytes = 3*4096*12288*2 + 4096*4
state_bytes = n*(4096*4+12288*2+2*12288*4)
explicit_bytes = weight_bytes + state_bytes + 8*512
reset_copies = n*(4096*4+(12288-4096)*2)
expected_copies = state_bytes
assert explicit_bytes == manifest['limits']['maximum_explicit_buffer_bytes']
assert expected_copies + reset_copies == manifest['limits']['maximum_owned_cpu_copy_bytes']
assert explicit_bytes + expected_copies + reset_copies < 1536*1024**2
host = (ROOT / 'probe.swift').read_text()
assert 'barrier:!' not in host and 'overlap' not in host
assert host.count('descriptor.maxTotalThreadsPerThreadgroup = 128') == 1
assert host.count('limited:limited)') == 3
assert 'if barrier' not in host
assert 'descriptor.threadGroupSizeIsMultipleOfThreadExecutionWidth =' not in host
assert 'let limited = try d.makeComputePipelineState(descriptor: descriptor, options: [], reflection: nil)' in host
assert 'e.memoryBarrier(scope:.buffers);e.setComputePipelineState(pipes["clef_silu_mul_to_f16"]!)' in host
assert 'let hidden = 4096, wide = 12288' in host
assert 'commands==136 && dispatches==560 && timingRows==120' in host
assert 'memcmp(' in host and 'try expected.unchanged();try s.immutableCopies();try w.unchanged()' in host
assert 'max_total_threads_per_threadgroup' not in (ROOT / 'shapes.metal').read_text()
print(json.dumps({'passed':True,'source_sha256':frozen['source_sha256'],
                  'explicit_peak_bytes':explicit_bytes,'owned_cpu_peak_bytes':expected_copies+reset_copies,
                  'combined_payload_peak_bytes':explicit_bytes+expected_copies+reset_copies,
                  'commands':136,'dispatches':560,'timing_rows':120,'compiler_or_gpu_executed':False}))
