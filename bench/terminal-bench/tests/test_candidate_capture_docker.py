"""Opt-in Docker proof with Harbor's actual collector and separate verifier.

Run with TBENCH_DOCKER_TEST=1. These fixtures make no inference requests.
"""
import asyncio
import json
import os
import uuid
from pathlib import Path

import pytest
from harbor.models.task.task import Task
from harbor.models.trial.paths import TrialPaths
from tbench import candidate_capture as capture, replay
from tbench.warm_docker import WarmDockerEnvironment

pytestmark = pytest.mark.skipif(os.environ.get('TBENCH_DOCKER_TEST') != '1', reason='set TBENCH_DOCKER_TEST=1 for Docker acceptance')
IMAGE = 'alexgshaw/headless-terminal:20251031'


@pytest.mark.parametrize('sidecar', [False, True])
def test_real_collection_and_independent_regrade(tmp_path, sidecar):
    task = tmp_path / 'task'
    (task / 'environment').mkdir(parents=True)
    (task / 'tests').mkdir()
    (task / 'instruction.md').write_text('Produce a synthetic checkpoint.\n')
    (task / 'environment/Dockerfile').write_text(f'FROM {IMAGE}\n')
    (task / 'tests/Dockerfile').write_text(f'FROM {IMAGE}\nCOPY . /tests\nRUN mkdir -p /app/absent-dir && touch /app/absent.txt\n')
    if sidecar:
        (task / 'environment/docker-compose.yaml').write_text(f'''services:
  main:
    build: .
    command: ["sh", "-c", "sleep infinity"]
  db:
    image: {IMAGE}
    command: ["sh", "-c", "sleep infinity"]
''')
    side = ', {source="/tmp/db.dump", destination="database/dump.txt", service="db"}' if sidecar else ''
    hook = '''\n[[verifier.collect]]
command="cp /tmp/live-state /tmp/db.dump"
service="db"
timeout_sec=10
''' if sidecar else ''
    (task / 'task.toml').write_text(f'''schema_version="1.0"
artifacts=[{{source="/app/out.txt", destination="main/output.txt"}}, "/app/absent.txt", "/app/absent-dir"{side}]
[task]
name="synthetic/checkpoint"
[environment]
cpus=1
memory_mb=256
[verifier]
environment_mode="separate"
timeout_sec=30
{hook}
''')
    db_test = 'test "$(cat /tmp/db.dump)" = first\n' if sidecar else ''
    (task / 'tests/test.sh').write_text('''#!/bin/sh
set -eu
test "$(cat /app/out.txt)" = first
test "$(cat /logs/artifacts/note.txt)" = retained
test ! -e /app/absent.txt
test ! -e /app/absent-dir
''' + db_test + 'printf 1 > /logs/verifier/reward.txt\n')
    paths = TrialPaths(tmp_path / 'running')
    paths.mkdir()
    env = WarmDockerEnvironment(environment_dir=task / 'environment', environment_name='synthetic-checkpoint',
        session_id='capture-' + uuid.uuid4().hex[:12], trial_paths=paths, task_env_config=Task(task).config.environment)

    async def exercise():
        try:
            await env.start(force_build=False)
            await env.exec(command='mkdir -p /app /logs/artifacts; printf first > /app/out.txt; printf retained > /logs/artifacts/note.txt')
            if sidecar:
                await env.service_exec('printf first > /tmp/live-state', service='db')
            collector = capture.Collector(env, task, paths.agent_dir / 'candidate-checkpoints')
            await collector.prepare()
            follower = asyncio.create_task(collector.follow())
            try:
                requested = await env.exec(command=f'touch {capture.REMOTE}/lean-1-session-1.request.json', user='nobody')
                assert requested.return_code == 0
                for _ in range(200):
                    reply = await env.exec(command=f'cat {capture.REMOTE}/lean-1-session-1.response.json 2>/dev/null')
                    if reply.return_code == 0:
                        break
                    await asyncio.sleep(0.1)
                else:
                    pytest.fail('checkpoint did not acknowledge collection')
                acknowledgement = json.loads(reply.stdout)
                captured = collector.output / 'lean-1-session-1'
                receipt = json.loads((captured / 'receipt.json').read_text())
                assert receipt['complete'], receipt
                assert capture.digest(receipt) == acknowledgement['receipt_digest']
                assert receipt['verifier_executed'] is False
                assert receipt['observed_absent'] == [{'service': None, 'source': '/app/absent.txt'}, {'service': None, 'source': '/app/absent-dir'}]
                assert (captured / 'artifacts/logs/artifacts/note.txt').read_text() == 'retained'
                assert receipt['hooks'] == [] if not sidecar else receipt['hooks'][0]['exit'] == 0
                # Later mutations cannot change this candidate's restored state.
                await env.exec(command='printf second > /app/out.txt')
                if sidecar:
                    await env.service_exec('printf second > /tmp/live-state', service='db')
                return captured
            finally:
                follower.cancel()
                with pytest.raises(asyncio.CancelledError):
                    await follower
        finally:
            await env.stop(delete=True)
    captured = asyncio.run(exercise())
    # This is the first verifier execution; the agent environment is gone.
    workspace = replay.Workspace(root=captured, source='checkpoint', full=False, note='synthetic sealed checkpoint', collected=captured)
    grade = replay.run_verifier(task, workspace, tmp_path / 'grade')
    (tmp_path / 'acceptance.json').write_text(json.dumps(grade, indent=2))
    assert grade['exit'] == 0 and grade['reward'] == 1, grade
