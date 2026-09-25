#!/usr/bin/env python3
"""Summarize joined trial timings and executor cost without rerunning inference."""
import argparse
import json
from pathlib import Path
from statistics import median

from prepare import sha
from seal_archive import TASKS, population


def timing(values):
    present = [v for v in values if v is not None]
    return {'recorded': len(present), 'missing': len(values) - len(present),
            'total_seconds': sum(present), 'median_seconds': median(present) if present else None,
            'min_seconds': min(present) if present else None,
            'max_seconds': max(present) if present else None}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('labels', 'executor_costs', 'out'):
        p.add_argument('--' + name.replace('_', '-'), type=Path, required=True)
    p.add_argument('--tasks-file', type=Path)
    a = p.parse_args()
    labels = json.loads(a.labels.read_text())['labels']
    costs = json.loads(a.executor_costs.read_text())
    tasks = a.tasks_file.read_text().splitlines() if a.tasks_file else TASKS
    if not tasks or len(tasks) != len(set(tasks)) or any(not t.strip() for t in tasks):
        raise ValueError("Declared tasks must be unique and nonempty")
    population(labels, tasks)
    population(costs['records'], tasks)
    if a.tasks_file and costs.get('tasks_sha256') != sha(a.tasks_file):
        raise ValueError('Cost ledger identifies different declared tasks')
    cost_keys = {(r['job'], r['trial'], r['task'], r['executor']) for r in costs['records']}
    if cost_keys != {(r['job'], r['trial'], r['task'], r['executor']) for r in labels}:
        raise ValueError('Timings and cost ledgers identify different attempts')
    arms = {}
    for arm in ('luna', 'astra'):
        group = [r for r in labels if r['executor'] == arm]
        passes = sum(r['reward'] == 1 for r in group)
        cost = costs['by_executor'][arm]
        arms[arm] = {
            'scheduled': len(group), 'official_passes': passes,
            'missing_grades': sum(r['reward'] is None for r in group),
            'executor_cost': cost,
            'known_list_price_usd_per_official_pass_lower_bound':
                cost['known_list_price_lower_bound_usd'] / passes if passes else None,
            'timing': {stage: timing([r.get('seconds', {}).get(stage) for r in group])
                       for stage in ('trial', 'environment_setup', 'agent_setup', 'agent_execution', 'verifier')},
        }
    result = {'schema': 'openagents.archive-resources.v1', 'by_executor': arms,
              'labels_sha256': sha(a.labels), 'executor_costs_sha256': sha(a.executor_costs),
              'notes': ['All attempted work contributes cost, including failed attempts.',
                        'A price lower bound per observed pass is not an invoice or a full-population estimate.',
                        'Unknown grades and usage remain explicit; no passes means undefined cost per pass.',
                        'Research reviews are accounted separately; they did not control these executor runs.',
                        'Trial timings include setup and verification; agent timings do not.',
                        'The shared host also ran candidate review and scoped verification builds.',
                        'Different executor models and budgets do not form a matched Coder ablation.']}
    if a.tasks_file:
        result['tasks_sha256'] = sha(a.tasks_file)
    a.out.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(arms, indent=2))


if __name__ == '__main__':
    main()
