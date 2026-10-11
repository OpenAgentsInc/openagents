#!/usr/bin/env python3
"""Require complete frozen coverage and summarize paired GPU command times."""
import argparse
from collections import Counter
import json
import math
from pathlib import Path
import statistics

parser = argparse.ArgumentParser()
parser.add_argument('run', type=Path)
args = parser.parse_args()
receipt = json.loads((args.run / 'receipt.json').read_text())
rows = [json.loads(line) for line in (args.run / 'stdout.jsonl').read_text().splitlines()]
complete = [x for x in rows if x.get('kind') == 'complete']
checks = [x for x in rows if x.get('kind') in {'small_correctness', 'weighted_correctness'}]
timings = [x for x in rows if x.get('kind') == 'timing']
bench = len(complete) == 1 and complete[0]['timing_included']
expected = {(n, rnd, slot, scope) for n in [512, 1024, 2048]
            for rnd in range(5) for slot in range(4)
            for scope in ['gate', 'up', 'down_accumulate', 'full_ffn_accumulate']} if bench else set()
counts = Counter((x['n'], x['round'], x['slot'], x['scope']) for x in timings)
small_keys = Counter((x['n'], x['m'], x['k'], x['accumulate']) for x in checks if x['kind'] == 'small_correctness')
weighted_keys = Counter((x['n'], x['accumulate']) for x in checks if x['kind'] == 'weighted_correctness')
small_expected = {(n, m, k, acc) for n, m, k in [(1,1,1),(3,7,127),(7,129,257),(63,1,4096),(64,128,65),(65,129,257),(257,1,12288)] for acc in [False, True]}
weighted_expected = {(65, False), (65, True), (512, True), (1024, True), (2048, True)}
def comparison_ok(value):
    return (value['passed'] and value['above_0_001'] == 0
            and math.isfinite(value['max_abs']) and 0 <= value['max_abs'] <= .001
            and value['values'] > 0 and not value['first_failures'])
def oracle_ok(value):
    return (value['passed'] and value['sample_count'] > 0
            and all(math.isfinite(value[k]) and 0 <= value[k] <= .001 for k in ['baseline_max_abs', 'candidate_max_abs']))
def expected_comparisons(scope):
    return {'gate', 'up', 'activation', 'down'} if scope == 'full_ffn_accumulate' else {'down' if scope == 'down_accumulate' else scope}
valid = (receipt['status'] == 'complete' and len(complete) == 1 and complete[0]['passed']
         and set(small_keys) == small_expected and all(x == 1 for x in small_keys.values())
         and set(weighted_keys) == weighted_expected and all(x == 1 for x in weighted_keys.values())
         and complete[0]['commands'] == (281 if bench else 38) and complete[0]['dispatches'] == (500 if bench else 68)
         and len(checks) == 19 and all(x['passed'] and x['guards_and_inputs_unchanged'] for x in checks)
         and all(comparison_ok(x['comparison']) and oracle_ok(x['f64_oracle'])
                 and x['f64_oracle']['all_outputs'] and x['f64_oracle']['sample_count'] == x['n'] * x['m']
                 for x in checks if x['kind'] == 'small_correctness')
         and all(set(x['comparisons']) == {'gate', 'up', 'activation', 'down'}
                 and all(comparison_ok(v) for v in x['comparisons'].values())
                 and set(x['f64_oracles']) == {'gate', 'up', 'down'}
                 and all(oracle_ok(v) and v['sample_count'] == 96 for v in x['f64_oracles'].values())
                 for x in checks if x['kind'] == 'weighted_correctness')
         and len(timings) == len(expected) and set(counts) == expected and all(x == 1 for x in counts.values())
         and all(x['variant'] == ['baseline', 'two_simd', 'two_simd', 'baseline'][x['slot']]
                 and x['output_gate_passed'] and set(x['comparisons']) == expected_comparisons(x['scope'])
                 and all(comparison_ok(v) for v in x['comparisons'].values())
                 and math.isfinite(x['gpu_ms']) and x['gpu_ms'] > 0 for x in timings))
result = {'coverage_and_correctness_passed': valid, 'numerical_gate': 'fixed absolute .001 for every gate/up/half-activation/down output; bit differences reported',
          'timing_included': bench, 'checks': len(checks), 'timing_rows': len(timings),
          'scope': 'same-weight synthetic-input projections; shared-machine microbenchmark, not model latency',
          'conditions': 'fixed ABBA order within each round; temporal drift and cache effects remain possible',
          'by_tokens': []}
if valid and bench:
    lookup = {(x['n'], x['round'], x['slot'], x['scope']): x['gpu_ms'] for x in timings}
    for n in [512, 1024, 2048]:
        item = {'n': n, 'scopes': {}}
        for scope in ['gate', 'up', 'down_accumulate', 'projection_sum', 'full_ffn_accumulate']:
            def value(rnd, slot):
                if scope == 'projection_sum':
                    return sum(lookup[n, rnd, slot, s] for s in ['gate', 'up', 'down_accumulate'])
                return lookup[n, rnd, slot, scope]
            baseline = [value(rnd, slot) for rnd in range(5) for slot in [0, 3]]
            candidate = [value(rnd, slot) for rnd in range(5) for slot in [1, 2]]
            ratios = [value(rnd, b) / value(rnd, a) for rnd in range(5) for a, b in [(0, 1), (3, 2)]]
            item['scopes'][scope] = {'baseline_median_gpu_ms': statistics.median(baseline),
                                     'two_simd_median_gpu_ms': statistics.median(candidate),
                                     'paired_two_simd_over_baseline_median': statistics.median(ratios),
                                     'paired_ratios': ratios}
        result['by_tokens'].append(item)
    result['material_full_ffn_gain_all_sizes'] = all(
        x['scopes']['full_ffn_accumulate']['paired_two_simd_over_baseline_median'] <= 0.90
        and x['scopes']['projection_sum']['paired_two_simd_over_baseline_median'] <= 0.90
        for x in result['by_tokens'])
(args.run / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
raise SystemExit(0 if valid else 1)
