#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: rules read from the pinned Bitcoin Core sources and Python transliterations of them decide each case outside Coax)
"""The cross-check of Bitcoin chapter 53. It reads the sigop counts from
the pinned script/script.cpp (CScript::GetSigOpCount),
script/interpreter.cpp (CountWitnessSigOps) and consensus/tx_verify.cpp
(GetLegacySigOpCount, GetP2SHSigOpCount, GetTransactionSigOpCost), the
block checks from validation.cpp (CheckBlock, ContextualCheckBlock's
weight and ConnectBlock's sigop limit) and the limits from
consensus/consensus.h and script/script.h. It checks that
coax/lib/bitcoin/sigops.coax names Core's reasons and constants, runs
transliterations of those functions over Core's sigopcount_tests and
miner_tests cases and mainnet block 514,999, and writes the transcript
the example must print:

  python3 -I coax/tests/bitcoin/sigops.py                       # compare with the fixture
  python3 -I coax/tests/bitcoin/sigops.py --coax coax/bin/coax  # also run the example

Block 514,999's sigop costs are also checked against mempool.space's,
which its own code computes.
"""
import hashlib, json, re, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import tx_reference as T  # noqa: E402
import merkle_reference as M  # noqa: E402
import script_reference as R  # noqa: E402
import txcheck as X  # noqa: E402

REPO = HERE.parents[2]
UP = HERE.parent / 'vectors' / 'upstream'
FIXTURE = REPO / 'bitcoin/fixtures/sigop-limits.out'
EXAMPLE = 'bitcoin/examples/sigop-limits.coax'
LIB = REPO / 'coax/lib/bitcoin/sigops.coax'
BLOCK = '00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097'
PINS = {
    'bitcoin-core/script.cpp': None,
    'bitcoin-core/script.h': None,
    'bitcoin-core/interpreter.cpp': None,
    'bitcoin-core/tx_verify.cpp': '45de31c846d87b202b46f1e314cbd07869c0b3e598cdc7b18342e2fe24de3fc0',
    'bitcoin-core/validation.cpp': '65e88786ccaaf4dd0ebd14e7a0b82d7fbe6b86a8f0b2f9de4cc2db7b983283d2',
    'bitcoin-core/consensus.h': 'aa55254b379a2cc5f1835b506e248f80b4f020120782d2780ce52ff7c8cb827d',
    'bitcoin-core/sigopcount_tests.cpp': '71fb92a1fce40f6c31339dd931cdb3f07fdc4587442e3862d1de107d3c93a225',
    'bitcoin-core/miner_tests.cpp': '2e8ec65462c8b6cae821bd22b95407c6c94a4710edea2f4b4f7d785a27f2c783',
    f'mainnet/{BLOCK}.raw': '7c0d1862284dcd240a63b5bcb75e956c3645ed5d7453799abb721ce06e6b2902',
    f'mainnet/{BLOCK}.spent.json': 'f65a8fa8f6284871947a4a45838a93c20b7254816204f7c670c1db3034ae2e09',
}
P2SH_FLAG, WITNESS_FLAG = 1 << 0, 1 << 11


def data(name):
    b = (UP / name).read_bytes()
    pin = PINS[name] or json.loads((UP.parent / 'manifest.json').read_text())
    if isinstance(pin, dict):
        pin = next(e['sha256'] for e in pin['files'] if e['path'] == 'upstream/' + name)
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
    cons = text('bitcoin-core/consensus.h')
    n = lambda name: int(one(rf'{name}\s*(?:=\s*|\{{)([0-9\']+)', cons).replace("'", ''))
    P = dict(sigops=n('MAX_BLOCK_SIGOPS_COST'), weight=n('MAX_BLOCK_WEIGHT'), scale=n('WITNESS_SCALE_FACTOR'))
    P['keys'] = int(one(r'MAX_PUBKEYS_PER_MULTISIG = (\d+);', text('bitcoin-core/script.h')))
    script = body(text('bitcoin-core/script.cpp'), 'unsigned int CScript::GetSigOpCount(bool fAccurate)')
    assert 'if (fAccurate && lastOpcode >= OP_1 && lastOpcode <= OP_16)' in script and 'n += MAX_PUBKEYS_PER_MULTISIG;' in script
    p2sh = body(text('bitcoin-core/script.cpp'), 'unsigned int CScript::GetSigOpCount(const CScript& scriptSig)')
    assert 'if (opcode > OP_16)\n            return 0;' in p2sh and 'return subscript.GetSigOpCount(true);' in p2sh
    interp = text('bitcoin-core/interpreter.cpp')
    w = body(interp, 'size_t static WitnessSigOps')
    assert 'if (witprogram.size() == WITNESS_V0_KEYHASH_SIZE)\n            return 1;' in w
    assert 'witprogram.size() == WITNESS_V0_SCRIPTHASH_SIZE && witness.stack.size() > 0' in w
    cw = body(interp, 'size_t CountWitnessSigOps')
    assert 'if (scriptPubKey.IsPayToScriptHash() && scriptSig.IsPushOnly())' in cw
    verify = text('bitcoin-core/tx_verify.cpp')
    cost = body(verify, 'int64_t GetTransactionSigOpCost')
    assert 'GetLegacySigOpCount(tx) * WITNESS_SCALE_FACTOR' in cost and 'if (tx.IsCoinBase())\n        return nSigOps;' in cost
    assert 'GetP2SHSigOpCount(tx, inputs) * WITNESS_SCALE_FACTOR' in cost
    val = text('bitcoin-core/validation.cpp')
    check = body(val, 'bool CheckBlock(const CBlock& block')
    reasons = re.findall(r'"(bad-[a-z-]+)"', check)
    assert reasons == ['bad-signet-blksig', 'bad-blk-length', 'bad-cb-missing', 'bad-cb-multiple', 'bad-blk-sigops'], reasons
    assert check.index('CheckMerkleRoot') < check.index('"bad-blk-length"') < check.index('CheckTransaction(*tx')
    assert 'block.vtx.size() * WITNESS_SCALE_FACTOR > MAX_BLOCK_WEIGHT || ::GetSerializeSize(TX_NO_WITNESS(block)) * WITNESS_SCALE_FACTOR > MAX_BLOCK_WEIGHT' in check
    assert 'nSigOps * WITNESS_SCALE_FACTOR > MAX_BLOCK_SIGOPS_COST' in check
    merkle = body(val, 'static bool CheckMerkleRoot')
    assert re.findall(r'"(bad-[a-z-]+)"', merkle) == ['bad-txnmrklroot', 'bad-txns-duplicate']
    ctx = body(val, 'static bool ContextualCheckBlock(')
    assert 'if (GetBlockWeight(block) > MAX_BLOCK_WEIGHT)' in ctx and '"bad-blk-weight"' in ctx
    assert 'if (nSigOpsCost > MAX_BLOCK_SIGOPS_COST) {\n            state.Invalid(BlockValidationResult::BLOCK_CONSENSUS, "bad-blk-sigops"' in val
    tests = text('bitcoin-core/sigopcount_tests.cpp')
    for want in ('s1.GetSigOpCount(true), 3U', 's1.GetSigOpCount(false), 21U', 'p2sh.GetSigOpCount(scriptSig), 3U',
                 '== MAX_PUBKEYS_PER_MULTISIG * WITNESS_SCALE_FACTOR', '== 2 * WITNESS_SCALE_FACTOR'):
        assert want in tests, want
    miner = text('bitcoin-core/miner_tests.cpp')
    assert 'assert(legacy_sigops == 20001);' in miner and 'CScript() << OP_0 << OP_0 << OP_0 << OP_NOP << OP_CHECKMULTISIG << OP_1' in miner
    return ['bad-txnmrklroot', 'bad-txns-duplicate'] + reasons[1:] + ['bad-blk-weight'], P


# ---- transliterations
def count(s, accurate, P):
    """CScript::GetSigOpCount(fAccurate)."""
    n, last, pc = 0, 0xff, 0
    while pc < len(s):
        ok, op, _, pc = R.get_op(s, pc)
        if not ok:
            break
        if op in (0xac, 0xad):
            n += 1
        elif op in (0xae, 0xaf):
            n += last - 0x50 if accurate and 0x51 <= last <= 0x60 else P['keys']
        last = op
    return n


def is_p2sh(s):
    return len(s) == 23 and s[0] == 0xa9 and s[1] == 0x14 and s[22] == 0x87


def p2sh_count(spk, sig, P):
    """CScript::GetSigOpCount(scriptSig)."""
    if not is_p2sh(spk):
        return count(spk, True, P)
    pc, last = 0, b''
    while pc < len(sig):
        ok, op, d, pc = R.get_op(sig, pc)
        if not ok or op > 0x60:
            return 0
        last = d
    return count(last, True, P)


def witness_sigops(sig, spk, witness, flags, P):
    """CountWitnessSigOps and WitnessSigOps."""
    def program(version, prog):
        if version == 0:
            if len(prog) == 20:
                return 1
            if len(prog) == 32 and witness:
                return count(witness[-1], True, P)
        return 0
    if not flags & WITNESS_FLAG:
        return 0
    wp = R.witness_program(spk)
    if wp:
        return program(*wp)
    if is_p2sh(spk) and R.push_only(sig):
        pc, last = 0, b''
        while pc < len(sig):
            _, _, last, pc = R.get_op(sig, pc)
        wp = R.witness_program(last)
        if wp:
            return program(*wp)
    return 0


def legacy(tx, P):
    return sum(count(i['script'], False, P) for i in tx['ins']) + sum(count(o['script'], False, P) for o in tx['outs'])


def tx_cost(tx, spks, flags, P):
    """GetTransactionSigOpCost."""
    n = legacy(tx, P) * P['scale']
    if T.is_coinbase(tx):
        return n
    for i, spk in zip(tx['ins'], spks):
        if flags & P2SH_FLAG and is_p2sh(spk):
            n += p2sh_count(spk, i['script'], P) * P['scale']
        n += witness_sigops(i['script'], spk, i['witness'], flags, P)
    return n


def block_bytes(header, txs):
    return header + T.compact(len(txs)) + b''.join(T.serialize(t, True) for t in txs)


def fixed(header, txs):
    root, _ = M.compute_root([T.txid(t) for t in txs])
    return header[:36] + root + header[68:]


def check_block(raw, reasons, P):
    """CheckBlock after CheckBlockHeader. Returns (reason, legacy count,
    stripped size, weight)."""
    txs = M.block_txs(raw)
    root, mutated = M.compute_root([T.txid(t) for t in txs])
    if root != raw[36:68]:
        return 'bad-txnmrklroot', 0, 0, 0
    if mutated:
        return 'bad-txns-duplicate', 0, 0, 0
    stripped = 80 + len(T.compact(len(txs))) + sum(len(T.serialize(t, False)) for t in txs)
    weight = stripped * (P['scale'] - 1) + len(raw)
    if not txs or len(txs) * P['scale'] > P['weight'] or stripped * P['scale'] > P['weight']:
        return 'bad-blk-length', 0, stripped, weight
    if not T.is_coinbase(txs[0]):
        return 'bad-cb-missing', 0, stripped, weight
    if any(T.is_coinbase(t) for t in txs[1:]):
        return 'bad-cb-multiple', 0, stripped, weight
    for t in txs:
        r = check_transaction(t, P)
        if r != 'ok':
            return r, 0, stripped, weight
    n = sum(legacy(t, P) for t in txs)
    if n * P['scale'] > P['sigops']:
        return 'bad-blk-sigops', n, stripped, weight
    return 'ok', n, stripped, weight


def check_transaction(tx, P):
    """Chapter 52's CheckTransaction, from txcheck.py."""
    return X.check_transaction(tx, X.rules()[1])


def weight_rule(reason, weight, P):
    if reason != 'ok':
        return reason
    return 'bad-blk-weight' if weight > P['weight'] else 'ok'


# ---- building
def tx(prevout, sig, outs, witness=()):
    return dict(version=(1).to_bytes(4, 'little'),
                ins=[dict(prevout=prevout, script=sig, sequence=b'\xff' * 4, witness=list(witness))],
                outs=[dict(value=v.to_bytes(8, 'little', signed=True), script=s) for v, s in outs],
                locktime=bytes(4), segwit=bool(witness))


NULL = bytes(32) + b'\xff' * 4


def cluster(checksig):
    """miner_tests' CreateBigSigOpsCluster, after a coinbase."""
    cb = tx(NULL, b'\x51\x51', [(0, b'\x51')])
    parent = tx(b'\x11' * 32 + bytes(4), bytes([0, 0, 0xac if checksig else 0, 0x51]), [(100000000, b'\x51')] * 50)
    pid = T.txid(parent)
    cms = bytes.fromhex('00000061ae51')
    kids = [tx(pid + k.to_bytes(4, 'little'), b'\x51', [(0, cms)] * 20) for k in range(50)]
    return [cb, parent] + kids


def coinbase_block(header, spk_n, witness_n):
    cb = tx(NULL, b'\x51\x51', [(0, bytes(spk_n))], [bytes(witness_n)] if witness_n >= 0 else ())
    return block_bytes(fixed(header, [cb]), [cb])


def cost_case(sig, witness, spk, flags, coinbase, P):
    t = tx(NULL if coinbase else b'\x22' * 32 + bytes(4), bytes.fromhex(sig), [(1, b'')], [bytes.fromhex(w) for w in witness])
    return tx_cost(t, [bytes.fromhex(spk)], flags, P)


# ---- the transcript
def transcript():
    reasons, P = rules()
    check_lib(reasons, P)
    F = P2SH_FLAG | WITNESS_FLAG
    out = ['Counting signature operations (GetSigOpCount):']
    out.append(f'  OP_CHECKSIG: {count(bytes([0xac]), False, P)}')
    key = '02' + '11' * 32
    multi = '51' + '21' + key + '21' + key + '52af'
    m = bytes.fromhex(multi)
    out.append(f'  1 <key> <key> 2 OP_CHECKMULTISIGVERIFY: legacy {count(m, False, P)}, accurate {count(m, True, P)}')
    core = bytes([0x51, 20]) + bytes(20) + bytes([20]) + bytes(20) + bytes([0x52, 0xae, 0x63, 0xac, 0x68])
    out.append(f'  1 <20 bytes> <20 bytes> 2 OP_CHECKMULTISIG OP_IF OP_CHECKSIG OP_ENDIF: legacy {count(core, False, P)}, accurate {count(core, True, P)}')
    p2sh = bytes.fromhex('a914' + '00' * 20 + '87')
    out.append(f'  that script under P2SH, spent with OP_0 <script>: {p2sh_count(p2sh, bytes([0, len(core)]) + core, P)}')

    out.append("Core's GetTxSigOpCost cases, under P2SH and WITNESS:")
    out.append(f'  spending a bare 1-of-2 multisig: {cost_case("0000", [], multi, F, False, P)}')
    sig = '000047' + multi
    ph = p2sh.hex()
    out.append(f'  the multisig in P2SH: {cost_case(sig, [], ph, F, False, P)} (without the P2SH flag: {cost_case(sig, [], ph, 0, False, P)})')
    wpkh = '0014' + '00' * 20
    out.append(f'  P2WPKH: {cost_case("", ["", ""], wpkh, F, False, P)} (without WITNESS: {cost_case("", ["", ""], wpkh, P2SH_FLAG, False, P)}; '
               f'as version 1: {cost_case("", ["", ""], "5114" + "00" * 20, F, False, P)}; as a coinbase: {cost_case("", ["", ""], wpkh, F, True, P)})')
    out.append(f'  P2WPKH in P2SH: {cost_case("16" + wpkh, ["", ""], ph, F, False, P)}')
    wsh = '0020' + '00' * 32
    out.append(f'  P2WSH of the multisig: {cost_case("", ["", "", multi], wsh, F, False, P)} (without WITNESS: {cost_case("", ["", "", multi], wsh, P2SH_FLAG, False, P)})')
    out.append(f'  P2WSH in P2SH: {cost_case("22" + wsh, ["", "", multi], ph, F, False, P)}')

    raw = data(f'mainnet/{BLOCK}.raw')
    header = raw[:80]
    txs = M.block_txs(raw)
    reason, n, stripped, weight = check_block(raw, reasons, P)
    out.append(f'Mainnet block 514,999, {len(txs)} transactions:')
    out.append(f'  {stripped} bytes without witnesses, {len(raw)} in all, weight {weight}')
    out.append(f'  legacy sigops {n}, cost {n * P["scale"]} of CheckBlock\'s {P["sigops"]}')
    out.append(f'  CheckBlock: {reason}; the weight limit: {weight_rule(reason, weight, P)}')
    spent = json.loads(data(f'mainnet/{BLOCK}.spent.json'))
    costs = [tx_cost(t, [bytes.fromhex(s['scriptpubkey']) for s in r['spent']] if r['spent'] != [None] else [], F, P) for t, r in zip(txs, spent)]
    same = sum(c == r['sigops'] for c, r in zip(costs, spent))
    assert same == len(txs)
    out.append('  cost of each transaction: ' + ' '.join(map(str, costs)))
    out.append(f'  {same} of {len(txs)} agree with mempool.space')
    bare = [tx_cost(t, [bytes.fromhex(s['scriptpubkey']) for s in r['spent']] if r['spent'] != [None] else [], 0, P) for t, r in zip(txs, spent)]
    out.append(f'  ConnectBlock\'s sigop cost: {sum(costs)} of {P["sigops"]}; without P2SH and WITNESS: {sum(bare)}')

    out.append('Blocks that break a rule:')
    run = lambda b: check_block(b, reasons, P)
    out.append(f'  514,999 with a changed Merkle root: {run(header[:40] + bytes([header[40] ^ 1]) + header[41:] + raw[80:])[0]}')
    out.append(f'  with its last transaction twice: {run(block_bytes(header, txs + [txs[-1]]))[0]}')
    rest = txs[1:]
    out.append(f'  without its coinbase: {run(block_bytes(fixed(header, rest), rest))[0]}')
    cb2 = dict(txs[0], ins=[dict(txs[0]['ins'][0])])
    s = bytearray(cb2['ins'][0]['script']); s[3] ^= 1; cb2['ins'][0]['script'] = bytes(s)
    assert txs[0]['segwit']
    more = txs + [cb2]
    out.append(f'  with a second coinbase: {run(block_bytes(fixed(header, more), more))[0]}')
    t1 = dict(txs[1], ins=[txs[1]['ins'][0], dict(txs[1]['ins'][1], prevout=txs[1]['ins'][0]['prevout'])])
    dup = [txs[0], t1] + txs[2:]
    out.append(f'  with transaction 1 spending one outpoint twice: {run(block_bytes(fixed(header, dup), dup))[0]}')
    for checksig in (True, False):
        c = cluster(checksig)
        r = run(block_bytes(fixed(header, c), c))
        out.append(f'  Core\'s sigop cluster, {len(c)} transactions and {r[1]} legacy sigops: {r[0]}')
    probe = coinbase_block(header, 999000, -1)
    spk_n = 999000 + 1000000 - len(probe)
    for d in (0, 1):
        r = run(coinbase_block(header, spk_n + d, -1))
        out.append(f'  a block of {r[2]} bytes without witnesses: {r[0]}')
    probe = coinbase_block(header, 25, 3000000)
    wit = 3000000 + 4000000 - (3 * (len(probe) - 8 - 3000000) + len(probe))
    for d in (0, 1):
        r = run(coinbase_block(header, 25, wit + d))
        out.append(f'  a block of weight {r[3]}, {r[2]} bytes without witnesses: CheckBlock {r[0]}, then {weight_rule(r[0], r[3], P)}')
    return '\n'.join(out) + '\n'


def check_lib(reasons, P):
    lib = LIB.read_text()
    codes = re.findall(r'(?m)^BLOCK_[A-Z_]+ :: (\d+);\s*// "([a-z-]+)"', lib)
    assert [r for _, r in codes] == reasons, codes
    listed = re.findall(r'"([a-z -]+)"', one(r'BLOCK_REASONS :: \[11\] string\.\[([^\]]+)\]', lib))
    for c, r in codes:
        assert listed[int(c)] == r, (c, r)
    assert int(one(r'(?m)^BLOCK_MAX_SIGOPS_COST :: (\d+);', lib)) == P['sigops']
    assert int(one(r'(?m)^SCRIPT_MAX_KEYS :: (\d+);', (REPO / 'coax/lib/bitcoin/script.coax').read_text())) == P['keys']
    assert int(one(r'(?m)^FEE_MAX_BLOCK_WEIGHT :: (\d+);', (REPO / 'coax/lib/bitcoin/fees.coax').read_text())) == P['weight']


def main():
    want = transcript()
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'sigop-limits fixture: {want.count(chr(10))} lines agree with Python')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
