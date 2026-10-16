#!/usr/bin/env python3
"""Watch the original #11862 recovery and return only its research coins."""
import argparse
import base64
from decimal import Decimal
import fcntl
import json
import os
from pathlib import Path
import subprocess
import time
import urllib.request

os.umask(0o077)
parser = argparse.ArgumentParser()
parser.add_argument('root', type=Path)
parser.add_argument('--return-address', required=True)
parser.add_argument('--preflight', action='store_true')
args = parser.parse_args()
root = args.root.resolve()
assert root == Path.home() / 'lnd-signet-recovery-11862'
assert args.return_address == 'tb1q8huxwfzrjldz03lfgjjsvm9ckcyz4wdjf4prud'
identities = {
    'watch': '03eb625a2a7d58fb7ae2d1b222c017947adc1ed6e08a6166af123e703ffa67cba3',
    'signer': '03eb625a2a7d58fb7ae2d1b222c017947adc1ed6e08a6166af123e703ffa67cba3',
    'peer': '02ec9ae486c45997cfd2fe1d65192084a6db658e86d2ca5bcd4f6e341352f50ba6',
}
ports = {'watch': 10069, 'signer': 10059, 'peer': 10079}
exercise = json.loads((root / 'exercise-result.json').read_text())
point = exercise['channel_point']
assert point == '8d0820f005aae439cff173b2395df92bbd9ed08ed5cead0dbf4753ca3cc4e8c6:1'
assert json.loads((root / 'scb-restore-result.json').read_text())['scb_restore_submitted']
lock = (root / 'recovery-observer.lock').open('a')
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)


def save(path, value, exclusive=False):
    stage = path if exclusive else path.with_suffix('.writing')
    with stage.open('x' if exclusive else 'w') as file:
        json.dump(value, file, indent=2)
        file.flush()
        os.fsync(file.fileno())
    if not exclusive:
        stage.replace(path)
    descriptor = os.open(path.parent, os.O_RDONLY)
    os.fsync(descriptor)
    os.close(descriptor)


def node(name, *command):
    assert name in identities
    output = subprocess.run([
        'docker', 'exec', 'oa-lnd-signet-11862-' + name, 'lncli',
        '--lnddir=/data', '--network=signet', '--rpcserver=127.0.0.1:' + str(ports[name]),
        *command,
    ], capture_output=True, timeout=45)
    if output.returncode:
        raise RuntimeError('The research node refused ' + command[0] + '.')
    return json.loads(output.stdout)


def core(method, *params):
    assert method in {'getblockchaininfo', 'getrawtransaction', 'getblockheader', 'gettxout'}
    cookie = (Path.home() / 'wavelength-spike/bitcoin-signet/signet/.cookie').read_text().strip()
    request = urllib.request.Request('http://127.0.0.1:38332/', data=json.dumps({
        'jsonrpc': '2.0', 'id': 'lnd-recovery', 'method': method, 'params': params,
    }).encode(), headers={'Authorization': 'Basic ' + base64.b64encode(cookie.encode()).decode(),
                         'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, timeout=15) as response:
        raw = response.read(2000001)
    assert len(raw) <= 2000000
    result = json.loads(raw, parse_float=Decimal)
    assert result.get('error') is None
    return result['result']


def container(name, field):
    return subprocess.check_output(['docker', 'inspect', 'oa-lnd-signet-11862-' + name,
                                   '--format', '{{' + field + '}}'], text=True, timeout=15).strip()


def check_originals_stopped():
    for name in ('watch', 'signer'):
        assert container(name + '-before-restore', '.State.Running') == 'false'


def tx_receipt(txid):
    tx = core('getrawtransaction', txid, True)
    if not tx.get('confirmations', 0):
        return None
    header = core('getblockheader', tx['blockhash'])
    value = {'txid': txid, 'height': header['height']}
    total_in = Decimal(0)
    for vin in tx['vin']:
        previous = core('getrawtransaction', vin['txid'], True)
        total_in += previous['vout'][vin['vout']]['value']
    value['fee_sat'] = int((total_in - sum(v['value'] for v in tx['vout'])) * 100000000)
    value['vsize'] = tx['vsize']
    value['returned_sat'] = sum(int(v['value'] * 100000000) for v in tx['vout']
                                if v['scriptPubKey'].get('address') == args.return_address)
    return value


check_originals_stopped()
chain = core('getblockchaininfo')
assert chain['chain'] == 'signet' and not chain['initialblockdownload']
if args.preflight:
    print(json.dumps({'preflight': True, 'height': chain['blocks'],
                      'original_writers_stopped': True, 'channel_point': point}))
    raise SystemExit(0)
state_path = root / 'recovery-observer-state.json'
state = json.loads(state_path.read_text()) if state_path.exists() else {
    'phase': 'wait_recovery', 'started_at_unix': time.time(),
    'deadline_unix': time.time() + 72 * 3600, 'channel_point': point,
    'return_address': args.return_address, 'returns': {},
}
assert state['channel_point'] == point and state['return_address'] == args.return_address
while state['phase'] not in {'done', 'needs_review', 'time_limit'}:
    if time.time() >= state['deadline_unix']:
        state['phase'] = 'time_limit'
        save(state_path, state)
        break
    try:
        check_originals_stopped()
        chain = core('getblockchaininfo')
        assert chain['chain'] == 'signet' and not chain['initialblockdownload']
        ready = True
        observations = {}
        for name in ('watch', 'peer'):
            result = root / ('recovery-return-' + name + '-result.json')
            if result.exists():
                state['returns'][name] = json.loads(result.read_text())['txid']
                continue
            info = node(name, 'getinfo')
            assert info['identity_pubkey'] == identities[name] and info['synced_to_chain']
            assert info['chains'] == [{'chain': 'bitcoin', 'network': 'signet'}]
            pending = node(name, 'pendingchannels')
            channels = node(name, 'listchannels')['channels']
            closed = [c for c in node(name, 'closedchannels')['channels'] if c['channel_point'] == point]
            balance = node(name, 'walletbalance')
            sweeps = node(name, 'wallet', 'pendingsweeps')
            observations[name] = {'pending': pending, 'closed': closed, 'balance': balance,
                                  'pending_sweeps': sweeps}
            if channels or any(v for k, v in pending.items() if k.endswith('channels')):
                ready = False
            if not closed or not closed[0].get('closing_tx_hash'):
                ready = False
            if int(balance['unconfirmed_balance']) or sweeps.get('pending_sweeps'):
                ready = False
        state['observations'] = observations
        # A hard signing failure removes an anchor from PendingSweeps without
        # spending it. Check this original output before declaring cleanup done.
        closing = next((c['closing_tx_hash'] for c in observations.get('watch', {}).get('closed', [])), None)
        if closing:
            anchor = core('gettxout', closing, 1, True)
            state['watcher_anchor_unspent'] = anchor is not None
            if anchor is not None:
                ready = False
        if ready:
            for name in ('watch', 'peer'):
                if name in state['returns']:
                    continue
                closing = observations[name]['closed'][0]['closing_tx_hash']
                receipt = tx_receipt(closing)
                assert receipt is not None
                state.setdefault('closures', {})[name] = receipt
                balance = observations[name]['balance']
                assert 0 < int(balance['confirmed_balance']) < 50000
                marker = root / ('recovery-return-' + name + '-started.json')
                if marker.exists():
                    state['phase'] = 'needs_review'
                    raise RuntimeError('A return has no saved transaction ID; inspect it before another send.')
                save(marker, {'at_unix': time.time(), 'address': args.return_address,
                              'sweep_all': True}, exclusive=True)
                sent = node(name, 'sendcoins', '--addr', args.return_address,
                            '--sweepall', '--sat_per_vbyte', '2')
                save(root / ('recovery-return-' + name + '-result.json'), sent, exclusive=True)
                state['returns'][name] = sent['txid']
            state['phase'] = 'wait_returns'
            receipts = {n: tx_receipt(t) for n, t in state['returns'].items()}
            if len(receipts) == 2 and all(receipts.values()):
                state['return_receipts'] = receipts
                state['phase'] = 'stop_empty_nodes'
                for name in ('watch', 'signer', 'peer'):
                    assert container(name, 'index .Config.Labels "issue"') == '11862'
                    if container(name, '.State.Running') == 'true':
                        assert int(node(name, 'walletbalance')['total_balance']) == 0
                        subprocess.run(['docker', 'stop', 'oa-lnd-signet-11862-' + name],
                                       check=True, capture_output=True, timeout=60)
                state.update(phase='done', completed_at_unix=time.time())
        state.pop('last_error', None)
    except Exception as error:
        # Do not print RPC responses, credentials, seeds, invoices, or preimages.
        state['last_error'] = type(error).__name__ + ': ' + str(error)[:240]
    state['checked_at_unix'] = time.time()
    state['height'] = chain['blocks']
    save(state_path, state)
    print(json.dumps({'phase': state['phase'], 'height': state['height'],
                      'error': state.get('last_error')}), flush=True)
    if state['phase'] not in {'done', 'needs_review', 'time_limit'}:
        time.sleep(60)
