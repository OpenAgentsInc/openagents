#!/usr/bin/env python3
"""Compile only the new Swift host; never compile or alter the Metal library."""
import json
import subprocess
from common import ROOT, sha, verify

frozen, manifest = verify()
output = ROOT / 'exporter'
assert not output.exists(), 'host executable already exists; no automatic rebuild'
command = ['xcrun', 'swiftc', '-O', '-num-threads', '2', str(ROOT / 'exporter.swift'), '-o', str(output)]
print(json.dumps({'command': command, 'metal_compilation': False}), flush=True)
subprocess.run(command, check=True)
assert verify() == (frozen, manifest), 'inputs changed during host compilation'
result = {'source_sha256': frozen['source_sha256'],
          'frozen_sources_sha256': sha(ROOT / 'frozen-sources.json'),
          'unchanged_library_sha256': manifest['library_sha256'],
          'executable_sha256': sha(output), 'gpu_executed': False}
with (ROOT / 'build-artifacts.json').open('x') as out:
    json.dump(result, out, indent=2); out.write('\n')
print(json.dumps(result), flush=True)
