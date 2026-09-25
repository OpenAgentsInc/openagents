#!/usr/bin/env python3
"""Apply a frozen union to retained scores without repeating model calls."""
import argparse
import importlib.util
import json
from pathlib import Path

helper = Path(__file__).parent.parent / '2026-09-25-truthful-checks/measure.py'
spec = importlib.util.spec_from_file_location('intervals', helper)
intervals = importlib.util.module_from_spec(spec)
spec.loader.exec_module(intervals)
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--scores', type=Path, required=True)
p.add_argument('--rule', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
result = json.loads(a.scores.read_text())
rule = json.loads(a.rule.read_text())
rows = result['predictions']
for row in rows:
    row['calls']['review'] = 'fail' if row['score'] is not None and row['score'] >= rule['source_review']['threshold'] else None
    row['calls']['report.audit'] = 'fail' if row['report_score'] is not None and row['report_score'] >= rule['report_audit']['threshold'] else None
    row['calls']['verdict.truthful'] = 'fail' if 'fail' in [row['calls']['review'], row['calls']['report.audit']] else None
signals = ['checks.final', 'verdict.combined', 'review', 'report.audit', 'verdict.truthful']
result.update(rule=rule,
    signals={s: intervals.stats(rows, s) for s in signals},
    paired_task_bootstrap={s: intervals.paired_bootstrap(rows, 'verdict.truthful', s) for s in ['checks.final', 'verdict.combined']},
    tasks_table={t:{s:intervals.stats([r for r in rows if r['task']==t],s) for s in signals} for t in sorted({r['task'] for r in rows})})
a.out.write_text(json.dumps(result, indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k in ['partition','trials','tasks','signals','paired_task_bootstrap']},indent=2))
