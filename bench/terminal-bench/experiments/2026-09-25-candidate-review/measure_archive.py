#!/usr/bin/env python3
"""Measure the frozen archive rule, retaining every declared attempt."""
import argparse
from collections import Counter
import json
from pathlib import Path

from measure_prospective import digest, intervals, measure
from seal_archive import population


def paired_rows(predictions, labels, tasks=None):
    population(predictions) if tasks is None else population(predictions, tasks)
    by_key = {(r['job'], r['trial']): r for r in labels}
    if len(by_key) != len(labels):
        raise ValueError('Duplicate official outcome')
    rows = []
    for prediction in predictions:
        label = by_key.pop((prediction['job'], prediction['trial']))
        if any(prediction[k] != label[k] for k in ('task', 'executor')):
            raise ValueError('Outcome attribution differs from the prediction')
        if label['reward'] not in (None, 0, 1):
            raise ValueError('Unexpected official outcome')
        rows.append(prediction | {'reward': label['reward'], 'original_outcome': label})
    if by_key:
        raise ValueError('Extra official outcomes')
    return rows


def separation(rows, signal):
    """Rank failures above passes within a task, with ties worth one half."""
    scores = []
    by_task = {}
    for task in sorted({r['task'] for r in rows}):
        failures = [r for r in rows if r['task'] == task and r['reward'] == 0]
        passes = [r for r in rows if r['task'] == task and r['reward'] == 1]
        pairs = []
        for failed in failures:
            for passed in passes:
                a, b = failed['calls'][signal] == 'fail', passed['calls'][signal] == 'fail'
                pairs.append(1.0 if a > b else .5 if a == b else 0.0)
        if pairs:
            by_task[task] = {'pairs': len(pairs), 'concordance': sum(pairs) / len(pairs)}
            scores.extend(pairs)
    return {'pairs': len(scores), 'concordance': sum(scores) / len(scores) if scores else None,
            'task_mean': sum(v['concordance'] for v in by_task.values()) / len(by_task) if by_task else None,
            'by_task': by_task}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('predictions', 'labels', 'out'):
        p.add_argument('--' + name, type=Path, required=True)
    a = p.parse_args()
    predictions = json.loads(a.predictions.read_text())
    labels = json.loads(a.labels.read_text())
    if labels['prediction_sha256'] != digest(a.predictions):
        raise ValueError('Outcomes do not identify these sealed predictions')
    rows = paired_rows(predictions['predictions'], labels['labels'])
    signals = list(rows[0]['calls'])
    if any(set(r['calls']) != set(signals) for r in rows):
        raise ValueError('Missing signal')
    graded = [r for r in rows if r['reward'] in (0, 1)]
    result = {
        'schema': 'openagents.archive-confirmation-measurement.v1',
        'prediction_sha256': digest(a.predictions), 'labels_sha256': digest(a.labels),
        'scheduled_attempts': {'original_setup_refusals': 72, 'unchanged_restart': len(rows)},
        'bootstrap_seed': 9584, 'bootstrap_resamples': 10000,
        'all': measure(rows, signals),
        'by_executor': {e: measure([r for r in rows if r['executor'] == e], signals)
                        for e in ('luna', 'astra')},
        'by_task': {t: {'trials': sum(r['task'] == t for r in rows),
                        'signals': {s: intervals.stats([r for r in graded if r['task'] == t], s)
                                    for s in signals}}
                    for t in sorted({r['task'] for r in rows})},
        'within_task': {s: separation(graded, s) for s in signals},
        'within_task_by_executor': {e: {s: separation([r for r in graded if r['executor'] == e], s)
                                        for s in signals} for e in ('luna', 'astra')},
        'review_coverage': dict(Counter(r['reproduced_call'] for r in rows)),
        'incremental_review_detections': [r['trial'] for r in rows
                                          if r['reproduced_call'] == 'fail'],
        'notes': [
            'Predictions were committed and pushed before opening official outcomes.',
            'No signal calls a candidate passed except the unchanged historical baseline signals.',
            'Every officially failed candidate remains in recall, including unavailable reviews.',
            'Skipped reviews are not standalone negative predictions; only the frozen OR is evaluated.',
            'Wilson intervals describe trials; paired differences resample whole task groups.',
            'Within-task pairs are dependent descriptive comparisons, not independent trials.',
            'Twelve selected archive task groups cannot establish TB4 completion or a Fable win.',
            'The 72 original setup refusals had no candidates or grades and are separate from accuracy.',
            'These outcomes become development evidence for any later change to the rule.',
        ],
        'predictions': rows,
    }
    a.out.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result['all'], indent=2))


if __name__ == '__main__':
    main()
