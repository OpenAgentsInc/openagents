#!/usr/bin/env python3
"""Apply the frozen citation recovery to retained mini-task observations."""
import argparse
import json
import os
from pathlib import Path
import subprocess

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--root', type=Path, required=True)
p.add_argument('--binary', type=Path, required=True)
a = p.parse_args()
env = dict(os.environ)
env['TYPESAFE_API_KEY'] = json.loads((Path.home() / '.openagents/jev.json').read_text())['api_key']
for source in sorted(a.root.glob('*/reproduced/review.json')):
    trial = source.parent.parent
    out = trial / 'reproduced-citations-v2'
    if not (out / 'review.json').exists():
        out.mkdir(exist_ok=True)
        with (out / 'process.log').open('w') as log:
            subprocess.run([str(a.binary), 'checks', 'reproduced-rejudge', '--input', str(trial / 'input.json'),
                            '--review', str(source), '--out', str(out)], env=env, stdout=log,
                           stderr=subprocess.STDOUT, timeout=200, check=True)
    review = json.loads((out / 'review.json').read_text())
    print(json.dumps({'candidate': trial.name, 'call': review['call'], 'score': review['score']}), flush=True)
