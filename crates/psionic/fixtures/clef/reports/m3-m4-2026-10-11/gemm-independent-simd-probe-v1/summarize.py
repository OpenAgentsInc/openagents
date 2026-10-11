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
expectations = [x for x in rows if x.get('kind') == 'expectations_checked']
expectation_keys = Counter(x['n'] for x in expectations)
bench = receipt['mode'] == 'bench'
expected = {(n, rnd, slot, scope) for n in [512, 1024, 2048]
            for rnd in range(5) for slot in range(4)
            for scope in ['gate', 'up', 'down_accumulate', 'full_ffn_accumulate']} if bench else set()
counts = Counter((x['n'], x['round'], x['slot'], x['scope']) for x in timings)
small_keys = Counter((x['n'], x['m'], x['k'], x['accumulate']) for x in checks if x['kind'] == 'small_correctness')
weighted_keys = Counter((x['n'], x['accumulate']) for x in checks if x['kind'] == 'weighted_correctness')
small_expected = {(n, m, k, acc) for n, m, k in [(1,1,1),(3,7,127),(7,129,257),(63,1,4096),(64,128,65),(65,129,257),(257,1,12288),(1,33,4096),(33,1,12288)] for acc in [False, True]}
weighted_expected = {(65, False), (65, True), (512, True), (1024, True), (2048, True)}
def comparison_ok(value):
    common = (value['passed'] is True and value['criterion_version'] == 2
              and value['criterion_violations'] == 0 and math.isfinite(value['max_abs'])
              and value['max_abs'] >= 0 and value['values'] > 0 and not value['first_failures'])
    if value['half']:
        # The output loop counts the intersection of abs>.001 and ULP>1;
        # separate maxima alone cannot reconstruct this per-element OR gate.
        return common and value['half_max_ulp'] >= 0 and value['half_above_one_ulp'] >= 0
    return common and value['above_0_001'] == 0 and value['max_abs'] <= .001
def oracle_ok(value):
    return (value['passed'] and value['sample_count'] > 0
            and all(math.isfinite(value[k]) and 0 <= value[k] <= .001 for k in ['baseline_max_abs', 'candidate_max_abs']))
def repeat_ok(value):
    return value.get('passed') is True and value.get('bitwise_equal') is True and type(value.get('bytes')) is int and value['bytes'] > 0
def expected_comparisons(scope):
    return {'gate', 'up', 'activation', 'down'} if scope == 'full_ffn_accumulate' else {'down' if scope == 'down_accumulate' else scope}
valid = (receipt['status'] == 'complete' and receipt.get('exit') == 0 and receipt.get('memory', {}).get('owned_group_stopped') is True
         and len(complete) == 1 and complete[0]['passed'] and complete[0]['timing_included'] == bench
         and set(small_keys) == small_expected and all(x == 1 for x in small_keys.values())
         and set(weighted_keys) == weighted_expected and all(x == 1 for x in weighted_keys.values())
         and complete[0]['commands'] == (295 if bench else 46) and complete[0]['dispatches'] == (523 if bench else 76)
         and len(checks) == 23 and all(x['passed'] and x['guards_and_inputs_unchanged'] for x in checks)
         and all(comparison_ok(x['comparison']) and oracle_ok(x['f64_oracle'])
                 and x['f64_oracle']['all_outputs'] and x['f64_oracle']['sample_count'] == x['n'] * x['m']
                 for x in checks if x['kind'] == 'small_correctness')
         and all(set(x['comparisons']) == {'gate', 'up', 'activation', 'down'}
                 and all(comparison_ok(v) for v in x['comparisons'].values())
                 and set(x['f64_oracles']) == {'gate', 'up', 'down'}
                 and all(oracle_ok(v) and v['sample_count'] == 96 for v in x['f64_oracles'].values())
                 for x in checks if x['kind'] == 'weighted_correctness')
         and set(expectation_keys) == ({512,1024,2048} if bench else set())
         and all(x == 1 for x in expectation_keys.values())
         and all(set(x['comparisons']) == {'gate','up','activation','down'}
                 and all(comparison_ok(v) for v in x['comparisons'].values())
                 and comparison_ok(x['standalone_down']) and oracle_ok(x['standalone_down_f64'])
                 and x['standalone_down_f64']['sample_count'] == 96
                 and len(x['baseline_sha256']) == 4 and len(x['candidate_sha256']) == 4
                 and len(x['standalone_down_sha256']) == 1
                 for x in expectations)
         and len(timings) == len(expected) and set(counts) == expected and all(x == 1 for x in counts.values())
         and all(x['variant'] == ['baseline', 'independent_simd', 'independent_simd', 'baseline'][x['slot']]
                 and x['output_gate_passed'] and set(x['repeats']) == expected_comparisons(x['scope'])
                 and all(repeat_ok(v) for v in x['repeats'].values())
                 and math.isfinite(x['gpu_ms']) and x['gpu_ms'] > 0
                 and math.isfinite(x['wall_ms']) and x['wall_ms'] > 0 for x in timings)
         and len([x for x in rows if x.get('kind') == 'half_ulp_self_test']) == 1
         and all(x.get('passed') is True and x.get('criterion_version') == 2 and x.get('cases') == 19 for x in rows if x.get('kind') == 'half_ulp_self_test')
         and complete[0]['explicit_peak_bytes'] <= 1024**3
         and complete[0]['combined_driver_and_expected_peak_bytes'] <= 1536*1024**2)
result = {'coverage_and_correctness_passed': valid, 'numerical_gate': 'f32 gate/up/down absolute .001; finite half activation absolute .001 OR at most one half ULP; independent f64 checks; exact matching-variant repeat bytes; frozen before first compile/run',
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
                                     'independent_simd_median_gpu_ms': statistics.median(candidate),
                                     'paired_independent_simd_over_baseline_median': statistics.median(ratios),
                                     'paired_ratios': ratios}
        result['by_tokens'].append(item)
    result['material_full_ffn_gain_all_sizes'] = all(
        x['scopes']['full_ffn_accumulate']['paired_independent_simd_over_baseline_median'] <= 0.90
        and x['scopes']['projection_sum']['paired_independent_simd_over_baseline_median'] <= 0.90
        for x in result['by_tokens'])
(args.run / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
raise SystemExit(0 if valid else 1)
