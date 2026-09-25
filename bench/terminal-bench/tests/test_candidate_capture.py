"""The checkpoint captures declared evidence and never invokes a verifier."""
import asyncio
import json
from pathlib import Path
from types import SimpleNamespace

import pytest
from harbor.models.task.task import Task
from tbench import candidate_capture as capture, candidates, replay
from test_candidates import retained
from test_replay import make_task


class Environment:
    def __init__(self, fail=False):
        self.events = []
        self.fail = fail

    async def service_exec(self, command, service=None, **kwargs):
        self.events.append(('hook', service, command))
        assert 'test.sh' not in command
        return SimpleNamespace(return_code=1 if self.fail or 'missing' in command else 0)

    async def candidate_pause(self, paused):
        self.events.append(('pause', paused))


class Handler:
    def __init__(self, artifacts, logger):
        self.artifacts = artifacts

    def begin_collection(self):
        pass

    def sidecar_services(self):
        return {'db'} if any(getattr(a, 'service', None) == 'db' for a in self.artifacts) else set()

    async def download_artifacts(self, environment, dest, services, **kwargs):
        environment.events.append(('download', sorted(services)))
        dest.mkdir(parents=True, exist_ok=True)
        path = dest / 'manifest.json'
        entries = json.loads(path.read_text()) if path.exists() else []
        for item in self.artifacts:
            source = item if isinstance(item, str) else item.source
            service = None if isinstance(item, str) else item.service
            if (service or 'main') not in services:
                continue
            file = dest / source.lstrip('/')
            status = 'failed' if 'missing' in source else 'ok'
            if status == 'ok':
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_text('captured state')
            entries.append({'source': source, 'service': service, 'status': status,
                            'type': 'file', 'destination': 'artifacts/' + source.lstrip('/')})
        path.write_text(json.dumps(entries))


def test_single_service_preserves_observed_absence(tmp_path):
    task = make_task(tmp_path)
    env = Environment()
    dest = tmp_path / 'capture'
    receipt = asyncio.run(capture.collect(env, task, dest, 'lean-1-session-1', handler_factory=Handler))
    assert receipt['complete'] and not receipt['verifier_executed']
    assert receipt['observed_absent'] == [{'service': None, 'source': '/app/missing.txt'}]
    entries = json.loads((dest / 'artifacts/manifest.json').read_text())
    assert entries[1]['status'] == 'empty'
    assert receipt['files'] == candidates.inventory(dest / 'artifacts')


def test_sidecar_capture_quiesces_main_and_resumes_on_failure(tmp_path):
    task = make_task(tmp_path)
    (task / 'task.toml').write_text('''schema_version = "1.0"
artifacts = [{source="/app/out.txt"}, {source="/tmp/db.dump", service="db"}]
[task]
name="terminal-bench/demo-task"
[verifier]
environment_mode="separate"
[[verifier.collect]]
command="snapshot-db"
service="db"
''')
    for fail in [False, True]:
        env = Environment(fail)
        receipt = asyncio.run(capture.collect(env, task, tmp_path / str(fail),
            'lean-1-session-1', handler_factory=Handler))
        assert receipt['complete'] is not fail
        assert env.events[0] == ('download', ['main'])
        assert env.events[1] == ('pause', True)
        assert env.events[-1] == ('pause', False)
        assert receipt['hooks'][0]['exit'] == int(fail)


def test_preflight_rejects_unknown_coverage(tmp_path):
    task = make_task(tmp_path)
    p = task / 'task.toml'
    p.write_text(p.read_text().replace('environment_mode = "separate"', 'environment_mode = "shared"'))
    assert not capture.preflight(task)['supported']


def test_sealed_capture_grades_even_when_workspace_snapshot_failed(tmp_path):
    trial, task, parent = retained(tmp_path, ('one',))
    dest = trial / 'agent/candidate-checkpoints/lean-1-session-1'
    receipt = asyncio.run(capture.collect(Environment(), task, dest, dest.name, handler_factory=Handler))
    moves = json.loads((parent / 'selection.json').read_text())
    moves[0]['snapshot_error'] = 'workspace too large'
    moves[0]['capture'] = {'receipt_digest': capture.digest(receipt)}
    (parent / 'selection.json').write_text(json.dumps(moves))
    def grade(task, workspace, out):
        source = replay._synthetic_source(out / 'source', task, workspace, None)
        assert (source / 'artifacts/app/out.txt').read_text() == 'captured state'
        assert json.loads((source / 'artifacts/manifest.json').read_text())[1]['status'] == 'empty'
        return {'exit': 0, 'reward': 1, 'exception': None}
    result = candidates.batch([trial], tmp_path / 'graded', runner=grade)
    assert result['oracle'][0]['any_candidate_passes'] is True
    (dest / 'artifacts/app/out.txt').write_text('tampered')
    with pytest.raises(replay.ReplayError, match='changed'):
        candidates.discover(trial)


def test_later_pass_survives_an_earlier_snapshot_error(tmp_path):
    trial, _, parent = retained(tmp_path)
    (parent / 'session-1/out.txt').unlink()
    result = candidates.batch([trial], tmp_path / 'out', runner=lambda *a: {'exit': 0, 'reward': 1})
    assert result['errors'] and result['oracle'][0]['any_candidate_passes'] is True
    assert not result['oracle'][0]['complete']


def test_cancelled_sidecar_collection_resumes_main_and_retains_failure(tmp_path):
    task = make_task(tmp_path)
    (task / 'task.toml').write_text('''schema_version="1.0"
artifacts=[{source="/tmp/db.dump", service="db"}]
[task]
name="terminal-bench/demo-task"
[verifier]
environment_mode="separate"
[[verifier.collect]]
command="snapshot-db"
service="db"
''')
    async def run():
        started = asyncio.Event()
        class Waiting(Environment):
            async def service_exec(self, command, service=None, **kwargs):
                started.set()
                await asyncio.Future()
        env = Waiting()
        dest = tmp_path / 'cancelled'
        worker = asyncio.create_task(capture.collect(env, task, dest, 'lean-1-session-1', handler_factory=Handler))
        await started.wait()
        worker.cancel()
        with pytest.raises(asyncio.CancelledError):
            await worker
        assert env.events[-1] == ('pause', False)
        assert json.loads((dest / 'receipt.json').read_text())['complete'] is False
    asyncio.run(run())


def test_orphan_capture_prevents_a_negative_oracle_claim(tmp_path):
    trial, task, _ = retained(tmp_path, ('one',))
    dest = trial / 'agent/candidate-checkpoints/lean-1-session-2'
    asyncio.run(capture.collect(Environment(), task, dest, dest.name, handler_factory=Handler))
    result = candidates.batch([trial], tmp_path / 'graded', runner=lambda *args: {'exit': 0, 'reward': 0})
    assert result['oracle'][0]['any_candidate_passes'] is None
    assert not result['oracle'][0]['complete']
    assert any('no retained executor acknowledgement' in e['error'] for e in result['errors'])


def test_parallel_or_protected_policies_are_refused_before_inference():
    policy = {'policy': {'executor': {'agent': 'microluna', 'microluna': {'lean': {'retain_candidates': True}}}}}
    capture.check_policy(policy)
    for field, value in [('lanes', 2), ('protect_candidates', True), ('retain_candidates', False)]:
        import copy
        other = copy.deepcopy(policy)
        other['policy']['executor']['microluna']['lean'][field] = value
        with pytest.raises(ValueError, match='sequential'):
            capture.check_policy(other)


def test_interrupted_session_before_checkpoint_keeps_oracle_unknown(tmp_path):
    trial, _, _ = retained(tmp_path, ('one',))
    (trial / 'agent/episode/artifacts/microluna-1-2.atif.jsonl').write_text('{}\n')
    result = candidates.batch([trial], tmp_path / 'graded', runner=lambda *args: {'exit': 0, 'reward': 0})
    assert result['oracle'][0]['any_candidate_passes'] is None
    assert any('session has no retained candidate' in row['error'] for row in result['errors'])
