#!/usr/bin/env python3
"""Require complete frozen coverage and summarize paired GPU command times."""
import argparse
from collections import Counter
import json
import math
import re
from pathlib import Path
import statistics

parser = argparse.ArgumentParser()
parser.add_argument('run', type=Path)
args = parser.parse_args()
receipt = json.loads((args.run / 'receipt.json').read_text())
rows = [json.loads(line) for line in (args.run / 'stdout.jsonl').read_text().splitlines()]
complete = [x for x in rows if x.get('kind') == 'complete']
roundtrips = [x for x in rows if x.get('kind') == 'input_roundtrip']
roundtrip_keys = Counter((x['n'], x['m'], x['k']) for x in roundtrips)
correctness = [x for x in rows if x.get('kind') == 'correctness_complete']
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
small_cases = json.loads((Path(__file__).resolve().parent / 'small-cases.json').read_text())['cases']
small_expected = {(n, m, k, acc) for n, m, k, _ in small_cases for acc in [False, True]}
roundtrip_expected = {(n, m, k) for n, m, k, _ in small_cases}
fixture_expected = {(n, m, k): fixture for n, m, k, fixture in small_cases}
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
def digest_ok(value): return isinstance(value, str) and re.fullmatch(r'[0-9a-f]{64}', value) is not None
def scope_bytes(n, name): return n * (4096 if name == 'down' else 12288) * (2 if name == 'activation' else 4)
def typed_comparison(value, n, name):
    return comparison_ok(value) and value['half'] == (name == 'activation') and value['values'] == n * (4096 if name == 'down' else 12288)
def repeat_ok(value):
    return value.get('passed') is True and value.get('bitwise_equal') is True and type(value.get('bytes')) is int and value['bytes'] > 0
def expected_comparisons(scope):
    return {'gate', 'up', 'activation', 'down'} if scope == 'full_ffn_accumulate' else {'down' if scope == 'down_accumulate' else scope}
valid = (receipt['status'] == 'complete' and receipt.get('exit') == 0 and receipt.get('memory', {}).get('owned_group_stopped') is True
         and len(complete) == 1 and complete[0]['passed'] and complete[0]['timing_included'] == bench
         and set(roundtrip_keys) == roundtrip_expected and all(x == 1 for x in roundtrip_keys.values())
         and len(roundtrips) == 12 and all(x['passed'] is True and x['guards_and_inputs_unchanged'] is True
             and x['slot_holes_unchanged'] is True and x['objects'] == 8 and x['logical_elements'] == 3072
             and 0 <= x['padded_zero_elements'] <= 3072
             and len(x['capacities_by_object_lane']) == 8
             and all(len(c) == 32 and all(type(v) is int and 1 <= v <= 64 for v in c) for c in x['capacities_by_object_lane'])
             for x in roundtrips)
         and len(correctness) == 1 and correctness[0]['roundtrip_checks'] == 12
         and correctness[0]['small_checks'] == 24 and correctness[0]['weighted_checks'] == 5
         and correctness[0]['fixed_f32_absolute_error_bound'] == .001
         and correctness[0]['half_activation_absolute_or_ulp_bound'] == [.001, 1]
         and correctness[0]['criterion_version'] == 2
         and set(small_keys) == small_expected and all(x == 1 for x in small_keys.values())
         and set(weighted_keys) == weighted_expected and all(x == 1 for x in weighted_keys.values())
         and complete[0]['commands'] == (319 if bench else 70) and complete[0]['dispatches'] == (547 if bench else 100)
         and len(checks) == 29 and all(x['passed'] and x['guards_and_inputs_unchanged'] for x in checks)
         and all(comparison_ok(x['comparison']) and oracle_ok(x['f64_oracle'])
                 and x['comparison']['half'] is False and x['comparison']['values'] == x['n'] * x['m']
                 and x['fixture'] == fixture_expected[x['n'], x['m'], x['k']]
                 and x['f64_oracle']['all_outputs'] and x['f64_oracle']['sample_count'] == x['n'] * x['m']
                 for x in checks if x['kind'] == 'small_correctness')
         and all(set(x['comparisons']) == {'gate', 'up', 'activation', 'down'}
                 and all(typed_comparison(v, x['n'], name) for name, v in x['comparisons'].items())
                 and set(x['f64_oracles']) == {'gate', 'up', 'down'}
                 and all(oracle_ok(v) and v['sample_count'] == 96 and v['all_outputs'] is False for v in x['f64_oracles'].values())
                 for x in checks if x['kind'] == 'weighted_correctness')
         and set(expectation_keys) == ({512,1024,2048} if bench else set())
         and all(x == 1 for x in expectation_keys.values())
         and all(set(x['comparisons']) == {'gate','up','activation','down'}
                 and all(typed_comparison(v, x['n'], name) for name, v in x['comparisons'].items())
                 and typed_comparison(x['standalone_down'], x['n'], 'down') and oracle_ok(x['standalone_down_f64'])
                 and x['standalone_down_f64']['sample_count'] == 96
                 and len(x['baseline_sha256']) == 4 and len(x['candidate_sha256']) == 4
                 and len(x['standalone_down_sha256']) == 1
                 and all(digest_ok(h) for k in ['baseline_sha256','candidate_sha256','standalone_down_sha256'] for h in x[k])
                 for x in expectations)
         and len(timings) == len(expected) and set(counts) == expected and all(x == 1 for x in counts.values())
         and all(x['variant'] == ['baseline', 'register_input', 'register_input', 'baseline'][x['slot']]
                 and x['output_gate_passed'] and set(x['repeats']) == expected_comparisons(x['scope'])
                 and all(repeat_ok(v) and v['bytes'] == scope_bytes(x['n'], name) for name, v in x['repeats'].items())
                 and math.isfinite(x['gpu_ms']) and x['gpu_ms'] > 0
                 and math.isfinite(x['wall_ms']) and x['wall_ms'] > 0 for x in timings)
         and len([x for x in rows if x.get('kind') == 'half_ulp_self_test']) == 1
         and all(x.get('passed') is True and x.get('criterion_version') == 2 and x.get('cases') == 19 for x in rows if x.get('kind') == 'half_ulp_self_test')
         and receipt['mode'] in ['check','bench']
         and not any(x.get('kind') in ['failure','oracle_failure','numerical_failure','roundtrip_failure','repeat_failure','nonfinite_failure'] for x in rows)
         and complete[0]['full_model_loaded'] is False
         and complete[0]['explicit_peak_bytes'] <= 1024**3
         and complete[0]['combined_driver_and_expected_peak_bytes'] <= 1536*1024**2)
result = {'coverage_and_correctness_passed': valid, 'numerical_gate': 'exact cooperative input/residual roundtrips before GEMM; f32 gate/up/down absolute .001; finite half activation absolute .001 OR at most one half ULP; independent f64 checks; exact matching-variant repeat bytes; frozen before first compile/run',
          'timing_included': bench, 'checks': len(checks), 'exact_input_roundtrips': len(roundtrips), 'timing_rows': len(timings),
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
                                     'register_input_median_gpu_ms': statistics.median(candidate),
                                     'paired_register_input_over_baseline_median': statistics.median(ratios),
                                     'paired_ratios': ratios}
        result['by_tokens'].append(item)
    result['material_full_ffn_gain_all_sizes'] = all(
        x['scopes']['full_ffn_accumulate']['paired_register_input_over_baseline_median'] <= 0.90
        and x['scopes']['projection_sum']['paired_register_input_over_baseline_median'] <= 0.90
        for x in result['by_tokens'])
(args.run / 'summary.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result, indent=2))
raise SystemExit(0 if valid else 1)
