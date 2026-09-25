#!/usr/bin/env python3
"""Join frozen review records to labels only in an explicitly selected partition."""
import argparse
import collections
import importlib.util
import json
from pathlib import Path

helper = Path(__file__).parent.parent / '2026-09-25-truthful-checks/measure.py'
spec = importlib.util.spec_from_file_location('intervals', helper)
intervals = importlib.util.module_from_spec(spec)
spec.loader.exec_module(intervals)

p = argparse.ArgumentParser()
p.add_argument('--manifest', type=Path, required=True)
p.add_argument('--rows', type=Path, required=True)
p.add_argument('--baseline', type=Path, required=True)
p.add_argument('--partition', choices=['calibration', 'held-out', 'prospective'], required=True)
p.add_argument('--threshold', type=float)
p.add_argument('--record', choices=['record','normalized'], default='record')
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
manifest = json.loads(a.manifest.read_text())
labels = {(r['job'],r['trial']):r for r in map(json.loads,a.rows.read_text().splitlines())}
baseline = {(r['job'],r['trial']):r for r in json.loads(a.baseline.read_text())['predictions']}
rows=[]
for item in manifest:
    if item['split'] != a.partition:
        continue
    key = (item['job'], item['trial'])
    label = labels[key]
    record = None
    if item.get('input'):
        path = Path(item['input']).parent / a.record / 'review.json'
        if path.exists():
            record = json.loads(path.read_text())
    scores = [f['score'] for f in record['findings'] if f['score'] is not None] if record else []
    rows.append({'job':key[0], 'trial':key[1], 'task':label['task'], 'reward':label['reward'],
                 'score':max(scores) if scores else None, 'reviewed':bool(record),
                 'known_luna_cost_usd': (record.get('reply') or {}).get('cost_usd') if record else None,
                 'known_jev_input_tokens':sum(f.get('jev_input_tokens') or 0 for f in record['findings']) if record else 0,
                 'milliseconds':record['milliseconds'] if record else None,
                 'calls': baseline[key]['calls'] if key in baseline else {}})

def measure(threshold):
    for row in rows:
        row['calls']['review'] = 'fail' if row['score'] is not None and row['score'] >= threshold else None
    return intervals.stats(rows,'review')

thresholds = [0.5,0.7,0.8,0.9,0.95] if a.partition == 'calibration' and a.threshold is None else [a.threshold]
if None in thresholds:
    raise ValueError('Comparison requires an explicitly frozen threshold')
table = {str(t):measure(t) for t in thresholds}
chosen = a.threshold
if a.partition == 'calibration' and chosen is None:
    eligible = [t for t in thresholds if table[str(t)]['fail_precision']['total'] >= 5 and table[str(t)]['fail_precision']['value'] >= 0.9]
    if eligible:
        chosen = max(eligible,key=lambda t:(table[str(t)]['failure_recall']['correct'],t))
result={'partition':a.partition,'trials':len(rows),'tasks':len(set(r['task'] for r in rows)),
        'threshold_table':table,'chosen':chosen,'reviewed':sum(r['reviewed'] for r in rows),
        'known_luna_cost_usd':sum(r['known_luna_cost_usd'] or 0 for r in rows),
        'known_jev_input_tokens':sum(r['known_jev_input_tokens'] for r in rows)}
if chosen is not None:
    measure(chosen)
    result['signals']={s:intervals.stats(rows,s) for s in ['checks.final','verdict.combined','review']}
    result['paired_task_bootstrap']={s:intervals.paired_bootstrap(rows,'review',s) for s in ['checks.final','verdict.combined']}
    result['tasks_table']={task:{s:intervals.stats([r for r in rows if r['task']==task],s) for s in ['checks.final','verdict.combined','review']} for task in sorted(set(r['task'] for r in rows))}
result['predictions']=rows
a.out.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k not in ['predictions','tasks_table']},indent=2))
