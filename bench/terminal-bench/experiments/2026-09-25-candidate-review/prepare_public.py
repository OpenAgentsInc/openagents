#!/usr/bin/env python3
"""Retain bounded public environment inputs without reading solutions or grades."""
import argparse
import hashlib
import json
from pathlib import Path
import sqlite3


def public_files(task):
    root = task / 'environment'
    files, omitted, hashes = {}, [], {}
    used = 0
    paths = sorted(root.rglob('*'), key=lambda p: (p.suffix not in {'.md', '.json', '.csv', '.yaml', '.db'}, str(p)))
    for p in paths:
        if p.is_symlink() or not p.is_file():
            continue
        name = str(p.relative_to(task))
        if p.stat().st_size > 1_000_000 or used >= 256_000 or len(files) >= 100:
            omitted.append(name + ': size or count bound')
            continue
        raw = p.read_bytes()
        if raw.startswith(b'SQLite format 3\x00'):
            try:
                with sqlite3.connect(p.resolve().as_uri() + '?mode=ro&immutable=1', uri=True) as db:
                    text = '\n'.join(db.iterdump())
                name += '.public-sql-dump'
            except sqlite3.Error:
                omitted.append(name + ': unreadable SQLite input')
                continue
        else:
            try:
                text = raw.decode('utf-8')
                if '\x00' in text:
                    raise UnicodeError()
            except UnicodeError:
                omitted.append(name + ': binary input')
                continue
        size = len(text.encode())
        if size > 128_000 or size + used > 256_000:
            omitted.append(name + ': complete file exceeds text bound')
            continue
        files[name] = text
        hashes[name] = hashlib.sha256(raw).hexdigest()
        used += size
    return files, omitted, hashes


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--manifest', type=Path, required=True)
    p.add_argument('--jobs', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    a = p.parse_args()
    manifest = json.loads(a.manifest.read_text())
    done = set()
    index = []
    for row in manifest:
        if row['task'] in done:
            continue
        done.add(row['task'])
        cfg = json.loads((a.jobs / row['job'] / row['trial'] / 'config.json').read_text())
        task = Path(cfg['task']['path'])
        if task.name != row['task'].split('/')[-1]:
            raise ValueError('Task identity mismatch')
        import re
        instruction = re.sub(r'<!--.*?-->', '', (task / 'instruction.md').read_text(), flags=re.S).strip()
        files, omitted, hashes = public_files(task)
        value = {'task': instruction, 'provided': files,
                 'coverage': 'Initial public environment files, never final candidate files. SQLite databases are complete SQL dumps. No absence establishes a defect. Omitted: ' + '; '.join(omitted)}
        dest = a.out / task.name
        dest.mkdir(parents=True, exist_ok=True)
        path = dest / 'spec.json'
        serialized = json.dumps(value, indent=2) + '\n'
        if path.exists() and path.read_text() != serialized:
            raise ValueError('Refusing to replace changed public inputs')
        path.write_text(serialized)
        index.append({'task': row['task'], 'split': row['split'], 'input': str(path),
                      'input_sha256': hashlib.sha256(serialized.encode()).hexdigest(), 'files': hashes})
    (a.out / 'manifest.json').write_text(json.dumps(index, indent=2) + '\n')
    print('Prepared', len(index), 'public task specifications')
