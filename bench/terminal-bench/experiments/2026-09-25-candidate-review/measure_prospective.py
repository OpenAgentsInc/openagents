#!/usr/bin/env python3
"""Measure sealed calls against official outcomes, preserving setup failures."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path

helper = Path(__file__).parent.parent / '2026-09-25-truthful-checks/measure.py'
spec = importlib.util.spec_from_file_location('intervals', helper)
intervals = importlib.util.module_from_spec(spec)
spec.loader.exec_module(intervals)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def measure(rows, signals):
    graded = [r for r in rows if r['reward'] in (0, 1)]
    return {
        'trials': len(rows), 'graded': len(graded),
        'tasks': len({r['task'] for r in graded}),
        'passes': sum(r['reward'] == 1 for r in graded),
        'failures': sum(r['reward'] == 0 for r in graded),
        'unknown_outcomes': [r['trial'] for r in rows if r['reward'] is None],
        'signals': {s: intervals.stats(graded, s) for s in signals},
        'paired_task_bootstrap': {
            s: {b: intervals.paired_bootstrap(graded, s, b)
                for b in ('checks.final', 'verdict.combined')}
            for s in signals if s not in ('checks.final', 'verdict.combined')},
        'false_alarms': {s: [r['trial'] for r in graded
                            if r['calls'][s] == 'fail' and r['reward'] == 1]
                         for s in signals},
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--predictions', type=Path, required=True)
    p.add_argument('--labels', type=Path, required=True)
    p.add_argument('--regrades', type=Path)
    p.add_argument('--out', type=Path, required=True)
    a = p.parse_args()
    predictions = json.loads(a.predictions.read_text())
    labels = json.loads(a.labels.read_text())
    if labels['prediction_sha256'] != digest(a.predictions):
        raise ValueError('Labels do not identify these sealed predictions')
    by_key = {(r['job'], r['trial']): r for r in labels['labels']}
    if len(by_key) != len(labels['labels']):
        raise ValueError('Duplicate official label')
    regrades = {}
    if a.regrades:
        for path in sorted(a.regrades.glob('*/grade.json')):
            v = json.loads(path.read_text())
            key = v['job'], v['trial']
            if key in regrades:
                raise ValueError('Duplicate regrade')
            regrades[key] = v | {'record_sha256': digest(path)}
    rows = []
    for prediction in predictions['predictions']:
        key = prediction['job'], prediction['trial']
        label = by_key.pop(key)
        if any(prediction[k] != label[k] for k in ('task', 'executor')):
            raise ValueError('Prediction and outcome attribution disagree')
        row = {k: prediction[k] for k in ('job', 'trial', 'task', 'executor', 'calls')}
        row.update(reward=label['reward'], original_outcome=label)
        if key in regrades:
            recovered = regrades.pop(key)
            if label['reward'] is not None or recovered['original_result_sha256'] != label['result_sha256']:
                raise ValueError('Regrade does not match an unknown original outcome')
            row.update(reward=recovered['result']['reward'], regrade=recovered)
        if row['reward'] not in (None, 0, 1):
            raise ValueError('Unexpected outcome')
        rows.append(row)
    if by_key or regrades:
        raise ValueError('Extra outcomes without sealed predictions')
    signals = list(rows[0]['calls'])
    if any(set(r['calls']) != set(signals) for r in rows):
        raise ValueError('Missing arm')
    mixed = [task for task in sorted({r['task'] for r in rows})
             if {r['reward'] for r in rows if r['task'] == task} >= {0, 1}]
    result = {
        'schema': 'openagents.prospective-check-measurement.v1',
        'prediction_sha256': digest(a.predictions), 'labels_sha256': digest(a.labels),
        'bootstrap_seed': 9584, 'bootstrap_resamples': 10000,
        'all': measure(rows, signals),
        'by_executor': {e: measure([r for r in rows if r['executor'] == e], signals)
                        for e in sorted({r['executor'] for r in rows})},
        'within_task': {t: {s: intervals.stats([r for r in rows if r['task'] == t and r['reward'] in (0, 1)], s)
                            for s in signals} for t in mixed},
        'notes': [
            'Predictions were sealed before the original outcome join.',
            'Environment-only regrades preserve all original unknown outcomes and candidate identities.',
            'Wilson intervals are descriptive trial-level intervals; paired intervals resample whole tasks.',
            'Undefined bootstrap precision is counted, not assigned zero.',
            'Eight task groups and one attempt per executor per task limit generalization.',
            'These outcomes are now development evidence for any later change to a rule.',
        ],
        'predictions': rows,
    }
    a.out.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: v for k, v in result['all'].items()
                      if k not in ('paired_task_bootstrap', 'false_alarms')}, indent=2))


if __name__ == '__main__':
    main()
