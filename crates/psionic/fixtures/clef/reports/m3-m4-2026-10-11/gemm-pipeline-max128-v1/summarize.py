#!/usr/bin/env python3
"""Validate complete probe coverage before interpreting its fixed speed gate."""
import argparse
import json
import math
from pathlib import Path
import re
import statistics

SIZES = [1, 17, 63, 64, 65, 512, 1024, 2048]
TIMED = [512, 1024, 2048]
SCOPES = ['normalized_pair', 'full_ffn']

def need(condition, reason):
    if not condition:
        raise ValueError(reason)

def positive(value):
    return type(value) in (int, float) and math.isfinite(value) and value > 0

def analyze(rows, receipt):
    need(receipt.get('status') == 'complete' and receipt.get('exit') == 0, 'guard did not complete successfully')
    need(receipt.get('mode') == 'bench', 'receipt is not a benchmark')
    for key in ['cleanup_error', 'monitor_failure', 'error']:
        need(not receipt.get(key), 'guard failure: ' + key)
    need(receipt.get('max_owned_group_bytes') == 2 * 1024**3 and receipt.get('minimum_available_bytes') == 12 * 1024**3 and receipt.get('minimum_start_bytes') == 14 * 1024**3 and receipt.get('deadline_seconds') == 60, 'receipt bounds differ')
    memory = receipt.get('memory', {})
    need(memory.get('owned_group_stopped') is True, 'owned group cleanup is unconfirmed')
    for key in ['memory_rejection', 'sampling_error', 'cleanup_error']:
        need(not memory.get(key), 'memory monitor failure: ' + key)
    expected_keys = {(n, r, scope, position) for n in TIMED for r in range(5)
                     for scope in SCOPES for position in range(4)}
    times = {}
    correct = {}
    cases = {}
    ends, devices, weights = [], [], []
    for row in rows:
        kind = row.get('kind')
        if kind == 'timing':
            key = (row.get('n'), row.get('round'), row.get('scope'), row.get('position'))
            need(key in expected_keys and key not in times, 'unexpected or duplicate timing key')
            need(row.get('variant') == ('default' if key[-1] in (0, 3) else 'max128'), 'ABBA order changed')
            need(row.get('exact_output_check_passed') is True, 'timed output check failed')
            need(positive(row.get('gpu_seconds')) and positive(row.get('host_seconds')), 'invalid timing')
            times[key] = row
        elif kind == 'correctness':
            key = (row.get('n'), row.get('variant'))
            need(key[0] in SIZES and key[1] in ['default', 'max128'] and key not in correct, 'unexpected or duplicate correctness key')
            if key[1] == 'default':
                need(row.get('all_outputs_finite') is True, 'baseline contains nonfinite output')
                hashes = row.get('outputs_sha256', {})
                need(set(hashes) == {'gate', 'up', 'activation', 'residual'}, 'baseline output set differs')
                need(all(isinstance(h, str) and re.fullmatch('[0-9a-f]{64}', h) for h in hashes.values()), 'invalid output hash')
            else:
                need(row.get('all_outputs_bitwise_equal') is True, 'max128 output differs')
                need(row.get('output_bytes') == key[0] * (12288 * 10 + 4096 * 4), 'output coverage differs')
            correct[key] = row
        elif kind == 'case_complete':
            n = row.get('n')
            need(n in SIZES and n not in cases, 'unexpected or duplicate case')
            need(row.get('immutable_weights_and_expectations') is True and row.get('redzones') is True, 'immutable or redzone check failed')
            need(type(row.get('explicit_live_bytes')) is int and 0 < row['explicit_live_bytes'] <= 1024**3, 'explicit case allocation exceeds limit')
            need(type(row.get('owned_cpu_bytes')) is int and row['owned_cpu_bytes'] >= 0, 'invalid CPU copy count')
            need(type(row.get('device_current_allocated_bytes')) is int and row['device_current_allocated_bytes'] >= 0, 'invalid device allocation count')
            need(row['owned_cpu_bytes'] + row['device_current_allocated_bytes'] <= 1536 * 1024**2, 'combined case allocation exceeds limit')
            cases[n] = row
        elif kind == 'complete':
            ends.append(row)
        elif kind == 'device':
            devices.append(row)
        elif kind == 'weights':
            weights.append(row)
        else:
            raise ValueError('unexpected or failed row: ' + str(kind))
    need(set(times) == expected_keys, 'timing coverage incomplete')
    need(set(correct) == {(n, v) for n in SIZES for v in ['default', 'max128']}, 'correctness coverage incomplete')
    need(set(cases) == set(SIZES), 'case coverage incomplete')
    need(len(ends) == len(devices) == len(weights) == 1, 'missing or duplicate completion/device/weight rows')
    pipeline_rows = devices[0].get('pipelines', [])
    expected_pipelines = {(name, 'default') for name in ['clef_rms_norm_to_f16','clef_gemm_64x128','clef_gemm_64x128_accumulate','clef_silu_mul_to_f16']} | {(name, 'max128') for name in ['clef_gemm_64x128','clef_gemm_64x128_accumulate']}
    pipeline_keys = [(p.get('name'), p.get('variant')) for p in pipeline_rows]
    need(len(pipeline_keys) == len(expected_pipelines) and set(pipeline_keys) == expected_pipelines, 'pipeline coverage differs')
    for p in pipeline_rows:
        threads = 128 if 'gemm' in p['name'] else 256
        need(p.get('threads') == threads and p.get('thread_execution_width') == 32, 'pipeline thread geometry differs')
        need(type(p.get('static_threadgroup_bytes')) is int and p['static_threadgroup_bytes'] >= 0, 'invalid static threadgroup size')
        need(type(p.get('max_total_threads')) is int and p['max_total_threads'] >= threads, 'pipeline capacity too small')
        if p['variant'] == 'max128':
            need(p.get('requested_max_total_threads') == p['max_total_threads'] == 128 and p.get('thread_group_multiple_hint') is False, 'candidate descriptor differs')
        else:
            need(p.get('requested_max_total_threads') == 0, 'baseline descriptor limit changed')
    end = ends[0]
    need((end.get('commands'), end.get('dispatches'), end.get('timing_rows')) == (136, 560, 120), 'command coverage differs')
    need(end.get('owned_cpu_bytes_after_cases') == 0, 'CPU copies remain live')
    need(end.get('q4_conversion_executed') is False and weights[0].get('q4_conversion_executed') is False, 'on-demand conversion is outside scope')
    need(type(end.get('explicit_peak_bytes')) is int and 0 < end['explicit_peak_bytes'] <= 1024**3, 'explicit peak exceeds limit')
    need(type(end.get('combined_peak_bytes')) is int and 0 < end['combined_peak_bytes'] <= 1536 * 1024**2, 'combined peak exceeds limit')
    output = []
    for n in TIMED:
        for scope in SCOPES:
            ratios = []
            default, max128 = [], []
            for r in range(5):
                values = [times[(n, r, scope, p)]['gpu_seconds'] for p in range(4)]
                ratios.append((values[1] + values[2]) / (values[0] + values[3]))
                default += [values[0], values[3]]
                max128 += [values[1], values[2]]
            output.append({'n': n, 'scope': scope, 'round_ratios': ratios,
                           'median_paired_ratio': statistics.median(ratios),
                           'default_gpu_median_seconds': statistics.median(default),
                           'max128_gpu_median_seconds': statistics.median(max128)})
    qualified = all(r['median_paired_ratio'] <= .90 for r in output if r['scope'] == 'full_ffn')
    return {'correctness_passed': True, 'speed_qualified': qualified,
            'fixed_gate': 'full_ffn median of five ABBA max128/default ratios <=0.90 at every size',
            'commands': 136, 'dispatches': 560, 'timing_rows': 120, 'results': output,
            'complete': end, 'attribution': 'Aggregate GPU command intervals; no inference of register spills or hardware utilization.'}

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    result = {'correctness_passed': False, 'speed_qualified': False}
    try:
        receipt = json.loads((args.output / 'receipt.json').read_text())
        rows = [json.loads(line) for line in (args.output / 'stdout.jsonl').read_text().splitlines()]
        result = analyze(rows, receipt)
    except Exception as error:
        result['error'] = str(error)
    with (args.output / 'summary.json').open('x') as output:
        json.dump(result, output, indent=2, allow_nan=False); output.write('\n')
    print(json.dumps(result, allow_nan=False))
    return 0 if result['correctness_passed'] and result['speed_qualified'] else 2

if __name__ == '__main__':
    raise SystemExit(main())
