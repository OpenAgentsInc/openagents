#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: records Core's historical spent coins and independently verifies raw transactions, headers and Merkle roots before pinning fixtures)
"""Record one historical block, its external spent coins and needed context.

No secret, wallet or write RPC. Public records are pinned by their SHA256.
The sparse context is a test boundary supplied by the independent chain
oracle; it is never a header-admission mechanism for a running Coax node.
"""
import argparse
from decimal import Decimal
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import urllib.request

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import tx_reference as T
import lightchain as L


def rpc(url, method, *params):
    q = dict(jsonrpc='2.0', id=1, method=method, params=params)
    # Use a bounded transfer; the public service accepts curl's HTTP stack.
    answer = subprocess.run(['curl', '--silent', '--show-error', '--fail', '--max-time', '40', url,
                             '-H', 'Content-Type: application/json', '--data-binary', '@-'],
                            input=json.dumps(q).encode(), capture_output=True, check=True, timeout=45)
    result = json.loads(answer.stdout, parse_float=Decimal)
    assert not result.get('error'), (method, result.get('error'))
    return result['result']


def sats(value):
    result = Decimal(value) * 100000000
    assert result == int(result)
    return int(result)


def record(url, height):
    ident = rpc(url, 'getblockhash', height)
    block = rpc(url, 'getblock', ident, 3)
    raw = bytes.fromhex(rpc(url, 'getblock', ident, 0))
    assert block['height'] == height and L.sha256d(raw[:80])[::-1].hex() == ident
    txs = block['tx']
    assert raw[80:] == T.compact(len(txs)) + b''.join(bytes.fromhex(t['hex']) for t in txs)
    ids = []
    for tx in txs:
        parsed = T.unserialize(bytes.fromhex(tx['hex']), True)
        digest = T.sha256d(T.serialize(parsed, False))
        assert digest[::-1].hex() == tx['txid']
        ids.append(digest)
    assert L.merkle(ids) == raw[36:68]
    all_ids = {tx['txid'] for tx in txs}
    coins = {}
    times = {height - 1}
    for tx in txs[1:]:
        for vin in tx['vin']:
            coin = vin['prevout']
            point = bytes.fromhex(vin['txid'])[::-1] + struct.pack('<I', vin['vout'])
            if vin['txid'] not in all_ids:
                coins[point.hex()] = [sats(coin['value']), coin['height'], coin['generated'], coin['scriptPubKey']['hex']]
            seq = vin['sequence']
            if height >= 419328 and tx['version'] >= 2 and not seq & (1 << 31) and seq & (1 << 22):
                times.add(max(0, coin['height'] - 1))
    original = None
    if height in (91842, 91880):
        origin = {91842: 91812, 91880: 91722}[height]
        old_hash = rpc(url, 'getblockhash', origin)
        old = rpc(url, 'getblock', old_hash, 3)['tx'][0]
        assert old['hex'] == txs[0]['hex'] and old['txid'] == txs[0]['txid']
        original = dict(height=origin, block=old_hash, txid=old['txid'], raw=old['hex'])
        # Core documents these two unspent-coin overwrite violations. Keep
        # the repeated original output in the supplied predecessor view,
        # rather than accidentally passing the exception with an empty map.
        for n, out in enumerate(old['vout']):
            script = bytes.fromhex(out['scriptPubKey']['hex'])
            if len(script) <= 10000 and script[:1] != b'\x6a':
                point = bytes.fromhex(old['txid'])[::-1] + struct.pack('<I', n)
                coins[point.hex()] = [sats(out['value']), origin, True, script.hex()]
    headers = {height: raw[:80].hex()}
    for h in sorted(times):
        current = rpc(url, 'getblockhash', h)
        for k in range(h, max(-1, h - 11), -1):
            if k in headers:
                break
            header = rpc(url, 'getblockheader', current, False)
            data = bytes.fromhex(header)
            assert L.sha256d(data)[::-1].hex() == current
            headers[k] = header
            current = data[4:36][::-1].hex()
    if height > 227931 and 227931 not in headers:
        anchor = rpc(url, 'getblockhash', 227931)
        headers[227931] = rpc(url, 'getblockheader', anchor, False)
    result = dict(schema=1, chain=1, network='mainnet', source=url, height=height, hash=ident,
                raw=raw.hex(), coins=coins, headers={str(h): v for h, v in sorted(headers.items())},
                transactions=len(txs), fees=sum(sats(t.get('fee', 0)) for t in txs[1:]),
                boundary='Core supplies the admitted parent chain and exact pre-block coins; this fixture checks the whole block against them, not a sync from genesis')
    if original:
        result['bip30_original'] = original
    return result


def record_first(network):
    assert network in ('testnet4', 'signet')
    base = f'https://mempool.space/{network}/api/'
    def get(path):
        with urllib.request.urlopen(base + path, timeout=30) as r:
            return r.read()
    ident = get('block-height/1').decode()
    raw = get('block/' + ident + '/raw')
    assert L.sha256d(raw[:80])[::-1].hex() == ident and raw[80] == 1
    tx = T.unserialize(raw[81:], True)
    assert T.is_coinbase(tx) and T.sha256d(T.serialize(tx, False)) == raw[36:68]
    genesis = get('block-height/0').decode()
    header = get('block/' + genesis + '/header').decode()
    assert L.sha256d(bytes.fromhex(header))[::-1].hex() == genesis
    assert raw[4:36][::-1].hex() == genesis
    return dict(schema=1, chain=3 if network == 'testnet4' else 4, network=network, source=base,
                height=1, hash=ident, raw=raw.hex(), coins={}, headers={'0': header, '1': raw[:80].hex()},
                transactions=1, fees=0, boundary='Admit genesis and block 1 through the own header loader, then validate the full block; all buried deployments activate here')


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--rpc', default='https://bitcoin-rpc.publicnode.com')
    p.add_argument('--network', choices=('mainnet', 'testnet4', 'signet'), default='mainnet')
    p.add_argument('--height', type=int, required=True)
    p.add_argument('--out', type=Path, required=True)
    a = p.parse_args()
    assert a.network == 'mainnet' or a.height == 1
    fixture = record(a.rpc, a.height) if a.network == 'mainnet' else record_first(a.network)
    a.out.parent.mkdir(parents=True, exist_ok=True)
    a.out.write_text(json.dumps(fixture, separators=(',', ':')) + '\n')
    print(f'{a.height}: {fixture["transactions"]} transactions, {len(fixture["coins"])} external spent coins, '
          f'{len(fixture["headers"])} context headers; SHA256 {hashlib.sha256(a.out.read_bytes()).hexdigest()}', flush=True)


if __name__ == '__main__':
    main()
