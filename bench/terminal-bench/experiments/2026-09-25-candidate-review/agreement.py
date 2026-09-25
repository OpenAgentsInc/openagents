#!/usr/bin/env python3
"""Calibrate agreement on development rows or apply a previously frozen rule."""
import argparse
import importlib.util
import itertools
import json
from pathlib import Path

helper = Path(__file__).parent.parent / '2026-09-25-truthful-checks/measure.py'
spec = importlib.util.spec_from_file_location('intervals',helper)
intervals = importlib.util.module_from_spec(spec)
spec.loader.exec_module(intervals)
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--source',type=Path,required=True)
p.add_argument('--readiness',type=Path,required=True)
p.add_argument('--rule',type=Path)
p.add_argument('--out',type=Path,required=True)
a = p.parse_args()
source = json.loads(a.source.read_text())
ready = json.loads(a.readiness.read_text())
if source['partition'] != ready['partition']:
    raise ValueError('Mismatched partitions')
ready_rows={(r['job'],r['trial']):r for r in ready['predictions']}
rows=source['predictions']
if set(ready_rows) != {(r['job'],r['trial']) for r in rows}:
    raise ValueError('Mismatched candidate sets')
for row in rows:
    other=ready_rows[(row['job'],row['trial'])]
    if (row['task'],row['reward']) != (other['task'],other['reward']):
        raise ValueError('Mismatched labels')
    row['readiness_score']=other['score']

def apply(source_at,ready_at):
    for r in rows:
        audit=r['report_score'] is not None and r['report_score'] >= .5
        agreement=r['score'] is not None and r['score']>=source_at and r['readiness_score'] is not None and r['readiness_score']>=ready_at
        r['calls']['verdict.agreement']='fail' if audit or agreement else None
    return intervals.stats(rows,'verdict.agreement')

rule=json.loads(a.rule.read_text()) if a.rule else None
table=[]
if rule is None:
    if source['partition']!='calibration':
        raise ValueError('Comparison requires a frozen rule')
    for s,r in itertools.product([.5,.7,.8,.9,.95],repeat=2):
        table.append(dict(source_at=s,readiness_at=r,metrics=apply(s,r)))
    eligible=[r for r in table if r['metrics']['fail_precision']['total']>=5 and r['metrics']['fail_precision']['value']>=.9]
    if eligible:
        chosen=max(eligible,key=lambda r:(r['metrics']['failure_recall']['correct'],r['metrics']['fail_precision']['correct']-r['metrics']['fail_precision']['total'],r['source_at'],r['readiness_at']))
        rule=dict(schema='openagents.agreement-rule.v1',source_at=chosen['source_at'],readiness_at=chosen['readiness_at'],report_at=.5,
            meaning='fail if concrete report audit reaches 0.5 or both source and readiness reach their cutoffs; otherwise unknown',
            calibration=chosen['metrics'])
result=dict(partition=source['partition'],trials=len(rows),tasks=len({r['task'] for r in rows}),rule=rule,calibration_grid=table)
if rule:
    if rule['report_at']!=.5:
        raise ValueError('The audit cutoff was frozen at 0.5')
    apply(rule['source_at'],rule['readiness_at'])
    signals=['checks.final','verdict.combined','verdict.agreement']
    result.update(signals={s:intervals.stats(rows,s) for s in signals},
        paired_task_bootstrap={s:intervals.paired_bootstrap(rows,'verdict.agreement',s) for s in signals[:-1]},
        tasks_table={t:{s:intervals.stats([r for r in rows if r['task']==t],s) for s in signals} for t in sorted({r['task'] for r in rows})})
result['predictions']=rows
a.out.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k not in ['calibration_grid','predictions','tasks_table']},indent=2))
