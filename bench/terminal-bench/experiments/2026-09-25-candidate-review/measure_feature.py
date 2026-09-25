#!/usr/bin/env python3
"""Calibrate or compare a retained feature, keeping unavailable rows in recall."""
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
p.add_argument('--baseline', type=Path, required=True)
p.add_argument('--records', type=Path, required=True)
p.add_argument('--partition', required=True)
p.add_argument('--feature', choices=['readiness', 'public-program', 'public-program-bounded'], required=True)
p.add_argument('--threshold', type=float)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
manifest = [r for r in json.loads(a.manifest.read_text()) if r['split'] == a.partition]
baseline = {(r['job'],r['trial']):r for r in json.loads(a.baseline.read_text())['predictions']}
rows = []
for item in manifest:
    original = baseline[(item['job'], item['trial'])]
    path = a.records / item['trial'] / a.feature / ('assessment.json' if a.feature == 'readiness' else 'observation.json')
    record = json.loads(path.read_text()) if path.exists() else None
    score = None
    if record and not record.get('error'):
        score = (record.get('assessment') or {}).get('failure_probability') if a.feature == 'readiness' else record.get('score')
    rows.append(dict(job=item['job'], trial=item['trial'], task=original['task'], reward=original['reward'],
        calls=dict(original['calls']), score=score, available=record is not None,
        error=record.get('error') if record else 'No record',
        known_cost_usd=(record.get('reply') or {}).get('cost_usd') if record else None,
        milliseconds=record.get('milliseconds') if record else None))

def measure(t):
    for r in rows:
        r['calls'][a.feature] = 'fail' if r['score'] is not None and r['score'] >= t else None
    return intervals.stats(rows, a.feature)

thresholds = [0.5,0.7,0.8,0.9,0.95] if a.partition == 'calibration' and a.threshold is None else [a.threshold]
if None in thresholds:
    raise ValueError('Comparison requires a frozen threshold')
table = {str(t):measure(t) for t in thresholds}
chosen = a.threshold
if chosen is None:
    eligible = [t for t in thresholds if table[str(t)]['fail_precision']['total']>=5 and table[str(t)]['fail_precision']['value']>=.9]
    if eligible:
        chosen = max(eligible, key=lambda t:(table[str(t)]['failure_recall']['correct'],t))
result = dict(partition=a.partition, feature=a.feature, trials=len(rows), tasks=len({r['task'] for r in rows}),
    available=sum(r['available'] for r in rows), chosen=chosen, threshold_table=table,
    known_cost_usd=sum(r['known_cost_usd'] or 0 for r in rows))
if chosen is not None:
    measure(chosen)
    result.update(signals={s:intervals.stats(rows,s) for s in ['checks.final','verdict.combined',a.feature]},
        paired_task_bootstrap={s:intervals.paired_bootstrap(rows,a.feature,s) for s in ['checks.final','verdict.combined']},
        tasks_table={t:{s:intervals.stats([r for r in rows if r['task']==t],s) for s in ['checks.final','verdict.combined',a.feature]} for t in sorted({r['task'] for r in rows})})
result['predictions'] = rows
a.out.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k not in ['predictions','tasks_table']},indent=2))
