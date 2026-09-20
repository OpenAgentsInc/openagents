"""Summarize verified Gym stores without spending locked items or fitting maps.

Run `gym compare` on each store first to verify its receipt chain. Intervals
resample paired items within a family and split; correlated corpus items limit
inference to this benchmark. They are descriptive, not production admission.
"""
import argparse
from collections import Counter
import json
import math
from pathlib import Path
import random


def interval(successes, total):
    if not total:
        return None
    z = 1.959963984540054
    p = successes / total
    center = (p + z * z / (2 * total)) / (1 + z * z / total)
    half = z * math.sqrt(p * (1 - p) / total + z * z / (4 * total * total)) / (1 + z * z / total)
    return [center - half, center + half]


def summarize(items, rows):
    present = [rows[i['id']] for i in items if i['id'] in rows]
    answered = [r for r in present if r['answered']]
    correct = sum(r['correct'] for r in answered)
    bins = [[] for _ in range(10)]
    for row in answered:
        bins[min(9, int(row['raw_top'] * 10))].append(row)
    ece = sum(abs(sum(r['raw_top'] - int(r['correct']) for r in bucket)) for bucket in bins)
    return {
        'expected': len(items), 'answered': len(answered),
        'refused': len(present) - len(answered), 'harness_missing': len(items) - len(present),
        'correct': correct, 'accuracy': correct / len(items),
        'wilson_95': interval(correct, len(items)),
        'ece_answered': ece / len(answered) if answered else None,
        'top_probability_brier': sum((r['raw_top'] - int(r['correct'])) ** 2 for r in answered) / len(answered) if answered else None,
        'confident_errors': sum(not r['correct'] and r['raw_top'] >= .9 for r in answered),
        'constant_correct': dict(sorted(Counter(str(i['truth']) for i in items).items())),
    }


def paired(items, a, b):
    # Missing/refused results count as incorrect, preserving the full denominator.
    deltas = [int(a.get(i['id'], {}).get('correct') is True) - int(b.get(i['id'], {}).get('correct') is True) for i in items]
    wins = deltas.count(1)
    losses = deltas.count(-1)
    n = wins + losses
    exact = min(1, 2 * sum(math.comb(n, k) for k in range(min(wins, losses) + 1)) / 2**n)
    rng = random.Random(9458)
    boot = sorted(sum(rng.choices(deltas, k=len(deltas))) / len(deltas) for _ in range(10000))
    return {'candidate_only_correct': wins, 'reference_only_correct': losses,
            'accuracy_difference': sum(deltas) / len(deltas),
            'paired_item_bootstrap_95': [boot[249], boot[9749]],
            'mcnemar_exact_two_sided': exact}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('suite', type=Path)
    parser.add_argument('stores', nargs='+', help='name=path; candidate must be named candidate')
    args = parser.parse_args()
    suite = json.loads(args.suite.read_text())
    items = [i for i in suite['items'] if i['partition'] != 'locked']
    expected = {i['id']: i for i in items}
    stores, identities, questions = {}, {}, set()
    for entry in args.stores:
        name, path = entry.split('=', 1)
        rows = [json.loads(line) for line in Path(path).read_text().splitlines() if line.strip()]
        by_id = {}
        identity = set()
        for r in rows:
            key = r['item_id']
            if r['suite_digest'] != suite['digest'] or key not in expected or key in by_id:
                raise ValueError(f'{name}: duplicate, locked, or incompatible row: {key}')
            if r['split'] != expected[key]['partition'] or r['family'] != expected[key]['family']:
                raise ValueError(f'{name}: partition or family mismatch: {key}')
            if r['answered'] and r['correct'] != (r['selected'] == str(expected[key]['truth'])):
                # Numeric and Boolean labels use the wire labels recorded by Gym.
                raise ValueError(f'{name}: correctness mismatch: {key}')
            by_id[key] = r
            identity.add(json.dumps(r['door_identity'], sort_keys=True))
            questions.add(r['question_digest'])
        if len(identity) != 1:
            raise ValueError(f'{name}: expected one model execution identity')
        stores[name] = by_id
        identities[name] = json.loads(identity.pop())
    if len(questions) != 1:
        raise ValueError('Question identities differ')
    result = {'suite': suite['name'], 'suite_digest': suite['digest'],
              'question_digest': questions.pop(), 'identities': identities,
              'bootstrap_seed': 9458, 'bootstrap_draws': 10000, 'locked_scored': 0, 'groups': {}, 'errors': {}}
    for family in sorted({i['family'] for i in items}):
        for split in ['calibration', 'development', 'all']:
            selected = [i for i in items if i['family'] == family and (split == 'all' or i['partition'] == split)]
            group = {'models': {name: summarize(selected, rows) for name, rows in stores.items()}}
            group['paired'] = {name: paired(selected, stores['candidate'], rows) for name, rows in stores.items() if name != 'candidate'}
            result['groups'][family + '/' + split] = group
    for name, rows in stores.items():
        result['errors'][name] = [{'item': i['id'], 'family': i['family'], 'split': i['partition'],
                                 'truth': i['truth'], 'selected': rows.get(i['id'], {}).get('selected'),
                                 'confidence': rows.get(i['id'], {}).get('raw_top'),
                                 'refusal': rows.get(i['id'], {}).get('refusal')}
                                for i in items if not rows.get(i['id'], {}).get('correct', False)]
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
