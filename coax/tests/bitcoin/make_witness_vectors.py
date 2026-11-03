#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: builds blocks with Bitcoin Core's blocktools.py and messages.py and decides them with Core's rules transliterated, outside Coax)
"""Writes coax/tests/bitcoin/witness_vectors.coax for Bitcoin chapter 49:

  python3 -I coax/tests/bitcoin/make_witness_vectors.py

The blocks are mainnet block 514,999 (pinned for chapter 46) and blocks
made from it with Bitcoin Core's functional-test framework:
test_framework/messages.py reads and writes them, and
add_witness_commitment from test_framework/blocktools.py (both pinned)
writes the commitment. "The small block" is block 514,999 cut to its
coinbase, one segwit transaction (its 11th) and one legacy transaction
(its 3rd), with the coinbase's commitment and reserved value removed and
then written again by add_witness_commitment. Every other case is the
small block with one change, and its header's Merkle root written again,
so that only the witness rules decide it.

Each case's verdict is decided here by Core's rules transliterated:
GetWitnessCommitmentIndex (consensus/validation.h), CheckWitnessMalleation
and IsBlockMutated (validation.cpp), with merkle_reference.py's
ComputeMerkleRoot for the mutated flag. witness.py, the chapter's
cross-check, prints the example's lines from the same verdicts.
"""
import ast
import copy
import hashlib
import sys
import types
from io import BytesIO
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import core_messages  # noqa: E402
import merkle_reference as MR  # noqa: E402

OUT = HERE / 'witness_vectors.coax'
UP = HERE.parent / 'vectors/upstream'
M = core_messages.load()
B514999 = '00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097'
B100000 = '000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506'
PINS = {
    f'mainnet/{B514999}.raw': '7c0d1862284dcd240a63b5bcb75e956c3645ed5d7453799abb721ce06e6b2902',
    f'mainnet/{B100000}.raw': 'b02256f05ce2fc0d152394b5cd2fa28fe4ab61100844c389581c4cffad88a2b1',
    'bitcoin-core/blocktools.py': '13de0034eb8f41e3435e8f2996ef217221b4450f21f7c24aef1d6e20bc1590df',
    'bitcoin-core/consensus_validation.h': '2875af77e8e0d4db5b210bc716f37713bd0bd8922dde60713e611b33cdf1806a',
}


def pinned(name):
    data = (UP / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == PINS[name], f'{name} differs from its pin'
    return data


def load_blocktools():
    """add_witness_commitment and get_witness_script, read out of
    blocktools.py with WITNESS_COMMITMENT_HEADER and run as written. The
    rest of blocktools.py imports most of Core's test framework, so only
    these are loaded; the names they use come from messages.py, and CScript
    of [OP_RETURN, data] is the one push it writes."""
    tree = ast.parse(pinned('bitcoin-core/blocktools.py').decode())
    want = {'add_witness_commitment', 'get_witness_script', 'WITNESS_COMMITMENT_HEADER'}
    keep = [n for n in tree.body
            if (isinstance(n, ast.FunctionDef) and n.name in want)
            or (isinstance(n, ast.Assign) and isinstance(n.targets[0], ast.Name) and n.targets[0].id in want)]
    assert len(keep) == 3

    def cscript(ops):
        assert ops[0] == 0x6a and len(ops[1]) < 76
        return bytes([0x6a, len(ops[1])]) + ops[1]
    ns = {'hash256': M.hash256, 'ser_uint256': M.ser_uint256, 'CScript': cscript, 'OP_RETURN': 0x6a,
          'CTxInWitness': M.CTxInWitness, 'CTxOut': M.CTxOut}
    exec(compile(ast.Module(body=keep, type_ignores=[]), 'blocktools.py', 'exec'), ns)
    return types.SimpleNamespace(**ns)


BT = load_blocktools()


def constants():
    """NO_WITNESS_COMMITMENT and MINIMUM_WITNESS_COMMITMENT from the pinned header."""
    text = pinned('bitcoin-core/consensus_validation.h').decode()
    assert 'inline constexpr int NO_WITNESS_COMMITMENT{-1};' in text
    assert 'inline constexpr size_t MINIMUM_WITNESS_COMMITMENT{38};' in text
    return -1, 38


NO_COMMITMENT, MINIMUM = constants()
HEADER = bytes([0x6a, 0x24]) + BT.WITNESS_COMMITMENT_HEADER


# ---- Core's rules
def commitment_index(block):
    """GetWitnessCommitmentIndex: the last coinbase output that matches."""
    pos = NO_COMMITMENT
    if block.vtx:
        for o, out in enumerate(block.vtx[0].vout):
            s = out.scriptPubKey
            if len(s) >= MINIMUM and s[:6] == HEADER:
                pos = o
    return pos


def has_witness(tx):
    return not tx.wit.is_null()


def check_witness_malleation(block, expect):
    """CheckWitnessMalleation: '' or Core's reject reason."""
    if expect:
        pos = commitment_index(block)
        if pos != NO_COMMITMENT:
            wit = block.vtx[0].wit.vtxinwit
            stack = wit[0].scriptWitness.stack if wit else []
            if len(stack) != 1 or len(stack[0]) != 32:
                return 'bad-witness-nonce-size'
            root = M.ser_uint256(block.calc_witness_merkle_root())
            if M.hash256(root + stack[0]) != block.vtx[0].vout[pos].scriptPubKey[6:38]:
                return 'bad-witness-merkle-match'
            return ''
    for tx in block.vtx:
        if has_witness(tx):
            return 'unexpected-witness'
    return ''


def is_coinbase(tx):
    return len(tx.vin) == 1 and tx.vin[0].prevout.hash == 0 and tx.vin[0].prevout.n == 0xffffffff


def is_block_mutated(block, check_witness_root):
    """IsBlockMutated: CheckMerkleRoot, the 64-byte rule for a block without
    a coinbase, then CheckWitnessMalleation."""
    root, mutated = MR.compute_root([tx.txid for tx in block.vtx])
    if root != M.ser_uint256(block.hashMerkleRoot):
        return 'bad-txnmrklroot'
    if mutated:
        return 'bad-txns-duplicate'
    if not block.vtx or not is_coinbase(block.vtx[0]):
        return '64-byte transaction' if any(len(tx.serialize_without_witness()) == 64 for tx in block.vtx) else ''
    return check_witness_malleation(block, check_witness_root)


# ---- the blocks
def read(raw):
    b = M.CBlock()
    b.deserialize(BytesIO(raw))
    assert b.serialize() == raw
    return b


MAINNET = read(pinned(f'mainnet/{B514999}.raw'))
OLD = read(pinned(f'mainnet/{B100000}.raw'))


def rehash(b):
    b.hashMerkleRoot = b.calc_merkle_root()
    return b


def bare(b):
    """The coinbase without its commitment and its witness."""
    cb = b.vtx[0]
    cb.vout = [o for o in cb.vout if o.scriptPubKey[:6] != HEADER]
    cb.wit = M.CTxWitness()
    return b


def small(nonce=0):
    b = M.CBlock(MAINNET)
    b.vtx = [copy.deepcopy(MAINNET.vtx[i]) for i in (0, 10, 2)]
    bare(b)
    BT.add_witness_commitment(b, nonce)
    return b


def edit(change):
    b = small()
    change(b)
    return rehash(b)


def stack(b):
    return b.vtx[0].wit.vtxinwit[0].scriptWitness


def commitment(b):
    return b.vtx[0].vout[commitment_index(b)]


def flip_witness(b):
    item = bytearray(b.vtx[1].wit.vtxinwit[0].scriptWitness.stack[0])
    item[10] ^= 1
    b.vtx[1].wit.vtxinwit[0].scriptWitness.stack[0] = bytes(item)


def flip_commitment(b):
    s = bytearray(commitment(b).scriptPubKey)
    s[6] ^= 1
    commitment(b).scriptPubKey = bytes(s)


def wrong(b):
    return M.CTxOut(0, HEADER + bytes(32))


def set_stack(items):
    def change(b):
        stack(b).stack = items
    return change


def no_coinbase_witness(b):
    b.vtx[0].wit = M.CTxWitness()


def append_wrong(b):
    b.vtx[0].vout.append(wrong(b))


def insert_wrong(b):
    b.vtx[0].vout.insert(0, wrong(b))


def extra_byte(b):
    commitment(b).scriptPubKey += b'\x00'


def short_commitment(b):
    o = commitment(b)
    o.scriptPubKey = o.scriptPubKey[:37]


def no_commitment(b):
    bare(b)


def no_witnesses(b):
    bare(b)
    b.vtx = [b.vtx[0], b.vtx[2]]


def repeat_last(b):
    b.vtx.append(copy.deepcopy(b.vtx[-1]))


# (name, block, segwit active, the verdict when the header's root is
# rewritten). The blocks under "mutated" keep their header's root.
CASES = [
    ('block 514,999', MAINNET, True),
    ('block 100,000', OLD, True),
    ('block 100,000', OLD, False),
    ('the small block', small(), True),
    ('the small block', small(), False),
    ('reserved value 7', small(7), True),
    ('a witness byte changed', edit(flip_witness), True),
    ('the commitment changed', edit(flip_commitment), True),
    ('a 31-byte reserved value', edit(set_stack([bytes(31)])), True),
    ('two reserved values', edit(set_stack([bytes(32), bytes(32)])), True),
    ('no coinbase witness', edit(no_coinbase_witness), True),
    ('a wrong commitment after it', edit(append_wrong), True),
    ('a wrong commitment before it', edit(insert_wrong), True),
    ('a 39-byte commitment', edit(extra_byte), True),
    ('a 37-byte commitment', edit(short_commitment), True),
    ('no commitment', edit(no_commitment), True),
    ('no commitment, no witnesses', edit(no_witnesses), True),
    ('no commitment, no witnesses', edit(no_witnesses), False),
]


def mutated_cases():
    base = small()
    root_changed = small()
    root_changed.hashMerkleRoot ^= 1
    witness = small()
    flip_witness(witness)
    repeated = small()
    repeat_last(repeated)
    return [
        ('the small block', base, True),
        ('a witness byte changed', witness, True),
        ('its last transaction repeated', repeated, True),
        ('its header root changed', root_changed, True),
        ('the small block', base, False),
    ]


MUTATED = mutated_cases()


def checks():
    # The small block's commitment is the one Core's blocktools writes, and
    # block 514,999's coinbase commits the same way (reserved value zero).
    assert commitment(MAINNET).scriptPubKey == BT.get_witness_script(MAINNET.calc_witness_merkle_root(), 0)
    assert commitment_index(MAINNET) == 1 and stack(MAINNET).stack == [bytes(32)]
    s = small()
    assert s.vtx[1].txid == MAINNET.vtx[10].txid and has_witness(s.vtx[1]) and not has_witness(s.vtx[2])
    w = edit(flip_witness)
    assert w.hashMerkleRoot == s.hashMerkleRoot and w.vtx[1].wtxid != s.vtx[1].wtxid


def verdicts():
    rows = []
    for name, b, expect in CASES:
        assert is_block_mutated(b, expect) == check_witness_malleation(b, expect), name
        rows.append((name, b.serialize(), expect, check_witness_malleation(b, expect), commitment_index(b)))
    return rows


def mutated_verdicts():
    return [(name, b.serialize(), expect, is_block_mutated(b, expect)) for name, b, expect in MUTATED]


def main():
    checks()
    rows = verdicts()
    more = mutated_verdicts()
    text = [
        '// Generated by make_witness_vectors.py from mainnet blocks 514,999 and',
        '// 100,000 and Bitcoin Core\'s test_framework/blocktools.py and messages.py,',
        '// pinned in coax/tests/vectors/upstream/ (see SOURCES.md there). Test data only.',
        '//',
        '// WT_CASES: a block, whether segwit is active, Core\'s reject reason from',
        '// CheckWitnessMalleation ("" for none) and GetWitnessCommitmentIndex.',
        '// WT_MUTATED: blocks for IsBlockMutated, whose header keeps its root.',
        'WtCase :: struct { name: string; hex: string; segwit: bool; reason: string; commitment: int; }',
        f'WT_CASES :: [{len(rows)}] WtCase.[',
    ]
    for name, raw, expect, reason, pos in rows:
        hexed = 'MK_BLOCK_514999' if raw == MAINNET.serialize() else 'MK_BLOCK_100000' if raw == OLD.serialize() else f'"{raw.hex()}"'
        text.append(f'  WtCase.{{name="{name}", hex={hexed}, segwit={str(expect).lower()}, reason="{reason}", commitment={pos}}},')
    text += ['];', f'WT_MUTATED :: [{len(more)}] WtCase.[']
    for name, raw, expect, reason in more:
        text.append(f'  WtCase.{{name="{name}", hex="{raw.hex()}", segwit={str(expect).lower()}, reason="{reason}", commitment=0}},')
    text += ['];', '']
    OUT.write_text('\n'.join(text))
    print(f'wrote {OUT.name}: {len(rows)} cases, {len(more)} for IsBlockMutated')


if __name__ == '__main__':
    main()
