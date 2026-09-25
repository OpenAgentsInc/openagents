#!/usr/bin/env python3
"""Seal every frozen arm's prospective calls without opening a grade file."""
import argparse
import hashlib
import json
from pathlib import Path

from fusion_bounds import verdict


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ['manifest', 'records', 'jobs', 'out']:
        p.add_argument('--' + name, type=Path, required=True)
    a = p.parse_args()
    here = Path(__file__).parent
    model_paths = {name: here / filename for name, filename in [
        ('verdict.fusion', 'frozen-fusion-model.json'), ('verdict.pooled', 'frozen-pooled-model.json')]}
    models = {name: json.loads(path.read_text()) for name, path in model_paths.items()}
    predictions = []
    for row in json.loads(a.manifest.read_text()):
        trial = a.jobs / row['job'] / row['trial']
        identities = {}

        def read(path, required=True):
            if not path.exists():
                if required:
                    raise ValueError('Unfinished evidence: ' + str(path))
                return {}
            identities[str(path)] = digest(path)
            return json.loads(path.read_text())

        c = read(trial / 'agent/episode/artifacts/composition.json')
        checks = (c.get('final_checks') or {}).get('verdicts', {})
        calls = {'checks.final': 'fail' if checks.get('failed', 0) else
                 'pass' if checks.get('passed', 0) and not checks.get('inconclusive', 0) else None,
                 'verdict.combined': ((c.get('verdict') or {}).get('final') or {}).get('call')}
        calls = {k: v if v in ['pass', 'fail'] else None for k, v in calls.items()}
        features = {}
        for prefix, path in [
            ('report', a.records / 'report-features-prospective' / (row['trial'] + '.json')),
            ('execution', a.records / 'execution-v2/execution-audit-prospective' / (row['trial'] + '.json')),
            ('audit', a.records / 'execution-v2/report-audit-prospective' / (row['trial'] + '.json'))]:
            value = read(path)
            if not value.get('error'):
                features.update({prefix + '.' + k: v.get('noul') for k, v in (value.get('answers') or {}).items()})
                if prefix == 'report' and 'admitted' in value:
                    features['report.admitted'] = float(value['admitted'])
        source = read(a.records / row['trial'] / 'astra/review.json', required=False)
        if not source:
            read(a.records / row['trial'] / 'astra/process.json')
        scores = [f['score'] for f in source.get('findings', []) if f.get('score') is not None]
        features['source'] = max(scores) if scores else None
        ready = read(a.records / row['trial'] / 'readiness/assessment.json', required=False)
        if not ready:
            read(a.records / row['trial'] / 'readiness/process.json')
        features['readiness'] = (ready.get('assessment') or {}).get('failure_probability') if not ready.get('error') else None
        old_audit = read(a.records / 'report-audit-prospective' / (row['trial'] + '.json'))
        old_score = old_audit.get('score') if not old_audit.get('error') else None
        high = lambda v, threshold: v is not None and v >= threshold
        source_score = features['source']
        for version, threshold in [('v1', .9), ('v2', .7)]:
            calls['source-union-' + version] = 'fail' if high(source_score, threshold) or high(old_score, .5) else None
        calls['verdict.agreement'] = 'fail' if high(old_score, .5) or (high(source_score, .8) and high(features['readiness'], .5)) else None
        observed = [features.get('execution.' + k) for k in ['observed', 'unresolved', 'required']]
        concrete = [features.get('audit.' + k) for k in ['observed', 'current', 'required']]
        def minimum(values):
            return min(values) if all(v is not None for v in values) else None
        calls['verdict.observed'] = 'fail' if high(minimum(observed), .5) or high(minimum(concrete), .7) else None
        model_scores = {}
        for name, model in models.items():
            call, bounds, pending = verdict(model, features, [])
            assert not pending
            calls[name] = call
            model_scores[name] = bounds[0]
        process = read(a.records / row['trial'] / 'reproduced/process.json')
        reproduced = read(a.records / row['trial'] / 'reproduced/review.json', required=False)
        if reproduced and process.get('review_sha256') != identities[str(a.records / row['trial'] / 'reproduced/review.json')]:
            raise ValueError('Reproduced review identity changed')
        calls['verdict.reproduced'] = 'fail' if process.get('call') == 'fail' and not process.get('error') else None
        for directory, name in [('reproduced-quoted', 'verdict.reproduced-quoted-v1'),
                                ('reproduced-citations-v2', 'verdict.reproduced-citations-v2')]:
            packet = read(a.records / row['trial'] / directory / 'review.json', required=bool(reproduced) and directory.endswith('v2'))
            calls[name] = 'fail' if packet.get('call') == 'fail' and not process.get('error') else None
        predictions.append({k: row[k] for k in ['job', 'trial', 'task']} | {
            'executor': 'astra' if 'astra-truth-control' in row['job'] else 'luna',
            'features': features, 'model_scores': model_scores, 'reproduced_score': reproduced.get('score'),
            'reproduced_error': process.get('error') or reproduced.get('error'),
            'calls': calls, 'records_sha256': identities})
    result = {'schema': 'openagents.blind-predictions.v2', 'contains_grades': False, 'partition': 'prospective',
              'manifest_sha256': digest(a.manifest), 'models_sha256': {k: digest(v) for k, v in model_paths.items()},
              'predictions': predictions}
    if a.out.exists():
        raise ValueError('Refusing to replace sealed predictions')
    a.out.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'trials': len(predictions), 'sha256': digest(a.out)}))


if __name__ == '__main__':
    main()
