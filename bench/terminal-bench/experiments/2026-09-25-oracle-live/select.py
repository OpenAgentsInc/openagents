"""Writes pool.json for the 2026-09-25 live oracle run's pre-registration.

Reads only retained repository files and the pinned TB4 checkout's
environment definitions (never a task's tests or solution): the Luna-sized
family's per-task table (Fable 5.1 figures, resources, and its R1 to R7
reasons), the metric target's labels, the oracle's tier 0 protocol, the
Fable pattern map's excluded tasks, and the tuned-task list the
contamination check reads. It runs no trial and calls no model.

Usage, from the repository root:

    python3 bench/terminal-bench/experiments/2026-09-25-oracle-live/select.py
"""
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = Path(sys.argv[1]) if len(sys.argv) > 1 else HERE.parents[3]
OUT = Path(sys.argv[2]) if len(sys.argv) > 2 else HERE / 'pool.json'
TB4 = Path.home() / '.openagents/terminal-bench/upstream/terminal-bench-v4.0.0/tasks'
EXP = ROOT / 'bench/terminal-bench/experiments'

family = json.loads((EXP / '2026-09-25-luna-sized-family/tasks.json').read_text())
labels = json.loads((EXP / '2026-09-25-metric-target/labels.json').read_text())
tuned = json.loads((ROOT / 'crates/coder-one/contamination-tuned.json').read_text())

# E1: the tasks Fable 5.1's winning runs were mapped from
# (docs/terminal-bench/2026-09-25-fable-pattern-map.md); the metric target's
# labels exclude the same eleven.
MAPPED = set(labels['excluded'])
# E2: the oracle's tier 0 population (experiments/2026-09-25-oracle/protocol.md).
ORACLE_TIER0 = set(re.findall(r'`([a-z0-9-]+)`', (EXP / '2026-09-25-oracle/protocol.md').read_text()
                              .split('That leaves 17 tasks:')[1].split('Workspaces:')[0]))
assert len(ORACLE_TIER0) == 17, ORACLE_TIER0
# E3: every task a Coder One or Microluna policy was tuned on, measured on,
# or studied for lessons.
TUNED = {row['task']: row['roles'] for row in tuned['tasks']}
# E4: the metric target's goal tasks, where its extraction, harness, and
# finish rule act. The other labeled tasks entered only as instruction text.
METRIC_POP = {row['task'] for row in labels['tasks']}
METRIC_GOAL = {row['task'] for row in labels['tasks'] if row['states_numeric_goal']}


def group(task):
    return task.split('-')[0]


def installs_python(task):
    """E7: the task's own environment definition puts Python 3 in the image.
    The oracle runs as `python3 oracle.py`."""
    text = ''
    for path in sorted((TB4 / task / 'environment').glob('Dockerfile*')):
        text += path.read_text(errors='replace')
    return bool(re.search(r'FROM\s+python:|FROM\s+\S*(miniforge|conda)\S*|\bpython3\b', text))


rows = {row['task']: row for row in family['tasks']}
excluded_names = MAPPED | ORACLE_TIER0 | set(TUNED) | METRIC_GOAL
out = []
for task in sorted(rows):
    row = rows[task]
    fable = row['fable']
    low = int(fable['by_effort']['low'].split('/')[0])
    minutes = round(fable['low_mean_trial_sec'] / 60, 1)
    rules = []
    if task in MAPPED:
        rules.append('E1 mapped by the Fable pattern map')
    if task in ORACLE_TIER0:
        rules.append('E2 in the oracle tier 0 population')
    if task in TUNED:
        rules.append('E3 tuned-task list: ' + ', '.join(TUNED[task]))
    if task in METRIC_GOAL:
        rules.append('E4 a metric target goal task')
    kin = sorted(t for t in excluded_names if t != task and group(t) == group(task))
    if kin:
        rules.append('E5 its group holds ' + ', '.join(kin))
    if row['resources']['gpus']:
        rules.append('E6 needs a GPU')
    if not installs_python(task):
        rules.append('E7 its environment installs no Python 3')
    if low < 1 or minutes > 45:
        rules.append(f'E8 Fable 5.1 low passes {low} of 5 in a mean {minutes} minutes')
    out.append({
        'task': task,
        'rules': rules,
        'eligible': not rules,
        'fable_low': fable['by_effort']['low'],
        'fable_all': fable['all_efforts'],
        'fable_low_mean_min': minutes,
        'fable_low_cost_per_pass_usd': fable['low_cost_per_pass_usd'],
        'luna_sized': row['luna_sized'],
        'family_reasons': [e['why'] for e in row['exclusions']],
        'metric_target_population': task in METRIC_POP,
        'resources': row['resources'],
    })

eligible = [r['task'] for r in out if r['eligible']]
strict = [r['task'] for r in out
          if not (set(r['rules']) - {x for x in r['rules'] if x.startswith(('E7', 'E8'))})
          and r['task'] not in METRIC_POP]
OUT.write_text(json.dumps({
    'schema': 'openagents.tbench.oracle-live-pool.v1',
    'experiment': '2026-09-25-oracle-live',
    'tb4_tasks': len(out),
    'strict_pool_before_e7_e8': strict,
    'eligible': eligible,
    'tasks': out,
}, indent=2) + '\n')
print(json.dumps({'eligible': eligible, 'strict_pool_before_e7_e8': strict}))
