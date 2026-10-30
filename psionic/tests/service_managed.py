#!/usr/bin/env python3
"""Opt-in check of our installed loopback unit; kills its child, then stops it.

Run on the owned Linux host: service_managed.py PORT OUTPUT_DIRECTORY.
The unit must already be installed and ready. No root or boot enablement.
"""
import json
import os
import signal
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

port = int(sys.argv[1])
out = Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
unit = 'psionic-systemone.service'
url = f'http://127.0.0.1:{port}'
record = {'schema': 'psionic-managed-operations-v1', 'address': url, 'unit': unit}

def command(*args):
    return subprocess.check_output(args, text=True, timeout=15).strip()

def get(path):
    with urllib.request.urlopen(url + path, timeout=30) as response:
        return json.load(response)

def ask():
    body = {'model': 'elephant-1', 'state': 'Water is wet.', 'questions': {
        'owned': {'type': 'choice', 'instructions': 'Is the water wet?',
                  'criteria': {'yes': None, 'no': None}}}}
    req = urllib.request.Request(url + '/v1/systemone', json.dumps(body).encode())
    with urllib.request.urlopen(req, timeout=30) as response:
        return json.load(response)

try:
    record['initial_health'] = get('/readyz')
    assert record['initial_health']['ready']
    record['before'] = ask()
    assert abs(record['before']['answers']['owned']['probabilities']['yes'] - .757196285) < .0005
    parent = int(command('systemctl', '--user', 'show', unit, '-p', 'MainPID', '--value'))
    child = int(command('pgrep', '-P', str(parent)))
    record['supervisor_pid'] = parent
    record['killed_child_pid'] = child
    started = time.monotonic()
    os.kill(child, signal.SIGKILL)
    deadline = started + 180
    while time.monotonic() < deadline:
        try:
            new_child = int(command('pgrep', '-P', str(parent)))
            health = get('/readyz')
            if new_child != child and health['ready']:
                break
        except (OSError, ValueError, subprocess.SubprocessError):
            pass
        time.sleep(.2)
    else:
        raise AssertionError('managed service did not recover within 180 seconds')
    record['replacement_child_pid'] = new_child
    record['restart_ready_ms'] = (time.monotonic() - started) * 1000
    record['after_health'] = health
    record['after'] = ask()
    assert record['after_health'] == record['initial_health']
    assert record['before']['answers'] == record['after']['answers']
    record['success'] = True
finally:
    subprocess.run(['systemctl', '--user', 'stop', unit], check=True, timeout=30)
    record['final_state'] = command('systemctl', '--user', 'show', unit, '-p', 'ActiveState', '--value')
    record['boot_state'] = command('systemctl', '--user', 'show', unit, '-p', 'UnitFileState', '--value')
    (out / 'operations.json').write_text(json.dumps(record, indent=2) + '\n')
    (out / 'journal.log').write_text(command('journalctl', '--user', '-u', unit, '-n', '100', '--no-pager') + '\n')
assert record['final_state'] == 'inactive' and record['boot_state'] == 'static'
print('managed original CUDA service: ready, golden answer, crash/re-warm/restart parity, clean stop pass')
