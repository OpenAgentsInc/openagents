"""Reproduce the v18 publication from outcome fields and retained run-card counts."""
from __future__ import annotations

import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parent
POLICY = '05aac15cefa419cfc3dc2db9225ace213bda351c252a9590d5d50de6b717c445'
ARTIFACT = 'cdbf781be1c00814ba61bfddb6d69a581f6e3c345215b97afa4bba42b656bcd7'


def read(path):
    return json.loads(path.read_text())


def wilson(passed, total):
    if not total:
        return None
    z = 1.959963984540054
    p = passed / total
    scale = 1 + z * z / total
    center = (p + z * z / (2 * total)) / scale
    radius = z * math.sqrt(p * (1 - p) / total + z * z / (4 * total**2)) / scale
    return [max(0, center - radius), min(1, center + radius)]


def counted_cost(usage):
    """Apply the frozen open-request rule; missing records never mean zero."""
    cost = usage['cost']
    lower = cost.get('lower_bound_usd')
    jev = usage['components']['jev'].get('cost_usd')
    if lower is None or jev is None:
        raise ValueError('Missing price evidence; counted cost is unknown')
    if cost.get('amount_usd') is None:
        if not cost.get('unknown_calls'):
            raise ValueError('An incomplete cost needs a retained unknown-call record')
        return max(lower, 0.09 + jev)
    return cost['amount_usd']


def per_pass(value, passes):
    return value / passes if passes else None


def measure(root=ROOT):
    records = root / 'records'
    state = read(records / 'original/state.json')
    tasks = read(root.parent / '2026-09-25-luna-sized-family/tasks.json')
    metadata = {r['task']: r for r in tasks['tasks'] if r['family']}
    loops = {(r['task'], r['label']): r for r in read(records / 'loop-counts.json')}
    counts = {(r['task'], r['label']): r for r in read(records / 'timing-counts.json')}
    batch = read(records / 'candidates/batch.json')
    oracle = {Path(r['trial']).name: r for r in batch['oracle']}
    assert len(state['trials']) == 18 and len(loops) == 18
    rows = []
    for r in state['trials']:
        key = (r['task'], r['label'])
        dest = records / 'attempts' / key[0] / key[1]
        attempt = read(dest / 'attempt.json')
        usage = read(dest / 'usage.json')
        outcome = read(dest / 'outcome.json')
        card_path = next((records / 'cards').glob('*' + r['trial'] + '.card.json'))
        card = read(card_path)
        identity = card['identity']
        assert identity['policy_digest'] == POLICY
        assert attempt['agent']['artifact_sha256'] == ARTIFACT
        assert attempt['task']['pin']['git_commit_id'] == '452bf305c6daa62fc59061d22133a7cbc7c1572e'
        assert identity['reward'] == attempt['outcome']['reward'] == r['reward']
        assert outcome['exception_info'] is None and outcome['verifier']['finished_at']
        loop = loops[key]
        if usage['cost']['unknown_calls']:
            assert 'time ran out' in (loop['stopped'] or '')
        full = loop['submitted_score']
        submitted_full = full[0] is not None and full[1] is not None and full[1] > 0 and full[0] == full[1]
        rows.append({
            'task': r['task'], 'attempt': r['label'], 'job': r['job'], 'trial': r['trial'],
            'split': metadata[r['task']]['split'], 'luna_sized': metadata[r['task']]['luna_sized'],
            'reward': r['reward'], 'tests_passed': identity['tests_passed'], 'tests_total': identity['tests_total'],
            'recorded_cost_usd': usage['cost']['amount_usd'],
            'recorded_lower_bound_usd': usage['cost']['lower_bound_usd'],
            'counted_cost_usd': counted_cost(usage), 'unknown_calls': usage['cost']['unknown_calls'],
            'jev_cost_usd': usage['components']['jev']['cost_usd'],
            'jev_requests': usage['components']['jev']['requests'],
            'luna_cost_lower_bound_usd': usage['components']['delegate']['cost_lower_bound_usd'],
            'luna_calls': usage['components']['delegate']['units']['model_calls'],
            'trial_seconds': attempt['timing']['total_ms'] / 1000,
            'agent_seconds': attempt['timing']['agent_execution_ms'] / 1000,
            'submitted_score': full, 'signal_gap': submitted_full and r['reward'] == 0,
            'full_score_in_any_session': any(total and passed == total for _, passed, total in loop['scores']),
            'baseline_entry_found': loop['baseline_commands_count'] > 0,
            'finish_refusals': sum(s['finish_refusals'] or 0 for s in loop['sessions']),
            'unverified_sessions': sum(bool(s['unverified']) for s in loop['sessions']),
            'session_statuses': [s['status'] for s in loop['sessions']],
            'bound_ended_trial': 'time ran out' in (loop['stopped'] or ''),
            'missing_program_turns': counts[key]['missing_program_turns'],
            'missing_program_turns_before_first_edit': counts[key]['missing_program_turns_before_first_edit'],
            'oracle_complete': oracle[r['trial']]['complete'],
            'any_candidate_passes': oracle[r['trial']]['any_candidate_passes'],
            'card': str(card_path.relative_to(root)),
        })
    groups = []
    for task in tasks['family']:
        group = [r for r in rows if r['task'] == task]
        assert {r['attempt'] for r in group} == {'a1', 'a2', 'a3'}
        passes = sum(r['reward'] == 1 for r in group)
        cost = sum(r['counted_cost_usd'] for r in group)
        seconds = sum(r['trial_seconds'] for r in group)
        fable = metadata[task]['fable']
        cpp = per_pass(cost, passes)
        threshold = {'payments-pipeline-fix': 0.625, 'cumulative-layout-shift': 1.438,
                     'telecom-entity-resolution': 0.605}.get(task, 0.1 * fable['low_cost_per_pass_usd'])
        groups.append({'task': task, 'split': metadata[task]['split'], 'passes': passes, 'attempts': len(group),
                       'wilson95': wilson(passes, len(group)), 'cost_per_pass_usd': cpp,
                       'trial_seconds_per_pass': per_pass(seconds, passes), 'counted_cost_usd': cost,
                       'mean_trial_seconds': seconds / len(group), 'fable_low': fable,
                       'cheap_win': passes >= 2 and cpp < threshold})
    pools = {}
    for name, group in [('confirmation', [r for r in rows if r['split'] == 'confirmation']),
                        ('development', [r for r in rows if r['split'] == 'development']),
                        ('unexposed_confirmation', [r for r in rows if r['task'] in tasks['confirmation'][:2]]),
                        ('luna_sized', [r for r in rows if r['luna_sized']]),
                        ('other', [r for r in rows if not r['luna_sized']])]:
        passed = sum(r['reward'] == 1 for r in group)
        pools[name] = {'passes': passed, 'attempts': len(group), 'wilson95': wilson(passed, len(group))}
    confirmation = [g for g in groups if g['split'] == 'confirmation']
    numeric_verdict = ('win' if sum(g['cheap_win'] for g in confirmation) >= 2 else
                       'loss' if all(g['passes'] < 2 for g in confirmation)
                       and pools['confirmation']['passes'] <= 1 else 'inconclusive')
    return {'schema': 'openagents.microluna-v18-publication.v1',
            'completed_cohort_outcome': numeric_verdict,
            'strict_protocol_verdict': 'inconclusive: source changed after setup-only starts and retry counters restarted',
            'rows': rows, 'tasks': groups, 'pools': pools,
            'totals': {k: sum(r[k] for r in rows) for k in ['recorded_lower_bound_usd', 'counted_cost_usd',
                'jev_cost_usd', 'jev_requests', 'luna_cost_lower_bound_usd', 'luna_calls', 'trial_seconds',
                'agent_seconds', 'unknown_calls', 'signal_gap', 'full_score_in_any_session',
                'baseline_entry_found', 'finish_refusals', 'unverified_sessions', 'bound_ended_trial',
                'missing_program_turns', 'missing_program_turns_before_first_edit', 'oracle_complete']}}


if __name__ == '__main__':
    output = measure()
    (ROOT / 'measurement.json').write_text(json.dumps(output, indent=2) + '\n')
    print(json.dumps(output['totals'], indent=2))
