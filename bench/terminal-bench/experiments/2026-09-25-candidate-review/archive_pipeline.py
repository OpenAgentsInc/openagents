#!/usr/bin/env python3
"""Check completed attempts while later candidates run; never open outcomes."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import time

from seal_archive import population


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('cohort', 'preflight', 'jobs', 'out', 'binary', 'runtime'):
        p.add_argument('--' + name, type=Path, required=True)
    a = p.parse_args()
    command = [sys.executable, str(Path(__file__).with_name('archive_checks.py')), '--completed-only']
    for name in ('cohort', 'preflight', 'jobs', 'out', 'binary', 'runtime'):
        command += ['--' + name, str(getattr(a, name))]
    deadline = time.monotonic() + 24 * 60 * 60
    while time.monotonic() < deadline:
        subprocess.run(command, check=True)
        rows = json.loads((a.out / 'manifest.json').read_text())
        print(json.dumps({'completed_checks': len(rows), 'planned': 72, 'grades_opened': False}), flush=True)
        if len(rows) >= 72:
            population(rows)
            return
        time.sleep(30)
    raise TimeoutError('Candidate/check pipeline did not finish; retained records remain available')


if __name__ == '__main__':
    main()
