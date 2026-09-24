import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

home = Path.home()
repo = home / '.cache/openagents/worktrees/microluna-iteration-speed'
source = home / '.cache/openagents/target-iteration-speed-musl/x86_64-unknown-linux-musl/release/coder-one'
artifact = home / '.cache/openagents/artifacts/coder-one-retained-5e9aa12daf'
uv = '/nix/store/ipjv9qq222qldhqmvg4g1bdz3frppg65-uv-0.11.21/bin/uv'
env = os.environ.copy()
env['TYPESAFE_API_KEY'] = json.loads((home / '.openagents/jev.json').read_text())['api_key']
env['OPENAGENTS_API_KEY'] = (home / '.openagents/bearer').read_text().strip()
env['CODEX_FORCE_AUTH_JSON'] = '1'
for key in ['ANTHROPIC_API_KEY', 'ANTHROPIC_AUTH_TOKEN', 'OPENAI_API_KEY', 'CLAUDE_CODE_OAUTH_TOKEN']:
    env.pop(key, None)
version = subprocess.check_output([str(source), '--version'], text=True).strip()
assert '5e9aa12daf' in version, version
digest = hashlib.sha256(source.read_bytes()).hexdigest()
artifact.parent.mkdir(parents=True, exist_ok=True)
if artifact.exists():
    assert hashlib.sha256(artifact.read_bytes()).hexdigest() == digest
else:
    shutil.copy2(source, artifact)

experiment = 'iteration-speed-9618'
directory = home / '.openagents/terminal-bench/experiments' / experiment
directory.mkdir(parents=True, exist_ok=True)
arms = ['coder-one-microluna-v13-retained']
pins = {
    'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(),
    'version': version,
    'artifact_sha256': digest,
    'artifact_path': str(artifact),
    'task_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=home / '.openagents/terminal-bench/upstream/terminal-bench-v4.0.0', text=True).strip(),
    'policies': {name: hashlib.sha256((repo / 'crates/coder-one/policies' / (name.removeprefix('coder-one-') + '.json')).read_bytes()).hexdigest() for name in arms},
    'tasks': ['session-window-debug', 'embedding-drift-monitor'],
    'attempts_per_arm_per_task': 3,
    'planned_spend_ceiling_usd': 1,
    'max_concurrent': 2,
    'classification': 'selected development comparison',
    'pricing': 'pinned binary estimates; include Jev; not a subscription invoice',
}
pinfile = directory / 'pins.json'
if pinfile.exists():
    assert json.loads(pinfile.read_text()) == pins, 'pins changed'
else:
    pinfile.write_text(json.dumps(pins, indent=2) + '\n')
command = [uv, 'run', '-q', 'tbench', 'suite', sys.argv[1], '--agent', arms[0], '--profile', 'tb4', '--tasks', ','.join(pins['tasks']), '--attempts', '3', '--max-concurrent', '2', '--max-cpus', '8', '--max-mem-gb', '32', '--min-free-disk-gb', '60']
for arm in arms:
    command += ['--agent-kwarg', f'artifact_path={artifact}', '--agent-kwarg', f'artifact_sha256={digest}']
if sys.argv[1] == 'run':
    command += ['--detach']
print(json.dumps({'experiment': experiment, 'pins': pins}, indent=2), flush=True)
subprocess.run(command, cwd=repo / 'bench/terminal-bench', env=env, check=True)
