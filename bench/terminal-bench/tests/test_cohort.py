import json
from pathlib import Path

import pytest
from tbench import cohort


def spec():
    return {'schema': cohort.SCHEMA, 'id': 'synthetic', 'budget_usd': '0.25',
            'reservation_usd': '0.12', 'concurrency': 2, 'retries': 1,
            'cost_rule': {'version': cohort.RULE, 'luna_bound_usd': '0.09'},
            'schedule': [{'id': str(n)} for n in range(3)]}


def usage(amount=None, lower=0.03, *, timed=True):
    return {'cost': {'amount_usd': amount, 'lower_bound_usd': lower, 'unknown_calls': int(amount is None)},
            'components': {'generation': {'cost_usd': 0}, 'jev': {'cost_usd': 0.002}, 'delegate': {'dispatches':
                [{'agent': 'microluna', 'status': 'timed_out' if timed else 'failed', 'charge': 'unknown' if amount is None else 'priced'}]}}}


def result(kind='graded', cost='0.02', reward=0):
    return {'kind': kind, 'reward': reward, 'cost': {'counted_usd': cost}}


def test_prices_distinguish_zero_bound_and_unknown():
    rule = spec()['cost_rule']
    assert cohort.price(usage(0, 0), rule)['kind'] == 'known-zero'
    assert cohort.price(usage(), rule)['counted_usd'] == '0.092'
    assert cohort.price(usage(lower=0.15), rule)['counted_usd'] == '0.15'
    assert cohort.price(usage(timed=False), rule)['counted_usd'] is None
    assert cohort.price(None, rule)['counted_usd'] is None
    assert cohort.price(None, rule, before_agent=True)['counted_usd'] == '0'
    with pytest.raises(cohort.CohortError):
        cohort.price(usage(), rule, before_agent=True)


def test_unknown_cost_and_nonfinite_numbers_fail_closed():
    for value in [None, True, -1, 'NaN', 'Infinity']:
        with pytest.raises(cohort.CohortError):
            cohort.money(value)
    with pytest.raises(cohort.CohortError):
        cohort.price(usage(0, 1), spec()['cost_rule'])


def test_concurrent_reservations_and_forced_small_budget_stop(tmp_path):
    with cohort.Journal(tmp_path, spec(), {'source': 'one'}) as journal:
        a = journal.reserve('0'); b = journal.reserve('1')
        assert journal.reserve('2') is None
        assert journal.accounting()['held_usd'] == '0.24'
        journal.settle(a['job'], result(cost='0.12'))
        assert journal.reserve('2') is None
        journal.settle(b['job'], result(cost='0.12'))
        assert journal.reserve('2') is None
        assert not journal.report()['complete']
        assert journal.accounting()['available_usd'] == '0.01'


def test_restart_keeps_inflight_identity_and_retry_count(tmp_path):
    with cohort.Journal(tmp_path, spec(), {}) as journal:
        row = journal.reserve('0')
    with cohort.Journal(tmp_path, spec(), {}) as journal:
        assert journal.reserve('0')['job'] == row['job']
        journal.settle(row['job'], result('infrastructure', '0'))
        retry = journal.reserve('0')
        assert retry['attempt'] == 2
        journal.settle(retry['job'], result('infrastructure', '0'))
    with cohort.Journal(tmp_path, spec(), {}) as journal:
        assert journal.reserve('0') is None
        assert len(journal.attempts()) == 2


def test_unknown_holds_do_not_free_budget_and_settlement_is_idempotent(tmp_path):
    with cohort.Journal(tmp_path, spec(), {}) as journal:
        row = journal.reserve('0')
        unknown = result('incomplete', None)
        journal.settle(row['job'], unknown)
        count = len(journal.events)
        journal.settle(row['job'], unknown)
        assert len(journal.events) == count
        assert journal.reserve('1') is None
        assert journal.accounting()['held_usd'] == '0.12'
        with pytest.raises(cohort.CohortError):
            journal.settle(row['job'], result())


def test_source_change_is_retained_and_exclusive_owner_is_enforced(tmp_path):
    with cohort.Journal(tmp_path, spec(), {'source': 'old'}):
        with pytest.raises(cohort.CohortError, match='owns'):
            cohort.Journal(tmp_path, spec(), {'source': 'old'})
    with pytest.raises(cohort.CohortError, match='frozen'):
        cohort.Journal(tmp_path, spec(), {'source': 'new'})
    events = [json.loads(x) for x in (tmp_path / 'ledger.jsonl').read_text().splitlines()]
    assert events[-1]['kind'] == 'deviation'


def test_corrupt_tail_is_not_silently_discarded(tmp_path):
    with cohort.Journal(tmp_path, spec(), {}):
        pass
    with (tmp_path / 'ledger.jsonl').open('a') as stream:
        stream.write('{incomplete')
    with pytest.raises(ValueError):
        cohort.Journal(tmp_path, spec(), {})


def test_reservation_breach_stops_new_launches(tmp_path):
    with cohort.Journal(tmp_path, spec(), {}) as journal:
        row = journal.reserve('0')
        journal.settle(row['job'], result(cost='0.2'))
        assert journal.reserve('1') is None
        assert journal.accounting()['reservation_breaches'] == [row['job']]


def test_v18_ledgers_reproduce_conservative_count_without_zero_coercion():
    root = Path(__file__).resolve().parents[1] / 'experiments/2026-09-25-microluna-v18-family/records/attempts'
    prices = [cohort.price(json.loads(p.read_text()), spec()['cost_rule']) for p in root.glob('*/*/usage.json')]
    assert len(prices) == 18
    assert sum(p['kind'] == 'open-request-bound' for p in prices) == 6
    assert sum(cohort.money(p['counted_usd']) for p in prices).quantize(cohort.money('0.00000001')) == cohort.money('0.96359699')


def test_other_unknown_calls_are_not_covered_by_the_deadline_rule():
    for change in ['generation', 'jev', 'dispatch']:
        value = usage()
        if change == 'generation':
            value['components']['generation']['unpriced_calls'] = 1
        elif change == 'jev':
            value['components']['jev']['unknown'] = 1
        else:
            value['components']['delegate']['dispatches'] *= 2
        assert cohort.price(value, spec()['cost_rule'])['counted_usd'] is None


def test_real_process_queue_retains_retry_and_stops_at_ceiling(tmp_path, monkeypatch):
    from types import SimpleNamespace
    from tbench import suite, host, candidate_capture
    from test_replay import make_task
    task = make_task(tmp_path)
    frozen = spec()
    frozen.update(budget_usd='0.18', concurrency=2, reservation_usd='0.12')
    for row in frozen['schedule']:
        row.update(task_path=str(task), profile='fixture', agent='fixture')
    monkeypatch.setattr(suite.Host, 'docker', lambda checkout: SimpleNamespace(free_disk_gb=lambda: 50))
    monkeypatch.setattr(host, 'docker_info', lambda: {'NCPU': 2, 'MemTotal': 2 * 1024**3})
    monkeypatch.setattr(cohort, 'identity', lambda *args: {'source': 'fixture'})
    monkeypatch.setattr(cohort, 'slot_request', lambda slot: SimpleNamespace(tasks=[SimpleNamespace(
        peak_resources=SimpleNamespace(cpus=1, memory_mb=256, gpus=0))]))
    jobs = tmp_path / 'jobs'
    launched = []
    script = tmp_path / 'fixture.py'
    script.write_text('''import json,sys,time
from pathlib import Path
trial=Path(sys.argv[1])/'trial'
trial.mkdir(parents=True)
time.sleep(.02)
if sys.argv[2]=='1':
 result={'finished_at':'now','agent_execution':None,'agent_result':None,'exception_info':{'exception_type':'EnvironmentStartTimeoutError'}}
else:
 result={'finished_at':'now','agent_execution':{'finished_at':'now'},'verifier_result':{'rewards':{'reward':0}}}
 usage=trial/'agent/episode/evaluation/usage.json'
 usage.parent.mkdir(parents=True)
 usage.write_text(json.dumps({'cost':{'amount_usd':.08,'lower_bound_usd':.08,'unknown_calls':0}}))
(trial/'result.json').write_text(json.dumps(result))
''')
    def launch(journal, slot, job, checkout):
        # A real subprocess observes a reservation already durably written.
        events = [json.loads(line) for line in journal.path.read_text().splitlines()]
        assert events[-1]['kind'] == 'reserve' and events[-1]['job'] == job
        launched.append(job)
        return cohort.subprocess.Popen([cohort.sys.executable, str(script), str(jobs / job), str(len(launched))], start_new_session=True)
    monkeypatch.setattr(cohort, 'launch', launch)
    with cohort.Journal(tmp_path / 'ledger', frozen, {'source': 'fixture'}) as journal:
        report = cohort.run(journal, jobs, tmp_path)
        assert len(report['attempts']) == 2
        assert [r['result']['kind'] for r in report['attempts']] == ['infrastructure', 'graded']
        assert report['accounting']['counted_usd'] == '0.08'
        assert report['accounting']['available_usd'] == '0.10'
        assert not report['complete']
    with cohort.Journal(tmp_path / 'ledger', frozen, {'source': 'fixture'}) as journal:
        again = cohort.run(journal, jobs, tmp_path)
        assert len(launched) == 2 and again['attempts'] == report['attempts']
        assert len([e for e in journal.events if e['kind'] == 'epoch']) == 2


def test_abandoned_launch_is_unknown_not_retried(tmp_path, monkeypatch):
    from types import SimpleNamespace
    from tbench import suite, host, candidate_capture
    frozen = spec()
    for row in frozen['schedule']:
        row.update(task_path='/fixture')
    monkeypatch.setattr(suite.Host, 'docker', lambda checkout: SimpleNamespace(free_disk_gb=lambda: 50))
    monkeypatch.setattr(host, 'docker_info', lambda: {'NCPU': 2, 'MemTotal': 2 * 1024**3})
    monkeypatch.setattr(candidate_capture, 'preflight', lambda _: {'supported': True})
    monkeypatch.setattr(suite, 'running_pid', lambda _: None)
    monkeypatch.setattr(cohort, 'identity', lambda *args: {})
    monkeypatch.setattr(cohort, 'slot_request', lambda slot: SimpleNamespace(tasks=[SimpleNamespace(
        peak_resources=SimpleNamespace(cpus=1, memory_mb=256, gpus=0))]))
    with cohort.Journal(tmp_path / 'ledger', frozen, {}) as journal:
        row = journal.reserve('0')
    with cohort.Journal(tmp_path / 'ledger', frozen, {}) as journal:
        report = cohort.run(journal, tmp_path / 'jobs', tmp_path)
        assert len(report['attempts']) == 1
        assert report['accounting']['unknown_jobs'] == [row['job']]
        assert report['accounting']['held_usd'] == '0.12'
        assert not report['accounting_complete']


def test_slots_may_run_their_own_manifest_under_one_budget(tmp_path, monkeypatch):
    """Matched arms interleave in one cohort: a slot's policy_path is the
    manifest it launches with, every manifest is pinned by digest, and each
    must keep the cost rule's Luna bound."""
    import subprocess
    from types import SimpleNamespace
    from test_replay import make_task
    task = make_task(tmp_path)
    checkout = tmp_path / 'checkout'
    checkout.mkdir()
    git = ['git', '-C', str(checkout), '-c', 'user.name=t', '-c', 'user.email=t@example.com']
    subprocess.run(git[:3] + ['init', '-q'], check=True)
    subprocess.run(git + ['commit', '-q', '--allow-empty', '-m', 'pin'], check=True)
    artifact = tmp_path / 'coder-one'
    artifact.write_bytes(b'binary')

    def manifest(name, spend=0.09, oracle=False):
        lean = {'retain_candidates': True, **({'oracle': {}} if oracle else {})}
        path = tmp_path / f'{name}.json'
        path.write_text(json.dumps({'policy': {'executor': {'agent': 'microluna', 'microluna': {
            'spend_usd': spend, 'lean': lean}}}}))
        return str(path)

    off, on = manifest('off'), manifest('on', oracle=True)
    frozen = spec()
    frozen.update(artifact_path=str(artifact),
                  artifact_sha256=cohort.hashlib.sha256(b'binary').hexdigest(), policy_path=off)
    frozen['schedule'] = [{'id': 'a-on', 'task_path': str(task), 'policy_path': on},
                          {'id': 'a-off', 'task_path': str(task)}]
    monkeypatch.setattr(cohort, 'slot_request', lambda slot: None)
    pinned = cohort.identity(checkout, frozen)
    digest = lambda path: cohort.hashlib.sha256(Path(path).read_bytes()).hexdigest()
    assert pinned['policy_sha256'] == digest(off)
    assert pinned['slot_policy_sha256'] == {'a-on': digest(on), 'a-off': digest(off)}
    # A cohort without per-slot manifests keeps its earlier identity shape.
    single = dict(frozen, schedule=[{'id': 'a-off', 'task_path': str(task)}])
    assert 'slot_policy_sha256' not in cohort.identity(checkout, single)

    refused = json.loads(json.dumps(frozen))
    refused['schedule'][0]['policy_path'] = manifest('dear', spend=0.2, oracle=True)
    with pytest.raises(cohort.CohortError, match='Luna bound|budget'):
        cohort.identity(checkout, refused)
    refused['schedule'][0]['policy_path'] = 'relative.json'
    with pytest.raises(cohort.CohortError, match='absolute'):
        cohort.identity(checkout, refused)
    refused['schedule'][0]['policy_path'] = ''
    with pytest.raises(cohort.CohortError, match='slot policy_path'):
        cohort.validate(refused)

    launched = []
    monkeypatch.setattr(cohort.subprocess, 'Popen', lambda command, **kw: launched.append(command))
    journal = SimpleNamespace(spec=frozen, directory=tmp_path)
    for slot in frozen['schedule']:
        cohort.launch(journal, dict(slot, profile='tb4', agent='arm'), 'job-' + slot['id'], checkout)
    policies = [[arg for arg in command if arg.startswith('policy=')] for command in launched]
    assert policies == [['policy=' + on], ['policy=' + off]]


def test_profile_cannot_run_another_task_or_hidden_attempts(tmp_path, monkeypatch):
    from types import SimpleNamespace
    from tbench import cli
    task = SimpleNamespace(path='tasks/demo')
    request = SimpleNamespace(checkout=tmp_path, profile=SimpleNamespace(n_attempts=1, retry={'max_retries':0}, install_only=False,
        environment={'import_path':'tbench.warm_docker:WarmDockerEnvironment'}),
        agent=SimpleNamespace(harbor_import_path='tbench.coder_one:CoderOneTunable'),
        panel=SimpleNamespace(select=lambda _: [task]))
    monkeypatch.setattr(cli, '_load', lambda *args: request)
    slot = {'profile':'tb4', 'agent':'fixture', 'task_path':str(tmp_path/'tasks/demo')}
    assert cohort.slot_request(slot).tasks == [task]
    with pytest.raises(cohort.CohortError, match='task_path differs'):
        cohort.slot_request(dict(slot, task_path=str(tmp_path/'other/demo')))
    request.profile.n_attempts = 3
    with pytest.raises(cohort.CohortError, match='single-attempt'):
        cohort.slot_request(slot)
