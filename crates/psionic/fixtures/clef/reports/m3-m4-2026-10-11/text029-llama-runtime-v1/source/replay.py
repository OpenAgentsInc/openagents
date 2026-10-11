"""Compare saved third-runtime rows using the unchanged official CPU-f32 head."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import sys
from common import HERE, ISOLATION, RUN, CPU, frozen_input, require, sha, write
sys.path.insert(0, str(RUN))
import mps_reference as ref

def main():
    p = argparse.ArgumentParser()
    p.add_argument('--output-dir', type=Path, required=True)
    p.add_argument('--capture-receipt', type=Path, required=True)
    a = p.parse_args()
    require(os.environ.get('CLEF_LLAMA_GUARDED') == '1', 'owned CPU head guard required')
    require(not a.output_dir.exists(), 'preserve previous output')
    a.output_dir.mkdir(parents=True)
    result = {'schema': 'clef.text029-llama-external-head.v1', 'status': 'running', 'completed': False,
              'gpu_executed': False, 'trunk_executed': False, 'layers': [], 'heads': {},
              'scope': 'Third-runtime attribution diagnostic only. No acceptance replacement or tolerance change.',
              'external_head': 'Official BF16 checkpoint weights loaded as CPU f32; same GGUF lexical rows.',
              'embedded_quantized_head_used_for_comparison': False}
    write(a.output_dir / 'receipt.json', result)
    reader = None
    try:
        import numpy as np
        import torch
        import transformers
        from safetensors.torch import load_file, save_file
        torch.set_num_threads(2); torch.set_num_interop_threads(1); torch.set_default_device('cpu')
        require(str(torch.__version__) == '2.9.1' and transformers.__version__ == '5.10.2', 'runtime versions differ')
        inp = frozen_input()
        capture = json.loads(a.capture_receipt.read_text())
        require(capture['completed'] and capture['record'] == inp['record'] and len(capture['files']) == 65, 'capture input/coverage differs')
        cpu = json.loads(CPU.read_text())
        native_dir = ISOLATION / 'native-1/results'
        native = json.loads((native_dir / 'receipt.json').read_text())
        require(native['completed'] and len(native['files']) == 65, 'native capture incomplete')
        def raw_rows(directory, metadata, name):
            entry = next(x for x in metadata['files'] if x['name'] == name)
            f = directory / 'rows' / name
            require(f.stat().st_size == 359 * 5120 * 4 and sha(f) == entry['sha256'], 'saved row identity differs')
            values = torch.from_numpy(np.fromfile(f, dtype='<f4').reshape(1, 359, 5120))
            require(bool(torch.isfinite(values).all()), 'nonfinite saved rows')
            return values
        spans = {'question': [192, 194], 'true_option': [205, 223], 'false_option': [229, 247]}
        for index in range(64):
            entry = cpu['layers'][index]
            require(entry['layer'] == index, 'CPU layer order differs')
            path = CPU.parent / entry['file']
            require(sha(path) == entry['file_sha256'], 'CPU layer file changed')
            original = load_file(str(path), device='cpu')['hidden']
            require(tuple(original.shape) == (1, 359, 5120) and original.dtype == torch.float32 and bool(torch.isfinite(original).all())
                    and ref.digest(ref.cpu_bytes(torch, original)) == entry['hidden_sha256'], 'CPU layer contents differ')
            llama = raw_rows(a.capture_receipt.parent, capture, f'l_out-{index}.f32')
            metal = raw_rows(native_dir, native, f'l_out-{index}.f32')
            result['layers'].append({'layer': index, 'llama_vs_cpu': ref.row_errors(llama, original),
                'llama_vs_native': ref.row_errors(llama, metal), 'native_vs_cpu': ref.row_errors(metal, original),
                'spans': {name: {'llama_vs_cpu': ref.row_errors(llama[:,lo:hi], original[:,lo:hi]),
                                 'llama_vs_native': ref.row_errors(llama[:,lo:hi], metal[:,lo:hi])}
                          for name, (lo,hi) in spans.items()}})
            del original, llama, metal
        require(sha(ref.HF / 'joint_schema_model.py') == ref.SOURCE_SHA and sha(ref.HF / 'joint_head.safetensors') == ref.HEAD_SHA, 'official head changed')
        spec = importlib.util.spec_from_file_location('llama_diag_official', ref.HF / 'joint_schema_model.py')
        official = importlib.util.module_from_spec(spec); sys.modules[spec.name] = official; spec.loader.exec_module(official)
        processor = transformers.AutoProcessor.from_pretrained(ref.HF, local_files_only=True)
        raw = (ref.NATIVE / 'text-029.request.json').read_bytes()
        record = official.encode_record(processor.tokenizer, json.loads(raw), processor=processor)
        require(ref.digest(raw) == inp['request_sha256'] and json.loads(json.dumps(ref.asdict(record))) == inp['record'], 'official IDs/spans/options differ')
        q = record.questions[1]
        require(q.question_id == '日本1' and list(q.option_ids) == ['true', 'false'] and
                spans == {'question': list(q.question_span), 'true_option': list(q.option_spans[0]), 'false_option': list(q.option_spans[1])}, 'diagnostic spans differ')
        reader = ref.StreamingGguf(); require(reader.state == inp['gguf_state'], 'GGUF snapshot differs')
        ids = list(record.input_ids)
        tokens = sorted({token for q in record.questions for lo,hi in q.option_spans for token in ids[lo:hi]})
        lexical = torch.from_numpy(reader.rows('output.weight', tokens))
        require(ref.digest(ref.cpu_bytes(torch, lexical)) == cpu['lexical_sha256'], 'same-GGUF lexical bytes differ')
        mapping = {token:index for index,token in enumerate(tokens)}
        remapped = [mapping.get(token,0) for token in ids]
        for q in record.questions:
            for lo,hi in q.option_spans:
                require([tokens[t] for t in remapped[lo:hi]] == ids[lo:hi], 'lexical remapping differs')
        hr = official.EncodedRecord(input_ids=tuple(remapped), questions=record.questions, record_id=record.record_id)
        hids = torch.tensor([remapped], dtype=torch.long); mask = torch.ones((1,359), dtype=torch.long)
        head = official.JointSchemaHead(**json.loads((ref.HF / 'joint_head_config.json').read_text()))
        head.load_state_dict(load_file(str(ref.HF / 'joint_head.safetensors'), device='cpu'), strict=True); head = head.float().eval()
        sys.path.insert(0, str(ISOLATION))
        from precision_hash import parameter_hashes
        before_head = parameter_hashes(torch, head)
        rows = {'cpu': load_file(str(CPU.parent / 'final-hidden.safetensors'), device='cpu')['hidden'],
                'native_observer': raw_rows(native_dir, native, 'result_norm.f32'),
                'llama': raw_rows(a.capture_receipt.parent, capture, 'result_norm.f32')}
        require(ref.digest(ref.cpu_bytes(torch, rows['cpu'])) == cpu['hidden_sha256'], 'CPU final rows changed')
        result['final_rows'] = {name: ref.row_errors(rows[left], rows[right]) for name,left,right in
                               [('llama_vs_cpu','llama','cpu'),('llama_vs_native','llama','native_observer'),('native_vs_cpu','native_observer','cpu')]}
        native_logits = {}
        for mode in ('staged','fused-fields'):
            items = [json.loads(line) for line in (ref.NATIVE / mode / 'rows.jsonl').read_text().splitlines() if line]
            items = sorted((v for v in items if v['request_sha256'] == 'sha256:' + inp['request_sha256']), key=lambda v:v['question_index'])
            require(len(items) == 3 and all(v['question'] == q.question_id and v['rows'][2:] == ['option:' + name for name in q.option_ids] for v,q in zip(items,record.questions)), 'native question ordering differs')
            native_logits[mode] = [v['logits'] for v in items]
            ref.native_response({'id': 'text-029', 'record': record}, mode, native_logits[mode])
        for name, hidden in rows.items():
            before = {key: ref.cpu_bytes(torch, value) for key,value in [('hidden',hidden),('lexical',lexical),('ids',hids),('mask',mask)]}
            # result_norm and the saved CPU final rows already include backbone RMSNorm.
            with torch.inference_mode():
                output = head(hidden, hids, mask, [hr], lexical)[0]
                normalized = head.hidden_norm(hidden)
                pools = [torch.stack([normalized[0,-1], head._mean_span(normalized[0],q.question_span),
                                     *(head._mean_span(normalized[0],span) for span in q.option_spans)]) for q in record.questions]
            logits = [v.tolist() for v in output]
            exact = all(torch.equal(v,torch.tensor(expected,dtype=torch.float32)) for v,expected in zip(output,cpu['logits'])) if name == 'cpu' else None
            if name == 'cpu': require(exact, 'unchanged A full-head replay differs')
            require(all(ref.cpu_bytes(torch,value) == before[key] for key,value in [('hidden',hidden),('lexical',lexical),('ids',hids),('mask',mask)]), 'head changed inputs')
            result['heads'][name] = {'logits':logits, 'probabilities':ref.distributions(logits), 'A_replay_exact':exact,
                'vs_cpu':ref.compare_logits(logits,cpu['logits']), 'vs_native':{mode:ref.compare_logits(logits,v) for mode,v in native_logits.items()},
                'pooled_rows_sha256':[ref.digest(ref.cpu_bytes(torch,v)) for v in pools],
                'hidden_sha256':ref.digest(ref.cpu_bytes(torch,hidden))}
            if name == 'llama':
                save_file({'hidden':hidden, 'lexical':lexical, 'ids':hids, 'mask':mask,
                           **{f'pooled_{i}':v for i,v in enumerate(pools)}},str(a.output_dir / 'head-inputs.safetensors'))
        require(parameter_hashes(torch,head) == before_head, 'head weights changed')
        reader.check_state()
        observer = result['heads']['native_observer']['vs_native']
        result['observer_replay_screen'] = {'raw_logit_limit':1e-3,'all_tops_required':True,
            'passed':all(v['max_abs_logit'] <= 1e-3 and v['all_tops_match'] for v in observer.values())}
        result.update(status='complete',completed=True,all64layers=True,A_head_replay_exact=True,
                      record=inp['record'],capture_receipt_sha256=sha(a.capture_receipt),
                      head_inputs_sha256=sha(a.output_dir / 'head-inputs.safetensors'),head_parameters=before_head)
    except BaseException as error:
        result.update(status='failed',error_type=type(error).__name__,error=str(error)); raise
    finally:
        if reader is not None: reader.close()
        write(a.output_dir / 'receipt.json',result)

if __name__ == '__main__':
    main()
