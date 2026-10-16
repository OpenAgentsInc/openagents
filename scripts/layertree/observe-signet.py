#!/usr/bin/env python3
"""Run the bounded signet experiment through the reviewed Rust research CLI."""
import argparse
import base64
import fcntl
import json
import os
from pathlib import Path
import ssl
import subprocess
import time
import urllib.request

os.umask(0o077)
parser = argparse.ArgumentParser()
parser.add_argument('root', type=Path)
parser.add_argument('endpoint')
parser.add_argument('release', type=Path)
parser.add_argument('--kill-at', type=int, required=True)
parser.add_argument('--binary', type=Path, required=True)
parser.add_argument('--preflight', action='store_true')
args = parser.parse_args()
root = args.root.resolve()
config = json.loads((root / 'service.json').read_text())
assert config['unroll_delay'] == 12
assert config['mirror_threshold'] == 2 and len(config['mirrors']) == 3
assert args.kill_at > time.time() - 2 * 86400
assert args.kill_at - time.time() <= 9 * 86400
assert args.endpoint == '10.128.15.212:8443'
cookie_path = Path(os.environ['LAYERTREE_CHAIN_COOKIE'])
assert cookie_path.name == '.cookie' and cookie_path.parent.name == 'signet'
lock = (root / 'observer.lock').open('a')
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)


def save(path, value):
    stage = path.with_suffix('.writing')
    with stage.open('w') as file:
        json.dump(value, file, indent=2)
        file.flush()
        os.fsync(file.fileno())
    stage.replace(path)
    descriptor = os.open(path.parent, os.O_RDONLY)
    os.fsync(descriptor)
    os.close(descriptor)


def rpc(method, params):
    assert method in {'getblockchaininfo', 'getblockcount', 'getrawtransaction', 'getblockheader'}
    cookie = cookie_path.read_text().strip()
    request = urllib.request.Request('http://127.0.0.1:38332/', data=json.dumps({
        'jsonrpc': '2.0', 'id': 'lt-observer', 'method': method, 'params': params,
    }).encode(), headers={'Authorization': 'Basic ' + base64.b64encode(cookie.encode()).decode(),
                         'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, timeout=15) as response:
        raw = response.read(1048577)
    assert len(raw) <= 1048576
    value = json.loads(raw)
    if value.get('error') is not None:
        raise RuntimeError('The scratch chain reader refused a request.')
    return value['result']


def chain():
    info = rpc('getblockchaininfo', [])
    assert info['chain'] == 'signet' and info['initialblockdownload'] is False
    return info['blocks']


def cli(*command):
    result = subprocess.run([str(args.binary), *map(str, command)],
                            capture_output=True, text=True, timeout=180)
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
    raise RuntimeError('The submitted tree transaction has no saved recovery data.')


def confirmation(txid):
    tx = rpc('getrawtransaction', [txid, True])
    if tx.get('confirmations', 0) == 0:
        return None
    return rpc('getblockheader', [tx['blockhash']])['height']


def available():
    # This untrusted public GET is only a reachability check. Every signing
    # request goes through the Rust client's verified and pinned channel.
    try:
        with urllib.request.urlopen('https://' + args.endpoint + '/health',
                                    context=ssl._create_unverified_context(), timeout=10) as response:
            return response.status == 200
    except Exception:
        return False


def restart_late_window(state, now, kill_at):
    """Retain an interrupted measurement and begin a full new week."""
    # A later miss stays in the history but cannot erase a full clean week
    # already observed. Keep supervising until the original kill and unroll.
    if 'qualified_soak' in state:
        return False
    previous = state.get('interrupted_soaks', [])
    handled = {row['txid'] for window in previous for row in window['late_heartbeats']}
    late = [row for row in state.get('late_heartbeats', []) if row['txid'] not in handled]
    if not late or 'soak_began_at_unix' not in state:
        return False
    state.setdefault('interrupted_soaks', []).append({
        'began_at_unix': state['soak_began_at_unix'],
        'planned_end_at_unix': state['soak_ends_at_unix'],
        'ended_at_unix': now, 'late_heartbeats': late,
    })
    state.update(soak_began_at_unix=now, soak_began_height=state['last_confirmation_height'],
                 soak_ends_at_unix=now + 7 * 86400,
                 qualification_window_fits_before_kill=now + 7 * 86400 + 1800 < kill_at)
    state.pop('elapsed_week_observed_at_unix', None)
    return True


if args.preflight:
    print(json.dumps({'preflight': True, 'height': chain(), 'endpoint_available': available(),
                      'kill_at': args.kill_at, 'publishes_or_spends': False}))
    raise SystemExit(0)

state_path = root / 'observer-state.json'
state = json.loads(state_path.read_text()) if state_path.exists() else {
    'phase': 'normal', 'step': 0, 'started_at_unix': time.time(),
    'kill_at_unix': args.kill_at, 'deadline_unix': args.kill_at + 2 * 86400,
    'normal_actions': ['genesis', 'join', 'update', 'exit', 'refresh'],
    'confirmations': [], 'missing_endpoint_checks': 0,
}
assert state['kill_at_unix'] == args.kill_at
restart_late_window(state, time.time(), args.kill_at)
save(state_path, state)

state.setdefault('pending_publications', [])
while time.time() <= state['deadline_unix']:
    height = None
    try:
        height = chain()
        if state['phase'] == 'paid':
            break
        if time.time() >= args.kill_at and state['phase'] != 'unroll':
            state['missing_endpoint_checks'] = 0 if available() else state['missing_endpoint_checks'] + 1
            if state['missing_endpoint_checks'] >= 3:
                state.update(phase='unroll', endpoint_lost_at_unix=time.time(),
                             cloud_deletion_evidence='pending_cloud_audit')
        if state['phase'] in {'normal', 'soak', 'soak_setup'} and (root / 'inflight.json').exists():
            inflight = json.loads((root / 'inflight.json').read_text())
            expected = state['normal_actions'][state['step']] if state['phase'] == 'normal' else 'refresh'
            assert inflight['action'] == expected
            state.update(phase='confirm', transaction_id=inflight['txid'], action=inflight['action'])
        phase = state['phase']
        if phase == 'normal':
            action = state['normal_actions'][state['step']]
            if (root / 'inflight.json').exists():
                inflight = json.loads((root / 'inflight.json').read_text())
                assert inflight['action'] == action
                result = {'txid': inflight['txid']}
            else:
                result = run(action)
            state.update(phase='confirm', transaction_id=result['txid'], action=action)
        elif phase == 'confirm':
            confirmed = confirmation(state['transaction_id'])
            if confirmed is not None:
                run('status')
                previous = state.get('last_confirmation_height')
                gap = None if previous is None else confirmed - previous
                row = {'action': state['action'], 'txid': state['transaction_id'],
                       'height': confirmed, 'observed_at_unix': time.time(), 'gap_blocks': gap}
                state['confirmations'].append(row)
                if 'soak_began_at_unix' in state and gap > config['unroll_delay'] // 2:
                    state.setdefault('late_heartbeats', []).append(row)
                state['last_confirmation_height'] = confirmed
                restart_late_window(state, time.time(), args.kill_at)
                capture = str(capture_for(state['transaction_id']))
                state['pending_publications'].append({'txid': state['transaction_id'], 'capture': capture})
                if state['step'] < len(state['normal_actions']) - 1:
                    state['step'] += 1
                    state['phase'] = 'normal'
                else:
                    state['phase'] = 'soak' if 'soak_began_at_unix' in state else 'soak_setup'
        elif phase in {'soak_setup', 'soak'}:
            if (phase == 'soak' and time.time() >= state['soak_ends_at_unix']
                    and height - state['last_confirmation_height'] < config['unroll_delay'] // 2):
                observed = time.time()
                state.setdefault('elapsed_week_observed_at_unix', observed)
                state.setdefault('qualified_soak', {
                    'began_at_unix': state['soak_began_at_unix'],
                    'ended_at_unix': state['soak_ends_at_unix'],
                    'observed_at_unix': observed, 'observed_height': height,
                })
            # Broadcast early enough to allow confirmation delays inside the
            # six-block target. The twelve-block exit delay stays unchanged.
            if height >= state['last_confirmation_height'] + config['unroll_delay'] // 3 - 1:
                result = run('refresh')
                state.update(phase='confirm', transaction_id=result['txid'], action='refresh')
            elif phase == 'soak_setup' and not state['pending_publications']:
                end = time.time() + 7 * 86400
                if end + 1800 >= args.kill_at:
                    raise RuntimeError('The planned kill leaves less than a full week of normal operation.')
                state.update(phase='soak', soak_began_at_unix=time.time(),
                             soak_began_height=height, soak_ends_at_unix=end)
        elif phase == 'unroll':
            result = cli('unroll', root)
            state['unroll_last_result'] = result
            if result['status'] == 'paid':
                state.update(phase='paid', paid_at_unix=time.time())
        elif phase != 'needs_review':
            raise RuntimeError('Unknown experiment phase.')
        # Public file retries never hold up the next due heartbeat. Each
        # outbox already contains the exact signed pair, so retries reuse IDs.
        if state['pending_publications'] and state['phase'] not in {'paid', 'unroll', 'needs_review'}:
            if height < state.get('last_confirmation_height', height) + 2:
                item = state['pending_publications'][0]
                try:
                    cli('mirror', root, item['capture'], root / 'nostr-outbox' / item['txid'])
                    state['pending_publications'].pop(0)
                    state.pop('last_publication_error', None)
                except Exception as error:
                    state['last_publication_error'] = str(error)[:500]
        state.pop('last_error', None)
    except Exception as error:
        message = str(error)[:500]
        state['last_error'] = message
        if 'Bitcoin is syncing signet' not in message:
            inflight_path = root / 'inflight.json'
            if inflight_path.exists() and state['phase'] in {'normal', 'soak', 'soak_setup'}:
                inflight = json.loads(inflight_path.read_text())
                state.update(phase='confirm', transaction_id=inflight['txid'], action=inflight['action'])
            elif state['phase'] not in {'confirm', 'unroll'}:
                state['phase'] = 'needs_review'
    state['checked_at_unix'] = time.time()
    save(state_path, state)
    print(json.dumps({'phase': state['phase'], 'height': height,
                      'last_error': state.get('last_error'),
                      'last_publication_error': state.get('last_publication_error')}), flush=True)
    time.sleep(15)
else:
    state.update(phase='time_limit', stopped_at_unix=time.time())
    save(state_path, state)
