#!/usr/bin/env python3
"""Verify compact publication hashes without running a workload."""
from pathlib import Path
import hashlib,json
root=Path(__file__).resolve().parent
manifest=json.loads((root/'publication-index.json').read_text())
for row in manifest['files']:
 data=(root/row['path']).read_bytes()
 assert len(data)==row['bytes'] and hashlib.sha256(data).hexdigest()==row['sha256'],row['path']
rows=[json.loads(line) for line in (root/'run-1/stdout.jsonl').read_text().splitlines()]
assert len([r for r in rows if r['kind']=='timing'])==120
assert len([r for r in rows if r['kind']=='correctness'])==16
assert rows[-1]['commands']==136 and rows[-1]['dispatches']==560
print('All publication hashes and fixed coverage verified.')
