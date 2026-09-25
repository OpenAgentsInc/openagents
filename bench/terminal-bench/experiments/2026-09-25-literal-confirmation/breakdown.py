#!/usr/bin/env python3
"""Describe each task after the published prediction seal and verified grade join."""
import argparse
from collections import Counter
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / '2026-09-25-candidate-review'))
from measure_archive import intervals, paired_rows, separation
from reproduce import sha, write


def describe(rows):
    graded = [r for r in rows if r['reward'] in (0, 1)]
    signals = list(rows[0]['calls'])
    if any(set(r['calls']) != set(signals) for r in rows):
        raise ValueError('Candidate signal coverage differs')
    return {
        'attempts': len(rows), 'graded': len(graded),
        'passes': sum(r['reward'] == 1 for r in graded),
        'failures': sum(r['reward'] == 0 for r in graded),
        'unknown_outcomes': [r['trial'] for r in rows if r['reward'] is None],
        'signals': {s: intervals.stats(graded, s) for s in signals},
        'false_alarms': {s: [r['trial'] for r in graded if r['reward'] == 1 and r['calls'][s] == 'fail']
                         for s in signals},
        'missed_failures': {s: [r['trial'] for r in graded if r['reward'] == 0 and r['calls'][s] != 'fail']
                            for s in signals},
        'review_coverage': dict(Counter(r['reproduced_call'] for r in rows)),
        'within_task': {s: separation(graded, s) for s in signals},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('protocol', 'predictions', 'labels', 'out'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    protocol = json.loads(args.protocol.read_text())
    predictions = json.loads(args.predictions.read_text())
    labels = json.loads(args.labels.read_text())
    if (predictions['protocol_sha256'] != sha(args.protocol)
            or labels['prediction_sha256'] != sha(args.predictions)):
        raise ValueError('Protocol, predictions, and joined outcomes do not identify the same study')
    rows = paired_rows(predictions['predictions'], labels['labels'], protocol['tasks'])
    result = {
        'schema': 'openagents.literal-confirmation-task-breakdown.v1',
        'protocol_sha256': sha(args.protocol), 'prediction_sha256': sha(args.predictions),
        'labels_sha256': sha(args.labels),
        'all': describe(rows),
        'by_task': {task: {
            'all': describe([r for r in rows if r['task'] == task]),
            'by_executor': {arm: describe([r for r in rows if r['task'] == task and r['executor'] == arm])
                            for arm in ('luna', 'astra')},
        } for task in protocol['tasks']},
        'notes': ['Descriptive task breakdown; no decision rule is refitted here.',
                  'Unknown grades are not passes or failures; unknown predictions remain in failure recall.',
                  'Within-task pairs are dependent. Per-task Wilson intervals do not establish transfer.',
                  'Use the frozen measurement for paired whole-task bootstrap comparisons.'],
    }
    write(args.out, result)
    print(json.dumps({'tasks': len(result['by_task']), 'attempts': len(rows)}))


if __name__ == '__main__':
    main()
