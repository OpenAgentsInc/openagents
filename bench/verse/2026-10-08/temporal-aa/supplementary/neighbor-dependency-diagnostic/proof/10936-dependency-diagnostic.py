import hashlib, json, os, subprocess, time
from pathlib import Path

scratch = Path(__file__).parent
root = Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents')
source = '290c1590726189dbdabb5693125cfa0fbca8a779'
binary = Path('/Users/christopherdavid/work/openagents-target-agent0/debug/examples/meteor_showcase_capture')
name = '10936-dependency-high-diagnostic'
env = os.environ.copy()
for key in ('VERSE_KIT_UNPINNED', 'VERSE_KIT_BAKE', 'VERSE_TEMPORAL_AA'):
    env.pop(key, None)
env.update(VERSE_QUALITY='high', VERSE_KIT_PACK='/Users/christopherdavid/.openagents/verse/zones-cache/c559955403b42861be3cc933ec572dafbe91c259bc2fa4c24a1cbab101a9998e.vtp', OPENAGENTS_CAPTURE_SOURCE_COMMIT=source)
command = [str(binary), str(scratch / name), '--live', '--settle-light', '--no-video', '--seconds', '9', '--impact-frame', '469', '--smoke-frame', '520', '--compare-temporal-aa', '--sequence', '439:484']
record = dict(source=source, profile='dev with optimized dependencies', features=['capture'], binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), command=command, environment={key: env[key] for key in ('VERSE_QUALITY', 'VERSE_KIT_PACK', 'OPENAGENTS_CAPTURE_SOURCE_COMMIT')}, purpose='Visual correctness diagnostic; no timing gate', start_unix=time.time())
manifest = scratch / (name + '-manifest.json')
manifest.write_text(json.dumps(record, indent=2) + '\n')
with (scratch / (name + '.log')).open('w') as log:
    result = subprocess.run(command, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
record.update(exit=result.returncode, end_unix=time.time())
manifest.write_text(json.dumps(record, indent=2) + '\n')
print(name + ' exit ' + str(result.returncode), flush=True)
raise SystemExit(result.returncode)
