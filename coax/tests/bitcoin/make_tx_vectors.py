#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: copies published vectors outside Coax)
"""Writes coax/tests/bitcoin/tx_vectors.coax and tx_core_vectors.coax for
Bitcoin chapter 34 from files pinned in coax/tests/vectors/upstream/:

  python3 -I coax/tests/bitcoin/make_tx_vectors.py

- Bitcoin Core's tx_valid.json and tx_invalid.json: every transaction, by
  its serialized hex. Their scripts and flags are for later chapters.
- Mainnet transactions: the three that tx_valid.json names by txid, and
  three segwit ones (P2WPKH, a Taproot key-path and a Taproot script-path
  spend) whose raw bytes come from mempool.space's API, each pinned by its
  SHA-256, with the weight and size its JSON publishes. rust-bitcoin
  0.32.5's transaction.rs publishes the P2WPKH one's txid, wtxid, weight
  442 and vsize 111, and the zero-input case below.
- Ambiguous bytes for Core's DecodeTx: three built here (the comments say
  how) and one legacy transaction with no inputs.

Every input's SHA-256 must equal its pin below. A txid is published for
every mainnet transaction; the wtxids of the two Taproot ones and the
legacy weights (4 x size) are not, so they are computed here with
tx_reference.py, which shares no code with Coax, and marked as computed.
"""
import hashlib, json, re, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import tx_reference as R  # noqa: E402

ROOT = HERE.parent / 'vectors' / 'upstream'
P2WPKH = 'f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206'
KEYPATH = '33e794d097969002ee05d336686fc03c9e15a597c1b9827669460fac98799036'
SCRIPTPATH = '37777defed8717c581b4c0509329550e344bdc14ac38f71fc050096887e535c8'
PINS = {
    'bitcoin-core/tx_valid.json': '0abeb1a6e4b942e4e43ef8d49b7356c1f6204bcb14163f24153f4d0ec5418011',
    'bitcoin-core/tx_invalid.json': '0c02ce44ff3a880458f9569a25589315a07f924fcaadac828613d4615776ca52',
    'rust-bitcoin/transaction.rs': '2e5cd2614aa667a2cf9da4b87cfc7fd17fbf26fc610a1ba79c0dce6f2e70be8d',
    f'mainnet/{P2WPKH}.hex': '4d9ba5d97cdb82847551d6a487e3479958b97b1c5cc9549c932c9bc261a2bf8e',
    f'mainnet/{P2WPKH}.json': '66d8bb646ababf838e207b78a0c6878c0d136709ea768b71d5ef78dc2a642f51',
    f'mainnet/{KEYPATH}.hex': '4864b8eda5c76a9ae3235130b6a0370f59533a4c20962c89ec1d4ce67d1fcecc',
    f'mainnet/{KEYPATH}.json': '49f1314013f8bffd9c13f8a45d54983a5f9b317c96d4b8dba1330e11a353a63b',
    f'mainnet/{SCRIPTPATH}.hex': '2098bb60eb4f47c230213ba7bd72a0b00dfcd6d295b59469b64ef1de8d72640d',
    f'mainnet/{SCRIPTPATH}.json': '8a9d44ab04ac3e4cf22472446690fcc5fad2b69ef9fb2d8711809338c51270b1',
}
src = {}
for name, pin in PINS.items():
    data = (ROOT / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == pin, name
    src[name] = data.decode()

# ---- Core's rows: [[prevouts], "hex", "flags"]; a one-string row is a comment.
core = []
for name, valid in [('bitcoin-core/tx_valid.json', True), ('bitcoin-core/tx_invalid.json', False)]:
    for row in json.loads(src[name]):
        if len(row) == 3 and isinstance(row[1], str):
            core.append((row[1], valid))
assert len(core) == 215

# ---- mainnet transactions
mainnet = []
named = None
for row in json.loads(src['bitcoin-core/tx_valid.json']):
    if len(row) == 1:
        m = re.fullmatch(r'The following is ([0-9a-f]{64})', row[0])
        if m: named = m.group(1)
        continue
    if named:
        tx = R.unserialize(bytes.fromhex(row[1]), True)
        size = len(row[1]) // 2
        mainnet.append(dict(txid=named, wtxid=named, tx=row[1], segwit=False, nin=len(tx['ins']), nout=len(tx['outs']),
                            size=size, weight=4 * size, source='tx_valid.json; weight 4 x size (computed)'))
        named = None
rust = src['rust-bitcoin/transaction.rs']
test = rust[rust.index('fn segwit_transaction()'):rust.index('fn consensus_serde()')]
rust_hex = ''.join(re.findall(r'[0-9a-f]+', re.search(r'hex!\(\s*(".*?")\s*\)', test, re.S).group(1).replace('\\\n', '')))
rust_txid, rust_wtxid = re.findall(r'"([0-9a-f]{64})"\.to_string\(\)', test)[1:3]
assert 'Weight::from_wu(442)' in test and 'realtx.vsize(), 111' in test
for txid in [P2WPKH, KEYPATH, SCRIPTPATH]:
    raw = src[f'mainnet/{txid}.hex'].strip()
    info = json.loads(src[f'mainnet/{txid}.json'])
    assert info['txid'] == txid and info['size'] == len(raw) // 2 and info['status']['confirmed']
    tx = R.unserialize(bytes.fromhex(raw), True)
    assert R.txid(tx)[::-1].hex() == txid and R.weight(tx) == info['weight']
    wtxid, source = R.wtxid(tx)[::-1].hex(), 'mempool.space, block %d; wtxid computed' % info['status']['block_height']
    if txid == P2WPKH:
        assert raw == rust_hex and rust_txid == txid and rust_wtxid == wtxid and info['weight'] == 442
        source = 'mempool.space, block %d; wtxid from rust-bitcoin 0.32.5' % info['status']['block_height']
    mainnet.append(dict(txid=txid, wtxid=wtxid, tx=raw, segwit=True, nin=len(tx['ins']), nout=len(tx['outs']),
                        size=info['size'], weight=info['weight'], source=source))
assert [m['txid'][:8] for m in mainnet] == ['23b397ed', 'c99c49da', 'f7fdd091', 'f5864806', '33e794d0', '37777def']

# ---- ambiguous bytes
# rust-bitcoin's tx_no_input_deserialization: the extended form of a
# transaction with no inputs, one output and no witness.
no_input = rust[rust.index('fn tx_no_input_deserialization()'):]
segwit_no_inputs = re.search(r'"([0-9a-f]+)"', no_input).group(1)
# A legacy transaction with no inputs and one output (rust-bitcoin's
# output): its extended reading takes the value's first byte as the input
# count and runs out of bytes.
zero_inputs = '01000000' + '00' + '01' + '00e1f50500000000' + '19' + '76a9140389035a9225b3839e2bbf32d826a1e222031fd888ac' + '00000000'


def both_ways(spk, first):
    """An extended transaction with one input, one output and one witness
    item that also reads as a legacy transaction with no inputs and one
    output. The legacy reading's value is the input count and the first 7
    outpoint bytes; outpoint byte 7 is its script length, set so that the
    legacy script ends 4 bytes before the end; outpoint byte 8 starts that
    script (0x4c, PUSHDATA1, pushes the rest; 0xff is an invalid op)."""
    rest = b'\x00' + bytes(4) + b'\x01' + (1000).to_bytes(8, 'little') + bytes([len(spk)]) + spk + b'\x01\x01\x51'
    length = (1 + 36 + len(rest) + 4) - 8 - 1 - 4
    outpoint = bytes([0x11] * 7) + bytes([length]) + first + bytes([length - 2]) + bytes([0x22] * 26)
    return (bytes.fromhex('02000000') + b'\x00\x01\x01' + outpoint + rest + bytes(4)).hex()


both_sane = both_ways(bytes.fromhex('0014') + bytes(20), b'\x4c')
legacy_sane = both_ways(b'\xff', b'\x4c')
neither_sane = both_ways(b'\xff', b'\xff')
for text, segwit in [(both_sane, True), (legacy_sane, False), (neither_sane, True), (zero_inputs, False)]:
    for allow in (True, False):
        if text == zero_inputs and allow: continue
        R.unserialize(bytes.fromhex(text), allow)
    tx, _ = R.decode(bytes.fromhex(text))
    assert tx['segwit'] == segwit
assert R.decode(bytes.fromhex(segwit_no_inputs)) [0] is None


def write():
    """Writes the two vector files."""
    out = ['// Generated by make_tx_vectors.py from the files pinned in',
           '// coax/tests/vectors/upstream/ (see SOURCES.md there). Test data only.', '',
           '// Mainnet transactions: the txid is published by the source named; the',
           '// size and weight by mempool.space for the segwit ones.',
           'TxMainnet :: struct { txid: string; wtxid: string; tx: string; segwit: bool; nin: int; nout: int; size: int; weight: int; }',
           'TX_MAINNET :: [%d] TxMainnet.[' % len(mainnet)]
    rows = []
    for m in mainnet:
        rows.append('  // %s %s\n  TxMainnet.{txid="%s", wtxid="%s", tx="%s", segwit=%s, nin=%d, nout=%d, size=%d, weight=%d}' % (
            m['txid'][:8], m['source'], m['txid'], m['wtxid'], m['tx'], 'true' if m['segwit'] else 'false', m['nin'], m['nout'], m['size'], m['weight']))
    out.append(',\n'.join(rows) + '];')
    out += ['// rust-bitcoin 0.32.5 tx_no_input_deserialization: the marker and flag,',
            '// no inputs, one output, no witnesses. Core refuses both readings.',
            'TX_SEGWIT_NO_INPUTS :: "%s";' % segwit_no_inputs,
            '// A legacy transaction with no inputs and one output.',
            'TX_ZERO_INPUTS :: "%s";' % zero_inputs,
            '// Built by make_tx_vectors.py: bytes that read both ways. Core keeps the',
            '// extended reading when its scripts parse, else the legacy one when its',
            '// scripts parse, else the extended one.',
            'TX_BOTH_SANE :: "%s";' % both_sane,
            'TX_LEGACY_SANE :: "%s";' % legacy_sane,
            'TX_NEITHER_SANE :: "%s";' % neither_sane]
    (HERE / 'tx_vectors.coax').write_text('\n'.join(out) + '\n')

    core_out = ['// Generated by make_tx_vectors.py from Bitcoin Core\'s tx_valid.json and',
                '// tx_invalid.json, pinned in coax/tests/vectors/upstream/bitcoin-core/',
                '// (see SOURCES.md there). Each transaction\'s hex, and whether it is',
                '// from tx_valid.json. Test data only.',
                'TxCore :: struct { tx: string; valid: bool; }',
                'TX_CORE :: [%d] TxCore.[\n%s];' % (len(core), ',\n'.join('  TxCore.{tx="%s", valid=%s}' % (h, 'true' if v else 'false') for h, v in core))]
    (HERE / 'tx_core_vectors.coax').write_text('\n'.join(core_out) + '\n')
    print(len(mainnet), 'mainnet,', len(core), 'Core rows')


if __name__ == '__main__':
    write()
