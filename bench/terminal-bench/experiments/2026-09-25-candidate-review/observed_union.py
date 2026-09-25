#!/usr/bin/env python3
"""Fit on calibration, or apply a frozen rule without reading official grades."""
import argparse
import importlib.util
import json
from pathlib import Path

helper = Path(__file__).parent.parent / '2026-09-25-truthful-checks/measure.py'
spec = importlib.util.spec_from_file_location('intervals', helper)
intervals = importlib.util.module_from_spec(spec)
spec.loader.exec_module(intervals)
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--manifest', type=Path, required=True)
p.add_argument('--records', type=Path, required=True)
p.add_argument('--baseline', type=Path)
p.add_argument('--partition', required=True)
p.add_argument('--rule', type=Path)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
manifest = [r for r in json.loads(a.manifest.read_text()) if r['split'] == a.partition]
baseline = {(r['job'], r['trial']): r for r in json.loads(a.baseline.read_text())['predictions']} if a.baseline else None
rows = []
for item in manifest:
    row = {k: item[k] for k in ['job', 'trial', 'task']}
    row['calls'] = {}
    if baseline is not None:
        b = baseline[(item['job'], item['trial'])]
        row.update(reward=b['reward'], calls=dict(b['calls']))
    for signal in ['execution-audit', 'report-audit']:
        path = a.records / f'{signal}-{a.partition}' / f"{item['trial']}.json"
        v = json.loads(path.read_text()) if path.exists() else {}
        row[signal] = v.get('score') if not v.get('error') else None
        row[signal + '-error'] = v.get('error') if v else 'No record'
        row[signal + '-digest'] = v.get('digest')
    rows.append(row)

def apply(rule):
    for r in rows:
        execution = r['execution-audit'] is not None and r['execution-audit'] >= rule['execution_at']
        report = rule.get('report_at') is not None and r['report-audit'] is not None and r['report-audit'] >= rule['report_at']
        r['calls']['verdict.observed'] = 'fail' if execution or report else None
    return intervals.stats(rows, 'verdict.observed') if baseline else None

rule = json.loads(a.rule.read_text()) if a.rule else None
grid = []
if rule is None:
    if a.partition != 'calibration' or baseline is None:
        raise ValueError('Only labeled calibration can fit a rule')
    for t in [.5,.7,.8,.9,.95]:
        candidate = dict(execution_at=.5, report_at=t)
        grid.append(dict(**candidate, metrics=apply(candidate)))
    eligible = [g for g in grid if g['metrics']['fail_precision']['total'] >= 5 and g['metrics']['fail_precision']['value'] >= .9]
    if eligible:
        best = max(eligible, key=lambda g:(g['metrics']['failure_recall']['correct'], g['metrics']['fail_precision']['correct']-g['metrics']['fail_precision']['total'],g['report_at']))
        rule = dict(execution_at=.5,report_at=best['report_at'])
    else:
        rule = dict(execution_at=.5,report_at=None)
    rule.update(schema='openagents.observed-verdict-rule.v1',calibration=apply(rule),
                meaning='Fail on an attributed unresolved execution failure or a concrete report admission; otherwise unknown. No pass calls.')
result = dict(partition=a.partition,trials=len(rows),tasks=len({r['task'] for r in rows}),rule=rule,calibration_grid=grid)
apply(rule)
if baseline:
    signals=['checks.final','verdict.combined','verdict.observed']
    result.update(signals={s:intervals.stats(rows,s) for s in signals},
        paired_task_bootstrap={s:intervals.paired_bootstrap(rows,'verdict.observed',s) for s in signals[:-1]},
        tasks_table={t:{s:intervals.stats([r for r in rows if r['task']==t],s) for s in signals} for t in sorted({r['task'] for r in rows})})
result['predictions']=rows
a.out.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k not in ['predictions','calibration_grid','tasks_table']},indent=2))
