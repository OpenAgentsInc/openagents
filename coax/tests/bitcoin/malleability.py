#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python decides each case outside Coax)
"""The cross-check of Bitcoin chapter 35. It builds every form of every
transaction the chapter's example changes, in Python that shares no code
with Coax, and writes the transcript the example must print:

  python3 -I coax/tests/bitcoin/malleability.py            # compare with the fixture
  python3 -I coax/tests/bitcoin/malleability.py --coax coax/bin/coax   # also run the example

It reads the transactions from the pinned sources: Bitcoin Core's
tx_valid.json (23b397ed and its four changed forms), the mempool.space
hex and JSON of f5864806 (its bytes, and the value and scriptPubKey of the
coin it spends), and BIP-143's P2SH-P2WPKH example as copied into
sighash_vectors.coax. Transactions are read and written by tx_reference.py
(a transliteration of Core), signatures checked by der_reference.py, and
the BIP-143 digest and RFC 6979 nonces are computed here.
"""
import hashlib, hmac, json, re, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import tx_reference as T  # noqa: E402
import der_reference as D  # noqa: E402

REPO = HERE.parents[2]
UP = HERE.parent / 'vectors' / 'upstream'
FIXTURE = REPO / 'bitcoin/fixtures/malleability.out'
EXAMPLE = 'bitcoin/examples/malleability.coax'
PINS = {
    'bitcoin-core/tx_valid.json': '0abeb1a6e4b942e4e43ef8d49b7356c1f6204bcb14163f24153f4d0ec5418011',
    'mainnet/f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206.hex': '4d9ba5d97cdb82847551d6a487e3479958b97b1c5cc9549c932c9bc261a2bf8e',
    'mainnet/f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206.json': '66d8bb646ababf838e207b78a0c6878c0d136709ea768b71d5ef78dc2a642f51',
}


def pinned(name):
    data = (UP / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == PINS[name], name
    return data.decode()


def shown(digest):
    return digest[::-1].hex()


def push(data, minimal=True):
    if minimal and len(data) <= 75:
        return bytes([len(data)]) + data
    return bytes([0x4C, len(data)]) + data


def strict(sig, flip):
    r, s = D.lax(sig[:-1])
    if flip:
        s = D.N - s
    def integer(v):
        b = v.to_bytes(32, 'big').lstrip(b'\0')
        if b[0] & 0x80: b = b'\0' + b
        return bytes([2, len(b)]) + b
    body = integer(r) + integer(s)
    return bytes([0x30, len(body)]) + body + sig[-1:]


def ids(raw):
    tx = T.unserialize(raw, True)
    return T.txid(tx), T.wtxid(tx), T.weight(tx)


def sha256d(b):
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def bip143(tx, index, code, amount, hashtype):
    """BIP-143's digest, written from the BIP's text."""
    anyone = hashtype & 0x80
    base = hashtype & 0x1F
    zero = bytes(32)
    prevouts = zero if anyone else sha256d(b''.join(i['prevout'] for i in tx['ins']))
    sequences = zero if anyone or base in (2, 3) else sha256d(b''.join(i['sequence'] for i in tx['ins']))
    outputs = sha256d(b''.join(o['value'] + T.compact(len(o['script'])) + o['script'] for o in tx['outs']))
    i = tx['ins'][index]
    msg = (tx['version'] + prevouts + sequences + i['prevout'] + T.compact(len(code)) + code
           + amount.to_bytes(8, 'little') + i['sequence'] + outputs + tx['locktime'] + hashtype.to_bytes(4, 'little'))
    return sha256d(msg)


def sign(secret, digest):
    """RFC 6979 with SHA-256, then low S, as Core and libsecp256k1 sign."""
    x, h = secret.to_bytes(32, 'big'), (int.from_bytes(digest, 'big') % D.N).to_bytes(32, 'big')
    v, k = b'\1' * 32, b'\0' * 32
    mac = lambda key, data: hmac.new(key, data, hashlib.sha256).digest()
    k = mac(k, v + b'\0' + x + h); v = mac(k, v)
    k = mac(k, v + b'\1' + x + h); v = mac(k, v)
    while True:
        v = mac(k, v)
        nonce = int.from_bytes(v, 'big')
        if 1 <= nonce < D.N:
            R = D.mul(nonce, D.G)
            r = R[0] % D.N
            s = pow(nonce, -1, D.N) * (int.from_bytes(digest, 'big') + r * secret) % D.N
            if r and s:
                return r, min(s, D.N - s)
        k = mac(k, v + b'\0'); v = mac(k, v)


def der(r, s):
    def integer(v):
        b = v.to_bytes(32, 'big').lstrip(b'\0')
        if b[0] & 0x80: b = b'\0' + b
        return bytes([2, len(b)]) + b
    body = integer(r) + integer(s)
    return bytes([0x30, len(body)]) + body


def hash160(b):
    return hashlib.new('ripemd160', hashlib.sha256(b).digest()).digest()


def with_script(raw, script):
    tx = T.unserialize(raw, True)
    tx['ins'][0]['script'] = script
    return T.serialize(tx, True)


def transcript():
    out = []
    p = out.append
    rows = [row[1] for row in json.loads(pinned('bitcoin-core/tx_valid.json')) if len(row) == 3 and isinstance(row[1], str)]
    mined = bytes.fromhex(rows[0])
    assert T.txid(T.unserialize(mined, False)).hex() == '63dd949ad0ca1e27eb8344cc9bdefcfa706375c903b6ad740a74d3cced97b323'
    seen = []

    def form(label, raw, sig):
        txid = ids(raw)[0]
        p(label)
        p(f'  txid: {shown(txid)}')
        p(f'  strict DER {str(D.valid_encoding(sig)).lower()}, low S {str(D.low_s(sig)).lower()}')
        if txid not in seen: seen.append(txid)

    p('23b397ed: a 1-of-2 bare multisig spend, as mined in 2011')
    tx = T.unserialize(mined, False)
    script = tx['ins'][0]['script']
    assert script[0] == 0 and script[1] == 71
    sig = script[2:]
    form('as mined', mined, sig)
    p("tx_valid.json's four changed forms, valid without BIP-66 and NULLDUMMY")
    labels = ['a byte stuffed into the signature', 'dummy 0x01ff instead of OP_0', 'dummy OP_1', 'dummy OP_1NEGATE']
    for k in range(1, 5):
        txid = ids(bytes.fromhex(rows[k]))[0]
        p(f'  {labels[k-1]}')
        p(f'    txid: {shown(txid)}')
        if txid not in seen: seen.append(txid)
    # The four rows are the mined form with these scriptSigs.
    rebuilt = [b'\0' + push(sig[:-1] + b'\xab' + sig[-1:]), b'\x01\xff' + push(sig), b'\x51' + push(sig), b'\x4f' + push(sig)]
    for k in range(4):
        assert with_script(mined, rebuilt[k]) == bytes.fromhex(rows[k + 1]), k
    same, low = strict(sig, False), strict(sig, True)
    p('built here from the mined form')
    form('strict DER, the same s', with_script(mined, b'\0' + push(same)), same)
    form('strict DER, s replaced by n - s: the canonical form', with_script(mined, b'\0' + push(low)), low)
    form('canonical, dummy OP_1', with_script(mined, b'\x51' + push(low)), low)
    form('canonical, the signature pushed with OP_PUSHDATA1', with_script(mined, b'\0' + push(low, False)), low)
    form('canonical, an extra OP_0 first', with_script(mined, b'\0\0' + push(low)), low)
    p(f'10 forms, {len(seen)} different txids')

    # ---- f5864806
    raw = bytes.fromhex(pinned('mainnet/f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206.hex').strip())
    meta = json.loads(pinned('mainnet/f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206.json'))
    value = meta['vin'][0]['prevout']['value']
    keyhash = bytes.fromhex(meta['vin'][0]['prevout']['scriptpubkey'])[2:]
    p('f5864806: a P2WPKH spend')
    tx = T.unserialize(raw, True)
    sig, key = tx['ins'][0]['witness']
    digest = bip143(tx, 0, b'\x76\xa9\x14' + keyhash + b'\x88\xac', value, 1)
    yes = lambda b: str(b).lower()
    p(f'spends {value} satoshis; BIP-143 digest computed')
    p(f'signature: {len(sig)} bytes, verifies {yes(D.checksig(sig, key, digest))}, low S {yes(D.low_s(sig))}')
    high = strict(sig, True)
    p(f's replaced by n - s: {len(high)} bytes, verifies {yes(D.checksig(high, key, digest))}, low S {yes(D.low_s(high))}')
    before = ids(raw)
    tx['ins'][0]['witness'] = [high, key]
    after = ids(T.serialize(tx, True))
    p(f'  txid : {shown(after[0])}')
    p(f'  wtxid: {shown(after[1])}')
    p(f'  txid unchanged {yes(before[0] == after[0])}, wtxid unchanged {yes(before[1] == after[1])}, weight {before[2]} -> {after[2]}')
    tx = T.unserialize(raw, True)
    tx['ins'][0]['script'] = b'\0'
    p(f'OP_0 in the empty scriptSig: txid unchanged {yes(ids(T.serialize(tx, True))[0] == before[0])}; BIP-141 requires it empty')

    # ---- BIP-143's P2SH-P2WPKH example
    text = (HERE / 'sighash_vectors.coax').read_text()
    v = dict(re.findall(r'(\w+)="?([0-9a-f]*)"?', re.findall(r'BtVec143\.\{[^}]*\}', text)[1]))
    p("BIP-143's P2SH-P2WPKH example, signed twice with its published key")
    unsigned = bytes.fromhex(v['tx'])
    secret = int(v['key'], 16)
    pub = D.mul(secret, D.G)
    pub = bytes([2 | pub[1] & 1]) + pub[0].to_bytes(32, 'big')
    code = bytes.fromhex(v['code'])[1:]
    p(f"the key's HASH160 is the script code's: {yes(hash160(pub) == code[3:23])}")
    names = ['SIGHASH_ALL', 'SIGHASH_ALL|ANYONECANPAY']
    wtx, legacy = [], []
    for k, hashtype in enumerate([1, 0x81]):
        tx = T.unserialize(unsigned, False)
        digest = bip143(tx, 0, code, int(v['amount']), hashtype)
        if k == 0: assert digest.hex() == v['sighash']
        one = der(*sign(secret, digest)) + bytes([hashtype])
        line = f'{names[k]}: {len(one)} bytes, verifies {yes(D.checksig(one, pub, digest))}'
        if k == 0: line += f", BIP-143's signature {yes(one[:-1].hex() == v['sig'])}"
        p(line)
        tx['ins'][0]['script'] = push(b'\x00\x14' + hash160(pub))
        tx['ins'][0]['witness'] = [one, pub]
        wtx.append(ids(T.serialize(tx, True)))
        tx = T.unserialize(unsigned, False)
        tx['ins'][0]['script'] = push(one) + push(pub)
        legacy.append(ids(T.serialize(tx, True)))
    p('as P2SH-P2WPKH')
    for w in wtx:
        p(f'  txid : {shown(w[0])}')
        p(f'  wtxid: {shown(w[1])}')
    p(f'  same txid {yes(wtx[0][0] == wtx[1][0])}, same wtxid {yes(wtx[0][1] == wtx[1][1])}')
    p('as if the coin were P2PKH')
    for w in legacy:
        p(f'  txid : {shown(w[0])}')
    p(f'  same txid {yes(legacy[0][0] == legacy[1][0])}')
    return '\n'.join(out) + '\n'


def main():
    want = transcript()
    got = FIXTURE.read_text()
    assert got == want, 'the fixture differs from the Python transcript'
    print(f'malleability fixture: {want.count(chr(10))} lines agree with Python')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE, '--run-steps', '250000000'] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
