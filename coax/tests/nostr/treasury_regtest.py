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
by the caller; without it, the bitcoind on PATH); COAX_RELAY is
nostr/relay/build.sh's relay (default nostr/relay/bin/relay); TREASURY is
nostr/tools/treasury.coax built by coax/web/tool.sh (default: its newest
build in coax/web/build/tools). OUT is made if it is missing. Everything runs on
127.0.0.1 (only CHAIN=public-signet reaches the network, and only its Esplora
URL): test keys only (SHA-256("chapter 104 <name>")), coins with no value.

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

The public signet, CHAIN=public-signet: no chain of our own. The harness
reads and writes the public signet through an Esplora HTTP API and keeps
Bitcoin Core offline (-signet -connect=0, no peers, no chain) as the
independent tool.

  CHAIN=public-signet BITCOIN_BIN=DIR COAX_RELAY=EXE TREASURY=EXE OUT=DIR python3 coax/tests/nostr/treasury_regtest.py
  The whole of it as one command, from the repository root (keep OUT: a rerun resumes from it):
  nostr/relay/build.sh && { coax/web/tool.sh nostr/tools/treasury.coax >/dev/null 2>&1; nix shell nixpkgs#bitcoind -c env CHAIN=public-signet OUT=$HOME/.openagents/ch104-public-signet python3 coax/tests/nostr/treasury_regtest.py; }
      ESPLORA=URL       the API (default https://mempool.space/signet/api)
      FEE_SAT=N         each transaction's fee (default 500; at least 1 sat/vB, at most 1000,
                        the lowest fee ceiling among the three signers)
      WAIT_SECONDS=N    how long to wait for each confirmation (default 2700)
      POLL_SECONDS=N    seconds between status reads (default 30)
  REHEARSE=1 CHAIN=public-signet ...   a rehearsal of the public path on a private signet: the
      harness starts the private signet node of CHAIN=signet, funds receive 0 from it, and
      serves a small Esplora on 127.0.0.1 that answers from that node and mines a block
      after each broadcast. The public path then runs against that URL, unchanged.
  CHAIN=fake-esplora ...   only that stand-in, until interrupted: it prints its URL, for a
      second process run with REHEARSE=1 ESPLORA=http://127.0.0.1:PORT CHAIN=public-signet.
      FAKE_MINE_DELAY=N  seconds between a broadcast and its block (default 3)

A person claims signet coins from a faucet to receive 0,
tb1qrpl09ws69aprwz205re9zpkn47ysezuww9cdfau2cfanuamd9ahsyy5tx2, by hand. The
harness never asks a faucet for anything. Then:

P1. GET /address/ADDR/utxo. With no confirmed output there it says what is
    missing and exits 3, having signed and sent nothing.
P2. It takes the largest confirmed output, reads its transaction
    (GET /tx/TXID/hex) and checks the txid, amount and script itself. The
    request pays 70% to receive 1 and the rest, less the fee, to change 0:
    both are the treasury's own scripts. The second request's PSBT pays
    receive 2 instead. Relay, agents and coordinator run as in step 3.
P3. Before broadcast (check_tx): this file parses the transaction and checks
    its inputs, outputs, fee, size and dust; verifies each witness script
    and each ECDSA signature over its own BIP-143 digest; offline Core
    decodes it (decoderawtransaction) and runs its script interpreter over
    every input (signrawtransactionwithkey with no keys and the spent
    outputs given: "complete" means every input verified), and must refuse
    the same transaction with an amount one satoshi off. Not checked: that
    the output is still unspent on a node of ours, and a node's mempool
    policy (testmempoolaccept needs the UTXO set).
P4. The transaction and its txid go into OUT/treasury-public-signet.state.json,
    then POST /tx. It polls GET /tx/TXID/status until a block holds it. On a
    timeout it exits 4; a rerun with the same OUT reads the state file and
    resumes with the recorded transaction. It never builds a second spend.
P5. Recovery without the coordinator: offline Core's descriptorprocesspsbt
    takes the descriptor from the wallet record with alice's and keyco's
    account keys and a PSBT that spends the confirmed spend's two outputs
    (their witness_utxo read back through Esplora) to receive 3, signs with
    Core's own signer and finalizes. P3's checks, then P4 for it.

Exit status: 0 passed, 1 failed, 3 nothing to spend at receive 0, 4 timed
out waiting for a block (rerun to resume).
"""
import base64
import hashlib
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
import keys_reference as K  # noqa: E402

OUT = Path(os.environ['OUT'])
OUT.mkdir(parents=True, exist_ok=True)
# Each has a default, so the public signet run is one command: bitcoind from PATH
# (nix shell nixpkgs#bitcoind), build.sh's relay, and tool.sh's newest build of the tool.
BIN = Path(os.environ.get('BITCOIN_BIN') or os.path.dirname(shutil.which('bitcoind') or '') or 'no-bitcoind')
RELAY_EXE = os.environ.get('COAX_RELAY') or str(ROOT / 'nostr/relay/bin/relay')
TREASURY_EXE = os.environ.get('TREASURY') or str(max(
    (f for f in (ROOT / 'coax/web/build/tools').glob('treasury-*') if re.fullmatch(r'treasury-[0-9a-f]{64}', f.name)),
    key=lambda f: f.stat().st_mtime, default='no-treasury-tool'))
CHAIN = os.environ.get('CHAIN', 'regtest')
assert CHAIN in ('regtest', 'signet', 'public-signet', 'fake-esplora')
PRIVATE_SIGNET = ['-signet', '-signetchallenge=51']
CHAIN_ARGS = ['-regtest'] if CHAIN == 'regtest' else PRIVATE_SIGNET
HRP = 'bcrt' if CHAIN == 'regtest' else 'tb'
REHEARSE = os.environ.get('REHEARSE') == '1'
MODE = CHAIN + ('-rehearsal' if CHAIN == 'public-signet' and REHEARSE else '')
RECEIVE_0 = 'tb1qrpl09ws69aprwz205re9zpkn47ysezuww9cdfau2cfanuamd9ahsyy5tx2'
H = 2**31
PATH = [48 + H, 1 + H, 0 + H, 2 + H]
NAMES = ('alice', 'keyco', 'enclave')
RESULT = {}
LOG = open(OUT / ('treasury-%s.log' % MODE), 'a' if CHAIN == 'public-signet' else 'w')   # a resumed run adds to its log


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
    def __init__(self, args=None, name='core', more=()):
        if not (BIN / 'bitcoind').exists():
            raise SystemExit('no bitcoind: set BITCOIN_BIN to the directory that holds bitcoind and bitcoin-cli, or put them on PATH')
        self.args = CHAIN_ARGS if args is None else args
        self.dir = OUT / name
        self.dir.mkdir(parents=True, exist_ok=True)
        self.port = free_port()
        self.p = subprocess.Popen([str(BIN / 'bitcoind')] + self.args + ['-datadir=%s' % self.dir, '-rpcport=%d' % self.port,
                                   '-port=%d' % free_port(), '-listen=0', '-rpcuser=t', '-rpcpassword=t', '-fallbackfee=0.0001',
                                   '-txindex=1', '-server=1', '-printtoconsole=0'] + list(more))
        for _ in range(100):
            try:
                self.cli('getblockchaininfo')
                return
            except subprocess.CalledProcessError:
                time.sleep(0.3)
        raise SystemExit('bitcoind did not start')

    def cli(self, *args, wallet=None):
        cmd = [str(BIN / 'bitcoin-cli'), self.args[0], '-datadir=%s' % self.dir, '-rpcport=%d' % self.port,
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


def relay_round(coins, pay, other, change, pay_sat, change_sat, fee_sat):
    """Steps 2 and 3 for every mode: the two requests and their PSBTs, then
    the Coax relay, the three key agents and the coordinator. Returns the
    file the coordinator wrote the transaction to and its two exit codes."""
    good = psbt(coins, [(pay_sat, pay), (change_sat, change)])
    evil = psbt(coins, [(pay_sat, other), (change_sat, change)])
    store_dir = OUT / 'artifacts'
    store_dir.mkdir(exist_ok=True)
    now = int(time.time())
    requests = []
    for name, p, nonce in (('good', good, 'b1' * 32), ('evil', evil, 'b2' * 32)):
        d = store(store_dir, p)
        r = fixture_request()
        r['psbt'] = {'digest': 'sha256:' + d, 'media_type': 'application/octet-stream', 'size': len(p)}
        r['outputs'] = [{'address': K.segwit_addr('tb', pay[0], pay[2:]), 'amount_sat': pay_sat, 'change': False},
                        {'address': K.segwit_addr('tb', 0, change[2:]), 'amount_sat': change_sat, 'change': True}]
        r['fee_sat'] = fee_sat
        r['requester'] = holder_x()
        r['nonce'] = nonce
        r['deadline'] = now + 3600
        path = OUT / ('request-%s.json' % name)
        path.write_text(K.body(r))
        requests.append(path)

    port = free_port()
    url = 'ws://127.0.0.1:%d' % port
    rlog = open(OUT / 'relay.log', 'w')
    relay = subprocess.Popen([RELAY_EXE, '--listen', '127.0.0.1', '--port', str(port), '--url', url],
                             stdout=rlog, stderr=subprocess.STDOUT, text=True)
    while 'listening' not in (OUT / 'relay.log').read_text():
        if relay.poll() is not None:
            raise SystemExit('the Coax relay did not start')
        time.sleep(0.2)
    common = ['--relay', url, '--dir', str(store_dir), '--now', str(now)]
    agents = [subprocess.Popen([TREASURY_EXE, 'agent', n, '--count', '2'] + common,
                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True) for n in NAMES]
    time.sleep(3)
    txfile = OUT / 'tx.hex'
    say('coordinator, request 1 (the PSBT the request states):')
    c1 = run([TREASURY_EXE, 'coordinator', '--request', str(requests[0]), '--out', str(txfile)] + common)
    say('coordinator, request 2 (a PSBT that pays someone the request does not name):')
    c2 = run([TREASURY_EXE, 'coordinator', '--request', str(requests[1]), '--out', str(OUT / 'evil.hex')] + common)
    outs = [a.communicate(timeout=120)[0] for a in agents]
    for n, o in zip(NAMES, outs):
        for line in o.splitlines():
            say('  [%s] %s' % (n, line))
    relay.terminate()
    RESULT['agents_signed_first_refused_second'] = all(
        'signed: ok' in o and 'refused: outputs' in o for o in outs)
    RESULT['coordinator'] = {'request_1_exit': c1, 'request_2_exit': c2}
    return txfile, c1, c2


# ---- the public signet path (CHAIN=public-signet) ----

ESPLORA = os.environ.get('ESPLORA', 'https://mempool.space/signet/api').rstrip('/')
FEE_SAT = int(os.environ.get('FEE_SAT', '500'))
FEE_CAP = 1000        # treasury.coax's lowest signer ceiling (TR_CEILING): above it keyco refuses
DUST = 330            # Core's dust threshold for a P2WSH output at its default 3 sat/vB
SMALLEST = 5000       # the smallest output worth splitting into two outputs and two fees
STATE = OUT / ('treasury-%s.state.json' % MODE)


class Stop(Exception):
    """Ends the run with an exit status and a message."""
    def __init__(self, status, message):
        super().__init__(message)
        self.status = status


def esplora(path, post=None):
    """One request: (HTTP status, body text). Nothing but the path, and the
    transaction on POST /tx, is sent; the User-Agent names no one."""
    req = urllib.request.Request(ESPLORA + path, data=None if post is None else post.encode(),
                                 headers={'User-Agent': 'treasury-harness', 'Accept': '*/*'})
    for attempt in range(4):
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                return r.status, r.read().decode().strip()
        except urllib.error.HTTPError as e:
            return e.code, e.read().decode(errors='replace').strip()
        except (urllib.error.URLError, OSError) as e:
            if post is not None or attempt == 3:    # a POST is not repeated blindly: the caller looks first
                raise Stop(1, 'no answer from %s%s: %s' % (ESPLORA, path, e))
            time.sleep(2 + 3 * attempt)


def explorer(txid):
    return '%s/tx/%s' % (ESPLORA[:-4] if ESPLORA.endswith('/api') else ESPLORA, txid)


def dsha(b):
    return K.sha(K.sha(b))


def parse_tx(raw):
    """A transaction's fields, read here from its bytes (BIP-144)."""
    pos = 0

    def take(n):
        nonlocal pos
        b = raw[pos:pos + n]
        assert len(b) == n, 'the transaction ends early'
        pos += n
        return b

    def count():
        n = take(1)[0]
        return n if n < 0xfd else int.from_bytes(take({0xfd: 2, 0xfe: 4, 0xff: 8}[n]), 'little')

    version = take(4)
    segwit = raw[4:6] == b'\x00\x01'
    if segwit:
        take(2)
    start = pos
    ins = []
    for _ in range(count()):
        outpoint = take(36)
        script = take(count())
        ins.append({'outpoint': outpoint, 'txid': outpoint[:32][::-1].hex(), 'vout': int.from_bytes(outpoint[32:], 'little'),
                    'script': script, 'sequence': take(4)})
    outs = []
    for _ in range(count()):
        value = int.from_bytes(take(8), 'little')
        outs.append((value, take(count())))
    body = raw[start:pos]
    wit = [[take(count()) for _ in range(count())] for _ in ins] if segwit else [[] for _ in ins]
    locktime = take(4)
    assert pos == len(raw), 'bytes after the transaction'
    base = version + body + locktime
    return {'version': version, 'ins': ins, 'outs': outs, 'wit': wit, 'locktime': locktime,
            'txid': dsha(base)[::-1].hex(), 'vsize': (3 * len(base) + len(raw) + 3) // 4}


def ecdsa_ok(pub, digest, der):
    """ECDSA verification over secp256k1 with keys_reference.py's integers:
    strict DER (BIP-66), low S, then r == (z/s G + r/s Q).x mod n."""
    if len(der) < 8 or der[0] != 0x30 or der[1] != len(der) - 2 or der[2] != 2:
        return False
    lr = der[3]
    if 4 + lr + 2 > len(der) or der[4 + lr] != 2 or 4 + lr + 2 + der[5 + lr] != len(der):
        return False
    rb, sb = der[4:4 + lr], der[6 + lr:]
    for b in (rb, sb):
        if not b or b[0] & 0x80 or (len(b) > 1 and b[0] == 0 and not b[1] & 0x80):
            return False
    r, s = int.from_bytes(rb, 'big'), int.from_bytes(sb, 'big')
    if not (0 < r < K.N and 0 < s <= K.N // 2):
        return False
    w = pow(s, -1, K.N)
    p = K.add(K.mul(int.from_bytes(digest, 'big') * w % K.N), K.mul(r * w % K.N, K.unser(pub)))
    return p is not None and p[0] % K.N == r


def witness_script(branch, index):
    keys = sorted(K.derive(agent_account(n)[1], branch, index) for n in NAMES)
    return b'\x52' + b''.join(b'\x21' + k for k in keys) + b'\x53\xae'


def ours(script):
    """(branch, index) of a treasury script among the first few, or None."""
    for branch in (0, 1):
        for index in range(4):
            if spk(branch, index) == script:
                return branch, index
    return None


def check_tx(core, txhex, prevouts, want_outs, label):
    """Everything checked before a broadcast. prevouts: (txid, vout, value,
    script) of each input, in order; want_outs: (value, script). Raises Stop
    on any difference; returns what was checked."""
    def need(ok, what):
        if not ok:
            raise Stop(1, '%s: not broadcast, %s' % (label, what))

    # a. this file's own reading of the bytes
    t = parse_tx(bytes.fromhex(txhex))
    need(t['version'] == (2).to_bytes(4, 'little') and t['locktime'] == b'\0' * 4, 'version 2 and locktime 0 expected')
    need([(i['txid'], i['vout']) for i in t['ins']] == [(p[0], p[1]) for p in prevouts], 'its inputs are not the expected outpoints')
    need(all(i['script'] == b'' for i in t['ins']), 'a segwit input carries a scriptSig')
    need(t['outs'] == list(want_outs), 'its outputs are not the stated ones')
    need(all(v >= DUST for v, _ in t['outs']), 'an output is under the dust threshold of %d sat' % DUST)
    need(all(ours(s) is not None for _, s in t['outs']), 'an output is not a treasury script')
    fee = sum(p[2] for p in prevouts) - sum(v for v, _ in t['outs'])
    need(t['vsize'] <= fee <= FEE_CAP, 'fee %d sat for %d vB is outside 1 sat/vB to %d sat' % (fee, t['vsize'], FEE_CAP))
    need(t['vsize'] <= 100000 // 4 and len(t['ins']) <= 16, 'not a standard size')
    # b. each witness: the script hashes to the spent program and is the 2-of-3 this file
    #    derives from the xpubs; two signatures, in key order, over this file's BIP-143 digest
    hash_prevouts = dsha(b''.join(i['outpoint'] for i in t['ins']))
    hash_sequence = dsha(b''.join(i['sequence'] for i in t['ins']))
    hash_outputs = dsha(b''.join(v.to_bytes(8, 'little') + K.ci(len(s)) + s for v, s in t['outs']))
    for j, (i, p) in enumerate(zip(t['ins'], prevouts)):
        where = ours(p[3])
        need(where is not None, 'input %d does not spend a treasury script' % j)
        ws = witness_script(*where)
        w = t['wit'][j]
        need(len(w) == 4 and w[0] == b'' and w[3] == ws and b'\0\x20' + K.sha(ws) == p[3], 'input %d has another witness script' % j)
        digest = dsha(t['version'] + hash_prevouts + hash_sequence + i['outpoint'] + K.ci(len(ws)) + ws +
                      p[2].to_bytes(8, 'little') + i['sequence'] + hash_outputs + t['locktime'] + (1).to_bytes(4, 'little'))
        keys = [ws[1 + 34 * k + 1:1 + 34 * k + 34] for k in range(3)]
        at = -1
        for sig in w[1:3]:
            need(sig[-1:] == b'\x01', 'input %d: a signature is not SIGHASH_ALL' % j)
            hit = [k for k in range(at + 1, 3) if ecdsa_ok(keys[k], digest, sig[:-1])]
            need(hit, 'input %d: a signature does not verify, or is out of key order' % j)
            at = hit[0]
    # c. Bitcoin Core decodes it to the same transaction
    d = core.cli('decoderawtransaction', txhex)
    need(d['txid'] == t['txid'] and d['vsize'] == t['vsize'], "Core's txid or size differs")
    need([(round(o['value'] * 1e8), o['scriptPubKey']['hex']) for o in d['vout']] == [(v, s.hex()) for v, s in want_outs],
         'Core decodes other outputs')
    # d. Bitcoin Core's script interpreter runs every input, with the standard flags, against the
    #    spent outputs given here. With no keys it signs nothing; "complete" is its verdict.
    def verdict(amounts):
        prev = [{'txid': p[0], 'vout': p[1], 'scriptPubKey': p[3].hex(), 'witnessScript': witness_script(*ours(p[3])).hex(),
                 'amount': '%.8f' % (a / 1e8)} for p, a in zip(prevouts, amounts)]
        return core.cli('signrawtransactionwithkey', txhex, '[]', json.dumps(prev))
    v = verdict([p[2] for p in prevouts])
    need(v.get('complete') is True and not v.get('errors') and v['hex'] == txhex, "Core's script check fails: %s" % json.dumps(v.get('errors')))
    wrong = verdict([p[2] + 1 for p in prevouts])
    need(wrong.get('complete') is False and wrong.get('errors'), "Core's script check accepted a wrong amount, so it proves nothing")
    return {'txid': t['txid'], 'vsize': t['vsize'], 'fee_sat': fee, 'sat_per_vb': round(fee / t['vsize'], 2)}


CHECKED = [
    'this file reads the transaction: version 2, locktime 0, the inputs are the expected outpoints, the outputs are the stated amounts and treasury scripts',
    'fee at least 1 sat/vB and at most %d sat; every output at least %d sat (dust); standard size' % (FEE_CAP, DUST),
    'each witness script hashes to the spent P2WSH program and is the 2-of-3 derived here from the three xpubs',
    'each signature is strict DER, low S, SIGHASH_ALL, and verifies over the BIP-143 digest computed here, in key order (pure Python ECDSA)',
    'Bitcoin Core (offline) decodes it to the same txid, size and outputs (decoderawtransaction)',
    "Bitcoin Core's script interpreter accepts every input against the spent outputs with the standard flags (signrawtransactionwithkey, no keys), and refuses the same transaction when an amount is one satoshi off",
    'the spent transaction, read through Esplora, hashes to its txid and holds the amount and script signed for',
]
NOT_CHECKED = [
    "that the spent output is still unspent: only Esplora's answer says so; no node of ours holds the public signet's UTXO set",
    "a node's mempool policy as a whole (testmempoolaccept needs the UTXO set); the broadcast itself is that check",
    'that Esplora tells the truth about confirmations: the block heights are its answers, and anyone can open the URLs',
]


def load_state():
    return json.loads(STATE.read_text()) if STATE.exists() else {}


def save_state(state):
    tmp = STATE.with_suffix('.tmp')
    tmp.write_text(json.dumps(state, indent=1))
    os.replace(tmp, STATE)


def fetch_tx(txid):
    """A transaction's bytes through Esplora, or None; its hash is checked here."""
    code, text = esplora('/tx/%s/hex' % txid)
    if code != 200:
        return None
    raw = bytes.fromhex(text)
    if parse_tx(raw)['txid'] != txid:
        raise Stop(1, 'Esplora answered /tx/%s/hex with another transaction' % txid)
    return raw


def broadcast_and_wait(entry, state, label):
    """Sends entry['hex'] unless the network already has it, then waits for
    a block. entry is in the state file before anything is sent."""
    txid = entry['txid']
    if fetch_tx(txid) is not None:
        say('%s: %s is already on the network; not sent again' % (label, txid))
    else:
        code, text = esplora('/tx', post=entry['hex'])
        if code != 200 or text != txid:
            raise Stop(1, '%s: POST /tx refused %s (HTTP %d: %s). The transaction is kept in %s; a rerun tries it again. '
                          'If its input was spent by someone else (the keys are published test keys), delete that file to start over.'
                       % (label, txid, code, text[:300], STATE))
        say('%s: broadcast %s' % (label, txid))
    entry['broadcast'] = True
    entry['url'] = explorer(txid)
    save_state(state)
    wait = int(os.environ.get('WAIT_SECONDS', '2700'))
    poll = float(os.environ.get('POLL_SECONDS', '2' if REHEARSE else '30'))
    until = time.time() + wait
    while True:
        try:
            code, text = esplora('/tx/%s/status' % txid)
            st = json.loads(text) if code == 200 else {}
        except (Stop, json.JSONDecodeError):
            st = {}
        if st.get('confirmed'):
            entry['block_height'] = st.get('block_height')
            entry['block_hash'] = st.get('block_hash')
            save_state(state)
            say('%s: confirmed in block %s: %s' % (label, st.get('block_height'), entry['url']))
            return
        if time.time() >= until:
            raise Stop(4, '%s: %s was broadcast but no block holds it after %d s. Run the same command again with the same OUT: '
                          'it resumes from %s and waits for this transaction; it does not build another.' % (label, txid, wait, STATE))
        say('%s: waiting for a block (%s unconfirmed, %d s left)' % (label, txid[:16], until - time.time()))
        time.sleep(poll)


def public_path():
    state = load_state()
    assert K.segwit_addr('tb', 0, spk(0, 0)[2:]) == RECEIVE_0
    RESULT['mode'] = ('rehearsal: the public path on a private signet (-signetchallenge=51) behind a local stand-in for Esplora'
                      if REHEARSE else 'public signet')
    RESULT['esplora'] = ESPLORA
    RESULT['receive_0'] = RECEIVE_0
    say('mode: %s; Esplora %s' % (RESULT['mode'], ESPLORA))
    code, tip = esplora('/blocks/tip/height')
    if code != 200 or not tip.isdigit():
        raise Stop(1, 'GET /blocks/tip/height: HTTP %d %s' % (code, tip[:200]))
    say('chain tip: block %s' % tip)
    RESULT['tip_at_start'] = int(tip)

    if 'spend' in state:
        say('resuming from %s: the spend %s was built by an earlier run' % (STATE, state['spend']['txid']))
    else:
        # P1. what is at receive 0
        code, text = esplora('/address/%s/utxo' % RECEIVE_0)
        if code != 200:
            raise Stop(1, 'GET /address/%s/utxo: HTTP %d %s' % (RECEIVE_0, code, text[:200]))
        utxos = json.loads(text)
        confirmed = sorted((u for u in utxos if u.get('status', {}).get('confirmed')), key=lambda u: (-u['value'], u['txid'], u['vout']))
        if not confirmed or confirmed[0]['value'] < SMALLEST:
            if not utxos:
                why = 'holds nothing'
            elif not confirmed:
                why = 'holds %d unconfirmed output(s) and none confirmed: wait for a block and run this again' % len(utxos)
            else:
                why = 'holds no output of at least %d sat (largest: %d sat)' % (SMALLEST, confirmed[0]['value'])
            raise Stop(3, 'Nothing to spend. The treasury\'s receive 0, %s, %s on the signet %s serves (tip %s).\n'
                          'Missing: signet coins at that address. A person must claim them from a signet faucet, by hand, '
                          'to %s (at least %d sat), wait for one confirmation, and run this command again.\n'
                          'Nothing was signed and nothing was broadcast.' % (RECEIVE_0, why, ESPLORA, tip, RECEIVE_0, SMALLEST))
        u = confirmed[0]
        s0 = spk(0, 0)
        raw = fetch_tx(u['txid'])
        if raw is None or parse_tx(raw)['outs'][u['vout']] != (u['value'], s0):
            raise Stop(1, 'the funding transaction %s does not hold %d sat at receive 0 in output %d' % (u['txid'], u['value'], u['vout']))
        say('receive 0 holds %d sat in %s:%d (block %s); its transaction hashes to that txid and pays that amount to receive 0'
            % (u['value'], u['txid'], u['vout'], u['status'].get('block_height')))
        state['funding'] = {'txid': u['txid'], 'vout': u['vout'], 'value_sat': u['value'],
                            'block_height': u['status'].get('block_height'), 'url': explorer(u['txid'])}

    core = Core(['-signet'], 'core-offline', ['-connect=0', '-dnsseed=0', '-fixedseeds=0'])
    try:
        net = core.cli('getnetworkinfo')
        chain = core.cli('getblockchaininfo')
        RESULT['core'] = {'version': net['subversion'], 'chain': chain['chain'], 'blocks': chain['blocks'],
                          'connections': net['connections'], 'peers': core.cli('getpeerinfo')}
        say('Bitcoin Core %s offline: chain %s, %d blocks, %d connections' % (net['subversion'], chain['chain'], chain['blocks'], net['connections']))
        if chain['blocks'] != 0 or net['connections'] != 0:
            raise Stop(1, 'the offline Core is not offline')

        f = state['funding']
        coin = (f['txid'], f['vout'], f['value_sat'], spk(0, 0))
        pay, other, change = spk(0, 1), spk(0, 2), spk(1, 0)
        if 'spend' not in state:
            # P2. requests, relay, agents, coordinator
            if not (280 <= FEE_SAT <= FEE_CAP):     # the spend is about 201 vB, the recovery about 263 vB
                raise Stop(1, 'FEE_SAT=%d: the fee must be 280 to %d sat (1 sat/vB for the larger transaction; the lowest signer ceiling)' % (FEE_SAT, FEE_CAP))
            pay_sat = (f['value_sat'] - FEE_SAT) * 7 // 10
            change_sat = f['value_sat'] - FEE_SAT - pay_sat
            say('request: %d sat to receive 1 (%s), %d sat to change 0 (%s), fee %d sat' % (
                pay_sat, K.segwit_addr('tb', 0, pay[2:]), change_sat, K.segwit_addr('tb', 0, change[2:]), FEE_SAT))
            txfile, c1, c2 = relay_round([coin], pay, other, change, pay_sat, change_sat, FEE_SAT)
            if not (RESULT['agents_signed_first_refused_second'] and c1 == 0 and c2 == 3):
                raise Stop(1, 'the signing round did not end as it must (all three sign request 1, all three refuse request 2); nothing broadcast')
            txhex = txfile.read_text().strip()
            # P3. before broadcast
            got = check_tx(core, txhex, [coin], [(pay_sat, pay), (change_sat, change)], 'spend')
            say('spend %s: %d vB, fee %d sat (%s sat/vB); checked before broadcast' % (got['txid'], got['vsize'], got['fee_sat'], got['sat_per_vb']))
            state['spend'] = dict(got, hex=txhex, pay_sat=pay_sat, change_sat=change_sat, broadcast=False,
                                  agents_signed_first_refused_second=True, coordinator=RESULT['coordinator'])
            save_state(state)       # before the broadcast: a rerun finds this transaction and never builds another
        sp = state['spend']
        RESULT['agents_signed_first_refused_second'] = sp['agents_signed_first_refused_second']
        RESULT['coordinator'] = sp['coordinator']
        for line in CHECKED:
            say('  checked before broadcast:', line)
        for line in NOT_CHECKED:
            say('  NOT checked before broadcast:', line)
        # P4. broadcast, confirm
        broadcast_and_wait(sp, state, 'spend')
        raw = fetch_tx(sp['txid'])
        on_chain = parse_tx(raw)['outs'] if raw else []
        sp['outputs_are_the_request'] = on_chain == [(sp['pay_sat'], pay), (sp['change_sat'], change)]
        save_state(state)

        # P5. recovery with Core alone: the descriptor, two account keys, no coordinator, no chain
        if 'recovery' not in state:
            if not sp['outputs_are_the_request']:
                raise Stop(1, 'the confirmed spend, read back through Esplora, does not hold the request\'s outputs')
            ins = [(sp['txid'], n, v, s) for n, (v, s) in enumerate(on_chain)]      # witness_utxo from the confirmed spend
            dest = spk(0, 3)
            out_sat = sp['pay_sat'] + sp['change_sat'] - FEE_SAT
            wallet = re.search(r'^KY_WALLET :: (".*");$', (ROOT / 'coax/tests/nostr/keys_fixtures.coax').read_text(), re.M)
            desc = json.loads(json.loads(wallet.group(1)))['descriptor'].split('#')[0]
            for n in ('alice', 'keyco'):
                fp, tpub, tprv = agent_account(n)
                desc = desc.replace(tpub, tprv)
            descs = []
            for branch in (0, 1):
                d = desc.replace('<0;1>', str(branch))
                descs.append({'desc': d + '#' + core.cli('getdescriptorinfo', d)['checksum'], 'range': [0, 20]})
            unsigned = base64.b64encode(psbt(ins, [(out_sat, dest)])).decode()
            done = core.cli('descriptorprocesspsbt', unsigned, json.dumps(descs), 'ALL', True, True)
            if not done.get('complete') or 'hex' not in done:
                raise Stop(1, "Core's descriptorprocesspsbt did not complete the recovery: %s" % json.dumps({k: v for k, v in done.items() if k != 'psbt'}))
            got = check_tx(core, done['hex'], ins, [(out_sat, dest)], 'recovery')
            say('recovery %s: Core signed both inputs from the descriptor, offline; %d sat to receive 3 (%s), %d vB, fee %d sat; checked before broadcast'
                % (got['txid'], out_sat, K.segwit_addr('tb', 0, dest[2:]), got['vsize'], got['fee_sat']))
            state['recovery'] = dict(got, hex=done['hex'], out_sat=out_sat, broadcast=False, spent=[[t, n] for t, n, _, _ in ins])
            save_state(state)
        else:
            say('resuming: the recovery %s was built by an earlier run' % state['recovery']['txid'])
        rc = state['recovery']
        broadcast_and_wait(rc, state, 'recovery')
        raw = fetch_tx(rc['txid'])
        back = parse_tx(raw) if raw else {'outs': [], 'ins': []}
        rc['outputs_are_receive_3'] = back['outs'] == [(rc['out_sat'], spk(0, 3))]
        rc['spent_the_spends_outputs'] = [[i['txid'], i['vout']] for i in back['ins']] == [[sp['txid'], 0], [sp['txid'], 1]]
        save_state(state)
    finally:
        core.stop()
        for k in ('funding', 'spend', 'recovery'):
            if k in state:
                RESULT[k] = {a: b for a, b in state[k].items() if a not in ('agents_signed_first_refused_second', 'coordinator')}
    RESULT['checked_before_broadcast'] = CHECKED
    RESULT['not_checked_before_broadcast'] = NOT_CHECKED
    return bool(RESULT['agents_signed_first_refused_second'] and RESULT['coordinator'] == {'request_1_exit': 0, 'request_2_exit': 3}
                and sp.get('block_height') and sp['outputs_are_the_request']
                and rc.get('block_height') and rc['outputs_are_receive_3'] and rc['spent_the_spends_outputs'])


# ---- a stand-in for Esplora on a private signet, for the rehearsal ----

def fake_esplora(node, mine):
    """The five Esplora endpoints the public path uses, answered by a private
    signet node; a block is dug a few seconds after each POST /tx. Returns
    the server; its URL is http://127.0.0.1:PORT."""
    delay = float(os.environ.get('FAKE_MINE_DELAY', '3'))
    lock = threading.Lock()      # scantxoutset runs one scan at a time; the miner and the reader share the node

    def rpc(*a, **kw):
        with lock:
            return node.cli(*a, **kw)

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *a):
            pass

        def answer(self, code, text):
            body = text.encode()
            self.send_response(code)
            self.send_header('Content-Type', 'text/plain')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            path = self.path.rstrip('/')
            try:
                if path == '/blocks/tip/height':
                    return self.answer(200, str(rpc('getblockcount')))
                m = re.fullmatch(r'/address/([a-z0-9]+)/utxo', path)
                if m:
                    found = rpc('scantxoutset', 'start', json.dumps(['addr(%s)' % m.group(1)]))
                    out = []
                    for u in found['unspents']:
                        if rpc('gettxout', u['txid'], u['vout'], True) == '':     # spent by a transaction in the mempool
                            continue
                        head = rpc('getblockheader', rpc('getblockhash', u['height']))
                        out.append({'txid': u['txid'], 'vout': u['vout'], 'value': round(u['amount'] * 1e8),
                                    'status': {'confirmed': True, 'block_height': u['height'], 'block_hash': head['hash'], 'block_time': head['time']}})
                    return self.answer(200, json.dumps(out))
                m = re.fullmatch(r'/tx/([0-9a-f]{64})/(hex|status)', path)
                if m:
                    try:
                        tx = rpc('getrawtransaction', m.group(1), True)
                    except subprocess.CalledProcessError:
                        tx = None
                    if m.group(2) == 'hex':
                        return self.answer(200, tx['hex']) if tx else self.answer(404, 'Transaction not found')
                    if not tx or 'blockhash' not in tx:
                        return self.answer(200, json.dumps({'confirmed': False}))
                    head = rpc('getblockheader', tx['blockhash'])
                    return self.answer(200, json.dumps({'confirmed': True, 'block_height': head['height'], 'block_hash': head['hash'], 'block_time': head['time']}))
                self.answer(404, 'not found')
            except subprocess.CalledProcessError as e:
                self.answer(400, (e.stderr or '').strip())

        def do_POST(self):
            if self.path.rstrip('/') != '/tx':
                return self.answer(404, 'not found')
            txhex = self.rfile.read(int(self.headers.get('Content-Length', '0'))).decode().strip()
            try:
                txid = rpc('sendrawtransaction', txhex)
            except subprocess.CalledProcessError as e:
                return self.answer(400, 'sendrawtransaction RPC error: ' + (e.stderr or '').strip())
            threading.Timer(delay, lambda: dig(node, 1, mine)).start()      # the stand-in's miner
            self.answer(200, txid)

    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def fake_chain():
    """The private signet of CHAIN=signet with 60,000 sat at receive 0, and
    the stand-in in front of it."""
    node = Core(PRIVATE_SIGNET, 'core')
    node.cli('createwallet', 'miner')
    mine = node.cli('getnewaddress', wallet='miner')
    dig(node, 101, mine)
    if not node.cli('scantxoutset', 'start', json.dumps(['addr(%s)' % RECEIVE_0]))['unspents']:
        txid = node.cli('sendtoaddress', RECEIVE_0, '0.00060000', wallet='miner')
        dig(node, 1, mine)
        say('stand-in: the private signet node paid 60000 sat to receive 0 in %s' % txid)
    server = fake_esplora(node, mine)
    say('stand-in: Esplora on http://127.0.0.1:%d, answered by a private signet node (-signet -signetchallenge=51)' % server.server_address[1])
    return node, server


def fake_main():
    node, server = fake_chain()
    say('ESPLORA=http://127.0.0.1:%d   (interrupt to stop)' % server.server_address[1])
    try:
        threading.Event().wait()
    except KeyboardInterrupt:
        pass
    finally:
        server.shutdown()
        node.stop()


def public_main():
    global ESPLORA
    node = server = None
    status = 1
    try:
        if REHEARSE and 'ESPLORA' not in os.environ:
            node, server = fake_chain()
            ESPLORA = 'http://127.0.0.1:%d' % server.server_address[1]
        if REHEARSE and not ESPLORA.startswith('http://127.0.0.1:'):
            raise Stop(1, 'REHEARSE=1 is for a stand-in on 127.0.0.1, not %s' % ESPLORA)
        ok = public_path()
        RESULT['passed'] = ok
        status = 0 if ok else 1
        say('PASSED' if ok else 'FAILED', json.dumps(RESULT))
    except Stop as e:
        status = e.status
        RESULT['passed'] = False
        RESULT['stopped'] = {'status': e.status, 'why': str(e)}
        say(str(e))
        say('exit status %d' % e.status)
    finally:
        if server:
            server.shutdown()
        if node:
            node.stop()
    (OUT / ('treasury-%s.json' % MODE)).write_text(json.dumps(RESULT, indent=1))
    sys.exit(status)


def main():
    if CHAIN == 'public-signet':
        return public_main()
    if CHAIN == 'fake-esplora':
        return fake_main()
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

        # 2. the requests and their PSBTs; 3. relay, agents, coordinator
        pay = bytes.fromhex(core.cli('getaddressinfo', core.cli('getnewaddress', '', 'bech32', wallet='miner'),
                                     wallet='miner')['scriptPubKey'])
        other = bytes.fromhex(core.cli('getaddressinfo', core.cli('getnewaddress', '', 'bech32', wallet='miner'),
                                       wallet='miner')['scriptPubKey'])
        change = spk(1, 0)
        txfile, c1, c2 = relay_round(coins, pay, other, change, 70000, 29000, 1000)

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
