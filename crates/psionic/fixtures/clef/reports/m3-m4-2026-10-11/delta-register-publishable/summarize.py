#!/usr/bin/env python3
"""Summarize one completed probe without dropping failed checks."""
import argparse
import json
from pathlib import Path
import statistics

parser = argparse.ArgumentParser()
parser.add_argument('run', type=Path)
args = parser.parse_args()
rows = [json.loads(line) for line in (args.run / 'stdout.jsonl').read_text().splitlines() if line]
receipt = json.loads((args.run / 'receipt.json').read_text())
checks = [row for row in rows if row.get('kind') in {'correctness', 'actual_shape_correctness'}]
failures = [row for row in rows if row.get('kind') == 'failure']
timings = []
for n in [1024, 2048]:
    for scope in ['prepare-and-scan', 'scan-only']:
        matching = [row for row in rows if row.get('kind') == 'timing' and row['n'] == n and row['scope'] == scope]
        baseline = {row['round']: row for row in matching if row['variant'] == 'clef_delta_block_scan'}
        candidate = {row['round']: row for row in matching if row['variant'] == 'clef_delta_block_scan_register_columns'}
        common = sorted(set(baseline) & set(candidate))
        if not common:
            continue
        ratios = [candidate[r]['gpu_ms'] / baseline[r]['gpu_ms'] for r in common]
        timings.append({'n': n, 'scope': scope, 'paired_rounds': common,
                        'baseline_gpu_ms_median': statistics.median(baseline[r]['gpu_ms'] for r in common),
                        'candidate_gpu_ms_median': statistics.median(candidate[r]['gpu_ms'] for r in common),
                        'paired_gpu_ratio_median': statistics.median(ratios),
                        'paired_gpu_ratios': ratios,
                        'candidate_faster_rounds': sum(ratio < 1 for ratio in ratios)})
complete = [row for row in rows if row.get('kind') == 'complete']
bench = receipt['mode'] == 'bench'
expected_checks = 24 if bench else 22
expected_timings = 48 if bench else 0
passed = (receipt['status'] == 'complete' and len(complete) == 1 and not failures
          and len(checks) == expected_checks and all(row['passed'] for row in checks)
          and sum(row.get('kind') == 'timing' for row in rows) == expected_timings
          and (not bench or all(len(row['paired_rounds']) == 6 for row in timings) and len(timings) == 4))
result = {'passed': passed, 'receipt': receipt, 'checks': checks, 'failures': failures,
          'timings': timings, 'complete': complete,
          'scope': 'One shared-machine synthetic DeltaNet comparison at real Flash dimensions; no model inference.',
          'acceptance': 'Every output and final state value must match frozen block16 bitwise; the independent f64 oracle keeps the existing bounds.',
          'limitations': ['Synthetic inputs, not captured model activations.',
                          'Short timing, not sustained full-model latency.',
                          'No clock, thermal, or bandwidth attribution.',
                          'No promotion without the unchanged full-model logit gate.']}
destination = args.run / 'summary.json'
if destination.exists():
    raise SystemExit('refusing to overwrite an existing summary')
destination.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'summary': str(destination), 'passed': passed}))
raise SystemExit(0 if passed else 1)
