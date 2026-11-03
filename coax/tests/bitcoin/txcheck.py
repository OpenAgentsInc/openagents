#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: rules read from the pinned Bitcoin Core sources and Python transliterations of them decide each case outside Coax)
"""The cross-check of Bitcoin chapter 52. It reads CheckTransaction from
the pinned consensus/tx_check.cpp and CheckTxInputs and IsFinalTx from
consensus/tx_verify.cpp, checks that the reject reasons and constants of
coax/lib/bitcoin/txcheck.coax are Core's, runs a transliteration of each
over Core's tx_valid.json and tx_invalid.json with chapter 34's Python
reader (tx_reference.py), and writes the transcript the example must print:

  python3 -I coax/tests/bitcoin/txcheck.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/txcheck.py --coax coax/bin/coax   # also run the example

Core's own test (transaction_tests.cpp) is the oracle for the vectors:
CheckTransaction refuses every BADTX case and passes every other one.
"""
import hashlib, json, re, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import tx_reference as T  # noqa: E402

REPO = HERE.parents[2]
UP = HERE.parent / 'vectors' / 'upstream' / 'bitcoin-core'
FIXTURE = REPO / 'bitcoin/fixtures/tx-checks.out'
EXAMPLE = 'bitcoin/examples/tx-checks.coax'
LIB = REPO / 'coax/lib/bitcoin/txcheck.coax'
PINS = {
    'tx_check.cpp': '66ff205c2bb482133eb864968815158e3474554870ad64e6e795fe313d803ed5',
    'tx_verify.cpp': '45de31c846d87b202b46f1e314cbd07869c0b3e598cdc7b18342e2fe24de3fc0',
    'amount.h': 'fbaf7f3186b066f911e74720117299e9b88132cf612a1b27dfe34ab55752a393',
    'consensus.h': 'aa55254b379a2cc5f1835b506e248f80b4f020120782d2780ce52ff7c8cb827d',
    'chainparams.cpp': 'cfadf1478377c32625280520655b598674e6d487f08d7f7a80ac69252104b5e1',
    'transaction_tests.cpp': 'eb0b3f38018ec8fa383e993ee5f0df4887d5ff2784cb80b4f648011e24ded5d5',
    'tx_valid.json': '0abeb1a6e4b942e4e43ef8d49b7356c1f6204bcb14163f24153f4d0ec5418011',
    'tx_invalid.json': '0c02ce44ff3a880458f9569a25589315a07f924fcaadac828613d4615776ca52',
}


def text(name):
    data = (UP / name).read_bytes()
    if PINS[name]:
        assert hashlib.sha256(data).hexdigest() == PINS[name], name
    return data.decode()


def one(pattern, s):
    found = re.findall(pattern, s)
    assert len(found) == 1, (pattern, found)
    return found[0]


def rules():
    check = text('tx_check.cpp')
    verify = text('tx_verify.cpp')
    body = verify[verify.index('bool Consensus::CheckTxInputs'):]
    reasons = re.findall(r'"(bad-[a-z-]+)"', check) + re.findall(r'"(bad-[a-z-]+)"', body)[:5]
    assert reasons == ['bad-txns-vin-empty', 'bad-txns-vout-empty', 'bad-txns-oversize', 'bad-txns-vout-negative',
                       'bad-txns-vout-toolarge', 'bad-txns-txouttotal-toolarge', 'bad-txns-inputs-duplicate',
                       'bad-cb-length', 'bad-txns-prevout-null', 'bad-txns-inputs-missingorspent',
                       'bad-txns-premature-spend-of-coinbase', 'bad-txns-inputvalues-outofrange',
                       'bad-txns-in-belowout', 'bad-txns-fee-outofrange'], reasons
    assert 'GetSerializeSize(TX_NO_WITNESS(tx)) * WITNESS_SCALE_FACTOR > MAX_BLOCK_WEIGHT' in check
    assert "tx.vin[0].scriptSig.size() < 2 || tx.vin[0].scriptSig.size() > 100" in check
    assert 'coin.IsCoinBase() && nSpendHeight - coin.nHeight < COINBASE_MATURITY' in body
    assert '(int64_t)tx.nLockTime < ((int64_t)tx.nLockTime < LOCKTIME_THRESHOLD ? (int64_t)nBlockHeight : nBlockTime)' in verify
    cons = text('consensus.h')
    n = lambda name: int(one(rf'{name}\s*(?:=\s*|\{{)([0-9\']+)', cons).replace("'", ''))
    money = one(r'MAX_MONEY\{([^}]+)\}', text('amount.h'))
    assert money == "21'000'000 * COIN"
    params = dict(maturity=n('COINBASE_MATURITY'), weight=n('MAX_BLOCK_WEIGHT'), scale=n('WITNESS_SCALE_FACTOR'),
                  max_money=21000000 * 100000000)
    cp = text('chainparams.cpp')
    params['csv'] = int(one(r'consensus\.CSVHeight = (\d+);', cp[cp.index('class CMainParams '):cp.index('class CTestNetParams ')]))
    return reasons, params


# ---- transliterations
def le(b):
    return int.from_bytes(b, 'little', signed=True)


def check_transaction(tx, P, stripped=None):
    """consensus/tx_check.cpp CheckTransaction."""
    if not tx['ins']:
        return 'bad-txns-vin-empty'
    if not tx['outs']:
        return 'bad-txns-vout-empty'
    size = len(T.serialize(tx, False)) if stripped is None else stripped
    if size * P['scale'] > P['weight']:
        return 'bad-txns-oversize'
    total = 0
    for o in tx['outs']:
        v = le(o['value'])
        if v < 0:
            return 'bad-txns-vout-negative'
        if v > P['max_money']:
            return 'bad-txns-vout-toolarge'
        total += v
        if not 0 <= total <= P['max_money']:
            return 'bad-txns-txouttotal-toolarge'
    if len({i['prevout'] for i in tx['ins']}) != len(tx['ins']):
        return 'bad-txns-inputs-duplicate'
    if T.is_coinbase(tx):
        if not 2 <= len(tx['ins'][0]['script']) <= 100:
            return 'bad-cb-length'
    elif any(i['prevout'] == bytes(32) + b'\xff' * 4 for i in tx['ins']):
        return 'bad-txns-prevout-null'
    return 'ok'


def check_tx_inputs(tx, coins, spend_height, P):
    """consensus/tx_verify.cpp CheckTxInputs; coins are (found, value,
    height, coinbase). Returns (reason, fee)."""
    if not all(c[0] for c in coins):
        return 'bad-txns-inputs-missingorspent', 0
    value_in = 0
    for found, value, height, coinbase in coins:
        if coinbase and spend_height - height < P['maturity']:
            return 'bad-txns-premature-spend-of-coinbase', 0
        value_in += value
        if not 0 <= value <= P['max_money'] or not 0 <= value_in <= P['max_money']:
            return 'bad-txns-inputvalues-outofrange', 0
    value_out = sum(le(o['value']) for o in tx['outs'])
    if value_in < value_out:
        return 'bad-txns-in-belowout', 0
    return 'ok', value_in - value_out


def is_final(tx, height, time):
    """tx_verify.cpp IsFinalTx."""
    lock = int.from_bytes(tx['locktime'], 'little')
    if lock == 0 or lock < (height if lock < 500000000 else time):
        return True
    return all(i['sequence'] == b'\xff' * 4 for i in tx['ins'])


def final_in_block(tx, height, mtp, time, P):
    return 'ok' if is_final(tx, height, mtp if height >= P['csv'] else time) else 'bad-txns-nonfinal'


# ---- the transcript
def cases():
    out = []
    for name, valid in (('tx_valid.json', True), ('tx_invalid.json', False)):
        note, in_comment = '', False
        for e in json.loads(text(name)):
            if len(e) == 1:
                if not in_comment:
                    note, in_comment = '', True
                if not e[0].startswith('Note'):
                    note = e[0]
                continue
            in_comment = False
            out.append((e[1], valid, e[2] == 'BADTX', note))
    return out


def transcript():
    reasons, P = rules()
    check_lib(reasons, P)
    tests = text('transaction_tests.cpp')
    assert 'BOOST_CHECK_MESSAGE(test[2].get_str() == "BADTX", strTest);' in tests
    out = ['CheckTransaction on Core\'s vectors:']
    counts = dict(valid=[0, 0], other=[0, 0])
    all_cases = cases()
    for hx, valid, bad, note in all_cases:
        tx = T.unserialize(bytes.fromhex(hx), True)
        code = check_transaction(tx, P)
        assert (code != 'ok') == bad, note
        if bad:
            out.append(f'  BADTX {note} ({len(tx["ins"])} inputs, {len(tx["outs"])} outputs): {code}')
        else:
            k = counts['valid' if valid else 'other']
            k[0] += 1; k[1] += code == 'ok'
    out.append(f'  tx_valid.json: {counts["valid"][1]} of {counts["valid"][0]} pass')
    out.append(f'  tx_invalid.json without BADTX: {counts["other"][1]} of {counts["other"][0]} pass (their scripts fail)')
    out.append(f'  no inputs: {check_transaction(T.unserialize(bytes.fromhex("01000000000000000000"), True), P)}')
    legacy = one(r'TX_LEGACY_OK :: "([0-9a-f]+)";', (REPO / 'coax/lib/bitcoin/tx.coax').read_text())
    out.append(f'  chapter 34\'s one-byte coinbase: {check_transaction(T.unserialize(bytes.fromhex(legacy), True), P)}')
    first = T.unserialize(bytes.fromhex(all_cases[0][0]), True)
    n = len(T.serialize(first, False))
    out.append(f'  tx_valid\'s first case, {n} bytes stripped: {check_transaction(first, P)}')
    for size in (P['weight'] // P['scale'], P['weight'] // P['scale'] + 1):
        out.append(f'  as if {size} bytes: {check_transaction(first, P, size)}')

    out.append('CheckTxInputs, one output of 1000000:')
    first['outs'][0]['value'] = (1000000).to_bytes(8, 'little')
    for spend in (598, 599, 600):
        code, fee = check_tx_inputs(first, [(True, 1500000, 500, True)], spend, P)
        out.append(f'  a coinbase coin from height 500, spent at {spend}: {code} (fee {fee})')
    for v in (999999, 1000000, P['max_money'], P['max_money'] + 1, -1):
        code, fee = check_tx_inputs(first, [(True, v, 500, False)], 501, P)
        out.append(f'  a coin of {v}: {code} (fee {fee})')
    out.append(f'  a coin that is spent: {check_tx_inputs(first, [(False, 0, 0, False)], 501, P)[0]}')
    ninth = T.unserialize(bytes.fromhex(all_cases[9][0]), True)
    each = P['max_money'] // 2 + 1
    k = len(ninth['ins'])
    out.append(f'  {k} coins of {each} each: {check_tx_inputs(ninth, [(True, each, 1, False)] * k, 10, P)[0]}')

    out.append('final in a block (lock time 1500000000, sequence 0xfffffffe):')
    tx = T.unserialize(bytes.fromhex(all_cases[0][0]), True)
    lock = 1500000000
    tx['locktime'] = lock.to_bytes(4, 'little')
    tx['ins'][0]['sequence'] = (0xfffffffe).to_bytes(4, 'little')
    out.append(f'  CSV active from {P["csv"]}')
    for h in (419327, 419328):
        out.append(f'  height {h}, median {lock - 1} and block time {lock + 1}: {final_in_block(tx, h, lock - 1, lock + 1, P)}')
        out.append(f'  height {h}, median {lock + 1} and block time {lock}: {final_in_block(tx, h, lock + 1, lock, P)}')
    tx['ins'][0]['sequence'] = b'\xff' * 4
    out.append(f'  with a final sequence, median and time 0: {final_in_block(tx, 419328, 0, 0, P)}')
    return '\n'.join(out) + '\n'


def check_lib(reasons, P):
    lib = LIB.read_text()
    listed = re.findall(r'"([a-z -]+)"', one(r'TXC_REASONS :: \[17\] string\.\[([^\]]+)\]', lib))
    assert listed[0] == 'ok' and listed[1:15] == reasons and listed[15] == 'bad-txns-nonfinal', listed
    codes = re.findall(r'(?m)^(TXC_[A-Z_]+) :: (\d+);\s*// "([a-z-]+)"', lib)
    assert [(int(c), r) for _, c, r in codes] == list(enumerate(listed))[1:16], codes
    assert int(one(r'(?m)^TXC_COINBASE_MATURITY :: (\d+);', lib)) == P['maturity']
    assert (int(one(r'(?m)^TXC_CB_MIN :: (\d+);', lib)), int(one(r'(?m)^TXC_CB_MAX :: (\d+);', lib))) == (2, 100)
    fees = (REPO / 'coax/lib/bitcoin/fees.coax').read_text()
    assert int(one(r'(?m)^FEE_MAX_MONEY :: (\d+);', fees)) == P['max_money']
    assert int(one(r'(?m)^FEE_MAX_BLOCK_WEIGHT :: (\d+);', fees)) == P['weight']
    assert int(one(r'(?m)^TX_SCALE :: (\d+);', (REPO / 'coax/lib/bitcoin/tx.coax').read_text())) == P['scale']


def main():
    want = transcript()
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'tx-checks fixture: {want.count(chr(10))} lines agree with Python')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
