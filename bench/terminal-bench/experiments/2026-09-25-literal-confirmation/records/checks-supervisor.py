"""Check completed attempts without reading official outcomes or duplicating work."""
import datetime
import fcntl
import json
import os
from pathlib import Path
import subprocess
import time

home = Path.home()
root = home / '.openagents/terminal-bench/experiments/truth-review-9584-literal-confirmation-v2'
launcher = Path('/tmp/truth9584-artifact-study-v2.sh')
job_names = ['archive--coder-one-truth-confirmation--9584-literal-artifact-v2',
             'archive--coder-one-truth-control--9584-literal-artifact-v2']
lock = (root / 'checks-supervisor.lock').open('a')
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
marker = root / 'checks-supervisor.json'
if marker.exists():
    raise RuntimeError('An existing supervisor record needs inspection before a restart')
record = {'pid': os.getpid(), 'started_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
          'state': 'watching', 'outcomes_read': False, 'passes': []}
def save():
    record['updated_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    temporary = marker.with_suffix('.tmp')
    temporary.write_text(json.dumps(record, indent=2) + '\n')
    temporary.replace(marker)
last_checked = -1
started = time.monotonic()
save()
while time.monotonic() - started < 8 * 3600:
    completed = sum(len(list((home / '.openagents/terminal-bench/jobs' / name).glob('*/result.json')))
                    for name in job_names)
    record['completed_attempts'] = completed
    if completed and completed > last_checked:
        number = len(record['passes']) + 1
        record['state'] = 'checking'
        save()
        log_path = root / f'checks-pass-{number:03d}.log'
        with log_path.open('x') as log:
            process = subprocess.run(['sh', str(launcher), 'check'], stdout=log, stderr=subprocess.STDOUT)
        record['passes'].append({'completed_at_start': completed, 'exit': process.returncode,
                                 'log': log_path.name})
        if process.returncode:
            record['state'] = 'check-failed'
            save()
            raise SystemExit(process.returncode)
        last_checked = completed
        record['state'] = 'watching'
    checked = len(list((root / 'checks').glob('*/combined.json')))
    record['checks'] = checked
    if completed == 90 and checked == 90:
        record['state'] = 'ready-to-seal'
        save()
        raise SystemExit(0)
    launch = json.loads((root / 'launch-wait.json').read_text())
    if launch['state'] == 'launcher-finished' and completed < 90:
        record['state'] = 'incomplete-cohort'
        save()
        raise SystemExit(1)
    save()
    time.sleep(60)
record['state'] = 'watch-deadline-exceeded'
save()
raise SystemExit(1)
