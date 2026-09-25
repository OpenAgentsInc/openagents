#!/usr/bin/env python3
"""Build label-free review inputs from retained Harbor output manifests."""
import argparse
import hashlib
import json
from pathlib import Path


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prepare(row, jobs, out):
    trial = jobs / row['job'] / row['trial']
    configs = [trial / 'config.json', trial.parent / 'config.json']
    task = None
    for cfg in configs:
        if not cfg.exists():
            continue
        config = json.loads(cfg.read_text())
        paths = [config.get('task', {}).get('path')]
        paths += [t.get('path') for t in config.get('tasks', [])]
        task = next((Path(p) for p in paths if p and Path(p).name == row['task'].split('/')[-1]), None)
        if task:
            break
    instruction = task / 'instruction.md' if task else None
    manifest = trial / 'artifacts/manifest.json'
    if not instruction or not instruction.exists() or not manifest.exists():
        return {'unavailable': 'Public instruction or final output manifest unavailable'}
    import re
    public = re.sub(r'<!--.*?-->', '', instruction.read_text(), flags=re.S).strip()
    files, omissions, hashes = {}, [], {}
    used = 0
    for entry in sorted(json.loads(manifest.read_text()), key=lambda e: e['source']):
        if entry['source'].startswith('/logs'):
            continue
        if entry['status'] != 'ok':
            omissions.append(entry['source'] + ': artifact unavailable')
            continue
        dest = trial / entry['destination']
        if not dest.resolve().is_relative_to(trial.resolve()):
            raise ValueError('Manifest path escapes trial')
        found = sorted(dest.rglob('*')) if dest.is_dir() else [dest]
        for path in found:
            if path.is_symlink() or not path.is_file():
                continue
            name = str(Path(entry['source']) / path.relative_to(dest)) if dest.is_dir() else entry['source']
            if any(p in {'.git', '__pycache__', 'node_modules', '.venv'} for p in Path(name).parts):
                continue
            size = path.stat().st_size
            if size > 160_000 - used or len(files) >= 100:
                omissions.append(name + ': exceeds input bounds')
                continue
            raw = path.read_bytes()
            try:
                text = raw.decode('utf-8')
                if '\x00' in text:
                    raise UnicodeError()
            except UnicodeError:
                omissions.append(name + ': binary data')
                continue
            used += len(raw)
            files[name] = text
            hashes[str(path.relative_to(trial))] = sha(path)
    coverage = 'Only final files collected by Harbor are present. No absence proves a missing deliverable. No toolchain, verifier, writer report, or execution result is supplied. Files are complete, not excerpted. Unavailable: ' + '; '.join(omissions)
    packet = {'task': public, 'files': files, 'coverage': coverage}
    dest = out / row['trial']
    dest.mkdir(parents=True, exist_ok=True)
    (dest / 'input.json').write_text(json.dumps(packet, indent=2) + '\n')
    return {'input': str(dest / 'input.json'), 'input_sha256': sha(dest / 'input.json'), 'files': hashes,
            'instruction_sha256': sha(instruction), 'manifest_sha256': sha(manifest), 'file_count': len(files), 'bytes': used}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--rows', type=Path, required=True)
    parser.add_argument('--jobs', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    rows = [json.loads(line) for line in args.rows.read_text().splitlines()]
    results = []
    for row in rows:
        result = {k: row[k] for k in ['job', 'trial', 'task', 'split']}
        result.update(prepare(row, args.jobs, args.out))
        results.append(result)
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / 'manifest.json').write_text(json.dumps(results, indent=2) + '\n')
    for part in ['calibration', 'held-out']:
        group = [r for r in results if r['split'] == part]
        print(part, len(group), 'with final text files', sum(bool(r.get('file_count')) for r in group))
