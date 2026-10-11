#!/usr/bin/env python3
"""Compile the frozen scheduling probe once; do not execute GPU work."""
import json
import subprocess
import sys
from common import ROOT, sha, verify

frozen, manifest = verify()
outputs = ['shapes.air', 'pointwise.air', 'kernels.metallib', 'probe', 'build-artifacts.json']
assert not any((ROOT / name).exists() for name in outputs), 'preserve prior build; no automatic retry'
subprocess.run([sys.executable, str(ROOT / 'source_check.py')], check=True)
subprocess.run([sys.executable, str(ROOT / 'summary_check.py')], check=True)
commands = [['xcrun', '-sdk', 'macosx', 'metal', '-std=metal4.0', '-c',
             str(ROOT / (name + '.metal')), '-o', str(ROOT / (name + '.air'))]
            for name in ['shapes', 'pointwise']]
commands += [['xcrun', '-sdk', 'macosx', 'metallib', str(ROOT / 'shapes.air'), str(ROOT / 'pointwise.air'), '-o', str(ROOT / 'kernels.metallib')],
             ['xcrun', 'swiftc', '-O', '-num-threads', '2', str(ROOT / 'probe.swift'), '-o', str(ROOT / 'probe')]]
for command in commands:
    print(json.dumps({'command': command}), flush=True)
    subprocess.run(command, check=True)
assert verify() == (frozen, manifest), 'source changed during build'
record = {'source_sha256': frozen['source_sha256'], 'guard_dependency_sha256': frozen['guard_dependency_sha256'],
          'frozen_sources_sha256': sha(ROOT / 'frozen-sources.json'),
          'artifacts': {name: sha(ROOT / name) for name in outputs[:-1]}, 'gpu_executed': False}
with (ROOT / 'build-artifacts.json').open('x') as output:
    json.dump(record, output, indent=2); output.write('\n')
print(json.dumps(record), flush=True)
