#!/usr/bin/env python3
"""Measure mini controls against their original, unmodified grader labels."""
import argparse
import importlib.util
import json
from pathlib import Path

helper = Path(__file__).parent.parent / '2026-09-25-truthful-checks/measure.py'
spec = importlib.util.spec_from_file_location('intervals', helper)
intervals = importlib.util.module_from_spec(spec)
spec.loader.exec_module(intervals)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    a = p.parse_args()
    groups = {}
    for group in sorted(a.root.iterdir()):
        if not group.is_dir():
            continue
        rows = []
        for path in sorted(group.glob('*/result.json')):
            r = json.loads(path.read_text())
            if 'grade' not in r:
                continue
            recovered = path.parent / 'reproduced-citations-v2/review.json'
            calls = {'strict': r['call'] if r['call'] == 'fail' else None}
            if recovered.exists():
                call = json.loads(recovered.read_text())['call']
                calls['recovered'] = call if call == 'fail' else None
            rows.append({'task': r['task'], 'variant': r['variant'],
                         'reward': r['grade']['reward'], 'calls': calls,
                         'candidate_identity': r['candidate_identity'],
                         'score': r['score'], 'native_cost_usd': r['known_native_cost_usd'],
                         'seconds': r['seconds'], 'source': str(path.relative_to(a.root))})
        if rows:
            signals = rows[0]['calls'].keys()
            if any(r['calls'].keys() != signals for r in rows):
                raise ValueError('Incomplete signal coverage')
            groups[group.name] = {
                'trials': len(rows), 'tasks': len({r['task'] for r in rows}),
                'signals': {s: intervals.stats(rows, s) for s in signals},
                'false_alarms_against_original_labels': {
                    s: [r['source'] for r in rows if r['reward'] == 1 and r['calls'][s] == 'fail']
                    for s in signals},
                'known_native_cost_usd': sum(r['native_cost_usd'] for r in rows),
                'rows': rows,
            }
    record = {'schema': 'openagents.mini-review-measurement.v1', 'groups': groups,
              'notes': ['Development controls, not held-out benchmark trials.',
                        'Labels are exactly those each original mini-grader recorded.',
                        'Do not pool repeated reviews as independent trials.',
                        'The original and shield-only cancellation good fixtures have reproduced defects that their graders missed.',
                        'The third pass changes the cancellation fixture and grader only; the review rule remains literal-v2 at 0.8.',
                        'Native list-price costs exclude Jev; see the deduplicated cost ledger.']}
    a.out.write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps({k: {i: v[i] for i in ['trials', 'tasks', 'signals', 'known_native_cost_usd']}
                      for k, v in groups.items()}, indent=2))


if __name__ == '__main__':
    main()
