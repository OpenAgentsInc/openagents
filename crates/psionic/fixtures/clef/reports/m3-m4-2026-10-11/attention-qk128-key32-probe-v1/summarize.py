#!/usr/bin/env python3
"""Summarize every fixed paired attention sample, retaining numerical failures."""
from pathlib import Path
import json
import math
import statistics
import sys
root = Path(sys.argv[1])
rows = [json.loads(line) for line in (root / 'stdout.jsonl').read_text().splitlines() if line.strip()]
record = json.loads((root / 'receipt.json').read_text())
timings = [r for r in rows if r.get('kind') == 'timing']
results = []
for heads in [16, 24]:
    for total in [1024, 4096, 16384]:
        subset = [r for r in timings if r['heads'] == heads and r['total'] == total]
        pairs = []
        for round_id in range(5):
            by = {r['keys']: r for r in subset if r['round'] == round_id}
            if set(by) == {32,128}:
                pairs.append({'round':round_id,'baseline_seconds':by[32]['gpu_seconds'],'candidate_seconds':by[128]['gpu_seconds'],'candidate_baseline_ratio':by[128]['gpu_seconds']/by[32]['gpu_seconds'] if all(isinstance(by[k].get('gpu_seconds'),(int,float)) and math.isfinite(by[k]['gpu_seconds']) and by[k]['gpu_seconds'] > 0 for k in [32,128]) else None})
        results.append({'heads':heads,'total':total,'pairs':pairs,'median_paired_ratio':statistics.median(p['candidate_baseline_ratio'] for p in pairs) if pairs and all(p['candidate_baseline_ratio'] is not None for p in pairs) else None,'all_five_pairs':len(pairs)==5})
complete = [r for r in rows if r.get('kind') == 'complete']
errors = []
mode = record.get('mode')
bench = mode == 'bench'
if mode not in {'check','bench'}:
    errors.append('receipt is not a check or benchmark')
if record.get('status') != 'complete' or record.get('exit') != 0:
    errors.append('owned runtime did not complete successfully')
expected_keys = {(h,t,r,k) for h in [16,24] for t in [1024,4096,16384] for r in range(5) for k in [32,128]} if bench else set()
actual_keys = [(r.get('heads'),r.get('total'),r.get('round'),r.get('keys')) for r in timings]
if len(actual_keys) != len(expected_keys) or set(actual_keys) != expected_keys:
    errors.append('timing keys are missing, duplicated, or unexpected')
for r in timings:
    for field in ['gpu_seconds','wall_seconds']:
        v=r.get(field)
        if not isinstance(v,(int,float)) or not math.isfinite(v) or v <= 0:
            errors.append('timing is not finite and positive')
small=[r for r in rows if r.get('kind')=='small_complete']
if len(small)!=12 or {r.get('index') for r in small}!=set(range(12)):
    errors.append('small-case completion keys differ')
large=[r for r in rows if r.get('kind')=='large_complete']
if len(large)!=6 or {(r.get('heads'),r.get('total')) for r in large}!={(h,t) for h in [16,24] for t in [1024,4096,16384]}:
    errors.append('large-case completion keys differ')
checks=[r for r in rows if r.get('kind') in ['comparison','sample_oracle','score_diagnostic']]
if len(checks)!=(216 if bench else 156):
    errors.append('comparison count differs')
comparison_keys=[r.get('label') for r in checks if r['kind']=='comparison']
if len(comparison_keys)!=len(set(comparison_keys)):
    errors.append('duplicate comparison labels')
sample_keys=[(r.get('heads'),r.get('total'),r.get('row'),r.get('head'),r.get('keys')) for r in checks if r['kind']=='sample_oracle']
if len(sample_keys)!=72 or len(sample_keys)!=len(set(sample_keys)):
    errors.append('sample oracle keys differ')
scores=[r for r in rows if r.get('kind')=='score_diagnostic']
if len(scores)!=3 or {r.get('index') for r in scores}!={0,1,2}:
    errors.append('score diagnostics missing or duplicated')
for r in scores:
    if r.get('different_bits')!=0 or r.get('finite') is not True or not r.get('samples') or any(x.get('passed') is not True for x in r['samples']):
        errors.append('score bitwise or independent f64 gate failed')
for r in checks:
    if r.get('kind')=='comparison' and ('-128-32' in r.get('label','') or r.get('label','').startswith('score-')) and (r.get('bitwise_required') is not True or r.get('different_bits')!=0):
        errors.append('cross-variant or diagnostic output is not bitwise equal')
    v=r.get('max_abs')
    if r.get('passed') is not True or r.get('failures')!=0 or not isinstance(v,(int,float)) or not math.isfinite(v):
        errors.append('a comparison or oracle failed')
if len(complete)!=1 or complete[0].get('passed') is not True or complete[0].get('mode')!=mode or complete[0].get('commands')!=(140 if bench else 80) or complete[0].get('dispatches')!=(140 if bench else 80) or complete[0].get('checks')!=(216 if bench else 156):
    errors.append('final completion record differs')
summary = {'validation_passed':not errors,'validation_errors':sorted(set(errors)), 'status':record['status'],'complete':complete,'comparisons':[r for r in rows if r.get('kind') in ['comparison','sample_oracle','score_diagnostic']],'timings':results,'scope':'shared machine; one final attention chunk per operation, not whole model; five alternating pairs; all raw samples retained','model_weights_loaded':False,'gates_unchanged':True,'additional_gate':'cross-variant attention outputs and all diagnostic QK scores must be bitwise equal','resource_receipt':record}
(root / 'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps({'status':record['status'],'results':results,'completed':bool(complete)},indent=2))

raise SystemExit(0 if not errors else 1)
