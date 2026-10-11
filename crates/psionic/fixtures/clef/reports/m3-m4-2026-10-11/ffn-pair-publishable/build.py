#!/usr/bin/env python3
"""Compile only the task-owned synthetic probe; parent guard owns this group."""
import hashlib
import json
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parent
combined = root / 'combined.metal'
combined.write_text((root / 'baseline.metal').read_text() + (root / 'pair.metal').read_text())
commands = [
    ['xcrun', '-sdk', 'macosx', 'metal', '-std=metal4.0', '-c', str(combined), '-o', str(root / 'combined.air')],
    ['xcrun', 'swiftc', '-O', str(root / 'probe.swift'), '-o', str(root / 'probe')],
]
for command in commands:
    print(json.dumps({'command': command}), flush=True)
    subprocess.run(command, check=True)
print(json.dumps({'compiled': True, 'gpu_executed': False,
                  'binary_sha256': hashlib.sha256((root / 'probe').read_bytes()).hexdigest()}), flush=True)
