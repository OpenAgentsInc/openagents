#!/usr/bin/env python3
"""Package the completed deployment-target GEMM qualification without artifacts."""
from pathlib import Path
import hashlib
import json

root = Path(__file__).resolve().parent
out = root / 'publication-v1'
out.mkdir(exist_ok=False)
sha = lambda value: hashlib.sha256(value).hexdigest()
replacements = {
    '<REPO>': '<REPO>',
    '<TASK_SCRATCH>': '<TASK_SCRATCH>',
    '<USER_HOME>': '<USER_HOME>',
}
manifest = {
    'identity': 'gemm-register-input-target26_2-publication-v1',
    'original_root': '<TASK_SCRATCH>/gemm-register-input-target26_2',
    'rules': [
        'Replace private absolute paths with labels; preserve all recorded original hashes.',
        'Split the bench JSONL at line boundaries without omitting or reordering records.',
        'Preserve all checks and timings, including the failed speed gate.',
        'Exclude executable, AIR, metallib, and model weights; retain their hashes.',
    ],
    'sources': [],
}

def copy(name, destination=None, split=False):
    raw = (root / name).read_bytes()
    cleaned = raw.decode()
    for old, new in replacements.items():
        cleaned = cleaned.replace(old, new)
    data = cleaned.encode()
    dest = destination or name
    outputs = []
    chunks = [data]
    if split:
        chunks = []
        block = b''
        for line in data.splitlines(keepends=True):
            if len(block) + len(line) > 55000:
                chunks.append(block)
                block = b''
            block += line
        if block:
            chunks.append(block)
        assert b''.join(chunks) == data
    for i, chunk in enumerate(chunks):
        target_name = f'{dest}/part-{i:02}.jsonl' if split else dest
        target = out / target_name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(chunk)
        outputs.append({'path': target_name, 'sha256': sha(chunk), 'bytes': len(chunk)})
    manifest['sources'].append({
        'source': name, 'original_sha256': sha(raw), 'original_bytes': len(raw),
        'sanitized_sha256': sha(data), 'published': outputs,
        'transformation': 'Path substitutions and ordered line split.' if split else 'Path substitutions only.',
    })

frozen = json.loads((root / 'frozen-sources.json').read_text())
for name in frozen['source_sha256']:
    copy(name, 'source/' + name)
for name in ['source-delta.json', 'source-changes.patch', 'frozen-sources.json', 'source-check.json', 'build-artifacts.json', 'build-lease-1.json', 'build-memory-lease-1.json', 'gpu-lease-1.json', 'gpu-memory-lease-1.json']:
    copy(name)
for directory in ['build-1', 'run-1']:
    for name in ['receipt.json', 'memory.jsonl', 'stderr.log', 'stdout.jsonl']:
        copy(directory + '/' + name, split=directory == 'run-1' and name == 'stdout.jsonl')
copy('run-1/summary.json')
copy('package.py', 'package.py')
(out / 'sanitization.json').write_text(json.dumps(manifest, indent=2) + '\n')
