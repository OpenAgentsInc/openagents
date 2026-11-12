#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Bitcoin Core runs the treasury's script and is the independent wallet; this file only drives it)
"""Chapter 104's network run: three key agents sign a 2-of-3 spend from
requests carried over a Coax relay, Bitcoin Core accepts and mines it, and a
coordinator outage is recovered with Bitcoin Core's own descriptor wallet.

  BITCOIN_BIN=DIR COAX_RELAY=EXE TREASURY=EXE OUT=DIR python3 coax/tests/nostr/treasury_regtest.py
  CHAIN=signet ... the same on a private signet (BIP-325): Bitcoin Core started with
               -signet -signetchallenge=51, a chain of its own whose blocks anyone may
               sign. Signet's rules, standardness and tb1 addresses, no faucet and no peers.
               It is not the public signet: nothing is broadcast to anyone.

BITCOIN_BIN holds the official bitcoind and bitcoin-cli (SHA256SUMS checked
by the caller); COAX_RELAY is nostr/relay/build.sh's relay; TREASURY is
nostr/tools/treasury.coax built by coax/web/tool.sh. Everything runs on
127.0.0.1 in regtest: test keys only (SHA-256("chapter 104 <name>")), coins
with no value.

1. Core funds the treasury's receive 0 and 1 (60,000 and 40,000 sat).
2. The holder's request and its PSBT: pay 70,000 to a Core address, 29,000
   change to change 0, fee 1,000. A second request states the same outputs
   but its PSBT pays someone else.
3. The Coax relay starts empty. The three key agents (treasury agent ...)
   subscribe; the coordinator seals each request to all three as 3188
   artifacts. Every agent signs the first and refuses the second; the
   coordinator combines two signatures for the first and has none for the
   second.
4. Core's testmempoolaccept and sendrawtransaction take the transaction, a
   block confirms it, and its outputs are the request's.
5. Recovery: Core funds receive 2; the coordinator is gone. A Core descriptor
   wallet is made from the descriptor in the wallet record (one key agent's
   copy) with alice's and keyco's account keys, rescans, finds that coin and
   the spend's change, and sends both out with its own signer; a block
   confirms it.
"""
import hashlib
import json
import os
import re
import socket
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
import keys_reference as K  # noqa: E402

OUT = Path(os.environ['OUT'])
BIN = Path(os.environ['BITCOIN_BIN'])
CHAIN = os.environ.get('CHAIN', 'regtest')
assert CHAIN in ('regtest', 'signet')
CHAIN_ARGS = ['-regtest'] if CHAIN == 'regtest' else ['-signet', '-signetchallenge=51']
HRP = 'bcrt' if CHAIN == 'regtest' else 'tb'
H = 2**31
PATH = [48 + H, 1 + H, 0 + H, 2 + H]
NAMES = ('alice', 'keyco', 'enclave')
RESULT = {}
LOG = open(OUT / ('treasury-%s.log' % CHAIN), 'w')


def say(*a):
    line = ' '.join(str(x) for x in a)
    print(line, flush=True)
    LOG.write(line + '\n')
    LOG.flush()


def free_port():
    s = socket.socket()
    s.bind(('127.0.0.1', 0))
    p = s.getsockname()[1]
    s.close()
    return p


class Core:
    def __init__(self):
        self.dir = OUT / 'core'
        self.dir.mkdir(parents=True, exist_ok=True)
        self.port = free_port()
        self.p = subprocess.Popen([str(BIN / 'bitcoind')] + CHAIN_ARGS + ['-datadir=%s' % self.dir, '-rpcport=%d' % self.port,
                                   '-port=%d' % free_port(), '-listen=0', '-rpcuser=t', '-rpcpassword=t', '-fallbackfee=0.0001',
                                   '-txindex=1', '-server=1', '-printtoconsole=0'])
        for _ in range(100):
            try:
                self.cli('getblockchaininfo')
                return
            except subprocess.CalledProcessError:
                time.sleep(0.3)
        raise SystemExit('bitcoind did not start')

    def cli(self, *args, wallet=None):
        cmd = [str(BIN / 'bitcoin-cli'), CHAIN_ARGS[0], '-datadir=%s' % self.dir, '-rpcport=%d' % self.port,
               '-rpcuser=t', '-rpcpassword=t']
        if wallet:
            cmd.append('-rpcwallet=%s' % wallet)
        r = subprocess.run(cmd + [str(a).lower() if isinstance(a, bool) else str(a) for a in args], capture_output=True, text=True)
        if r.returncode:
            raise subprocess.CalledProcessError(r.returncode, cmd, r.stdout, r.stderr)
        out = r.stdout.strip()
        try:
            return json.loads(out)
        except json.JSONDecodeError:
            return out

    def stop(self):
        try:
            self.cli('stop')
        except subprocess.CalledProcessError:
            pass
        self.p.wait(timeout=60)


def dig(core, n, address):
    """n more blocks to address. Regtest finds each at once; signet's proof of
    work takes about five million hashes a block, more than one call's tries."""
    want = core.cli('getblockcount') + n
    while core.cli('getblockcount') < want:
        core.cli('generatetoaddress', want - core.cli('getblockcount'), address, 20000000)


def agent_account(name):
    k, c = K.master(K.sha(('chapter 104 ' + name).encode()))
    for idx in PATH:
        k, c = K.ckd_priv(k, c, idx)
    fp, tpub = K.account(K.sha(('chapter 104 ' + name).encode()), PATH)
    b = K.b58decode(tpub)
    tprv = K.b58check(bytes.fromhex('04358394') + b[4:45] + b'\0' + k.to_bytes(32, 'big'))
    return fp, tpub, tprv


def spk(branch, index):
    return K.wsh_script(2, [K.derive(agent_account(n)[1], branch, index) for n in NAMES])


def fixture_request():
    text = (ROOT / 'coax/tests/nostr/keys_fixtures.coax').read_text()
    m = re.search(r'^KY_REQUEST :: (".*");$', text, re.M)
    return json.loads(json.loads(m.group(1)))


def psbt(ins, outs):
    tx = (2).to_bytes(4, 'little') + K.ci(len(ins))
    for txid, vout, _, _ in ins:
        tx += bytes.fromhex(txid)[::-1] + vout.to_bytes(4, 'little') + b'\0' + b'\xfd\xff\xff\xff'
    tx += K.ci(len(outs)) + b''.join(v.to_bytes(8, 'little') + K.ci(len(s)) + s for v, s in outs) + (0).to_bytes(4, 'little')
    p = b'psbt\xff' + K.kv(b'\0', tx) + b'\0'
    for _, _, v, s in ins:
        p += K.kv(b'\x01', v.to_bytes(8, 'little') + K.ci(len(s)) + s) + b'\0'
    return p + b'\0' * len(outs)


def store(dir_, data):
    d = hashlib.sha256(data).hexdigest()
    (dir_ / d).write_bytes(data)
    return d


def holder_x():
    return K.xonly(int.from_bytes(K.sha(b'chapter 104 nostr holder'), 'big'))


def run(cmd, **kw):
    r = subprocess.run(cmd, capture_output=True, text=True, **kw)
    for line in (r.stdout + r.stderr).splitlines():
        say('  ' + line)
    return r.returncode


def main():
    core = Core()
    try:
        core.cli('createwallet', 'miner')
        mine = core.cli('getnewaddress', wallet='miner')
        dig(core, 101, mine)

        # 1. fund receive 0 and 1
        coins = []
        for idx, amount in ((0, 60000), (1, 40000)):
            s = spk(0, idx)
            txid = core.cli('sendtoaddress', K.segwit_addr(HRP, 0, s[2:]), '%.8f' % (amount / 1e8), wallet='miner')
            tx = core.cli('getrawtransaction', txid, True)
            vout = next(o['n'] for o in tx['vout'] if o['scriptPubKey']['hex'] == s.hex())
            coins.append((txid, vout, amount, s))
        dig(core, 1, mine)
        say('funded receive 0 and 1:', [(c[0][:16], c[1], c[2]) for c in coins])

        # 2. the requests and their PSBTs
        pay = bytes.fromhex(core.cli('getaddressinfo', core.cli('getnewaddress', '', 'bech32', wallet='miner'),
                                     wallet='miner')['scriptPubKey'])
        other = bytes.fromhex(core.cli('getaddressinfo', core.cli('getnewaddress', '', 'bech32', wallet='miner'),
                                       wallet='miner')['scriptPubKey'])
        change = spk(1, 0)
        good = psbt(coins, [(70000, pay), (29000, change)])
        evil = psbt(coins, [(70000, other), (29000, change)])
        store_dir = OUT / 'artifacts'
        store_dir.mkdir(exist_ok=True)
        now = int(time.time())
        requests = []
        for name, p, nonce in (('good', good, 'b1' * 32), ('evil', evil, 'b2' * 32)):
            d = store(store_dir, p)
            r = fixture_request()
            r['psbt'] = {'digest': 'sha256:' + d, 'media_type': 'application/octet-stream', 'size': len(p)}
            r['outputs'] = [{'address': K.segwit_addr('tb', pay[0], pay[2:]), 'amount_sat': 70000, 'change': False},
                            {'address': K.segwit_addr('tb', 0, change[2:]), 'amount_sat': 29000, 'change': True}]
            r['requester'] = holder_x()
            r['nonce'] = nonce
            r['deadline'] = now + 3600
            path = OUT / ('request-%s.json' % name)
            path.write_text(K.body(r))
            requests.append(path)

        # 3. relay, agents, coordinator
        port = free_port()
        url = 'ws://127.0.0.1:%d' % port
        rlog = open(OUT / 'relay.log', 'w')
        relay = subprocess.Popen([os.environ['COAX_RELAY'], '--listen', '127.0.0.1', '--port', str(port), '--url', url],
                                 stdout=rlog, stderr=subprocess.STDOUT, text=True)
        while 'listening' not in (OUT / 'relay.log').read_text():
            if relay.poll() is not None:
                raise SystemExit('the Coax relay did not start')
            time.sleep(0.2)
        common = ['--relay', url, '--dir', str(store_dir), '--now', str(now)]
        agents = [subprocess.Popen([os.environ['TREASURY'], 'agent', n, '--count', '2'] + common,
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True) for n in NAMES]
        time.sleep(3)
        txfile = OUT / 'tx.hex'
        say('coordinator, request 1 (the PSBT the request states):')
        c1 = run([os.environ['TREASURY'], 'coordinator', '--request', str(requests[0]), '--out', str(txfile)] + common)
        say('coordinator, request 2 (a PSBT that pays someone the request does not name):')
        c2 = run([os.environ['TREASURY'], 'coordinator', '--request', str(requests[1]), '--out', str(OUT / 'evil.hex')] + common)
        outs = [a.communicate(timeout=120)[0] for a in agents]
        for n, o in zip(NAMES, outs):
            for line in o.splitlines():
                say('  [%s] %s' % (n, line))
        relay.terminate()
        RESULT['agents_signed_first_refused_second'] = all(
            'signed: ok' in o and 'refused: outputs' in o for o in outs)
        RESULT['coordinator'] = {'request_1_exit': c1, 'request_2_exit': c2}

        # 4. broadcast and confirm
        tx = txfile.read_text().strip() if c1 == 0 else ''
        acc = core.cli('testmempoolaccept', json.dumps([tx]))[0] if tx else {}
        say('testmempoolaccept:', json.dumps(acc))
        txid = core.cli('sendrawtransaction', tx) if acc.get('allowed') else ''
        dig(core, 1, mine)
        got = core.cli('getrawtransaction', txid, True) if txid else {}
        vouts = [(round(o['value'] * 1e8), o['scriptPubKey']['hex']) for o in got.get('vout', [])]
        RESULT['spend'] = {'txid': txid, 'accepted': bool(acc.get('allowed')), 'confirmations': got.get('confirmations', 0),
                           'outputs_are_the_request': vouts == [(70000, pay.hex()), (29000, change.hex())]}
        say('spend:', json.dumps(RESULT['spend']))

        # 5. recovery without the coordinator, with Core's own wallet
        s2 = spk(0, 2)
        rtx = core.cli('sendtoaddress', K.segwit_addr(HRP, 0, s2[2:]), '0.00050000', wallet='miner')
        dig(core, 1, mine)
        wallet = re.search(r'^KY_WALLET :: (".*");$', (ROOT / 'coax/tests/nostr/keys_fixtures.coax').read_text(), re.M)
        desc = json.loads(json.loads(wallet.group(1)))['descriptor'].split('#')[0]
        for n in ('alice', 'keyco'):
            fp, tpub, tprv = agent_account(n)
            desc = desc.replace(tpub, tprv)
        descs = []
        for branch, internal in ((0, False), (1, True)):
            d = desc.replace('<0;1>', str(branch))
            d = d + '#' + core.cli('getdescriptorinfo', d)['checksum']
            descs.append({'desc': d, 'timestamp': 0, 'range': [0, 20], 'active': True, 'internal': internal})
        core.cli('createwallet', 'recovery', False, True, '', False, True)   # blank descriptor wallet with keys
        imp = core.cli('importdescriptors', json.dumps(descs), wallet='recovery')
        bal = core.cli('getbalances', wallet='recovery')['mine']['trusted']
        dest = core.cli('getnewaddress', wallet='miner')
        sent = core.cli('sendall', json.dumps([dest]), wallet='recovery')
        dig(core, 1, mine)
        st = core.cli('getrawtransaction', sent['txid'], True) if sent.get('complete') else {}
        ins = [(i['txid'], i['vout']) for i in st.get('vin', [])]
        RESULT['recovery'] = {'imported': all(x.get('success') for x in imp), 'found_sat': round(bal * 1e8),
                              'complete': sent.get('complete'), 'txid': sent.get('txid'),
                              'confirmations': st.get('confirmations', 0),
                              # the receive 2 coin and the spend's change at change 0: Core found both itself
                              'spent': sorted(t for t, _ in ins), 'spent_receive_2_and_change': sorted(t for t, _ in ins) == sorted([rtx, txid])}
        say('recovery:', json.dumps(RESULT['recovery']))
        RESULT['core'] = core.cli('getnetworkinfo')['subversion']
        info = core.cli('getblockchaininfo')
        RESULT['chain'] = {'chain': info['chain'], 'signet_challenge': info.get('signet_challenge'), 'blocks': info['blocks']}
    finally:
        core.stop()
    ok = (RESULT['agents_signed_first_refused_second'] and RESULT['coordinator'] == {'request_1_exit': 0, 'request_2_exit': 3}
          and RESULT['spend']['accepted'] and RESULT['spend']['confirmations'] >= 1 and RESULT['spend']['outputs_are_the_request']
          and RESULT['recovery']['complete'] and RESULT['recovery']['confirmations'] >= 1 and RESULT['recovery']['found_sat'] == 50000 + 29000 and RESULT['recovery']['spent_receive_2_and_change'])
    RESULT['passed'] = bool(ok)
    (OUT / ('treasury-%s.json' % CHAIN)).write_text(json.dumps(RESULT, indent=1))
    say('PASSED' if ok else 'FAILED', json.dumps(RESULT))
    sys.exit(0 if ok else 1)


if __name__ == '__main__':
    main()
