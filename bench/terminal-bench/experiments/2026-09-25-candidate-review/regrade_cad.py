#!/usr/bin/env python3
"""Recover setup-failed CAD grades without changing candidates or assertions."""
import argparse
import concurrent.futures
import difflib
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import time

from tbench import replay
from reproduce import snapshot, tree, write


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--records', type=Path, required=True)
    p.add_argument('--jobs', type=Path, required=True)
    p.add_argument('--tasks', type=Path, required=True)
    a = p.parse_args()
    labels = json.loads((a.records / 'prospective-official-labels.json').read_text())['labels']
    rows = [r for r in labels if r['reward'] is None]
    tasks = {}
    for row in rows:
        trial = a.jobs / row['job'] / row['trial']
        original = json.loads((trial / 'result.json').read_text())
        if 'Cannot uninstall vtk 9.2.6' not in original['exception_info']['exception_message']:
            raise ValueError('Unknown outcome is not the recorded VTK setup failure')
        task = Path(json.loads((trial / 'config.json').read_text())['task']['path'])
        if task.name not in tasks:
            dest = a.tasks / task.name
            if dest.exists():
                raise ValueError('Use a new task-copy directory')
            shutil.copytree(task, dest)
            dockerfile = dest / 'tests/Dockerfile'
            before = dockerfile.read_text()
            original_line = "RUN pip install --no-cache-dir 'gnucleus-freecad-validator[render]==0.1.3'"
            if before.count(original_line) != 1:
                raise ValueError('Unexpected verifier dependency declaration')
            after = before.replace(original_line, "RUN pip install --no-cache-dir --ignore-installed --no-deps 'vtk==9.7.0'\n" + original_line)
            dockerfile.write_text(after)
            old_files, new_files = tree(task), tree(dest)
            changed = [k for k in set(old_files) | set(new_files) if old_files.get(k) != new_files.get(k)]
            if changed != ['tests/Dockerfile']:
                raise ValueError('An assertion or another task input changed')
            write(a.records / 'cad-regrade' / (task.name + '-environment.json'), {
                'task': task.name, 'original_task': str(task), 'regrade_task': str(dest),
                'before': old_files, 'after': new_files,
                'diff': ''.join(difflib.unified_diff(before.splitlines(True), after.splitlines(True),
                                                  fromfile='original/tests/Dockerfile', tofile='regrade/tests/Dockerfile')),
                'reason': 'Install the VTK 9.7.0 wheel already selected by the original resolver without uninstalling conda-owned VTK 9.2.6 files. Verifier assertions are unchanged.'})
            tasks[task.name] = dest

    def run(row):
        trial = a.jobs / row['job'] / row['trial']
        task = tasks[row['task'].split('/')[-1]]
        out = a.records / 'cad-regrade' / row['trial']
        started = time.monotonic()
        with tempfile.TemporaryDirectory(prefix='truth9584-cad-grade-') as tmp:
            root = Path(tmp)
            identity = snapshot(trial, root)
            workspace = replay.Workspace(root, 'attributed-snapshot', True,
                                         'Only the primary executor wrote; all collected final artifacts match.')
            result = replay.run_verifier(task, workspace, out)
            if tree(root) != identity['files']:
                raise ValueError('Regrading changed the retained candidate copy')
        record = {k: row[k] for k in ['job', 'trial', 'task', 'executor']} | {
            'original_reward': None, 'original_result_sha256': row['result_sha256'],
            'snapshot': identity, 'result': result, 'seconds': time.monotonic() - started}
        write(out / 'grade.json', record)
        return {'trial': row['trial'], 'reward': result['reward'], 'exception': result['exception']}

    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        for result in pool.map(run, rows):
            print(json.dumps(result), flush=True)


if __name__ == '__main__':
    main()
