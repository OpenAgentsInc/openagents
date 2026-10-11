#!/usr/bin/env python3
"""Compare offset and cloned int64 indices using saved lexical rows only."""
import json
import os
from pathlib import Path
import sys
import time
import head_capture as capture
from immutable_probe import raw, digest, compare, write
from mac_quiet_bench import file_sha

if not {'gpu', 'memory'} <= set(os.environ.get('OPENAGENTS_LEASES', '').split(',')):
    raise SystemExit('requires GPU and memory leases plus an owned-process guard')
output = Path(sys.argv[1])
if output.exists():
    raise SystemExit('output exists; preserve the previous probe')
torch, official = capture.setup(capture.BASE/'artifacts/clef-bf16')
from safetensors.torch import load
torch.mps.set_per_process_memory_fraction(min(1., 1024**3/torch.mps.recommended_max_memory()))
inputs = capture.BASE/'reference27b/head-inputs-014-034'
report = {'kind': 'mps_offset_int64_lexical_gather', 'torch': str(torch.__version__),
          'model_loaded': False, 'immutable_inputs_each_repeat': True, 'cases': []}
began = time.monotonic()
for case in capture.CASES:
    row = json.loads((inputs/(case+'.json')).read_text())
    data = (inputs/row['before']['file']).read_bytes()
    assert digest(data) == row['before']['sha256']
    pristine = load(data)
    names = ['remapped_input_ids', 'lexical', 'attention_mask']
    immutable = {name: raw(torch, pristine[name]) for name in names}
    spans = [span for q in row['records_before'][0]['questions'] for span in q['option_spans']]
    del pristine
    for mode in ['offset', 'cloned_offset_zero']:
        for repeat in range(2):
            cpu = {k: v for k, v in load(data).items() if k in names}
            assert all(x['exact'] for x in compare(torch, cpu, immutable).values())
            gpu = {k: v.to('mps') for k, v in cpu.items()}
            for span in spans:
                start, end = span
                ids = gpu['remapped_input_ids'][0, start:end]
                if mode == 'cloned_offset_zero':
                    ids = ids.clone()
                    assert ids.storage_offset() == 0
                expected_ids = load(data)['remapped_input_ids'][0, start:end]
                expected_table = load(data)['lexical']
                expected = expected_table[expected_ids]
                result = gpu['lexical'][ids]
                mean = result.mean(dim=0)
                result_cpu = result.detach().cpu().clone()
                mean_cpu = mean.detach().cpu().clone()
                detail = {'id': case, 'mode': mode, 'repeat': repeat, 'span': span,
                          'index_storage_offset': ids.storage_offset(),
                          'gather_exact': raw(torch, result_cpu) == raw(torch, expected),
                          'mean_max_absolute_difference': float((mean_cpu.float()-expected.mean(dim=0).float()).abs().max()),
                          'mean_finite': bool(torch.isfinite(mean_cpu).all()),
                          'cpu_source_after': compare(torch, cpu, immutable),
                          'gpu_source_after': compare(torch, gpu, immutable)}
                report['cases'].append(detail)
                write(output, report)
                if not all(x['exact'] for x in detail['gpu_source_after'].values()):
                    break  # Do not submit corrupted indices to another gather.
report.update(completed=True, elapsed_seconds=time.monotonic()-began)
write(output, report)
print(json.dumps({'cases': len(report['cases']), 'elapsed_seconds': report['elapsed_seconds']}), flush=True)
