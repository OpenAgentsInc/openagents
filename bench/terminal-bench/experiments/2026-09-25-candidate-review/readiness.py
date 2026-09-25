#!/usr/bin/env python3
"""Build and assess label-free final-result packets, including unavailable source."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import subprocess

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--manifest', type=Path, required=True)
p.add_argument('--audits', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
p.add_argument('--binary', type=Path, required=True)
p.add_argument('--partition', required=True)
a = p.parse_args()
rows = [r for r in json.loads(a.manifest.read_text()) if r['split'] == a.partition]


def run(row):
    audit_path = a.audits / (row['trial'] + '.json')
    audit = json.loads(audit_path.read_text()) if audit_path.exists() else {}
    packet = Path(row['input']) if row.get('input') else None
    candidate = json.loads(packet.read_text()) if packet and packet.exists() else {
        'task': audit.get('public_task', ''), 'files': {},
        'coverage': 'No final source packet was retained. This does not establish that a deliverable is absent.'}
    if not candidate['task']:
        return row['trial'], 'no public task'
    report = audit.get('selected_report')
    if report and len(report.encode()) > 64000:
        report = report.encode()[:31000].decode(errors='ignore') + '\n[Middle omitted]\n' + report.encode()[-31000:].decode(errors='ignore')
    value = {'candidate': candidate, 'report': report,
             'observations': 'No independent command output is supplied. Checks described in the report are writer claims, not independently replayed tests.'}
    output = a.out / row['trial'] / 'readiness'
    output.mkdir(parents=True, exist_ok=True)
    source = output / 'input.json'
    payload = json.dumps(value, indent=2)+'\n'
    if source.exists() and source.read_text() != payload:
        raise ValueError('Readiness packet changed; preserve the previous record')
    source.write_text(payload)
    if (output / 'assessment.json').exists():
        return row['trial'], 'retained'
    with (output / 'process.log').open('w') as log:
        try:
            result = subprocess.run([str(a.binary), 'checks', 'readiness', '--input', str(source), '--out', str(output)], stdout=log, stderr=subprocess.STDOUT, timeout=210)
            status = 'exit '+str(result.returncode)
        except subprocess.TimeoutExpired:
            status = 'process deadline; usage unavailable'
    (output / 'process.json').write_text(json.dumps({'status': status, 'input_sha256': hashlib.sha256(payload.encode()).hexdigest()},indent=2)+'\n')
    return row['trial'], status


with ThreadPoolExecutor(max_workers=4) as pool:
    for trial, status in pool.map(run, rows):
        print(trial, status, flush=True)
