#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a real Bitcoin Core bitcoind, and for signet the public network, decide what a synced node must hold)
"""The live acceptance of Bitcoin chapter 72: bitcoin/examples/live/sync-node.coax
syncs from Bitcoin Core peers over TCP.

  python3 -I coax/tests/bitcoin/sync_live.py --bitcoind PATH/bitcoind regtest
  python3 -I coax/tests/bitcoin/sync_live.py signet [--blocks 200]

regtest starts bitcoind in a temporary data directory, mines 2,500 blocks
with wallet transactions in the last 100, and runs the node three times
against four connections to it: with a minimum chain work of 2,200 blocks
(headers go through presync), with one of 3,000 blocks (the chain is
refused), and with Core's regtest minimum of zero. Every stored block is
compared byte for byte with bitcoind's getblock. bitcoind is stopped
through RPC, and only the process this script started is killed if it
won't stop.

signet resolves Core's signet DNS seeds, syncs every header from genesis
through presync, downloads the last --blocks blocks, and compares the best
header with mempool.space's signet block at that height.
"""
import base64
import json
import os
import socket
import struct
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
TOOL = 'bitcoin/examples/live/sync-node.coax'
COAX = os.environ.get('COAX', str(REPO / 'coax/bin/coax'))
SIGNET_SEEDS = ['seed.signet.bitcoin.sprovoost.nl', 'seed.signet.achownodes.xyz']


def run_tool(args, seconds):
    """Runs the node, echoing its lines as they come; (exit status, its output)."""
    env = dict(os.environ, COAX=COAX)
    cmd = [str(REPO / 'coax/web/tool.sh'), TOOL, '--now', str(int(time.time()))] + args + ['--seconds', str(seconds)]
    p = subprocess.Popen(cmd, cwd=REPO, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    lines = []
    for line in p.stdout:
        if 'rebuilding' not in line:
            lines.append(line)
            sys.stdout.write(line)
            sys.stdout.flush()
    return p.wait(), ''.join(lines)


def read_blocks(path):
    data, out, at = Path(path).read_bytes(), {}, 0
    while at < len(data):
        height, size = struct.unpack_from('<II', data, at)
        out[height] = data[at + 8:at + 8 + size]
        at += 8 + size
    return out


class Rpc:
    def __init__(self, port, cookie):
        self.url = f'http://127.0.0.1:{port}/'
        self.cookie = cookie

    def call(self, method, *params, batch=None):
        body = batch if batch is not None else {'jsonrpc': '2.0', 'id': 0, 'method': method, 'params': list(params)}
        auth = base64.b64encode(Path(self.cookie).read_bytes().strip()).decode()
        req = urllib.request.Request(self.url, json.dumps(body).encode(), {'Authorization': 'Basic ' + auth, 'Content-Type': 'application/json'})
        with urllib.request.urlopen(req, timeout=600) as r:
            res = json.loads(r.read())
        if batch is not None:
            return [x['result'] for x in sorted(res, key=lambda x: x['id'])]
        if res.get('error'):
            raise RuntimeError(res['error'])
        return res['result']

    def many(self, method, params):
        return self.call(None, batch=[{'jsonrpc': '2.0', 'id': i, 'method': method, 'params': p} for i, p in enumerate(params)])


def free_port():
    s = socket.socket()
    s.bind(('127.0.0.1', 0))
    port = s.getsockname()[1]
    s.close()
    return port


def regtest(bitcoind):
    work = Path(tempfile.mkdtemp(prefix='ch72-regtest-'))
    p2p, rpc_port = free_port(), free_port()
    log = open(work / 'bitcoind.log', 'w')
    proc = subprocess.Popen([bitcoind, '-regtest', f'-datadir={work}', f'-port={p2p}', f'-rpcport={rpc_port}', '-listen=1',
                             '-bind=127.0.0.1', '-server=1', '-fallbackfee=0.0001', '-txindex=0', '-printtoconsole=0'],
                            stdout=log, stderr=log)
    print(f'bitcoind {proc.pid}: regtest in {work}, P2P port {p2p}')
    rpc = Rpc(rpc_port, work / 'regtest/.cookie')
    try:
        for _ in range(300):
            try:
                rpc.call('getblockcount')
                break
            except Exception:
                time.sleep(0.2)
        version = rpc.call('getnetworkinfo')['subversion']
        rpc.call('createwallet', 'w')
        addr = rpc.call('getnewaddress', '', 'bech32')
        rpc.call('generatetoaddress', 2400, addr)
        sends = 0
        for _ in range(100):
            for _ in range(5):
                rpc.call('sendtoaddress', rpc.call('getnewaddress', '', 'bech32'), 0.1)
                sends += 1
            rpc.call('generatetoaddress', 1, addr)
        height = rpc.call('getblockcount')
        tip = rpc.call('getbestblockhash')
        print(f'{version}: {height} blocks mined, {sends} wallet transactions in the last 100, tip {tip[:16]}')
        hashes = rpc.many('getblockhash', [[h] for h in range(height + 1)])
        raws = rpc.many('getblock', [[h, 0] for h in hashes])
        peers = []
        for _ in range(4):
            peers += ['--peer', f'127.0.0.1:{p2p}']
        results = {}
        for label, min_work in [('presync', 2 * 2200), ('refused', 2 * 3000), ('Core default', None)]:
            out = work / f'blocks-{label.split()[0]}.dat'
            print(f'\n--- regtest, {label}: minimum chain work {min_work if min_work is not None else "of chainparams (0)"}')
            args = ['--chain', 'regtest', '--out', str(out)] + peers
            if min_work is not None:
                args += ['--min-work', str(min_work)]
            code, text = run_tool(args, 600)
            if label == 'refused':
                ok = code == 3 and 'too little work' in text and not out.exists()
                print(f'check: refused with nothing stored: {ok}')
                results[label] = ok
                continue
            blocks = read_blocks(out) if out.exists() else {}
            same = sum(1 for h, b in blocks.items() if b.hex() == raws[h])
            ok = code == 0 and len(blocks) == height and same == height and f'best header {height} ' in text and tip in text
            print(f'check: {len(blocks)} blocks stored, {same} byte for byte equal to getblock, tip {tip[:16]}: {ok}')
            results[label] = ok
        return all(results.values())
    finally:
        try:
            rpc.call('stop')
            proc.wait(timeout=60)
        except Exception:
            proc.kill()        # only the bitcoind this script started
            proc.wait()
        log.close()
        print(f'bitcoind {proc.pid} stopped')


def signet(blocks):
    ips = []
    for seed in SIGNET_SEEDS:
        try:
            for a in socket.getaddrinfo(seed, 38333, socket.AF_INET, socket.SOCK_STREAM):
                if a[4][0] not in ips:
                    ips.append(a[4][0])
        except OSError as e:
            print(f'{seed}: {e}')
    print(f'signet seeds: {len(ips)} IPv4 addresses')
    if not ips:
        return False
    peers = []
    for ip in ips[:12]:
        peers += ['--peer', f'{ip}:38333']
    work = Path(tempfile.mkdtemp(prefix='ch72-signet-'))
    out = work / 'blocks.dat'
    code, text = run_tool(['--chain', 'signet', '--blocks', str(blocks), '--out', str(out)] + peers, 3600)
    line = [x for x in text.splitlines() if x.startswith('  headers:')]
    if not line:
        return False
    height = int(line[0].split('best header ')[1].split(' ')[0])
    ours = line[0].split('(')[1].split(')')[0]
    try:
        with urllib.request.urlopen(f'https://mempool.space/signet/api/block-height/{height}', timeout=30) as r:
            theirs = r.read().decode().strip()
    except Exception as e:
        theirs = f'unavailable ({e})'
    stored = read_blocks(out) if out.exists() else {}
    ok = code == 0 and ours == theirs and len(stored) == blocks
    print(f'check: best header {height} is {ours}; mempool.space says {theirs}; {len(stored)} blocks stored: {ok}')
    return ok


def main():
    args = sys.argv[1:]
    ok = True
    if 'regtest' in args:
        bitcoind = args[args.index('--bitcoind') + 1] if '--bitcoind' in args else 'bitcoind'
        ok = regtest(bitcoind) and ok
    if 'signet' in args:
        ok = signet(int(args[args.index('--blocks') + 1]) if '--blocks' in args else 200) and ok
    print('live acceptance:', 'passed' if ok else 'FAILED')
    sys.exit(0 if ok else 1)


if __name__ == '__main__':
    main()
