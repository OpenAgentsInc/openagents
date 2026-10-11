#!/usr/bin/env python3
"""Compile the frozen attention probe. This script does not run Metal work."""
from pathlib import Path
import hashlib
import json
import os
import subprocess
root = Path(__file__).resolve().parent
repo = Path(os.environ['OPENAGENTS_WORKTREE'])
names = ['baseline.metal', 'candidate.metal', 'probe.swift', 'build.py', 'guard.py', 'manifest.json', 'summarize.py']
def hashes():
    return {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in names}
manifest = json.loads((root / 'manifest.json').read_text())
assert hashlib.sha256((repo / manifest['production_source']).read_bytes()).hexdigest() == manifest['production_sha256']
assert hashlib.sha256((root / 'baseline.metal').read_bytes()).hexdigest() == manifest['production_sha256']
before = hashes()
commands = [['xcrun', '-sdk', 'macosx', 'metal', '-std=metal4.0', '-c', str(root / name), '-o', str(root / (name + '.air'))] for name in ['baseline.metal', 'candidate.metal']]
commands += [['xcrun', '-sdk', 'macosx', 'metallib', str(root / 'baseline.metal.air'), str(root / 'candidate.metal.air'), '-o', str(root / 'kernels.metallib')], ['xcrun', 'swiftc', '-O', '-num-threads', '2', str(root / 'probe.swift'), '-o', str(root / 'probe')]]
for command in commands:
    print(json.dumps({'command': command}), flush=True)
    subprocess.run(command, check=True)
assert hashes() == before, 'probe source changed during build'
record = {'source_sha256': before, 'artifacts': {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in ['probe', 'kernels.metallib']}, 'gpu_executed': False}
(root / 'build-artifacts.json').write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record), flush=True)
