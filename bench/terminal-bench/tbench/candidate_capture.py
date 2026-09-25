"""Collect task artifacts at an executor barrier, without running the verifier."""
from __future__ import annotations

import asyncio
import hashlib
import json
import logging
import re
import shlex
import shutil
import tempfile
from pathlib import Path

from harbor.models.task.task import Task
from harbor.environments.base import EnvironmentCapabilities
from harbor.trial.artifact_handler import ArtifactHandler

VERSION = 'candidate-checkpoint-v1'
REMOTE = '/opt/openagents/candidate-checkpoints'
IDENTITY = re.compile(r'lean-[1-9][0-9]{0,8}-session-[1-9][0-9]{0,8}')


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def check_policy(policy):
    executor = (policy or {}).get('policy', {}).get('executor', {})
    lean = executor.get('microluna', {}).get('lean') or {}
    if (executor.get('agent') != 'microluna' or not lean.get('retain_candidates')
            or lean.get('lanes', 1) > 1 or lean.get('protect_candidates')):
        raise ValueError('candidate capture requires sequential Microluna with retain_candidates and no protected lanes')


def preflight(task_path: Path) -> dict:
    """Check the collection contract before an agent or verifier is started."""
    task = Task(task_path)
    config = task.config
    reasons = []
    if config.steps:
        reasons.append('multi-step tasks need step-specific checkpoint support')
    if any(not (a if isinstance(a, str) else a.source).startswith('/') for a in config.artifacts):
        reasons.append('candidate capture requires absolute artifact sources')
    if not config.artifacts:
        reasons.append('no task-declared artifacts; replay coverage is unknown')
    mode = config.verifier.environment_mode
    if str(getattr(mode, 'value', mode)) != 'separate':
        reasons.append('candidate replay requires a separate verifier environment')
    hooks = [hook.model_dump(mode='json') for hook in config.verifier.collect]
    if sum(hook['timeout_sec'] for hook in hooks) > 90:
        reasons.append('collection hooks exceed the 90-second hook budget')
    artifacts = [a.model_dump(mode='json') if hasattr(a, 'model_dump') else a for a in config.artifacts]
    # No test script or verifier environment is copied into the running agent.
    return {'schema': VERSION, 'supported': not reasons, 'reasons': reasons,
            'task_checksum': task.checksum, 'artifacts': artifacts, 'hooks': hooks,
            'max_capture_seconds': 120, 'verifier_executed': False}


def sealed_inventory(root: Path) -> dict:
    from .candidates import inventory
    return inventory(root)


class CopySource:
    """Checkpoint destinations are not Harbor's mounted final artifact directory."""
    def __init__(self, environment):
        self.environment = environment
        self.capabilities = getattr(environment, 'capabilities', EnvironmentCapabilities()).model_copy(update={'mounted': False})

    def __getattr__(self, name):
        return getattr(self.environment, name)


async def collect(environment, task_path: Path, destination: Path, identity: str,
                  *, handler_factory=ArtifactHandler) -> dict:
    """Capture main artifacts, pause main, then capture sidecars and resume."""
    if not IDENTITY.fullmatch(identity):
        raise ValueError('invalid checkpoint identity')
    coverage = preflight(task_path)
    if not coverage['supported']:
        raise ValueError('; '.join(coverage['reasons']))
    destination.mkdir(parents=True, exist_ok=False)
    receipt = {'schema': VERSION, 'id': identity, 'coverage': coverage,
               'complete': False, 'hooks': [], 'observed_absent': [], 'verifier_executed': False}
    task = Task(task_path)
    handler = handler_factory(artifacts=task.config.artifacts, logger=logging.getLogger(__name__))
    handler.begin_collection()
    sidecars = handler.sidecar_services()
    sidecars |= {h.service for h in task.config.verifier.collect if h.service != 'main'}
    paused = False
    try:
        async with asyncio.timeout(120):
            for services in [{'main'}, sidecars]:
                if not services:
                    continue
                if 'main' not in services:
                    # The executor is waiting at a barrier; pausing also stops
                    # background processes before sidecar evidence is captured.
                    await environment.candidate_pause(True)
                    paused = True
                for hook in task.config.verifier.collect:
                    if hook.service not in services:
                        continue
                    result = await environment.service_exec(hook.command, service=hook.service,
                                                           timeout_sec=int(hook.timeout_sec), user=hook.user)
                    receipt['hooks'].append({'service': hook.service, 'command_sha256':
                        hashlib.sha256(hook.command.encode()).hexdigest(), 'exit': result.return_code})
                    if result.return_code:
                        raise ValueError(f'collection hook failed in service {hook.service}')
                await handler.download_artifacts(CopySource(environment), destination / 'artifacts',
                    source_artifacts_dir=Path('/logs/artifacts'), services=services)
                entries = json.loads((destination / 'artifacts/manifest.json').read_text())
                for entry in entries:
                    if (entry.get('service') or 'main') not in services:
                        continue
                    if entry.get('status') == 'failed':
                        source = shlex.quote(entry['source'])
                        probe = await environment.service_exec(
                            f'test -e {source} || test -L {source}',
                            service=entry.get('service'), timeout_sec=10, user='root')
                        if probe.return_code == 1:
                            target = (destination / entry['destination']).resolve()
                            if not target.is_relative_to((destination / 'artifacts').resolve()):
                                raise ValueError('artifact destination escapes its checkpoint')
                            if target.is_dir():
                                shutil.rmtree(target)
                            elif target.exists() or target.is_symlink():
                                target.unlink()
                            entry['status'] = 'empty'
                            entry['observed_absent'] = True
                            receipt['observed_absent'].append({'service': entry.get('service'), 'source': entry['source']})
                (destination / 'artifacts/manifest.json').write_text(json.dumps(entries, indent=2) + '\n')
            entries = json.loads((destination / 'artifacts/manifest.json').read_text())
            if any(e.get('status') not in ('ok', 'empty') for e in entries):
                raise ValueError('artifact collection has an error; see its manifest')
            receipt['files'] = sealed_inventory(destination / 'artifacts')
            receipt['complete'] = True
    except Exception as error:
        receipt['error'] = str(error)
    finally:
        if paused:
            # Cancellation cannot strand a paused task container.
            try:
                await asyncio.shield(environment.candidate_pause(False))
            except Exception as error:
                receipt['complete'] = False
                receipt['resume_error'] = str(error)
        (destination / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    return receipt


class Collector:
    def __init__(self, environment, task: Path, output: Path):
        self.environment, self.task, self.output = environment, task, output
        self.seen = set()

    async def prepare(self):
        if not callable(getattr(self.environment, 'candidate_pause', None)):
            raise ValueError('candidate capture requires the warm Docker environment')
        made = await self.environment.exec(command=f'mkdir -p {REMOTE} && chmod 1777 {REMOTE}', user='root')
        if made.return_code:
            raise ValueError('could not prepare the candidate checkpoint directory')
        self.output.mkdir(parents=True, exist_ok=False)

    async def follow(self):
        while True:
            result = await self.environment.exec(command=f'ls {REMOTE}/*.request.json 2>/dev/null || true', user='root')
            for raw in (result.stdout or '').splitlines():
                name = Path(raw).name.removesuffix('.request.json')
                if not IDENTITY.fullmatch(name) or name in self.seen:
                    continue
                self.seen.add(name)
                # Only an identity crosses the barrier. The request supplies
                # neither commands nor host paths; the task supplies all hooks.
                receipt = await collect(self.environment, self.task, self.output / name, name)
                acknowledgement = {'schema': VERSION, 'id': name, 'complete': receipt['complete'],
                                   'receipt_digest': digest(receipt)}
                with tempfile.TemporaryDirectory() as directory:
                    path = Path(directory) / 'response.json'
                    path.write_text(json.dumps(acknowledgement))
                    target = f'{REMOTE}/{name}.response.json'
                    await self.environment.upload_file(str(path), target + '.tmp')
                    moved = await self.environment.exec(command=f'mv {shlex.quote(target + ".tmp")} {shlex.quote(target)}', user='root')
                    if moved.return_code:
                        raise ValueError('could not publish the candidate acknowledgement')
            await asyncio.sleep(0.2)


from .warm_docker import WarmDockerEnvironment


class CheckpointVerifierEnvironment(WarmDockerEnvironment):
    """Restore observed absence before Harbor uploads a checkpoint's files."""
    async def start(self, force_build):
        await super().start(force_build)
        receipt = json.loads((self.trial_paths.agent_dir / 'checkpoint-receipt.json').read_text())
        declared = receipt['coverage']['artifacts']
        sources = {a if isinstance(a, str) else a['source'] for a in declared} | {'/logs/artifacts'}
        for entry in receipt['observed_absent']:
            source = entry['source']
            if source not in sources or not source.startswith('/') or source == '/' or '..' in Path(source).parts:
                raise ValueError('invalid observed-absence source')
            # Separate verifiers receive sidecar exports in their main service,
            # just as Harbor's ordinary final collection does.
            result = await self.exec(command=f'rm -rf -- {shlex.quote(source)}', user='root')
            if result.return_code:
                raise ValueError('could not restore observed absence in the verifier')
