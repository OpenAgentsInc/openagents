#!/usr/bin/env python3
"""Close the original synthetic asset channel and return its spendable BTC."""
import argparse
import base64
from decimal import Decimal
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time
import urllib.request

os.umask(0o077)
parser = argparse.ArgumentParser()
parser.add_argument('--preflight', action='store_true')
args = parser.parse_args()
home = Path.home()
root = home / 'lnd-assets-signet-11863'
identities = {
    'a': '03aa6b9a40dae6df5abc0c59cd112589d816eda03c87d62d001d39523f25c382a7',
    'b': '03b526516a02d005964b4a6d0dbbde33cce1c5e1125dceb1ccc510258873bc9ce5',
}
image = 'lightninglabs/lightning-terminal@sha256:fd6d6040f78e0a342437312f3a3c154f4b63d79172c6b466e2dfd242b06b4a1c'
assert json.loads((root / 'identities.json').read_text())['nodes'] == identities
assert json.loads((root / 'setup.json').read_text())['image'] == image
point_pattern = re.compile(r'^[a-f0-9]{64}:[0-9]+$')


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


def process(command, timeout=60):
    result = subprocess.run(command, capture_output=True, timeout=timeout)
    if result.returncode:
        # A failed mutation may already have effects. Its saved start marker
        # prevents another call; neither raw RPC output nor proofs enter logs.
        raise RuntimeError('A research command was refused; inspect its original action.')
    return json.loads(result.stdout) if result.stdout.strip() else {}


def cli(node, kind, *command):
    assert node in identities and kind in {'lncli', 'tapcli'}
    prefix = ['docker', 'exec', 'oa-lnd-assets-signet-11863-' + node, kind,
              '--network=signet', '--rpcserver=127.0.0.1:' + str(10089 if node == 'a' else 10099)]
    prefix += (['--lnddir=/data/lnd'] if kind == 'lncli' else [
        '--tlscertpath=/data/lnd/tls.cert', '--macaroonpath=/data/tap/data/signet/admin.macaroon'])
    return process(prefix + list(command), timeout=180 if command[0] == 'closechannel' else 60)


def once(label, intent, action):
    result = root / ('cleanup-' + label + '-result.json')
    if result.exists():
        return json.loads(result.read_text())
    marker = root / ('cleanup-' + label + '-started.json')
    if marker.exists():
        raise RuntimeError('An original cleanup action has no result; inspect it before retrying.')
    save(marker, dict(intent, at_unix=time.time()), exclusive=True)
    value = action()
    save(result, value, exclusive=True)
    return value


def core(method, *params):
    assert method in {'getblockchaininfo', 'getrawtransaction', 'getblockheader', 'gettxout', 'getblockhash'}
    cookie = (home / 'wavelength-spike/bitcoin-signet/signet/.cookie').read_text().strip()
    request = urllib.request.Request('http://127.0.0.1:38332/', data=json.dumps({
        'jsonrpc': '2.0', 'id': 'asset-cleanup', 'method': method, 'params': params,
    }).encode(), headers={'Authorization': 'Basic ' + base64.b64encode(cookie.encode()).decode(),
                         'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, timeout=15) as response:
        raw = response.read(2000001)
    assert len(raw) <= 2000000
    value = json.loads(raw, parse_float=Decimal)
    assert value.get('error') is None
    return value['result']


class NotReady(Exception):
    pass


def check():
    chain = core('getblockchaininfo')
    assert chain['chain'] == 'signet'
    if chain['initialblockdownload']:
        raise NotReady('The research chain is syncing.')
    assert core('getblockhash', 0) == '00000008819873e925422c1ff0f99f7cc9bbb232af63a077a480a3633bee1ef6'
    for node, identity in identities.items():
        info = cli(node, 'lncli', 'getinfo')
        assert info['identity_pubkey'] == identity
        if not info['synced_to_chain']:
            raise NotReady('The research node is syncing.')
        assert info['chains'] == [{'chain': 'bitcoin', 'network': 'signet'}]
        actual = subprocess.check_output(['docker', 'inspect', 'oa-lnd-assets-signet-11863-' + node,
                                          '--format', '{{.Config.Image}}'], text=True).strip()
        assert actual == image
    return chain['blocks']


def unspent(point):
    assert point_pattern.fullmatch(point)
    txid, index = point.split(':')
    return core('gettxout', txid, int(index), True)


def receipt(txid, address=None):
    tx = core('getrawtransaction', txid, True)
    if not tx.get('confirmations', 0):
        return None
    before = sum(core('getrawtransaction', i['txid'], True)['vout'][i['vout']]['value']
                 for i in tx['vin'])
    fee = int((before - sum(o['value'] for o in tx['vout'])) * 100000000)
    assert 0 <= fee <= 10000
    return {'txid': txid, 'height': core('getblockheader', tx['blockhash'])['height'],
            'fee_sat': fee, 'vsize': tx['vsize'],
            'returned_sat': sum(int(o['value'] * 100000000) for o in tx['vout']
                                if address and o['scriptPubKey'].get('address') == address),
            'inputs': [i['txid'] + ':' + str(i['vout']) for i in tx['vin']]}


def inventory(asset_id, export=False):
    records, carriers = [], set()
    for node in identities:
        assets = cli(node, 'tapcli', 'assets', 'list', '--all_script_key_types', '--show_leased').get('assets', [])
        assert len(assets) <= 32
        managed = cli(node, 'tapcli', 'assets', 'utxos', '--all_script_key_types', '--show_leased')['managed_utxos']
        for point in managed:
            assert point_pattern.fullmatch(point)
            carriers.add(point)
        for index, asset in enumerate(sorted(assets, key=lambda a: a['script_key'])):
            assert not asset.get('is_spent')
            assert asset['asset_genesis']['asset_id'] == asset_id
            assert asset['asset_genesis']['name'] == 'openagents-signet-test-11863'
            point = asset['chain_anchor']['anchor_outpoint']
            assert point in carriers and point_pattern.fullmatch(point)
            output = unspent(point)
            if output is None or output['confirmations'] < 3:
                return None
            row = {'node': node, 'asset_id': asset_id, 'units': int(asset['amount']),
                   'script_key': asset['script_key'], 'anchor_outpoint': point,
                   'anchor_sat': int(output['value'] * 100000000)}
            if export:
                name = '11863-final-' + str(index) + '.bin'
                # Proof reads can be retried; no invoice, payment, or chain
                # mutation is created by exporting and verifying this proof.
                try:
                    cli(node, 'tapcli', 'proofs', 'export', '--asset_id=' + asset_id,
                        '--script_key=' + asset['script_key'], '--proof_file=/data/scratch/' + name)
                    path = root / node / 'scratch' / name
                    os.chmod(path, 0o600)
                    verified = cli(node, 'tapcli', 'proofs', 'verify', '--proof_file=/data/scratch/' + name)
                except (RuntimeError, subprocess.TimeoutExpired):
                    raise NotReady('A final asset proof is not available yet.') from None
                assert verified['valid'] is True
                row.update(proof_sha256=hashlib.sha256(path.read_bytes()).hexdigest(), proof_verified=True)
            records.append(row)
    if sum(row['units'] for row in records) != 1000000:
        return None
    return records, carriers


height = check()
experiment = json.loads((root / 'observer-state.json').read_text())
if args.preflight:
    print(json.dumps({'network': 'signet', 'height': height, 'phase': experiment['phase'],
                      'ready': experiment['phase'] == 'rfq_done', 'mutation': False}))
    raise SystemExit(0)
address = json.loads((root / 'cleanup-return-address.json').read_text())['address']
assert re.fullmatch(r'tb1q[a-z0-9]{38}', address)
lock = (root / 'cleanup.lock').open('a')
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
path = root / 'cleanup-state.json'
state = json.loads(path.read_text()) if path.exists() else {
    'phase': 'wait_exercise', 'started_at_unix': time.time(),
    'deadline_unix': experiment['deadline_unix'], 'return_address': address, 'returns': {},
}
assert state['return_address'] == address
original_lock = None
if state['phase'] not in {'wait_exercise', 'done', 'needs_review', 'time_limit'}:
    assert experiment['phase'] == 'rfq_done'
    original_lock = (root / 'observer.lock').open('a')
    fcntl.flock(original_lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
while state['phase'] not in {'done', 'needs_review', 'time_limit'}:
    try:
        if time.time() >= state['deadline_unix']:
            state['phase'] = 'time_limit'
            save(path, state)
            break
        state['height'] = check()
        if state['phase'] == 'wait_exercise':
            experiment = json.loads((root / 'observer-state.json').read_text())
            if experiment['phase'] in {'needs_review', 'time_limit'}:
                raise RuntimeError('The original asset experiment needs review before cleanup.')
            if experiment['phase'] == 'rfq_done':
                original_lock = (root / 'observer.lock').open('a')
                try:
                    fcntl.flock(original_lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError:
                    original_lock.close()
                    original_lock = None
                    raise NotReady('The original experiment is saving its final result.')
                minted = json.loads((root / 'asset.json').read_text())
                assert minted['synthetic_only'] and minted['name'] == 'openagents-signet-test-11863'
                funding = json.loads((root / 'channel-funding-result.json').read_text())
                state.update(phase='close', asset_id=minted['asset_id'],
                             channel_point=funding['txid'] + ':' + str(funding['output_index']))
        elif state['phase'] == 'close':
            point = state['channel_point']
            for node in identities:
                channels = cli(node, 'lncli', 'listchannels')['channels']
                assert len(channels) == 1 and channels[0]['channel_point'] == point
                assert channels[0]['active'] and not channels[0]['pending_htlcs']
            once('close', {'channel_point': point, 'sat_per_vbyte': 4, 'max_fee_rate': 8},
                 lambda: cli('a', 'lncli', 'closechannel', '--chan_point=' + point,
                             '--sat_per_vbyte=4', '--max_fee_rate=8'))
            state['phase'] = 'wait_close'
        elif state['phase'] == 'wait_close':
            closed = []
            for node in identities:
                pending = cli(node, 'lncli', 'pendingchannels')
                if any(pending.get(k) for k in ['pending_open_channels', 'pending_closing_channels',
                                               'pending_force_closing_channels', 'waiting_close_channels']):
                    break
                assert not cli(node, 'lncli', 'listchannels')['channels']
                matches = [c for c in cli(node, 'lncli', 'closedchannels')['channels']
                           if c['channel_point'] == state['channel_point']]
                if len(matches) != 1:
                    break
                assert matches[0]['close_type'] == 'COOPERATIVE_CLOSE'
                tx = core('getrawtransaction', matches[0]['closing_tx_hash'], True)
                if tx.get('confirmations', 0) < 3:
                    break
                assert not cli(node, 'lncli', 'wallet', 'pendingsweeps').get('pending_sweeps')
                closed.append(matches[0])
            if len(closed) == 2:
                assert closed[0]['closing_tx_hash'] == closed[1]['closing_tx_hash']
                state['close'] = receipt(closed[0]['closing_tx_hash'])
                state['phase'] = 'proofs'
        elif state['phase'] == 'proofs':
            result = inventory(state['asset_id'])
            if result:
                records, carriers = inventory(state['asset_id'], export=True)
                state.update(phase='return', retained_assets=records, carriers=sorted(carriers))
                save(root / 'cleanup-public-proofs.json', records)
        elif state['phase'] == 'return':
            for node in identities:
                if node in state['returns']:
                    continue
                candidates = []
                for utxo in cli(node, 'lncli', 'listunspent', '--min_confs=1', '--max_confs=1000000')['utxos']:
                    point = utxo['outpoint']
                    if point in state['carriers'] or unspent(point) is None:
                        continue
                    assert re.fullmatch(r'(0014[a-f0-9]{40}|5120[a-f0-9]{64})', utxo['pk_script'])
                    candidates.append(point)
                assert len(candidates) <= 10
                if not candidates:
                    state['returns'][node] = {'empty': True}
                    continue
                # Select only these confirmed, ordinary BTC inputs. Asset
                # carriers cannot be selected even if a wallet lease is lost.
                intent = {'inputs': sorted(candidates), 'return_address': address, 'sat_per_vbyte': 4}
                command = ['sendcoins', '--addr=' + address, '--sweepall', '--sat_per_vbyte=4', '--min_confs=1']
                command += ['--utxo=' + point for point in sorted(candidates)]
                sent = once('return-' + node, intent, lambda: cli(node, 'lncli', *command))
                state['returns'][node] = {'txid': sent['txid']}
                save(path, state)
            state['phase'] = 'wait_returns'
        elif state['phase'] == 'wait_returns':
            confirmed = True
            for node, returned in state['returns'].items():
                if returned.get('empty'):
                    continue
                result = receipt(returned['txid'], address)
                if result is None:
                    confirmed = False
                    continue
                assert result['returned_sat'] > 0 and not set(result['inputs']).intersection(state['carriers'])
                returned.update(result)
            if confirmed:
                for node in identities:
                    for utxo in cli(node, 'lncli', 'listunspent', '--min_confs=0', '--max_confs=1000000')['utxos']:
                        point = utxo['outpoint']
                        if unspent(point) is not None and point not in state['carriers']:
                            raise RuntimeError('An additional spendable output needs review before stopping.')
                state['phase'] = 'stop'
        elif state['phase'] == 'stop':
            for point in state['carriers']:
                assert unspent(point) is not None
            for node in identities:
                subprocess.run(['docker', 'stop', 'oa-lnd-assets-signet-11863-' + node],
                               check=True, capture_output=True, timeout=90)
            state.update(phase='done', completed_at_unix=time.time(), unspent_recoverable_sat=0,
                         retained_carrier_sat=sum(int(unspent(p)['value'] * 100000000) for p in state['carriers']),
                         private_databases_and_proofs_retained=True, test_nodes_stopped=True)
            save(root / 'cleanup-public-result.json', state)
    except NotReady as error:
        state['waiting_for'] = str(error)
    except Exception as error:
        state.update(phase='needs_review', last_error=type(error).__name__ + ': ' + str(error)[:240])
    state['checked_at_unix'] = time.time()
    save(path, state)
    print(json.dumps({'phase': state['phase'], 'height': state.get('height'),
                      'error': state.get('last_error')}), flush=True)
    if state['phase'] not in {'done', 'needs_review', 'time_limit'}:
        time.sleep(60)
