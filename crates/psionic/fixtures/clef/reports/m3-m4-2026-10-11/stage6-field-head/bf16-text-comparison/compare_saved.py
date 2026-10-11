#!/usr/bin/env python3
"""Recheck saved Stage6 text responses against official BF16, without model calls."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
from types import SimpleNamespace

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, required=True)
    parser.add_argument('--run', type=Path, required=True)
    parser.add_argument('--suite', type=Path, required=True)
    parser.add_argument('--rules', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    # Loading this module defines the validation functions; it launches no work.
    sys.path.insert(0, str(args.rules.resolve().parent))
    spec = importlib.util.spec_from_file_location('saved_image_parity_rules', args.rules)
    rules = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(rules)
    reference = json.loads(args.reference.read_text())
    if reference.get('kind') != 'cloudflare_reference' or reference.get('dtype') != 'bfloat16' or not reference.get('completed'):
        raise ValueError('A completed official BF16 reference is required')
    if reference['cases_sha256'] != sha(args.suite):
        raise ValueError('Reference suite hash differs')
    cases = rules.load_suite(SimpleNamespace(text_suite=args.suite, model='clef'))
    previous = {row['id']: row for row in reference['cases']}
    if len(previous) != len(reference['cases']) or set(previous) != {case['id'] for case, _, _ in cases}:
        raise ValueError('Reference coverage differs')
    if len(cases) != 40 or sum(len(request['questions']) for _, request, _ in cases) != 104:
        raise ValueError('Expected 40 requests and 104 questions')
    captured = [json.loads(line) for line in (args.run/'records.jsonl').read_text().splitlines()]
    response_records = {(row['mode'], row['id']): row for row in captured if row.get('kind') == 'response'}
    if len(response_records) != 80 or len([row for row in captured if row.get('kind') == 'response']) != 80:
        raise ValueError('Saved HTTP records are incomplete or duplicated')
    modes = []
    for mode in ['staged', 'fused-fields']:
        record = {'kind': 'offline_native_text_parity', 'mode': mode, 'reference_sha256': sha(args.reference),
                  'cases_sha256': sha(args.suite), 'validation_rules_sha256': sha(args.rules),
                  'saved_run_summary_sha256': sha(args.run/'summary.json'),
                  'suite': 'text', 'model_execution': False, 'cases': [], 'http_refusals': [],
                  'acceptance': 'all top answers, identical token counts, and maximum probability difference <= 0.02',
                  'native_token_ids_observed': False}
        for case, request, digests in cases:
            old = previous[case['id']]
            if old['image_sha256'] != digests or old['source_request_sha256'] != case['source_request_sha256']:
                raise ValueError('Reference request identity differs')
            request_path = args.run/(case['id']+'.request.json')
            if request_path.read_bytes() != case['request_body'].encode():
                raise ValueError('Saved request bytes differ from exact suite encoding')
            response_path = args.run/(mode+'-'+case['id']+'.response.json')
            response = json.loads(response_path.read_text())
            http = response_records[mode, case['id']]
            if http['status'] != 200 or http['request_sha256'] != 'sha256:'+sha(request_path) or http['response'] != response:
                raise ValueError('Saved HTTP status, request hash, or response differs')
            ref_response = old['response']
            rules.validate_response(ref_response, request)
            rules.validate_response(response, request)
            rules.validate_logits(old['logits'], request)
            if len(old['input_ids']) != ref_response['usage']['input_tokens']:
                raise ValueError('Reference token count differs from captured IDs')
            selected = {key: rules.top_answer(answer) for key, answer in response['answers'].items()}
            reference_selected = {key: rules.top_answer(answer) for key, answer in ref_response['answers'].items()}
            if reference_selected != old['top_answers']:
                raise ValueError('Reference saved top answers differ')
            comparisons = {}
            for key, answer in response['answers'].items():
                other = ref_response['answers'][key]
                delta = (abs(answer['noul']-other['noul']) if answer['type']=='noul' else
                         max(abs(value-other['probabilities'][option]) for option,value in answer['probabilities'].items()))
                comparisons[key] = {'type': answer['type'], 'native_top_answer': rules.top_answer(answer),
                    'reference_top_answer': rules.top_answer(other),
                    'matches_reference': rules.top_answer(answer)==rules.top_answer(other),
                    'native_top_two_margin': rules.top_two_margin(answer),
                    'reference_top_two_margin': rules.top_two_margin(other),
                    'maximum_probability_difference': delta}
            record['cases'].append({'id':case['id'], 'request_sha256':sha(request_path),
                'source_request_sha256':case['source_request_sha256'], 'response_sha256':sha(response_path),
                'http_status':http['status'], 'response':response, 'top_answers':selected,
                'matches_reference':selected==old['top_answers'], 'matches_expected':True,
                'tokens_match':response['usage']['input_tokens']==ref_response['usage']['input_tokens'],
                'question_comparisons':comparisons,
                'maximum_probability_difference':max(x['maximum_probability_difference'] for x in comparisons.values())})
        rules.summarize_native(record, True)
        record['requests'] = len(record['cases'])
        comparisons = [x for row in record['cases'] for x in row['question_comparisons'].values()]
        record['questions'] = len(comparisons)
        record['top_agreements'] = sum(x['matches_reference'] for x in comparisons)
        record['mismatches'] = [dict(id=row['id'],question=question,**comparison)
            for row in record['cases'] for question,comparison in row['question_comparisons'].items()
            if not comparison['matches_reference']]
        modes.append(record)
    result = {'kind':'stage6_offline_reference_comparison', 'model_execution':False,
              'script_sha256':sha(__file__), 'reference_sha256':sha(args.reference),
              'validation_rules_sha256':sha(args.rules), 'modes':modes}
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
    print(json.dumps([{key: mode[key] for key in ['mode','requests','questions','top_agreements',
        'maximum_probability_difference','original_M1_numeric_gate','top_answers_and_tokens_match','passed']} for mode in modes],indent=2))

if __name__ == '__main__':
    main()
