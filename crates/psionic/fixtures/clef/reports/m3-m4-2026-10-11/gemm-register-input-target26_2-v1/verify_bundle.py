#!/usr/bin/env python3
"""Verify scrubbed files and restore the complete ordered bench log."""
from pathlib import Path
import hashlib
import json

root = Path(__file__).resolve().parent
sha = lambda value: hashlib.sha256(value).hexdigest()
manifest = json.loads((root / 'sanitization.json').read_text())
for source in manifest['sources']:
    parts = []
    for item in source['published']:
        data = (root / item['path']).read_bytes()
        assert len(data) == item['bytes']
        assert sha(data) == item['sha256'], item['path']
        parts.append(data)
    joined = b''.join(parts)
    assert sha(joined) == source['sanitized_sha256'], source['source']
    if source['source'] == 'run-1/stdout.jsonl':
        path = root / 'run-1/stdout-restored.jsonl'
        if path.exists():
            assert path.read_bytes() == joined
        else:
            path.write_bytes(joined)
        rows = [json.loads(line) for line in joined.splitlines()]
        assert sum(row['kind'] == 'timing' for row in rows) == 240
        assert sum(row['kind'] == 'input_roundtrip' for row in rows) == 12
        assert sum(row['kind'] == 'small_correctness' for row in rows) == 24
        assert sum(row['kind'] == 'weighted_correctness' for row in rows) == 5
        assert rows[-1]['commands'] == 319 and rows[-1]['dispatches'] == 547
for item in manifest['publication_files']:
    assert sha((root / item['path']).read_bytes()) == item['sha256']
print('Published hashes and all 319 command records verified.')
