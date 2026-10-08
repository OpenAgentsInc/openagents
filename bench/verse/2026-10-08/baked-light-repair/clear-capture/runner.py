import hashlib, json, os, subprocess, sys, time
from pathlib import Path
scratch = Path(__file__).parent
root = Path('/Users/christopherdavid/.codex/worktrees/bb65/openagents')
target = Path('/Users/christopherdavid/work/openagents-target-agent0')
source = '290c1590726189dbdabb5693125cfa0fbca8a779'
binary = target / 'debug/examples/baked_light_capture'
name = '10907-published-clear'
preflight = len(sys.argv) > 1 and sys.argv[1] == 'preflight'
if preflight: name = '10907-published-default-preflight'
env = os.environ.copy()
env.pop('VERSE_KIT_UNPINNED', None)
env.update(VERSE_QUALITY='high', VERSE_HOME=str(scratch / '10907-capture-cache'),
           VERSE_KIT_PACK='/Users/christopherdavid/.openagents/verse/zones-cache/c559955403b42861be3cc933ec572dafbe91c259bc2fa4c24a1cbab101a9998e.vtp',
           VERSE_KIT_BAKE=str(scratch / '10907-matched-bake/090f106f9c459f62f9cd67902a18531b7e00f9d15e292e0b60a0dd7b231b0b6b.vlay'),
           OPENAGENTS_CAPTURE_SOURCE_COMMIT=source)
command = [str(binary), str(scratch / name), '128', '2']
command += ['--preflight-only'] if preflight else ['--destruction-only', '--repair-hold-seconds', '600']
record = dict(source=source, profile='dev with optimized dependencies', features=['capture','dev-destruction'],
              binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), command=command,
              environment={key: env[key] for key in ('VERSE_QUALITY','VERSE_HOME','VERSE_KIT_PACK','VERSE_KIT_BAKE','OPENAGENTS_CAPTURE_SOURCE_COMMIT')},
              removed_environment=['VERSE_KIT_UNPINNED'], start_unix=time.time())
manifest = scratch / (name + '-manifest.json')
manifest.write_text(json.dumps(record, indent=2) + '\n')
print('Starting ' + name, flush=True)
with (scratch / (name + '.log')).open('w') as log:
 result = subprocess.run(command, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
record.update(exit=result.returncode, end_unix=time.time())
manifest.write_text(json.dumps(record, indent=2) + '\n')
print(name + ' exit ' + str(result.returncode), flush=True)
raise SystemExit(result.returncode)
