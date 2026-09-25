#!/usr/bin/env python3
"""Join already-frozen predictions to recorded comparator outcomes."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path

helper=Path(__file__).parent.parent/'2026-09-25-truthful-checks/measure.py'
spec=importlib.util.spec_from_file_location('intervals',helper)
intervals=importlib.util.module_from_spec(spec);spec.loader.exec_module(intervals)
p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--predictions',type=Path,required=True)
p.add_argument('--baseline',type=Path,required=True)
p.add_argument('--signal',default='verdict.fusion')
p.add_argument('--out',type=Path,required=True)
a=p.parse_args()
v=json.loads(a.predictions.read_text())
if v.get('unresolved',0):raise ValueError('Predictions still need inference')
baseline={(r['job'],r['trial']):r for r in json.loads(a.baseline.read_text())['predictions']}
rows=[];unknown=[];seen=set()
for prediction in v['predictions']:
    key=prediction['job'],prediction['trial']
    if key in seen:raise ValueError('Duplicate candidate')
    seen.add(key);b=baseline[key]
    if prediction['task'].split('/')[-1]!=b['task'].split('/')[-1]:raise ValueError('Task mismatch')
    row=dict(prediction);row['reward']=b['reward'];row['calls']=dict(b['calls'])|prediction['calls']
    if b['reward'] not in [0,1]:unknown.append(row)
    else:rows.append(row)
signals=['checks.final','verdict.combined',a.signal]
result=dict(partition=v['partition'],trials=len(rows),tasks=len({r['task'] for r in rows}),unknown_outcomes=unknown,
            prediction_sha256=hashlib.sha256(a.predictions.read_bytes()).hexdigest(),
            signals={s:intervals.stats(rows,s) for s in signals},
            paired_task_bootstrap={s:intervals.paired_bootstrap(rows,a.signal,s) for s in signals[:-1]},
            tasks_table={t:{s:intervals.stats([r for r in rows if r['task']==t],s) for s in signals} for t in sorted({r['task'] for r in rows})},
            predictions=rows)
a.out.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k not in ['predictions','tasks_table','unknown_outcomes']},indent=2))
