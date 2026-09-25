#!/usr/bin/env python3
"""Measure development-only literal replay, preserving the frozen comparison."""
import argparse
import json
from pathlib import Path
import statistics

from measure_archive import paired_rows, separation
from measure_prospective import digest, measure


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('replay', 'predictions', 'labels', 'out'):
        p.add_argument('--' + name, type=Path, required=True)
    a = p.parse_args()
    provenance = json.loads((a.replay / 'provenance.json').read_text())
    labels = json.loads(a.labels.read_text())
    if not provenance['development_only'] or provenance['prediction_sha256'] != digest(a.predictions):
        raise ValueError('Replay does not identify this development population')
    if labels['prediction_sha256'] != digest(a.predictions):
        raise ValueError('Labels do not identify the frozen population')
    rows = paired_rows(json.loads(a.predictions.read_text())['predictions'], labels['labels'])
    records = json.loads((a.replay / 'records.json').read_text())
    by_key = {(r['job'], r['trial']): r for r in records}
    if len(by_key) != len(records):
        raise ValueError('Duplicate replay candidate')
    for row in rows:
        record = by_key.pop((row['job'], row['trial']))
        if any(row[k] != record[k] for k in ('task', 'executor', 'candidate_identity')):
            raise ValueError('Replay candidate attribution differs')
        if record['call'] not in ('fail', 'unknown'):
            raise ValueError('Unexpected literal check call')
        if not record.get('error'):
            report = a.replay / row['trial'] / 'report.json'
            plan = a.replay / 'plans' / row['task'] / 'plan.json'
            if digest(report) != record['report_sha256'] or digest(plan) != record['plan_sha256']:
                raise ValueError('Replay artifacts differ')
            if record.get('candidate_unchanged') is not True:
                raise ValueError('Replay lacks candidate preservation evidence')
            expected = json.loads(report.read_text())['call'] or 'unknown'
            if record['call'] != expected:
                raise ValueError('Replay call differs from its report')
        elif record['call'] != 'unknown':
            raise ValueError('Unavailable evidence cannot produce a failure call')
        row['calls']['literal.artifacts'] = 'fail' if record['call'] == 'fail' else None
        row['calls']['literal.or.executed'] = (
            'fail' if 'fail' in (record['call'], row['calls']['verdict.executed']) else None)
    if by_key:
        raise ValueError('Extra replay candidates')
    signals = list(rows[0]['calls'])
    result = {
        'schema': 'openagents.literal-artifact-development-measurement.v1',
        'development_only': True,
        'prediction_sha256': digest(a.predictions), 'labels_sha256': digest(a.labels),
        'replay_sha256': digest(a.replay / 'records.json'),
        'all': measure(rows, signals),
        'by_executor': {e: measure([r for r in rows if r['executor'] == e], signals)
                        for e in ('luna', 'astra')},
        'within_task': {s: separation(rows, s) for s in signals},
        'runtime': {'model_calls': 0, 'sum_seconds': sum(r['seconds'] for r in records),
                    'median_seconds': statistics.median(r['seconds'] for r in records),
                    'unavailable': [r['trial'] for r in records if r.get('error')]},
        'notes': [
            'All 72 labels were opened before this component was written; no confirmation claim.',
            'OR reuses original executed-review predictions; no new review or threshold search.',
            'This replay ran all original reviews previously, so no cost saving was realized here.',
            'Matched literal artifacts are necessary conditions, never proof of task completion.',
            'Original official labels and baseline predictions remain unchanged.',
        ],
        'predictions': rows,
    }
    for signal in ('literal.artifacts', 'literal.or.executed'):
        counts = result['all']['signals'][signal]
        if counts['unknown'] + counts['fail_precision']['total'] != result['all']['graded']:
            raise ValueError('Failure-only calls and abstentions do not cover the graded population')
    a.out.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'signals': result['all']['signals'], 'runtime': result['runtime']}, indent=2))


if __name__ == '__main__':
    main()
