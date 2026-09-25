"""Replays the retained microluna-v18 launch history through the cohort journal.

The v18 family ran under an older driver. These tests feed its retained launch
logs and outcome records, in their original order, through `tbench cohort`
accounting to show what the journal would have done. They read only files
under `experiments/2026-09-25-microluna-v18-family/records/`; they start no
trial and call no model.
"""
import json
import re
from pathlib import Path

import pytest
from tbench import cli, cohort

RECORDS = Path(__file__).resolve().parents[1] / 'experiments/2026-09-25-microluna-v18-family/records'
TASKS = ['payments-pipeline-fix', 'mp-checkpoint-consolidation', 'cumulative-layout-shift',
         'live-database-cutover', 'telecom-entity-resolution', 'photonic-waveguide-routing']
# The harness commit before and after the source-only contamination repair.
BEFORE = '3a25a0ff1f6ac1c90cc356e848ac7b4d2f37e570'
AFTER = '0f2d7e6bf443c3292e70a3c5f658f27abe88a700'
ARTIFACT = 'cdbf781be1c00814ba61bfddb6d69a581f6e3c345215b97afa4bba42b656bcd7'
POLICY = '05aac15cefa419cfc3dc2db9225ace213bda351c252a9590d5d50de6b717c445'
EVENT = re.compile(r'^\S+ (?P<event>launched|finished) (?P<job>tb4--coder-one-microluna-v18--'
                   r'(?P<task>[a-z-]+)--family-(?P<label>a[123])r?-\d{8}T\d{6})\b')
CENT = cohort.money('0.00000001')


def spec(cohort_id):
    """The v18 protocol as a cohort: $3.00, two at once, one rerun per slot."""
    return {'schema': cohort.SCHEMA, 'id': cohort_id, 'budget_usd': '3.00',
            'reservation_usd': '0.12', 'concurrency': 2, 'retries': 1,
            'cost_rule': {'version': cohort.RULE, 'luna_bound_usd': '0.09'},
            'schedule': [{'id': f'{task}-a{n}'} for n in (1, 2, 3) for task in TASKS]}


def identity(commit):
    tasks = {}
    for path in RECORDS.glob('attempts/*/*/attempt.json'):
        record = json.loads(path.read_text())
        tasks[f'{path.parent.parent.name}-{path.parent.name}'] = record['task']['checksum']
    assert len(tasks) == 18
    return {'source_commit': commit, 'source_dirty': None, 'artifact_sha256': ARTIFACT,
            'policy_sha256': POLICY, 'tasks': tasks}


def completed_attempts():
    """Maps each completed v18 job name to its retained attempt directory."""
    found = {}
    for path in RECORDS.glob('attempts/*/*/attempt.json'):
        found[json.loads(path.read_text())['attempt']['job']] = path.parent
    return found


def outcome(jobs_dir, name, rule):
    """Rebuilds the Harbor job directory from retained records and inspects it."""
    trial = jobs_dir / name / 'trial'
    trial.mkdir(parents=True)
    setup_only = RECORDS / 'setup-refusals' / (name + '-contamination-void')
    if setup_only.is_dir():
        (result,) = json.loads((setup_only / 'outcomes.json').read_text())
        (trial / 'result.json').write_text(json.dumps(result))
    else:
        attempt = completed_attempts()[name]
        (trial / 'result.json').write_text((attempt / 'outcome.json').read_text())
        usage = trial / 'agent/episode/evaluation/usage.json'
        usage.parent.mkdir(parents=True)
        usage.write_text((attempt / 'usage.json').read_text())
    return cohort.inspect(jobs_dir / name, rule)


def replay(journal, log, jobs_dir):
    """Feeds one driver log through the journal in its original order.

    Returns the launches the journal refused and the jobs still running
    when the log ends.
    """
    running, refused = {}, []
    for line in log.read_text().splitlines():
        match = EVENT.match(line)
        if not match:
            continue
        name = match['job']
        if match['event'] == 'launched':
            row = journal.reserve(f"{match['task']}-{match['label']}")
            if row is None:
                refused.append(name)
                continue
            journal.started(row['job'], len(journal.events))
            running[name] = row['job']
        elif name in running:
            journal.settle(running.pop(name), outcome(jobs_dir, name, journal.spec['cost_rule']))
    return refused, running


def test_v18_launch_history_counts_setup_starts_and_refuses_change_and_extra_rerun(tmp_path):
    ledger, jobs = tmp_path / 'ledger', tmp_path / 'jobs'
    frozen = spec('microluna-v18')

    # First epoch: four contamination refusals and one cancelled start.
    with cohort.Journal(ledger, frozen, identity(BEFORE)) as journal:
        refused, running = replay(journal, RECORDS / 'original/driver-void-contamination.log', jobs)
        assert refused == []
        # The driver was stopped during the layout start's image build.
        assert list(running) == ['tb4--coder-one-microluna-v18--cumulative-layout-shift--family-a1-20260925T032441']
        for name, job in running.items():
            journal.settle(job, outcome(jobs, name, frozen['cost_rule']))
        rows = list(journal.attempts().values())
        assert len(rows) == 5
        assert [(r['slot'], r['attempt']) for r in rows] == [
            ('payments-pipeline-fix-a1', 1), ('mp-checkpoint-consolidation-a1', 1),
            ('mp-checkpoint-consolidation-a1', 2), ('payments-pipeline-fix-a1', 2),
            ('cumulative-layout-shift-a1', 1)]
        # Each start is counted by the declared rule: failed before the agent
        # ran, with no usage record, so it is a known zero, not an unknown.
        assert {r['result']['kind'] for r in rows} == {'infrastructure'}
        assert {r['result']['cost']['kind'] for r in rows} == {'known-zero-before-agent'}
        report = journal.report()
        assert report['accounting']['counted_usd'] == '0'
        assert report['accounting']['unknown_jobs'] == []
        assert report['accounting_complete'] and report['graded'] == 0

    # The harness source then changed. The journal refuses to continue and
    # keeps the attempted identity as a deviation.
    with pytest.raises(cohort.CohortError, match='frozen'):
        cohort.Journal(ledger, frozen, identity(AFTER))
    deviation = json.loads((ledger / 'ledger.jsonl').read_text().splitlines()[-1])
    assert deviation['kind'] == 'deviation'
    assert deviation['attempted_identity']['source_commit'] == AFTER

    # Even with the original source restored, the restarted driver's history
    # is refused where it exceeded the rerun allowance: payments and
    # checkpoint had already used their one rerun. The layout slot had used
    # only its first start, so its relaunch is its permitted rerun.
    with cohort.Journal(ledger, frozen, identity(BEFORE)) as journal:
        refused, running = replay(journal, RECORDS / 'original/driver.log', jobs)
        assert refused == [
            'tb4--coder-one-microluna-v18--payments-pipeline-fix--family-a1-20260925T032803',
            'tb4--coder-one-microluna-v18--mp-checkpoint-consolidation--family-a1-20260925T032843']
        assert running == {}
        layout = [r for r in journal.attempts().values() if r['slot'] == 'cumulative-layout-shift-a1']
        assert [r['attempt'] for r in layout] == [1, 2]
        report = journal.report()
        assert report['graded'] == 16 and not report['complete']
        assert len(report['deviations']) == 1


def test_v18_completed_epoch_stores_both_spend_bounds(tmp_path):
    ledger, jobs = tmp_path / 'ledger', tmp_path / 'jobs'
    # The completed epoch as its own frozen cohort, pinned to the repaired source.
    frozen = spec('microluna-v18-epoch-2')
    with cohort.Journal(ledger, frozen, identity(AFTER)) as journal:
        refused, running = replay(journal, RECORDS / 'original/driver.log', jobs)
        assert refused == [] and running == {}
        report = journal.report()
    assert report['complete'] and report['accounting_complete']
    assert report['graded'] == 18 and report['passes'] == 0

    stored = json.loads((ledger / 'report.json').read_text())['accounting']
    assert cohort.money(stored['lower_bound_usd']).quantize(CENT) == cohort.money('0.68201621')
    assert cohort.money(stored['upper_bound_usd']).quantize(CENT) == cohort.money('0.96359699')
    assert stored['counted_usd'] == stored['upper_bound_usd'] and stored['held_usd'] == '0'
    assert len(stored['open_request_jobs']) == 6 and stored['unknown_jobs'] == []
    # The old driver coerced the six unknown totals to zero and recorded $0.4161044.
    assert cohort.money(stored['lower_bound_usd']) > cohort.money('0.4161044')

    summary = cohort.status(ledger)
    assert summary['lower_bound_usd'] == stored['lower_bound_usd']
    assert summary['upper_bound_usd'] == stored['upper_bound_usd']
    assert summary['unknown_cost_count'] == 6
    assert cohort.money(summary['remaining_usd']) == cohort.money('3.00') - cohort.money(stored['counted_usd'])


def test_status_reads_without_writing_and_tolerates_a_line_being_written(tmp_path, capsys):
    frozen = spec('status-check')
    with cohort.Journal(tmp_path, frozen, identity(AFTER)) as journal:
        row = journal.reserve('payments-pipeline-fix-a1')
        journal.started(row['job'], 1)
        before = (tmp_path / 'ledger.jsonl').read_bytes()
        # Status does not need the driver's lock.
        summary = cohort.status(tmp_path)
        assert summary['running'] == 1 and summary['held_usd'] == '0.12'
        assert summary['upper_bound_usd'] == '0.12' and summary['lower_bound_usd'] == '0'
        assert summary['remaining_usd'] == '2.88' and summary['unknown_cost_count'] == 0
        assert cli.main(['cohort', 'status', '--output', str(tmp_path)]) == 0
        assert json.loads(capsys.readouterr().out) == summary
        assert (tmp_path / 'ledger.jsonl').read_bytes() == before
    with (tmp_path / 'ledger.jsonl').open('a') as stream:
        stream.write('{"kind": "sett')
    assert cohort.status(tmp_path)['unterminated_tail'] is True
    with pytest.raises(ValueError):
        cohort.Journal(tmp_path, frozen, identity(AFTER))
