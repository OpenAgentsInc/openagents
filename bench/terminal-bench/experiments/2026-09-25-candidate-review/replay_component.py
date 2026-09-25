#!/usr/bin/env python3
"""Replay the source/report union from retained records, without credentials."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--manifest', type=Path, required=True)
p.add_argument('--records', type=Path, required=True)
p.add_argument('--binary', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
rows = json.loads(a.manifest.read_text())
a.out.mkdir(parents=True, exist_ok=True)


def run(row):
    path = a.records / ('report-audit-'+row['split']) / (row['trial']+'.json')
    audit = json.loads(path.read_text()) if path.exists() else None
    packet = a.records / row['trial'] / 'input.json'
    if packet.exists() and hashlib.sha256(packet.read_bytes()).hexdigest() != row['input_sha256']:
        raise ValueError('Candidate input does not match its manifest')
    candidate = json.loads(packet.read_text()) if packet.exists() else {
        'task': audit['public_task'] if audit else 'Unavailable public task.',
        'files': {}, 'coverage': 'No retained packet.'}
    path = packet.parent / 'astra/review.json'
    review = json.loads(path.read_text()) if path.exists() else None
    fixture = {'schema':'openagents.coder-one.component-fixture.v1','component':'verify.truthful',
        'source':{'job':row['job'],'trial':row['trial']},
        'input':{'candidate':candidate,'report':audit.get('selected_report') if audit else None,'review':review,'audit':audit},'retained':None}
    with tempfile.TemporaryDirectory(prefix='truth-replay-') as directory:
        Path(directory, 'verify.truthful.json').write_text(json.dumps(fixture))
        process = subprocess.run([str(a.binary),'component','run','verify.truthful','--fixture',directory,'--jev','off','--no-record','--json'],capture_output=True,text=True,timeout=15,check=True)
        value = json.loads(process.stdout)
        (a.out / (row['trial']+'.json')).write_text(json.dumps(value,indent=2)+'\n')
    return row['trial']


with ThreadPoolExecutor(max_workers=4) as pool:
    for trial in pool.map(run, rows):
        print(trial, flush=True)
