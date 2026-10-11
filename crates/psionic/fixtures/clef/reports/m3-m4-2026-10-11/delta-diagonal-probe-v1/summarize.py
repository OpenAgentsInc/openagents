#!/usr/bin/env python3
"""Summarize every check and the fixed six paired timing rounds without dropping failures."""
import argparse,json,statistics
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('directory',type=Path);a=p.parse_args()
receipt=json.loads((a.directory/'receipt.json').read_text())
records=[json.loads(line) for line in (a.directory/'stdout.jsonl').read_text().splitlines() if line.strip()]
checks=[r for r in records if r.get('kind') in ('correctness','actual_shape_correctness')]
failures=[r for r in records if r.get('kind')=='failure']
complete=[r for r in records if r.get('kind')=='complete']
variants=['sequential','block32','diagonal32']
timings=[]
benchmarking=receipt.get('mode')=='bench'
if receipt.get('mode') not in ('check','bench'):
    failures.append({'kind':'wrong_receipt_mode','mode':receipt.get('mode')})
if len(complete)==1 and complete[0].get('timing_included') is not benchmarking:
    failures.append({'kind':'mode_completion_disagreement'})
if not benchmarking and any(r.get('kind')=='timing' for r in records):
    failures.append({'kind':'unexpected_timing_in_check_mode'})
shapes=[('Flash',1024),('Flash',2048),('27B',1024),('27B',2048)] if benchmarking else []
for shape,n in shapes:
    rows=[r for r in records if r.get('kind')=='timing' and r['model_shape']==shape and r['n']==n]
    samples={(r['variant'],r['round']):r for r in rows}
    expected={(v,i) for v in variants for i in range(6)}
    if len(samples)!=len(rows) or set(samples)!=expected:
        failures.append({'kind':'missing_or_duplicate_timing','model_shape':shape,'n':n,'observed':len(rows)})
        continue
    medians={v:statistics.median(samples[v,i]['gpu_ms'] for i in range(6)) for v in variants}
    ratios={v:[samples[v,i]['gpu_ms']/samples['sequential',i]['gpu_ms'] for i in range(6)] for v in variants[1:]}
    timings.append({'model_shape':shape,'n':n,'gpu_ms_medians':medians,'paired_ratios_vs_sequential':ratios,'paired_ratio_medians_vs_sequential':{v:statistics.median(xs) for v,xs in ratios.items()},'diagonal_vs_block_paired_ratios':[samples['diagonal32',i]['gpu_ms']/samples['block32',i]['gpu_ms'] for i in range(6)]})
expected_checks=26 if benchmarking else 22
passed=receipt.get('status')=='complete' and len(checks)==expected_checks and all(r.get('passed') is True for r in checks) and not failures and len(complete)==1 and complete[0].get('correctness_passed') is True
result={'status':'passed' if passed else 'failed','scope':'Synthetic one-scan comparison; not model-logit or sustained-latency acceptance.','correctness_bounds':'Unchanged 2e-5+2e-4*abs(reference); prepared n1 output exact versus sequential; candidate state exact versus original block.','timing_scope':'Production sequential scan versus each block preparation+scan; common normalization/kq prep excluded from all three.','receipt':receipt,'checks':checks,'failures':failures,'complete':complete,'timings':timings}
(a.directory/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'status':result['status'],'checks':len(checks),'timings':timings,'failures':failures},indent=2))
raise SystemExit(0 if passed else 1)
