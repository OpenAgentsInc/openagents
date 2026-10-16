#!/usr/bin/env python3
"""Bound the original synthetic signet mint, transfer, and RFQ experiment."""
import base64
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.request

os.umask(0o077)
home = Path.home()
root = home / 'lnd-assets-signet-11863'
source = home / 'openagents-target-agent2/debug/lnd-signet-assets'
lock = (root / 'observer.lock').open('a')
fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
identities = json.loads((root / 'identities.json').read_text())['nodes']
assert set(identities) == {'a', 'b'}
assert json.loads((root / 'setup.json').read_text())['network'] == 'signet'


def save(path, value, exclusive=False):
    temporary = path if exclusive else path.with_suffix('.writing')
    with temporary.open('x' if exclusive else 'w') as file:
        json.dump(value, file, indent=2)
        file.flush()
        os.fsync(file.fileno())
    if not exclusive:
        temporary.replace(path)
    descriptor = os.open(path.parent, os.O_RDONLY)
    os.fsync(descriptor)
    os.close(descriptor)


def process(command, timeout=90):
    result = subprocess.run(command, capture_output=True, timeout=timeout)
    if result.returncode:
        directory = root / 'private'
        directory.mkdir(mode=0o700, exist_ok=True)
        save(directory / ('command-failure-' + str(time.time_ns()) + '.json'),
             {'stdout': result.stdout[-65536:].decode(errors='replace'),
              'stderr': result.stderr[-65536:].decode(errors='replace')}, exclusive=True)
        raise RuntimeError('A research command was refused; inspect its private result before retrying.')
    return json.loads(result.stdout) if result.stdout.strip() else {}


def cli(node, kind, *args):
    assert node in identities and kind in {'lncli', 'tapcli'}
    command = ['docker', 'exec', 'oa-lnd-assets-signet-11863-' + node, kind,
               '--network=signet', '--rpcserver=127.0.0.1:' + str(10089 if node == 'a' else 10099)]
    if kind == 'lncli':
        command += ['--lnddir=/data/lnd']
    else:
        command += ['--tlscertpath=/data/lnd/tls.cert',
                    '--macaroonpath=/data/tap/data/signet/admin.macaroon']
    return process(command + list(args))


def once(label, action):
    result = root / (label + '-result.json')
    if result.exists():
        return json.loads(result.read_text())
    marker = root / (label + '-started.json')
    if marker.exists():
        raise RuntimeError('The original ' + label + ' has no saved result; inspect it before retrying.')
    save(marker, {'at_unix': time.time(), 'action': label}, exclusive=True)
    value = action()
    save(result, value, exclusive=True)
    return value


def core(method, *params):
    assert method in {'getblockchaininfo', 'getrawtransaction', 'getblockheader'}
    cookie = (home / 'wavelength-spike/bitcoin-signet/signet/.cookie').read_text().strip()
    req = urllib.request.Request('http://127.0.0.1:38332/', data=json.dumps({
        'jsonrpc': '2.0', 'id': 'signet-assets', 'method': method, 'params': params,
    }).encode(), headers={'Authorization': 'Basic ' + base64.b64encode(cookie.encode()).decode(),
                         'Content-Type': 'application/json'})
    with urllib.request.urlopen(req, timeout=15) as response:
        data = response.read(2000001)
    assert len(data) <= 2000000
    result = json.loads(data)
    assert result.get('error') is None
    return result['result']


def confirmed(asset):
    txid = asset['chain_anchor']['anchor_outpoint'].split(':')[0]
    tx = core('getrawtransaction', txid, True)
    if not tx.get('confirmations', 0):
        return None
    return {'txid': txid, 'height': core('getblockheader', tx['blockhash'])['height'],
            'anchor_outpoint': asset['chain_anchor']['anchor_outpoint']}


def assets(node):
    return [a for a in cli(node, 'tapcli', 'assets', 'list').get('assets', [])
            if a['asset_genesis']['name'] == 'openagents-signet-test-11863' and not a.get('is_spent')]


def scratch(*args):
    environment = dict(os.environ, OPENAGENTS_WALLET_HOME=str(home / 'wavelength-spike/ldk-home'))
    command = [str(home / 'openagents-target-agent2/debug/openagents'), '--json', 'x402', 'node', *args]
    result = subprocess.run(command, env=environment, capture_output=True, timeout=90)
    if result.returncode:
        raise RuntimeError('The existing scratch wallet refused a research request.')
    return json.loads(result.stdout)


if sys.argv[1:] == ['--preflight']:
    chain = core('getblockchaininfo')
    assert chain['chain'] == 'signet' and not chain['initialblockdownload']
    print(json.dumps(dict(process([str(source), str(root), 'inspect']),
                          preflight=True, height=chain['blocks'], maximum_funding_sat=150000)))
    raise SystemExit(0)
assert not sys.argv[1:], 'Use no arguments, or --preflight.'

state_path = root / 'observer-state.json'
state = json.loads(state_path.read_text()) if state_path.exists() else {
    'phase': 'wait_initial_funding', 'started_at_unix': time.time(),
    'deadline_unix': time.time() + 72 * 3600, 'maximum_funding_sat': 150000,
}
while state['phase'] not in {'rfq_done', 'needs_review', 'time_limit'}:
    try:
        if time.time() >= state['deadline_unix']:
            state['phase'] = 'time_limit'
            save(state_path, state)
            break
        chain = core('getblockchaininfo')
        assert chain['chain'] == 'signet' and not chain['initialblockdownload']
        state['height'] = chain['blocks']
        for node in identities:
            info = cli(node, 'lncli', 'getinfo')
            assert info['identity_pubkey'] == identities[node]
            assert info['chains'] == [{'chain': 'bitcoin', 'network': 'signet'}]
        if state['phase'] == 'wait_initial_funding':
            if int(cli('a', 'lncli', 'walletbalance')['confirmed_balance']) >= 5000:
                state['phase'] = 'mint'
        elif state['phase'] == 'mint':
            once('mint', lambda: cli('a', 'tapcli', 'assets', 'mint', '--type=normal',
                                     '--name=openagents-signet-test-11863', '--supply=1000000'))
            once('mint-finalize', lambda: cli('a', 'tapcli', 'assets', 'mint', 'finalize', '--sat_per_vbyte=2'))
            state['phase'] = 'wait_mint'
        elif state['phase'] == 'wait_mint':
            found = assets('a')
            if len(found) == 1 and int(found[0]['amount']) == 1000000:
                receipt = confirmed(found[0])
                if receipt:
                    asset_id = found[0]['asset_genesis']['asset_id']
                    value = {'asset_id': asset_id, 'name': 'openagents-signet-test-11863',
                             'synthetic_only': True, 'supply': 1000000, 'mint': receipt}
                    path = root / 'asset.json'
                    if path.exists():
                        assert json.loads(path.read_text()) == value
                    else:
                        save(path, value, exclusive=True)
                    state['phase'] = 'transfer'
        elif state['phase'] == 'transfer':
            asset = json.loads((root / 'asset.json').read_text())
            once('universe-sync', lambda: cli('b', 'tapcli', 'universe', 'sync', '--universe_host=127.0.0.1:10089'))
            address = once('asset-address', lambda: cli('b', 'tapcli', 'addrs', 'new',
                           '--asset_id=' + asset['asset_id'], '--amt=10000',
                           '--proof_courier_addr=universerpc://127.0.0.1:10089'))
            once('asset-transfer', lambda: cli('a', 'tapcli', 'assets', 'send',
                                               '--addr=' + address['encoded'], '--sat_per_vbyte=2'))
            state['phase'] = 'wait_transfer'
        elif state['phase'] == 'wait_transfer':
            asset = json.loads((root / 'asset.json').read_text())
            found = [a for a in assets('b') if a['asset_genesis']['asset_id'] == asset['asset_id']]
            if len(found) == 1 and int(found[0]['amount']) == 10000:
                receipt = confirmed(found[0])
                if receipt:
                    path = root / 'b/scratch/11863-transfer-proof.bin'
                    once('proof-export', lambda: cli('b', 'tapcli', 'proofs', 'export',
                         '--asset_id=' + asset['asset_id'], '--script_key=' + found[0]['script_key'],
                         '--proof_file=/data/scratch/11863-transfer-proof.bin'))
                    os.chmod(path, 0o600)
                    proof = once('proof-verify', lambda: cli('b', 'tapcli', 'proofs', 'verify',
                                                           '--proof_file=/data/scratch/11863-transfer-proof.bin'))
                    assert proof['valid'] is True
                    state['onchain_transfer'] = dict(receipt, asset_id=asset['asset_id'], units=10000,
                        proof_sha256=hashlib.sha256(path.read_bytes()).hexdigest(), proof_verified=True)
                    save(root / 'onchain-public-result.json', state['onchain_transfer'])
                    state['phase'] = 'wait_recycled_liquidity'
        elif state['phase'] == 'wait_recycled_liquidity':
            all_sent = True
            for node, amount in [('a', 120000), ('b', 20000)]:
                path = root / ('extra-fund-' + node + '-result.json')
                if path.exists():
                    continue
                info = scratch('info')
                assert info['network'] == 'signet'
                if info['balance']['onchain_spendable_sats'] < amount + 5000:
                    all_sent = False
                    break
                address = once('extra-address-' + node, lambda: cli(node, 'lncli', 'newaddress', 'p2wkh'))['address']
                assert address.startswith('tb1q')
                once('extra-fund-' + node, lambda: scratch('send', address, '--sats', str(amount)))
            if all_sent:
                state['phase'] = 'wait_channel_funds'
        elif state['phase'] == 'wait_channel_funds':
            a = int(cli('a', 'lncli', 'walletbalance')['confirmed_balance'])
            b = int(cli('b', 'lncli', 'walletbalance')['confirmed_balance'])
            if a >= 120000 and b >= 20000:
                peers = cli('a', 'lncli', 'listpeers')['peers']
                if not any(p['pub_key'] == identities['b'] for p in peers):
                    cli('a', 'lncli', 'connect', identities['b'] + '@127.0.0.1:9799')
                once('fund-channel-cli', lambda: process([str(source), str(root), 'fund-channel'], timeout=180))
                state['phase'] = 'wait_channel'
        elif state['phase'] == 'wait_channel':
            ready = True
            for node in identities:
                channels = cli(node, 'lncli', 'listchannels')['channels']
                if len(channels) != 1 or not channels[0]['active']:
                    ready = False
                    continue
                assert channels[0]['private'] and channels[0]['custom_channel_data']
                try:
                    cli(node, 'lncli', 'getchaninfo', channels[0]['scid'])
                except RuntimeError:
                    ready = False
            if ready:
                once('rfq-exercise', lambda: process([str(source), str(root), 'exercise'], timeout=240))
                state.update(phase='rfq_done', completed_at_unix=time.time(), cleanup_pending=True)
        state.pop('last_error', None)
    except Exception as error:
        state['phase'] = 'needs_review'
        state['last_error'] = type(error).__name__ + ': ' + str(error)[:240]
    state['checked_at_unix'] = time.time()
    save(state_path, state)
    print(json.dumps({'phase': state['phase'], 'height': state.get('height'),
                      'error': state.get('last_error')}), flush=True)
    if state['phase'] not in {'rfq_done', 'needs_review', 'time_limit'}:
        time.sleep(60)
