#!/usr/bin/env python3
"""Replay the head from immutable saved bytes, with an optional index clone."""
import inspect
import math
import json
import os
from pathlib import Path
import sys
import textwrap
import time
import types
import head_capture as capture
from immutable_probe_finite import raw, digest, compare, write
from mac_quiet_bench import file_sha

if not {'gpu', 'memory'} <= set(os.environ.get('OPENAGENTS_LEASES', '').split(',')):
    raise SystemExit('requires GPU and memory leases plus an owned-process guard')
output = Path(sys.argv[1])
if output.exists():
    raise SystemExit('output exists; preserve the previous probe')
checkpoint = capture.BASE/'artifacts/clef-bf16'
inputs = capture.BASE/'reference27b/head-inputs-014-034'
metadata = json.loads((inputs/'run.json').read_text())
assert file_sha(checkpoint/'joint_head.safetensors') == metadata['head_sha256']
torch, official = capture.setup(checkpoint)
from safetensors.torch import load, load_file
torch.mps.set_per_process_memory_fraction(min(1., 2*1024**3/torch.mps.recommended_max_memory()))
head = official.JointSchemaHead(**json.loads((checkpoint/'joint_head_config.json').read_text()))
cpu_weights = load_file(str(checkpoint/'joint_head.safetensors'))
immutable_weights = {name: raw(torch, value) for name, value in cpu_weights.items()}
head.load_state_dict(cpu_weights, strict=True)
head = head.to(dtype=torch.bfloat16).to(device='mps').eval()
original_linear = torch.nn.functional.linear
linear_mode = {'clone': False, 'calls': 0, 'offset_calls': 0, 'offsets': set()}
def checked_linear(input, weight, bias=None):
    linear_mode['calls'] += 1
    offset = weight.storage_offset() != 0 or (bias is not None and bias.storage_offset() != 0)
    if offset:
        linear_mode['offset_calls'] += 1
        linear_mode['offsets'].add((tuple(weight.shape), weight.storage_offset(), None if bias is None else bias.storage_offset()))
    if linear_mode['clone']:
        if weight.storage_offset() != 0:
            weight = weight.clone()
            assert weight.storage_offset() == 0
        if bias is not None and bias.storage_offset() != 0:
            bias = bias.clone()
            assert bias.storage_offset() == 0
    return original_linear(input, weight, bias)
torch.nn.functional.linear = checked_linear
original_forward = official.JointSchemaHead.forward
source = textwrap.dedent(inspect.getsource(original_forward))
old = 'token_ids = input_ids[batch_index, start:end]'
assert source.count(old) == 1
cloned_source = source.replace(old, old+'.clone()')
namespace = dict(vars(official))
exec(compile(cloned_source, '<scratch-token-id-clone>', 'exec'), namespace)
cloned_forward = namespace['forward']
report = {'kind': 'official_bf16_head_immutable_input_and_parameter_views', 'torch': str(torch.__version__),
          'trunk_loaded': False, 'head_sha256': metadata['head_sha256'],
          'original_forward_sha256': digest(source.encode()), 'cloned_forward_sha256': digest(cloned_source.encode()),
          'experimental_changes': ['Clone lexical token-ID slices before indexing', 'Clone nonzero-offset weight and bias views at functional.linear call time'],
          'model_weights_checked_each_call': True,
          'diagnostic_only': True, 'capture_status': metadata['status'],
          'max_mps_allocator_bytes': 2*1024**3, 'immutable_inputs_each_repeat': True, 'cases': []}
began = time.monotonic()
for case in capture.CASES:
    row = json.loads((inputs/(case+'.json')).read_text())
    data = (inputs/row['before']['file']).read_bytes()
    assert digest(data) == row['before']['sha256']
    pristine = load(data)
    names = ['hidden', 'remapped_input_ids', 'attention_mask', 'lexical']
    immutable = {name: raw(torch, pristine[name]) for name in names}
    record = row['records_before'][0]
    questions = tuple(official.EncodedQuestion(q['question_id'], q['question_type'], tuple(q['question_span']),
                      tuple(tuple(s) for s in q['option_spans']), tuple(q['option_ids'])) for q in record['questions'])
    encoded = official.EncodedRecord(input_ids=tuple(pristine['remapped_input_ids'][0].tolist()),
                                    questions=questions, record_id=record['record_id'])
    del pristine
    for mode, forward, clone_linear in [('original', original_forward, False),
                                        ('cloned_token_ids', cloned_forward, False),
                                        ('cloned_linear_parameters', original_forward, True),
                                        ('cloned_indices_and_linear_parameters', cloned_forward, True)]:
        head.forward = types.MethodType(forward, head)
        linear_mode['clone'] = clone_linear
        for repeat in range(2):
            # Reload the same BF16 parameter values for every diagnostic call.
            head.load_state_dict(cpu_weights, strict=True)
            weights_before = compare(torch, dict(head.named_parameters()), immutable_weights)
            if not all(item['exact'] for item in weights_before.values()):
                report['parameter_upload_failure'] = {'id':case,'mode':mode,'repeat':repeat,'weights':weights_before}
                write(output, report)
                raise ValueError('head weights differ from immutable BF16 bytes before inference')
            linear_mode.update(calls=0, offset_calls=0, offsets=set())
            cpu = {k: v for k, v in load(data).items() if k in names}
            cpu_before = compare(torch, cpu, immutable)
            assert all(x['exact'] for x in cpu_before.values())
            gpu = {k: v.to('mps') for k, v in cpu.items()}
            gpu_before = compare(torch, gpu, immutable)
            if not all(x['exact'] for x in gpu_before.values()):
                report['upload_failure'] = {'id':case, 'mode':mode, 'repeat':repeat, 'gpu_source_before':gpu_before}
                write(output, report)
                raise ValueError('upload disagrees with immutable input bytes')
            sentinel = torch.ones((1, 1031), dtype=torch.long, device='mps')
            with torch.inference_mode():
                result = head(gpu['hidden'], gpu['remapped_input_ids'], gpu['attention_mask'], [encoded], gpu['lexical'])[0]
            gpu_after = compare(torch, gpu, immutable)
            cpu_after = compare(torch, cpu, immutable)
            weights_after = compare(torch, dict(head.named_parameters()), immutable_weights)
            logits = [value.detach().cpu().clone().float().tolist() for value in result]
            detail = {'id': case, 'mode': mode, 'repeat': repeat, 'cpu_source_before': cpu_before,
                      'gpu_source_before': gpu_before, 'gpu_source_after': gpu_after, 'cpu_source_after': cpu_after,
                      'sentinel_unchanged': bool((sentinel.cpu().clone()==1).all()), 'logits': logits,
                      'linear_calls':linear_mode['calls'], 'linear_nonzero_parameter_offset_calls':linear_mode['offset_calls'],
                      'linear_offsets': sorted(linear_mode['offsets'], key=str),
                      'weights_exact_before': all(x['exact'] for x in weights_before.values()),
                      'weights_exact_after': all(x['exact'] for x in weights_after.values()),
                      'changed_weights':{k:v for k,v in weights_after.items() if not v['exact']},
                      'logits_finite': all(math.isfinite(value) for values in logits for value in values),
                      'max_abs_logit_vs_full_bf16': max(abs(x-y) for a,b in zip(logits,row['bf16_logits']) for x,y in zip(a,b))}
            report['cases'].append(detail)
            write(output, report)
report.update(completed=True, elapsed_seconds=time.monotonic()-began)
write(output, report)
print(json.dumps({'cases': len(report['cases']), 'elapsed_seconds': report['elapsed_seconds']}), flush=True)
