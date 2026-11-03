#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: rules read from the pinned Bitcoin Core sources and Python transliterations of them decide each case outside Coax)
"""The cross-check of Bitcoin chapter 54. It reads GetBlockSubsidy,
ContextualCheckBlock's BIP-34 rule, ConnectBlock's fee total and its
coinbase rule and the BIP-30 exception blocks from the pinned
validation.cpp, CScript's push_int64 from script/script.h, COIN and
MAX_MONEY from consensus/amount.h, the halving intervals and BIP-34
heights from chainparams.cpp, and the expected values of Core's
validation_tests.cpp. It checks that coax/lib/bitcoin/subsidy.coax names
Core's reasons and constants, runs transliterations over the 14 pinned
mainnet coinbases and block 514,999, and writes the transcript the
example must print:

  python3 -I coax/tests/bitcoin/subsidy.py                       # compare with the fixture
  python3 -I coax/tests/bitcoin/subsidy.py --coax coax/bin/coax  # also run the example
"""
import hashlib, json, re, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import tx_reference as T  # noqa: E402
import merkle_reference as M  # noqa: E402

REPO = HERE.parents[2]
UP = HERE.parent / 'vectors' / 'upstream'
FIXTURE = REPO / 'bitcoin/fixtures/subsidy-coinbase.out'
EXAMPLE = 'bitcoin/examples/subsidy-coinbase.coax'
LIB = REPO / 'coax/lib/bitcoin/subsidy.coax'
BLOCK = '00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097'


def data(name):
    b = (UP / name).read_bytes()
    m = json.loads((UP.parent / 'manifest.json').read_text())
    pin = next(e['sha256'] for e in m['files'] if e['path'] == 'upstream/' + name)
    assert hashlib.sha256(b).hexdigest() == pin, name
    return b


def text(name):
    return data(name).decode()


def one(pattern, s):
    found = re.findall(pattern, s)
    assert len(found) == 1, (pattern, found)
    return found[0]


def body(src, start):
    i = src.index(start)
    depth, j = 0, src.index('{', i)
    while True:
        depth += {'{': 1, '}': -1}.get(src[j], 0)
        j += 1
        if depth == 0:
            return src[i:j]


def rules():
    amount = text('bitcoin-core/amount.h')
    coin = int(one(r'CAmount COIN\{([0-9\']+)\};', amount).replace("'", ''))
    P = dict(coin=coin, max_money=int(one(r'MAX_MONEY\{([0-9\']+) \* COIN\};', amount).replace("'", '')) * coin)
    val = text('bitcoin-core/validation.cpp')
    sub = body(val, 'CAmount GetBlockSubsidy(int nHeight')
    assert 'int halvings = nHeight / consensusParams.nSubsidyHalvingInterval;' in sub
    assert 'if (halvings >= 64)\n        return 0;' in sub and 'nSubsidy >>= halvings;' in sub
    P['initial'] = int(one(r'CAmount nSubsidy = (\d+) \* COIN;', sub)) * coin
    ctx = body(val, 'static bool ContextualCheckBlock(')
    assert 'DeploymentActiveAfter(pindexPrev, chainman, Consensus::DEPLOYMENT_HEIGHTINCB)' in ctx
    assert 'CScript expect = CScript() << nHeight;' in ctx and '"bad-cb-height"' in ctx
    assert 'scriptSig.size() < expect.size() ||\n            !std::equal(expect.begin(), expect.end(), block.vtx[0]->vin[0].scriptSig.begin())' in ctx
    assert 'nFees += txfee;\n            if (!MoneyRange(nFees)) {' in val and '"bad-txns-accumulated-fee-outofrange"' in val
    assert 'CAmount blockReward = nFees + GetBlockSubsidy(pindex->nHeight, params.GetConsensus());' in val
    assert 'if (block.vtx[0]->GetValueOut() > blockReward && state.IsValid()) {' in val and '"bad-cb-amount"' in val
    P['bip30'] = {int(h): x for h, x in re.findall(r'block_index\.nHeight==(\d+) && block_index\.GetBlockHash\(\) == uint256\{"([0-9a-f]{64})"\}', val)}
    assert sorted(P['bip30']) == [91842, 91880]
    push = body(text('bitcoin-core/script.h'), 'CScript& push_int64(int64_t n)')
    assert 'if (n == -1 || (n >= 1 && n <= 16))' in push and 'push_back(n + (OP_1 - 1));' in push
    assert 'else if (n == 0)\n        {\n            push_back(OP_0);' in push and '*this << CScriptNum::serialize(n);' in push
    cp = text('bitcoin-core/chainparams.cpp')
    main = body(cp, 'class CMainParams')
    P['halving'] = int(one(r'nSubsidyHalvingInterval = (\d+);', main))
    P['bip34'] = int(one(r'BIP34Height = (\d+);', main))
    P['regtest'] = int(one(r'nSubsidyHalvingInterval = (\d+);', body(cp, 'class CRegTestParams')))
    tests = text('bitcoin-core/validation_tests.cpp')
    assert 'TestBlockSubsidyHalvings(150); // As in regtest' in tests and 'TestBlockSubsidyHalvings(1000); // Just another interval' in tests
    P['supply'] = int(one(r'BOOST_CHECK_EQUAL\(nSum, CAmount\{(\d+)\}\);', tests))
    assert 'for (int nHeight = 0; nHeight < 14000000; nHeight += 1000) {' in tests and 'nSum += nSubsidy * 1000;' in tests
    return P


def subsidy(height, halving, P):
    halvings = height // halving
    return 0 if halvings >= 64 else P['initial'] >> halvings


def total(halving, P):
    return sum(subsidy(i * halving, halving, P) * halving for i in range(64))


def script_num(n):
    # CScriptNum::serialize: little-endian magnitude, sign in the top bit.
    if n == 0:
        return b''
    neg, a, out = n < 0, abs(n), bytearray()
    while a:
        out.append(a & 0xff)
        a >>= 8
    if out[-1] & 0x80:
        out.append(0x80 if neg else 0)
    elif neg:
        out[-1] |= 0x80
    return bytes(out)


def height_script(n):
    if n == -1 or 1 <= n <= 16:
        return bytes([n + 0x50])
    if n == 0:
        return b'\x00'
    s = script_num(n)
    assert len(s) < 0x4c
    return bytes([len(s)]) + s


def starts_with_height(cb, height):
    return cb['ins'][0]['script'].startswith(height_script(height))


def height_check(cb, height, P):
    if height < P['bip34']:
        return 'ok'
    return 'ok' if starts_with_height(cb, height) else 'bad-cb-height'


def claim(cb):
    return sum(int.from_bytes(o['value'], 'little') for o in cb['outs'])


def add_fee(total_, fee, P):
    s = total_ + fee
    return s if 0 <= s <= P['max_money'] else None


def amount_check(cb, height, fees, P):
    return 'bad-cb-amount' if claim(cb) > subsidy(height, P['halving'], P) + fees else 'ok'


def transcript():
    P = rules()
    check_lib(P)
    yes = lambda b: 'yes' if b else 'no'
    h = P['halving']
    out = [f'GetBlockSubsidy on mainnet, a halving every {h} blocks:']
    for i in range(35):
        out.append(f'  halving {i} from height {i * h}: {subsidy(i * h, h, P)}')
    last = max(x for x in range(0, 64 * h, h) if subsidy(x, h, P) > 0) + h - 1
    out.append(f'  the last satoshi at height {last}; the halving at 64: {subsidy(64 * h, h, P)}')
    # Core's subsidy_limit_test sums every 1000th height times 1000.
    core_sum = sum(subsidy(x, h, P) * 1000 for x in range(0, 14000000, 1000))
    assert total(h, P) == core_sum == P['supply']
    out.append(f'  every subsidy there will be: {total(h, P)} satoshis')
    r = P['regtest']
    out.append(f'  regtest, every {r} blocks: {subsidy(149, r, P)} at 149, {subsidy(150, r, P)} at 150, {total(r, P)} in all')
    for interval in (h, 150, 1000):  # Core's TestBlockSubsidyHalvings
        prev = P['initial'] * 2
        for k in range(64):
            assert subsidy(k * interval, interval, P) == prev // 2
            prev = subsidy(k * interval, interval, P)
        assert subsidy(64 * interval, interval, P) == 0

    out.append('The height a coinbase starts with (CScript() << nHeight):')
    for x in (0, 1, 16, 17, 127, 128, 255, 256, 32768, 227931, 514999, 840000):
        out.append(f'  {x}: {height_script(x).hex()}')

    out.append('Pinned mainnet coinbases, each proved in its header by its Merkle branch:')
    proofs = json.loads(data('mainnet/coinbase-proofs.json'))
    ids = []
    for p in proofs:
        header = bytes.fromhex(p['header'])
        cb = T.unserialize(bytes.fromhex(p['coinbase']), True)
        assert T.is_coinbase(cb)
        ids.append(T.txid(cb))
        proved = M.sha256d(header)[::-1].hex() == p['hash'] and \
            M.root_from_branch(ids[-1], [bytes.fromhex(b) for b in p['branch']], 0) == header[36:68]
        height = p['height']
        out.append(f'  {height}: proved {yes(proved)}, starts with its height {yes(starts_with_height(cb, height))}, '
                   f'BIP-34 {height_check(cb, height, P)}, claims {claim(cb)} of subsidy {subsidy(height, h, P)} and fees {p["fees"]}: '
                   f'{amount_check(cb, height, p["fees"], P)}')
    out.append('Coinbases with the same txid (BIP-30):')
    for a, z in ((0, 3), (1, 2)):
        pa, pz = proofs[a], proofs[z]
        exception = P['bip30'].get(pz['height']) == pz['hash']
        out.append(f'  {pa["height"]} and {pz["height"]}: the same txid {yes(ids[a] == ids[z])}; '
                   f'{pz["height"]} is one of Core\'s BIP-30 exceptions: {yes(exception)}')

    out.append('Mainnet block 514,999:')
    raw = data(f'mainnet/{BLOCK}.raw')
    txs = M.block_txs(raw)
    spent = json.loads(data(f'mainnet/{BLOCK}.spent.json'))
    fees = 0
    for t, s in zip(txs[1:], spent[1:]):
        fee = sum(x['value'] for x in s['spent']) - claim(t)
        assert fee == s['fee'] and fee >= 0
        fees = add_fee(fees, fee, P)
    out.append(f'  fees from the outputs its transactions spend: {fees}')
    cb = txs[0]
    out.append(f'  the coinbase claims {claim(cb)} of subsidy {subsidy(514999, h, P)} and fees {fees}: {amount_check(cb, 514999, fees, P)}')
    o0 = dict(cb['outs'][0], value=(int.from_bytes(cb['outs'][0]['value'], 'little') + 1).to_bytes(8, 'little'))
    more = dict(cb, outs=[o0] + cb['outs'][1:])
    out.append(f'  one satoshi more: {amount_check(more, 514999, fees, P)}')
    out.append(f'  its height: {height_check(cb, 514999, P)}')
    s = bytearray(cb['ins'][0]['script']); s[1] ^= 1
    bent = dict(cb, ins=[dict(cb['ins'][0], script=bytes(s))])
    out.append(f'  with the height\'s first byte changed: {height_check(bent, 514999, P)} at 514999, {height_check(bent, 227930, P)} at 227930')
    over = 'bad-txns-accumulated-fee-outofrange' if add_fee(P['max_money'], 1, P) is None else 'ok'
    out.append(f'  fees whose total passes MAX_MONEY: {over}')
    return '\n'.join(out) + '\n'


def check_lib(P):
    lib = LIB.read_text()
    codes = re.findall(r'(?m)^SUBSIDY_[A-Z_]+ :: (\d+);\s*// "([a-z-]+)"', lib)
    assert [r for _, r in codes] == ['bad-cb-height', 'bad-cb-amount', 'bad-txns-accumulated-fee-outofrange'], codes
    listed = re.findall(r'"([a-z -]+)"', one(r'SUBSIDY_REASONS :: \[5\] string\.\[([^\]]+)\]', lib))
    for c, r in codes:
        assert listed[int(c)] == r, (c, r)
    assert int(one(r'(?m)^SUBSIDY_INITIAL :: (\d+) \* CHAIN_COIN;', lib)) * P['coin'] == P['initial']
    assert int(one(r'(?m)^SUBSIDY_MAX_HALVINGS :: (\d+);', lib)) == 64


def main():
    want = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'subsidy-coinbase fixture: {want.count(chr(10))} lines agree with Python')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
