#!/usr/bin/env python3
"""Run bounded lifecycle checks on the existing encrypted qualification guest."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import time

ROOT = Path('/space/qualification-11814/encrypted-1')
TOOLS = Path('/space/config/deployment-tools')
TURNS = {'turn': 'cwc-turn-start', 'memory': 'cwc-turn-memory',
    'memory-replacement': 'cwc-turn-memory-replacement', 'network': 'cwc-turn-network'}


def action(name):
    result = subprocess.run(['python3', str(TOOLS / 'qualification_computers.py'),
        '--run', 'encrypted-1', name], capture_output=True, timeout=40)
    if result.returncode:
        raise RuntimeError('The qualification action failed: ' + name)
    value = json.loads(result.stdout)
    print(json.dumps({'action': name, **value}), flush=True)
    return value


def wait(read, ready, seconds):
    deadline = time.monotonic() + seconds
    while True:
        value = read()
        if ready(value):
            return value
        if value.get('state') in ('failed', 'deleted', 'cancelled'):
            raise RuntimeError('The qualification resource reached an unexpected final state.')
        if time.monotonic() >= deadline:
            raise RuntimeError('The qualification deadline expired; inspect saved receipts before continuing.')
        time.sleep(5)


def stopped():
    return wait(lambda: action('status'), lambda v: v.get('state') == 'stopped', 900)


def awake():
    current = action('status')
    if current.get('state') == 'stopped':
        action('wake')
    return wait(lambda: action('status'), lambda v: v.get('state') == 'awake', 600)


def turn(name):
    receipt = ROOT / (TURNS[name] + '.json')
    if not receipt.exists():
        awake()
        action(name)
    saved = json.loads(receipt.read_text())
    if not saved.get('id'):
        raise RuntimeError('A turn submission is uncertain; inspect its receipt before continuing.')
    value = wait(lambda: action(name + '-status'),
        lambda v: v.get('exit_code') is not None, 120)
    if value['exit_code'] != 0:
        raise RuntimeError('The guest qualification command failed.')
    return value


def memory_proof(value):
    lines = value.get('stdout', '').strip().splitlines()
    if len(lines) != 4:
        raise RuntimeError('The guest memory proof has an unexpected shape.')
    expected = json.loads((ROOT / 'memory-canary.json').read_text())['sha256']
    if lines[3].split()[0] != expected:
        raise RuntimeError('The guest memory canary changed.')
    return {'boot_id': lines[0], 'pid': int(lines[1]),
        'start_ticks': int(lines[2]), 'canary_sha256': expected}


def main(phase):
    if os.getuid() != 0 or socket.gethostname().split('.')[0] != 'oa-space-service-control':
        raise RuntimeError('Run lifecycle qualification on the owned control node.')
    seed = json.loads((ROOT / 'cwc-seed.json').read_text())
    if seed.get('id') != 'cmp_ef8caf1babae3b48ed87':
        raise RuntimeError('The encrypted qualification computer differs.')
    evidence = {'computer': seed['id'], 'phase': phase}
    if phase == 'checkpoint':
        evidence['memory'] = memory_proof(turn('turn'))
        stopped()
    elif phase in ('resume', 'replacement'):
        original = json.loads((ROOT / 'lifecycle-checkpoint.json').read_text())['memory']
        evidence['memory'] = memory_proof(turn('memory' if phase == 'resume' else 'memory-replacement'))
        if evidence['memory'] != original:
            raise RuntimeError('The guest process changed across suspension.')
        stopped()
    else:
        result = turn('network')
        checks = json.loads(result.get('stdout', ''))
        if not isinstance(checks, list) or len(checks) != 7 or not all(c.get('denied') is True for c in checks):
            raise RuntimeError('The guest reached a protected endpoint.')
        evidence['denials'] = checks
        stopped()
    evidence['observed_unix'] = int(time.time())
    evidence['helper_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    target = ROOT / ('lifecycle-' + phase + '.json')
    target.write_text(json.dumps(evidence, indent=2) + '\n')
    target.chmod(0o600)
    print(json.dumps({'qualification': evidence}), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('phase', choices=['checkpoint', 'resume', 'replacement', 'network'])
    main(parser.parse_args().phase)
