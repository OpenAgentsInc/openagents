#!/usr/bin/env python3
"""Run the separate, bounded signet HALT and public-package recovery check."""
import argparse
import base64
import fcntl
import hashlib
import json
import os
from pathlib import Path
import secrets
import ssl
import subprocess
import time
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('root', type=Path)
parser.add_argument('endpoint')
parser.add_argument('release', type=Path)
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--preflight', action='store_true')
args = parser.parse_args()
os.umask(0o077)
root = args.root.resolve()
assert root.name == 'layertree-signet-halt'
assert args.endpoint == '10.128.15.213:8443'
assert args.binary.is_file()
lock = (root / 'halt-observer.lock').open('a')
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
cookie_path = Path(os.environ['LAYERTREE_CHAIN_COOKIE'])
config = json.loads((root / 'service.json').read_text())
assert config['unroll_delay'] == 12 and config['mirror_threshold'] == 2
assert config['tree_id'] == '6da34b5acfb8f376333b97d1f1a04fa9de69d2bc2cbcc0b36c4b7a5bf34163a9'
effective = json.loads((args.release / 'log-witness.json').read_text())['effective_at']


def save(path, value):
    temporary = path.with_suffix('.new')
    with temporary.open('w') as file:
        json.dump(value, file, indent=2)
        file.flush()
        os.fsync(file.fileno())
    os.replace(temporary, path)
    descriptor = os.open(path.parent, os.O_RDONLY)
    os.fsync(descriptor)
    os.close(descriptor)


def rpc(method, params):
    assert method in {'getblockchaininfo', 'getrawtransaction', 'getblockheader'}
    cookie = cookie_path.read_text().strip()
    request = urllib.request.Request('http://127.0.0.1:38332/', data=json.dumps({
        'jsonrpc': '2.0', 'id': 'lt-halt', 'method': method, 'params': params,
    }).encode(), headers={'Authorization': 'Basic ' + base64.b64encode(cookie.encode()).decode(),
                         'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, timeout=15) as response:
        raw = response.read(1048577)
    assert len(raw) <= 1048576
    value = json.loads(raw)
    if value.get('error') is not None:
        raise RuntimeError('The scratch chain reader refused a request.')
    return value['result']


def height():
    info = rpc('getblockchaininfo', [])
    assert info['chain'] == 'signet' and not info['initialblockdownload']
    return info['blocks']


def confirmation(txid):
    tx = rpc('getrawtransaction', [txid, True])
    if tx.get('confirmations', 0) == 0:
        return None
    return rpc('getblockheader', [tx['blockhash']])['height']


def guest_ready():
    # This public GET is only a sync/reachability observation. Every signing
    # call uses the Rust client's fresh attestation and pinned TLS connection.
    with urllib.request.urlopen('https://' + args.endpoint + '/health',
                                context=ssl._create_unverified_context(), timeout=10) as response:
        raw = response.read(65537)
    assert len(raw) <= 65536
    value = json.loads(raw)
    return value['chain'] == 'signet' and value['initialblockdownload'] is False


def invoke(*command):
    return subprocess.run([str(args.binary), *map(str, command)],
                          capture_output=True, text=True, timeout=180)


def cli(*command):
    result = invoke(*command)
    if result.returncode:
        raise RuntimeError(result.stderr.strip()[:500])
    return json.loads(result.stdout)


def run(action):
    return cli('run', root, args.endpoint, args.release, action)


def capture_for(txid):
    for capture in sorted(root.glob('capture-*'), reverse=True):
        path = capture / 'state-after.json'
        if path.exists() and json.loads(path.read_text()).get('outpoint') == txid + ':0':
            return capture
    raise RuntimeError('The original tree has no saved recovery package.')


if args.preflight:
    print(json.dumps({'preflight': True, 'height': height(), 'guest_ready': guest_ready(),
                      'release_effective_at': effective, 'publishes_or_spends': False}))
    raise SystemExit(0)

key_path = root / 'private' / 'nostr.key'
if not key_path.exists():
    descriptor = os.open(key_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, 'w') as file:
        file.write(secrets.token_hex(32) + '\n')
        file.flush()
        os.fsync(file.fileno())

path = root / 'halt-observer-state.json'
state = json.loads(path.read_text()) if path.exists() else {
    'phase': 'await_guest', 'started_at_unix': time.time(),
    'deadline_unix': time.time() + 26 * 3600,
    'funding_outpoint': '239a7c41b9d5410d314f659fff211ac71cb1c1f7ed085f59f18669fab7c09a7c:1',
}
save(path, state)
while time.time() < state['deadline_unix']:
    tip = None
    try:
        tip = height()
        phase = state['phase']
        if phase == 'paid':
            break
        if phase == 'await_guest':
            if time.time() >= effective and guest_ready() and confirmation(state['funding_outpoint'].split(':')[0]) is not None:
                state['phase'] = 'genesis'
        elif phase == 'genesis':
            if (root / 'inflight.json').exists():
                pending = json.loads((root / 'inflight.json').read_text())
                assert pending['action'] == 'genesis'
                txid = pending['txid']
            else:
                txid = run('genesis')['txid']
            state.update(phase='confirm_genesis', tree_transaction=txid)
        elif phase == 'confirm_genesis':
            confirmed = confirmation(state['tree_transaction'])
            if confirmed is not None:
                checked = run('status')
                assert checked['status'] == 'attested' and not checked['halted']
                state.update(phase='publish', tree_confirmation_height=confirmed,
                             first_valid_unroll_block=confirmed + config['unroll_delay'])
        elif phase == 'publish':
            result = cli('mirror', root, capture_for(state['tree_transaction']), root / 'nostr-outbox')
            assert result['readback_verified']
            state.update(phase='halt', public_mirror=result)
        elif phase == 'halt':
            marker = root / 'halt-started.json'
            if not marker.exists():
                save(marker, {'tree_transaction': state['tree_transaction'], 'at_unix': time.time()})
                result = run('halt')
                assert result['halted']
            checked = run('status')
            assert checked['halted'] and checked['outpoint'] == state['tree_transaction'] + ':0'
            state.update(phase='check_refusal', halted_at_height=tip, halted_at_unix=time.time(),
                         halted_state=checked)
        elif phase == 'check_refusal':
            marker = root / 'halt-refusal-started.json'
            if marker.exists():
                raise RuntimeError('Inspect the original post-HALT signing check before resuming.')
            save(marker, {'at_unix': time.time()})
            refused = invoke('run', root, args.endpoint, args.release, 'refresh')
            # Never accept a timeout, unrelated error, or successful transition
            # as evidence that the enclave stopped signing.
            assert refused.returncode != 0 and 'Halted' in refused.stderr
            assert not (root / 'inflight.json').exists()
            checked = run('status')
            assert checked['halted'] and checked['outpoint'] == state['tree_transaction'] + ':0'
            save(root / 'halt-signing-refusal.json', {'refusal': 'Halted', 'at_height': tip,
                                                       'checked_state': checked})
            state.update(phase='unroll', signing_refusal='Halted')
        elif phase == 'unroll':
            # This command reads saved public bytes and the independent Core
            # node. It does not contact the enclave, even while it is online.
            result = cli('unroll', root)
            state['unroll'] = result
            if result.get('one_block_early'):
                tests = result.get('testmempoolaccept', [])
                assert len(tests) == 1 and tests[0].get('allowed') is False
                state['one_block_early_refusal'] = result
            if result['status'] == 'paid':
                assert len(result['payouts']) == 5 and all(p['amount_sat'] == 1000 for p in result['payouts'])
                assert result['fee_child_confirmations'] > 0
                assert result['unroll_confirmation_height'] >= state['first_valid_unroll_block']
                state.update(phase='paid', paid_at_unix=time.time())
        if state['phase'] != 'needs_review':
            state.pop('last_error', None)
    except Exception as error:
        state['last_error'] = (str(error) or type(error).__name__)[:500]
        if state['phase'] == 'genesis' and (root / 'inflight.json').exists():
            pending = json.loads((root / 'inflight.json').read_text())
            assert pending['action'] == 'genesis'
            state.update(phase='confirm_genesis', tree_transaction=pending['txid'])
        elif state['phase'] in {'halt', 'check_refusal'}:
            state['phase'] = 'needs_review'
    state['checked_at_unix'] = time.time()
    save(path, state)
    print(json.dumps({'phase': state['phase'], 'height': tip, 'last_error': state.get('last_error')}), flush=True)
    time.sleep(60)
else:
    state.update(phase='time_limit', stopped_at_unix=time.time())
    save(path, state)
