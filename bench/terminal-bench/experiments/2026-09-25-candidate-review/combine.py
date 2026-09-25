#!/usr/bin/env python3
"""Measure the committed union without fitting on the comparison partition."""
import argparse
import importlib.util
import json
from pathlib import Path

helper = Path(__file__).parent.parent / '2026-09-25-truthful-checks/measure.py'
spec = importlib.util.spec_from_file_location('intervals', helper)
intervals = importlib.util.module_from_spec(spec)
spec.loader.exec_module(intervals)

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--reviews', type=Path, required=True)
p.add_argument('--audits', type=Path, required=True)
p.add_argument('--rule', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
review = json.loads(a.reviews.read_text())
rule = json.loads(a.rule.read_text())
source_at = rule['source_review']['threshold']
report_at = rule['report_audit']['threshold']
if review['chosen'] != source_at:
    raise ValueError('Review measurement must use the frozen source threshold')
rows = review['predictions']
for row in rows:
    path = a.audits / (row['trial'] + '.json')
    audit = json.loads(path.read_text()) if path.exists() else None
    if audit and (audit['job'], audit['trial']) != (row['job'], row['trial']):
        raise ValueError('Report audit belongs to a different candidate')
    row['report_score'] = audit['score'] if audit else None
    row['report_input_tokens'] = audit.get('input_tokens') if audit else None
    row['report_milliseconds'] = audit.get('milliseconds') if audit else None
    row['calls']['report.audit'] = 'fail' if row['report_score'] is not None and row['report_score'] >= report_at else None
    row['calls']['verdict.truthful'] = 'fail' if 'fail' in [row['calls']['report.audit'], row['calls']['review']] else None
signals = ['checks.final', 'verdict.combined', 'review', 'report.audit', 'verdict.truthful']
result = {'schema': 'openagents.truthful-checks-measurement.v1',
          'partition': review['partition'], 'trials': len(rows),
          'tasks': len({r['task'] for r in rows}), 'rule': rule,
          'signals': {s: intervals.stats(rows, s) for s in signals},
          'paired_task_bootstrap': {s: intervals.paired_bootstrap(rows, 'verdict.truthful', s)
                                    for s in ['checks.final', 'verdict.combined']},
          'known_reviewer_cost_usd': review['known_luna_cost_usd'],
          'known_jev_input_tokens': review['known_jev_input_tokens'] + sum(r['report_input_tokens'] or 0 for r in rows),
          'tasks_table': {t: {s: intervals.stats([r for r in rows if r['task'] == t], s) for s in signals}
                          for t in sorted({r['task'] for r in rows})},
          'predictions': rows}
a.out.write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({k: v for k, v in result.items() if k not in ['rule', 'tasks_table', 'predictions']}, indent=2))
