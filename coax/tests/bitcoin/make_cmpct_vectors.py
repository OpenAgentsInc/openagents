#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: builds and serializes compact blocks with Bitcoin Core's messages.py and siphash.py, outside Coax)
"""Writes coax/tests/bitcoin/cmpct_vectors.coax for Bitcoin chapter 73:

  python3 -I coax/tests/bitcoin/make_cmpct_vectors.py

- Block 413,567 (Bitcoin Core's src/bench/data/block413567.raw, pinned for
  chapter 46): its coinbase, and the cmpctblock Core's functional-test
  framework builds for it with one nonce. messages.py's HeaderAndShortIDs
  computes the SipHash keys and the short IDs, with siphash256 from the
  pinned test_framework/crypto/siphash.py, and P2PHeaderAndShortWitnessIDs
  writes the payload. Its SHA-256 and first short IDs are kept, not its
  9,000-odd bytes.
- The blocks of Core's src/test/blockencodings_tests.cpp, built as
  BuildBlockTestCase and BuildTransactionTestCase build them. Core draws
  their previous-block hash and outpoints from FastRandomContext(42); here
  each draw is SHA-256 of "blockencodings_tests" and a counter, in the same
  order. The tests' outcomes depend only on which transactions are where,
  not on those values. Each block's nonce is ground until the header meets
  regtest's target, 0x207fffff, as the test does.
- Payloads written by messages.py for the chapter's checks: the test
  block's cmpctblock, a getblocktxn for indexes 0, 1, 3 and 4 (Core's
  TransactionsRequestSerializationTest), one for 0xffff, the raw deltas of
  TransactionsRequestDeserializationOverflowTest, and a blocktxn.
"""
import hashlib
import sys
from io import BytesIO
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import core_messages  # noqa: E402

OUT = HERE / 'cmpct_vectors.coax'
UP = HERE.parent / 'vectors/upstream'
M = core_messages.load()
PINS = {
    'bitcoin-core/block413567.raw': '71964cee18c58675784846d498944b35daa41e36b6f65a7e8feb291def924cce',
    'bitcoin-core/blockencodings_tests.cpp': None,
}
NONCE_413567 = 0x0b1e55ed413567
NONCE_514999 = 0x0b1e55ed514999
NONCE_TEST = 0x0b1e55ed000042
REGTEST_BITS = 0x207fffff


def pinned(name):
    data = (UP / name).read_bytes()
    if PINS[name]:
        assert hashlib.sha256(data).hexdigest() == PINS[name], f'{name} differs from its pin'
    return data


def read_block(raw):
    b = M.CBlock()
    b.deserialize(BytesIO(raw))
    assert b.serialize() == raw
    return b


def cmpct(block, nonce, prefill=(0,)):
    """The cmpctblock payload messages.py writes for block (version 2:
    wtxids, witnesses), and its HeaderAndShortIDs."""
    h = M.HeaderAndShortIDs()
    h.initialize_from_block(block, nonce=nonce, prefill_list=list(prefill), use_witness=True)
    p = h.to_p2p()
    return p.serialize(), h


# ---- Core's test blocks
class Draws:
    """Stand-ins for FastRandomContext(uint256{42})'s rand256, in order."""
    def __init__(self):
        self.n = 0

    def rand256(self):
        self.n += 1
        return int.from_bytes(hashlib.sha256(b'blockencodings_tests %d' % self.n).digest(), 'little')


def test_tx():
    """BuildTransactionTestCase: one input with a 10-byte scriptSig and a
    null outpoint, one output of 42 satoshis, version 2."""
    tx = M.CTransaction()
    tx.version = 2
    tx.vin = [M.CTxIn(M.COutPoint(0, 0xffffffff), bytes(10), 0xffffffff)]
    tx.vout = [M.CTxOut(42, b'')]
    return tx


def copy_tx(tx):
    return M.CTransaction(tx)


def solve(block):
    """Grind nNonce until the header meets the target, as the test does."""
    block.nNonce = 0
    target = M.uint256_from_compact(block.nBits)
    while M.uint256_from_str(M.hash256(block.serialize()[:80])) > target:
        block.nNonce += 1


def test_block(draws):
    """BuildBlockTestCase: three transactions; the second spends one
    outpoint, the third ten."""
    block = M.CBlock()
    tx = test_tx()
    block.vtx = [copy_tx(tx)]
    block.nVersion = 42
    block.hashPrevBlock = draws.rand256()
    block.nBits = REGTEST_BITS
    tx.vin[0].prevout = M.COutPoint(draws.rand256(), 0)
    block.vtx.append(copy_tx(tx))
    tx.vin += [M.CTxIn(M.COutPoint(0, 0xffffffff), b'', 0xffffffff) for _ in range(9)]
    for i in range(10):
        tx.vin[i].prevout = M.COutPoint(draws.rand256(), 0)
    block.vtx.append(copy_tx(tx))
    block.hashMerkleRoot = block.calc_merkle_root()
    solve(block)
    return block


def extra_case():
    """ReceiveWithExtraTransactions: a transaction not in the block, then
    the test block with a fourth transaction (only its outpoint's hash set,
    so its index stays 0xffffffff)."""
    draws = Draws()
    mtx = test_tx()
    mtx.vin[0].prevout = M.COutPoint(draws.rand256(), 0)
    non_block = copy_tx(mtx)
    block = test_block(draws)
    mtx = test_tx()
    mtx.vin[0].prevout = M.COutPoint(draws.rand256(), 0xffffffff)
    block.vtx.append(copy_tx(mtx))
    block.hashMerkleRoot = block.calc_merkle_root()
    solve(block)
    return non_block, block


def empty_block():
    """EmptyBlockRoundTripTest: the coinbase alone."""
    draws = Draws()
    block = M.CBlock()
    block.vtx = [test_tx()]
    block.nVersion = 42
    block.hashPrevBlock = draws.rand256()
    block.nBits = REGTEST_BITS
    block.hashMerkleRoot = block.calc_merkle_root()
    solve(block)
    return block


TEST = test_block(Draws())
NON_BLOCK, TEST4 = extra_case()
EMPTY = empty_block()
BIG = read_block(pinned('bitcoin-core/block413567.raw'))


def request(blockhash, absolute):
    r = M.BlockTransactionsRequest(blockhash)
    r.from_absolute(absolute)
    return r.serialize()


def overflow_request(blockhash):
    """The raw deltas of TransactionsRequestDeserializationOverflowTest."""
    deltas = [0x7000, 0x10000 - 0x7000 - 2, 0]
    return M.ser_uint256(blockhash) + M.ser_compact_size(len(deltas)) + b''.join(M.ser_compact_size(d) for d in deltas)


def vectors():
    big, hb = cmpct(BIG, NONCE_413567)
    assert len(BIG.vtx) == 1557 and len(hb.shortids) == 1556
    k0, k1 = hb.get_siphash_keys()
    test_payload, _ = cmpct(TEST, NONCE_TEST)
    hash_test = M.uint256_from_str(M.hash256(TEST.serialize()[:80]))
    hash_req = Draws().rand256()
    blocktxn = M.BlockTransactions(hash_test, [TEST.vtx[1]]).serialize()
    return {
        'big': big, 'k0': k0, 'k1': k1, 'short': hb.shortids[:3],
        'coinbase': BIG.vtx[0].serialize(),
        'test_payload': test_payload,
        'request': request(hash_req, [0, 1, 3, 4]),
        'request_max': request(hash_req, [0xffff]),
        'request_overflow': overflow_request(hash_req),
        'blocktxn': blocktxn,
    }


def main():
    v = vectors()
    hexes = [
        ('CB_COINBASE_413567', v['coinbase'].hex(), "Block 413,567's coinbase (its first transaction)."),
        ('CB_SHA_413567', hashlib.sha256(v['big']).hexdigest(),
         f"SHA-256 of the {len(v['big'])}-byte cmpctblock payload messages.py writes for block 413,567, coinbase prefilled."),
        ('CB_TEST_BLOCK', TEST.serialize().hex(), "BuildBlockTestCase's block: three transactions."),
        ('CB_TEST_BLOCK4', TEST4.serialize().hex(), 'ReceiveWithExtraTransactions\' block: four transactions.'),
        ('CB_NON_BLOCK_TX', NON_BLOCK.serialize().hex(), 'ReceiveWithExtraTransactions\' transaction outside the block.'),
        ('CB_EMPTY_BLOCK', EMPTY.serialize().hex(), "EmptyBlockRoundTripTest's block: the coinbase alone."),
        ('CB_TEST_CMPCT', v['test_payload'].hex(), 'The cmpctblock messages.py writes for CB_TEST_BLOCK with CB_NONCE_TEST.'),
        ('CB_REQUEST', v['request'].hex(), 'A getblocktxn for indexes 0, 1, 3 and 4 (messages.py).'),
        ('CB_REQUEST_MAX', v['request_max'].hex(), 'A getblocktxn for index 0xffff (messages.py).'),
        ('CB_REQUEST_OVERFLOW', v['request_overflow'].hex(), 'Deltas 0x7000, 0x8ffe and 0: past 0xffff at the third.'),
        ('CB_BLOCKTXN', v['blocktxn'].hex(), "A blocktxn with CB_TEST_BLOCK's second transaction (messages.py)."),
    ]
    text = [
        '// Generated by make_cmpct_vectors.py from Bitcoin Core\'s block413567.raw,',
        '// test_framework/messages.py and test_framework/crypto/siphash.py, and the blocks',
        '// of src/test/blockencodings_tests.cpp, pinned in coax/tests/vectors/upstream/',
        '// (see SOURCES.md there). Test data only.',
        '',
    ]
    for name, value, note in hexes:
        text += [f'// {note}', f'{name} :: "{value}";']
    text += [
        f'CB_NONCE_413567 :: cast(u64) 0x{NONCE_413567:x};',
        f'CB_NONCE_514999 :: cast(u64) 0x{NONCE_514999:x};',
        f'CB_NONCE_TEST :: cast(u64) 0x{NONCE_TEST:x};',
        '// The SipHash keys for block 413,567 and CB_NONCE_413567, and its first short IDs.',
        f'CB_K0_413567 :: cast(u64) 0x{v["k0"]:x};',
        f'CB_K1_413567 :: cast(u64) 0x{v["k1"]:x};',
        f'CB_SHORT_413567 :: [3] u64.[{", ".join(f"0x{s:x}" for s in v["short"])}];',
        f'CB_SIZE_413567 :: {len(v["big"])};',
        '',
    ]
    OUT.write_text('\n'.join(text))
    print(f'wrote {OUT.name}: block 413,567\'s cmpctblock is {len(v["big"])} bytes')


if __name__ == '__main__':
    main()
