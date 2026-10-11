#!/usr/bin/env python3
"""Capture two official BF16 head inputs; evaluate only the head in CPU f32.

Capture requires an external owned-process memory/deadline guard and GPU/memory
leases. Evaluation is a separate process after the full reference model exits.
"""
import argparse
from dataclasses import asdict
import importlib.util
import json
import os
from pathlib import Path
import sys
import time

ROOT = Path(os.environ['CLEF_WORKTREE'])
BASE = Path(os.environ['CLEF_DIAGNOSTIC_SCRATCH'])
sys.path.insert(0, str(ROOT / 'crates/psionic/fixtures/clef/tools'))
import image_parity as parity
from mac_quiet_bench import file_sha
CASES = ('text-014', 'text-034')


def setup(checkpoint):
    for name in ['RAYON_NUM_THREADS', 'OMP_NUM_THREADS', 'OPENBLAS_NUM_THREADS',
                 'VECLIB_MAXIMUM_THREADS', 'MKL_NUM_THREADS']:
        os.environ[name] = '2'
    os.environ['HF_HUB_OFFLINE'] = '1'
    os.environ['TRANSFORMERS_OFFLINE'] = '1'
    os.environ['PYTORCH_ENABLE_MPS_FALLBACK'] = '0'
    import torch
    torch.set_num_threads(2)
    torch.set_num_interop_threads(1)
    spec = importlib.util.spec_from_file_location('bounded_official_head_reference', checkpoint / 'joint_schema_model.py')
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return torch, module


def compact_ids(ids):
    unique = sorted(set(ids))
    mapping = {token: index for index, token in enumerate(unique)}
    remapped = [mapping[token] for token in ids]
    assert [unique[index] for index in remapped] == ids
    return unique, remapped


def tensor_snapshot(torch, inputs):
    hidden, ids, mask, records, output_weight = inputs
    if len(records) != 1 or hidden.ndim != 3 or hidden.shape[0] != 1:
        raise ValueError('capture requires exactly one record')
    if not 1 <= hidden.shape[1] <= 2048 or hidden.shape[2] != 5120:
        raise ValueError('capture exceeds the two-case 27B hidden-state bound')
    if hidden.dtype != torch.bfloat16 or output_weight.dtype != torch.bfloat16:
        raise ValueError('capture must preserve original BF16 hidden and lexical tensors')
    original_ids = ids.detach().cpu().clone()
    unique, remapped = compact_ids(original_ids[0].tolist())
    # Inspection of official JointSchemaHead.forward confirms the only output
    # embedding use is indexing option-token rows, followed by a mean. Gather
    # every input token so remapping preserves all of those lookups exactly.
    indexes = torch.tensor(unique, device=output_weight.device, dtype=torch.long)
    return {'hidden': hidden.detach().cpu().clone().contiguous(),
            'input_ids': original_ids.contiguous(),
            'attention_mask': mask.detach().cpu().clone().contiguous(),
            'lexical': output_weight.index_select(0, indexes).detach().cpu().clone().contiguous(),
            'lexical_token_ids': torch.tensor(unique, dtype=torch.long),
            'remapped_input_ids': torch.tensor([remapped], dtype=torch.long)}, [asdict(record) for record in records]


def capture(args):
    leases = os.environ.get('OPENAGENTS_LEASES', '').split(',')
    if 'gpu' not in leases or 'memory' not in leases:
        raise ValueError('capture requires GPU and memory leases plus an external process guard')
    checkpoint = args.checkpoint.resolve()
    if args.output.exists():
        raise ValueError('capture output already exists; preserve the prior capture')
    args.output.mkdir(parents=True)
    torch, official = setup(checkpoint)
    from safetensors.torch import save_file
    from transformers import AutoProcessor
    torch.mps.set_per_process_memory_fraction(min(1.0, 60 * 1024**3 / torch.mps.recommended_max_memory()))
    requests = ROOT / 'crates/psionic/fixtures/clef/e2e/requests.jsonl'
    old_path = BASE / 'reference27b/text-mps-bf16.json'
    previous = json.loads(old_path.read_text())
    if parity.suite_sha(argparse.Namespace(text_suite=requests)) != previous['cases_sha256']:
        raise ValueError('request suite changed')
    if file_sha(checkpoint / 'joint_schema_model.py') != previous['reference_source_sha256']:
        raise ValueError('official reference source changed')
    for name, key in [('joint_head.safetensors', 'head_sha256'), ('config.json', 'config_sha256'),
                      ('tokenizer.json', 'tokenizer_sha256'), ('joint_head_config.json', 'head_config_sha256'),
                      ('verified.json', 'verified_manifest_sha256')]:
        if file_sha(checkpoint / name) != previous[key]:
            raise ValueError(f'{name} differs from the completed reference')
    checkpoint_record = parity.validate_checkpoint(checkpoint, 60)
    old = {row['id']: row for row in previous['cases']}
    cases = [(case, request) for case, request, _ in parity.load_suite(
        argparse.Namespace(text_suite=requests, model='clef')) if case['id'] in CASES]
    processor = AutoProcessor.from_pretrained(checkpoint, local_files_only=True)
    encoded = {}
    for case, request in cases:
        value = official.encode_record(processor.tokenizer, request, processor=processor)
        if len(value.input_ids) > 2048:
            raise ValueError('capture token bound exceeded')
        parity.validate_input_ids(list(value.input_ids), old[case['id']]['input_ids'], old[case['id']]['response'])
        encoded[case['id']] = value
    report = {'kind': 'official_bf16_head_inputs', 'status': 'loading', 'cases': [],
              'checkpoint': str(checkpoint), 'device': 'mps', 'dtype': 'bfloat16', 'threads': 2,
              'torch': str(torch.__version__), 'mps_fallback': False,
              'conditions': 'shared machine; diagnostic only; no acceptance thresholds changed',
              'source_sha256': file_sha(checkpoint / 'joint_schema_model.py'),
              'script_sha256': file_sha(Path(__file__)), 'prior_reference_sha256': file_sha(old_path),
              'head_sha256': previous['head_sha256'], 'head_config_sha256': previous['head_config_sha256'],
              'verified_manifest_sha256': previous['verified_manifest_sha256'],
              'selected_cases': list(CASES), **checkpoint_record}
    parity.write(args.output / 'run.json', report)
    model, processor = official.load_release_model(checkpoint, device='mps', dtype=torch.bfloat16,
        local_files_only=True, attn_implementation='eager', low_cpu_mem_usage=True)
    capture = parity.ReferenceCapture()
    active = {}
    def before_head(_model, inputs):
        if 'before' in active:
            raise ValueError('head pre-hook called more than once')
        active['before'], active['records_before'] = tensor_snapshot(torch, inputs)
        parity.validate_input_ids(active['before']['input_ids'][0].tolist(), capture.expected,
                                  {'usage': {'input_tokens': len(capture.expected)}})
        if active['records_before'] != [asdict(encoded[capture.case_id])]:
            raise ValueError('head input records differ from fresh IDs or spans')
    def after_head(_model, inputs, output):
        if 'before' not in active or 'after' in active:
            raise ValueError('head post-hook missing or duplicated')
        active['after'], active['records_after'] = tensor_snapshot(torch, inputs)
        active['head_logits'] = [row.detach().float().cpu().clone().tolist() for row in output[0]]
    hooks = [model.register_forward_pre_hook(capture.before), model.register_forward_hook(capture.after),
             model.head.register_forward_pre_hook(before_head), model.head.register_forward_hook(after_head)]
    try:
        for case, request in cases:
            active.clear()
            capture.begin(case['id'], list(encoded[case['id']].input_ids))
            began = time.monotonic()
            response = None
            try:
                with torch.inference_mode():
                    response = official.systemone(model, processor, request)
                capture.validate(response)
                parity.validate_response(response, request)
                parity.validate_logits(capture.logits, request)
                if active['head_logits'] != capture.logits:
                    raise ValueError('head and model logits differ')
                tensors_equal = {key: torch.equal(value, active['after'][key]) for key, value in active['before'].items()}
                records_equal = active['records_before'] == active['records_after']
                row = {'id': case['id'], 'response': response, 'bf16_logits': capture.logits,
                       'prior_bf16_logits': old[case['id']]['logits'],
                       'input_ids': capture.after_ids, 'records_before': active['records_before'],
                       'records_after': active['records_after'], 'tensors_unchanged': tensors_equal,
                       'records_unchanged': records_equal, 'seconds': time.monotonic()-began,
                       'request_body': case['request_body'], 'source_request_sha256': case['source_request_sha256']}
                for phase in ['before', 'after']:
                    path = args.output / f"{case['id']}-{phase}.safetensors"
                    save_file(active[phase], str(path))
                    row[phase] = {'file': path.name, 'bytes': path.stat().st_size, 'sha256': file_sha(path)}
                parity.write(args.output / f"{case['id']}.json", row)
                if not all(tensors_equal.values()) or not records_equal:
                    raise ValueError('official head changed an input during forward')
                report['cases'].append(row)
                report['status'] = 'running'
                parity.write(args.output / 'run.json', report)
                print(json.dumps({'case': case['id'], 'tokens':len(capture.after_ids),
                                  'input_copies_unchanged':True, 'seconds':row['seconds']}), flush=True)
            except BaseException as error:
                failure = capture.diagnostic(error, response)
                failure['head_records_before'] = active.get('records_before')
                failure['head_records_after'] = active.get('records_after')
                parity.write(args.output / f"{case['id']}-failure.json", parity.diagnostic_json(failure))
                raise
        report['status'] = 'complete'
    except BaseException as error:
        report.update(status='failed', error_type=type(error).__name__, error=str(error))
        raise
    finally:
        for hook in hooks:
            hook.remove()
        parity.write(args.output / 'run.json', report)
    return 0


def evaluate(args):
    if 'memory' not in os.environ.get('OPENAGENTS_LEASES', '').split(','):
        raise ValueError('offline head evaluation requires a memory lease')
    if (args.output / 'f32-head.json').exists():
        raise ValueError('offline result already exists; preserve it')
    capture = json.loads((args.output / 'run.json').read_text())
    if capture['status'] != 'complete' and not args.allow_failed_capture:
        raise ValueError('head capture failed; use explicit diagnostic-only handling')
    rows = [json.loads((args.output / (case + '.json')).read_text()) for case in CASES]
    if [row['id'] for row in rows] != list(CASES):
        raise ValueError('head capture cases are incomplete')
    for row in rows:
        if not row['records_unchanged'] or any(not same for key, same in row['tensors_unchanged'].items() if key != 'attention_mask'):
            raise ValueError('head input corruption is not confined to the attention mask')
    checkpoint = args.checkpoint.resolve()
    if file_sha(checkpoint / 'joint_head.safetensors') != capture['head_sha256']:
        raise ValueError('head weights changed')
    if file_sha(checkpoint / 'joint_head_config.json') != capture['head_config_sha256']:
        raise ValueError('head configuration changed')
    if file_sha(checkpoint / 'joint_schema_model.py') != capture['source_sha256']:
        raise ValueError('official head source changed')
    torch, official = setup(checkpoint)
    from safetensors.torch import load_file
    head = official.JointSchemaHead(**json.loads((checkpoint / 'joint_head_config.json').read_text()))
    state = load_file(str(checkpoint / 'joint_head.safetensors'))
    head.load_state_dict(state, strict=True)
    del state
    head = head.to(device='cpu', dtype=torch.float32).eval()
    result = {'kind': 'official_f32_head_on_exact_bf16_inputs', 'device':'cpu', 'dtype':'float32',
              'threads':2, 'torch':str(torch.__version__), 'capture_sha256':file_sha(args.output / 'run.json'),
              'head_sha256':capture['head_sha256'], 'source_sha256':capture['source_sha256'],
              'script_sha256':file_sha(Path(__file__)), 'cases':[],
              'diagnostic_only':True, 'capture_status':capture['status'],
              'failed_capture_explicitly_allowed':args.allow_failed_capture,
              'mask_semantics':'Only original before-mask snapshots are used; official head reads their sum for sequence length. The observed post-mask mutation remains a capture failure.'}
    for row in rows:
        path = args.output / row['before']['file']
        if file_sha(path) != row['before']['sha256']:
            raise ValueError('captured tensor file changed')
        tensors = load_file(str(path))
        ids = tensors['input_ids'][0].tolist()
        unique, remapped = compact_ids(ids)
        if unique != tensors['lexical_token_ids'].tolist() or remapped != tensors['remapped_input_ids'][0].tolist():
            raise ValueError('compact lexical table does not reconstruct exact input IDs')
        if ids != row['input_ids']:
            raise ValueError('stored input IDs disagree')
        if not bool((tensors['attention_mask'] == 1).all()) or tensors['attention_mask'].numel() != len(ids):
            raise ValueError('before-mask is not the exact unpadded all-ones mask')
        if not bool(torch.isfinite(tensors['hidden']).all()) or not bool(torch.isfinite(tensors['lexical']).all()):
            raise ValueError('before hidden/lexical inputs contain nonfinite values')
        before = row['records_before'][0]
        questions = tuple(official.EncodedQuestion(q['question_id'], q['question_type'], tuple(q['question_span']),
            tuple(tuple(span) for span in q['option_spans']), tuple(q['option_ids'])) for q in before['questions'])
        encoded = official.EncodedRecord(input_ids=tuple(remapped), questions=questions, record_id=before['record_id'])
        with torch.inference_mode():
            output = head(tensors['hidden'].float(), tensors['remapped_input_ids'], tensors['attention_mask'],
                          [encoded], tensors['lexical'].float())[0]
        logits = [values.detach().tolist() for values in output]
        request = json.loads(row['request_body'])
        parity.validate_logits(logits, request)
        comparisons = {}
        for question, values, old in zip(questions, output, row['bf16_logits']):
            current = values.softmax(-1).tolist()
            previous = torch.tensor(old, dtype=torch.float32).softmax(-1).tolist()
            comparisons[question.question_id] = {'type':question.question_type,
                'option_ids':list(question.option_ids), 'bf16_probabilities':previous, 'f32_probabilities':current,
                'bf16_top':question.option_ids[max(range(len(previous)), key=previous.__getitem__)],
                'f32_top':question.option_ids[max(range(len(current)), key=current.__getitem__)],
                'max_abs_dlogit':max(abs(x-y) for x,y in zip(values.tolist(),old)),
                'max_abs_dp':max(abs(x-y) for x,y in zip(current,previous))}
        result['cases'].append({'id':row['id'], 'f32_logits':logits, 'comparisons':comparisons})
        parity.write(args.output / 'f32-head.json', result)
        print(json.dumps(result['cases'][-1]), flush=True)
    result['completed'] = True
    parity.write(args.output / 'f32-head.json', result)
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['capture','evaluate','self-test'])
    parser.add_argument('--allow-failed-capture', action='store_true', help='diagnostic only: evaluate intact before-inputs while preserving a failed post-mask check')
    parser.add_argument('--checkpoint', type=Path, default=BASE / 'artifacts/clef-bf16')
    parser.add_argument('--output', type=Path, default=BASE / 'reference27b/head-inputs-014-034')
    args = parser.parse_args()
    if args.mode == 'self-test':
        assert compact_ids([8,2,8,9]) == ([2,8,9],[1,0,1,2])
        assert compact_ids([0]) == ([0],[0])
        parity.capture_self_test()
        print('compact token-table identity and strict capture self-tests passed; no torch import/model run')
        return 0
    return capture(args) if args.mode == 'capture' else evaluate(args)

if __name__ == '__main__':
    raise SystemExit(main())
