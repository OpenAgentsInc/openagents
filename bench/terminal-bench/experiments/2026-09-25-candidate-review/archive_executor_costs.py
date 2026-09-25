#!/usr/bin/env python3
"""Account for every archive executor attempt from retained usage ledgers."""
import argparse
from collections import defaultdict
import json
from pathlib import Path

from prepare import sha
from seal_archive import population


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('manifest', 'jobs', 'out'):
        p.add_argument('--' + name, type=Path, required=True)
    a = p.parse_args()
    rows = json.loads(a.manifest.read_text())
    population(rows)
    records = []
    for row in rows:
        path = a.jobs / row['job'] / row['trial'] / 'agent/episode/evaluation/usage.json'
        record = {k: row[k] for k in ('job', 'trial', 'task', 'executor')}
        if path.exists():
            usage = json.loads(path.read_text())
            record.update(usage_sha256=sha(path), usage=usage)
        else:
            record['missing_usage'] = True
        records.append(record)
    by_executor = {}
    for arm in ('luna', 'astra'):
        group = [r for r in records if r['executor'] == arm]
        components = defaultdict(float)
        for r in group:
            for component, value in r.get('usage', {}).get('components', {}).items():
                components[component] += value.get('cost_lower_bound_usd') or 0
        by_executor[arm] = {
            'scheduled': len(group),
            'known_list_price_lower_bound_usd': sum(r.get('usage', {}).get('cost', {}).get('lower_bound_usd') or 0 for r in group),
            'unknown_calls': sum(r.get('usage', {}).get('cost', {}).get('unknown_calls') or 0 for r in group),
            'missing_usage_trials': [r['trial'] for r in group if r.get('missing_usage')],
            'components_lower_bound_usd': dict(components),
        }
    result = {'schema': 'openagents.archive-executor-costs.v1', 'by_executor': by_executor,
              'notes': ['List-price estimates are not subscription invoices.',
                        'Each attempt contributes its one final ledger, including failed attempts.',
                        'Missing usage is unknown, never a zero-cost inference claim.',
                        'Reproduced review and Jev check costs are recorded separately.',
                        'Original r1 setup refusals occurred before any model call.'],
              'records': records}
    a.out.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(by_executor, indent=2))


if __name__ == '__main__':
    main()
