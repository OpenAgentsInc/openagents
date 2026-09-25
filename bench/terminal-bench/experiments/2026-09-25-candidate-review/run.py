#!/usr/bin/env python3
"""Run frozen review inputs with bounded process concurrency and resumable outputs."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import subprocess
import time

p = argparse.ArgumentParser()
p.add_argument('--manifest', type=Path, required=True)
p.add_argument('--binary', type=Path, required=True)
p.add_argument('--partition', choices=['calibration', 'held-out', 'prospective'], required=True)
p.add_argument('--limit', type=int)
p.add_argument('--model', default='gpt-6-luna')
p.add_argument('--record', default='record')
a = p.parse_args()
rows = [r for r in json.loads(a.manifest.read_text()) if r['split'] == a.partition and r.get('file_count')]
if a.limit:
    rows = rows[:a.limit]


def run(row):
    source = Path(row['input'])
    out = source.parent / a.record
    if (out / 'review.json').exists():
        return row['trial'], 'retained'
    out.mkdir(exist_ok=True)
    started = time.time()
    with (out / 'process.log').open('w') as log:
        try:
            result = subprocess.run([str(a.binary), 'checks', 'review', '--input', str(source), '--out', str(out), '--model', a.model], stdout=log, stderr=subprocess.STDOUT, timeout=390)
            status = f'exit {result.returncode}'
        except subprocess.TimeoutExpired:
            status = 'process deadline; unknown usage if interrupted'
    (out / 'process.json').write_text(json.dumps({'seconds': time.time()-started, 'status': status}, indent=2)+'\n')
    return row['trial'], status


with ThreadPoolExecutor(max_workers=4) as pool:
    for trial, status in pool.map(run, rows):
        print(trial, status, flush=True)
