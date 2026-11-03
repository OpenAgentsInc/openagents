#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Bitcoin Core's messages.py, siphash.py and transliterated blockencodings.cpp decide each case outside Coax)
"""The cross-check of Bitcoin chapter 73. It computes every line the
chapter's example prints, in Python that shares no code with Coax, and
compares the transcript with the fixture:

  python3 -I coax/tests/bitcoin/cmpct.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/cmpct.py --coax coax/bin/coax   # also run the example

Messages are Bitcoin Core's test_framework/messages.py: HeaderAndShortIDs
and P2PHeaderAndShortWitnessIDs for cmpctblock (with siphash256 from the
pinned test_framework/crypto/siphash.py), BlockTransactionsRequest for
getblocktxn and BlockTransactions for blocktxn. Rebuilding is
PartiallyDownloadedBlock's InitData and FillBlock (blockencodings.cpp)
transliterated, with chapter 49's IsBlockMutated from
make_witness_vectors.py. High-bandwidth peers are
MaybeSetPeerAsAnnouncingHeaderAndIDs (net_processing.cpp) transliterated,
and every constant is read from the pinned sources.
"""
import copy
import hashlib
import re
import subprocess
import sys
from io import BytesIO
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import make_cmpct_vectors as V  # noqa: E402
import make_witness_vectors as W  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/compact-blocks.out'
EXAMPLE = 'bitcoin/examples/compact-blocks.coax'
M = V.M
CORE = V.UP / 'bitcoin-core'


def pinned(name):
    return (CORE / name).read_text()


def constants():
    h, cpp, net = pinned('blockencodings.h'), pinned('blockencodings.cpp'), pinned('net_processing.cpp')
    assert 'static constexpr int SHORTTXIDS_LENGTH = 6;' in h
    assert 'MAX_BLOCK_WEIGHT / MIN_SERIALIZABLE_TRANSACTION_WEIGHT' in cpp
    depth = int(re.search(r'static const int MAX_CMPCTBLOCK_DEPTH = (\d+);', net).group(1))
    blocktxn = int(re.search(r'static const int MAX_BLOCKTXN_DEPTH = (\d+);', net).group(1))
    assert 'lNodesAnnouncingHeaderAndIDs.size() > 3' in net
    return depth, blocktxn, 3


MAX_CMPCTBLOCK_DEPTH, MAX_BLOCKTXN_DEPTH, HB_PEERS = constants()
OK, INVALID, FAILED = 'ok', 'invalid', 'failed'
NONE, MEMPOOL, EXTRA, COLLIDED = 'none', 'mempool', 'extra', 'collided'


def short_id(keys, wtxid):
    return M.calculate_shortid(keys[0], keys[1], M.uint256_from_str(wtxid))


# ---- PartiallyDownloadedBlock
class Partial:
    """InitData's result: the slots and where each came from."""
    def __init__(self):
        self.header = None
        self.txn = []
        self.source = []
        self.counts = {}


def init_data(p2p, pool, extra):
    """InitData. p2p is a read P2PHeaderAndShortWitnessIDs; pool is the
    mempool as (wtxid, tx) in scan order; extra is (key, tx)."""
    part = Partial()
    if p2p.header.nBits == 0 or (not p2p.shortids and not p2p.prefilled_txn):
        return INVALID, part
    if len(p2p.shortids) + len(p2p.prefilled_txn) > 4000000 // 40:
        return INVALID, part
    count = len(p2p.shortids) + len(p2p.prefilled_txn)
    part.header = p2p.header
    part.txn = [None] * count
    last = -1
    for i, pre in enumerate(p2p.prefilled_txn):
        if not pre.tx.vin and not pre.tx.vout:
            return INVALID, part
        last += pre.index + 1
        if last > 0xffff or last > len(p2p.shortids) + i:
            return INVALID, part
        part.txn[last] = pre.tx
    ids = {}
    offset = 0
    for i, sid in enumerate(p2p.shortids):
        while part.txn[i + offset] is not None:
            offset += 1
        ids[sid] = i + offset
    if len(ids) != len(p2p.shortids):
        return FAILED, part
    h = M.HeaderAndShortIDs(p2p)
    keys = h.get_siphash_keys()
    source = [NONE] * count
    available = 0
    for wtxid, tx in pool:
        slot = ids.get(short_id(keys, wtxid))
        if slot is not None:
            if source[slot] == NONE:
                part.txn[slot], source[slot] = tx, MEMPOOL
                available += 1
            elif source[slot] != COLLIDED:
                part.txn[slot], source[slot] = None, COLLIDED
                available -= 1
        if available == len(ids):
            break
    for key, tx in extra:
        slot = ids.get(short_id(keys, key))
        if slot is not None:
            if source[slot] == NONE:
                part.txn[slot], source[slot] = tx, EXTRA
                available += 1
            elif source[slot] != COLLIDED and part.txn[slot].wtxid != tx.wtxid:
                part.txn[slot], source[slot] = None, COLLIDED
                available -= 1
        if available == len(ids):
            break
    part.source = source
    part.counts = {'prefilled': len(p2p.prefilled_txn), MEMPOOL: source.count(MEMPOOL), EXTRA: source.count(EXTRA)}
    return OK, part


def missing(part):
    return [i for i, tx in enumerate(part.txn) if tx is None]


def fill_block(part, vtx_missing, segwit):
    """FillBlock: (status, block, IsBlockMutated's reason)."""
    if part.header is None or part.header.nBits == 0:
        return INVALID, None, ''
    block = M.CBlock(part.header)
    block.vtx = []
    k = 0
    for tx in part.txn:
        if tx is None:
            if k >= len(vtx_missing):
                return INVALID, None, ''
            tx = vtx_missing[k]
            k += 1
        block.vtx.append(tx)
    if k != len(vtx_missing):
        return INVALID, None, ''
    reason = W.is_block_mutated(block, segwit)
    return (FAILED if reason else OK), block, reason


# ---- MaybeSetPeerAsAnnouncingHeaderAndIDs
def hb_choose(lst, peer, inbound, provides=True, blocksonly=False):
    if blocksonly or not provides:
        return None, None
    outbound = 0
    for i, p in enumerate(lst):
        if p == peer:
            lst.pop(i)
            lst.append(peer)
            return None, None
        if not inbound[p]:
            outbound += 1
    if inbound[peer] and len(lst) >= HB_PEERS and outbound == 1 and not inbound[lst[0]]:
        lst[0], lst[1] = lst[1], lst[0]
    lst.append(peer)
    off = None
    if len(lst) > HB_PEERS:
        off = lst.pop(0)
    return peer, off


def getdata_reply(height, tip, direct):
    return 'cmpctblock' if direct and height >= tip - MAX_CMPCTBLOCK_DEPTH else 'block'


def getblocktxn_reply(count, have, height, tip):
    if count == 0:
        return 'disconnect'
    if not have:
        return 'nothing'
    return 'blocktxn' if height >= tip - MAX_BLOCKTXN_DEPTH else 'block'


# ---- the transcript
def payload(block, nonce, prefill=(0,)):
    return V.cmpct(block, nonce, prefill)[0]


def read_cmpct(raw):
    p = M.P2PHeaderAndShortWitnessIDs()
    p.deserialize(BytesIO(raw))
    return p


def tx_of(raw):
    t = M.CTransaction()
    t.deserialize(BytesIO(raw))
    return t


def receive(out, raw, block, pool, extra, quiet):
    p2p = read_cmpct(raw)
    status, part = init_data(p2p, pool, extra)
    if status != OK:
        out.append(f'  init: {status}')
        return
    c = part.counts
    out.append(f'  init: ok, {c["prefilled"]} prefilled, {c[MEMPOOL]} from the mempool, {c[EXTRA]} from the extra pool')
    need = missing(part)
    if need:
        out.append('  missing: ' + ' '.join(str(i) for i in need))
    vtx = []
    if need:
        blockhash = M.uint256_from_str(M.hash256(raw[:80]))
        req = M.BlockTransactionsRequest(blockhash)
        req.from_absolute(need)
        reply = M.BlockTransactions(blockhash, [block.vtx[i] for i in req.to_absolute()]).serialize()
        if not quiet:
            out.append(f'  getblocktxn: {req.serialize().hex()}')
            out.append(f'  blocktxn: {len(need)} transactions, {len(reply)} bytes')
        got = M.BlockTransactions()
        got.deserialize(BytesIO(reply))
        vtx = got.transactions
    status, rebuilt, reason = fill_block(part, vtx, True)
    line = f'  fill: {status}'
    if status == FAILED:
        line += f', {reason}'
    if status == OK:
        line += f', the block byte for byte: {str(rebuilt.serialize() == block.serialize()).lower()}'
    out.append(line)


def transcript():
    out = []
    big = V.BIG
    raw, h = V.cmpct(big, V.NONCE_413567)
    out.append(f'block 413,567, nonce {V.NONCE_413567:016x}')
    out.append(f'  SHA-256(header || nonce): {hashlib.sha256(big.serialize()[:80] + V.NONCE_413567.to_bytes(8, "little")).hexdigest()}')
    k0, k1 = h.get_siphash_keys()
    out.append(f'  k0: {k0:016x}, k1: {k1:016x}')
    out.append('  short IDs of transactions 1 to 3: ' + ' '.join(f'{s:012x}' for s in h.shortids[:3]))
    out.append(f'  cmpctblock: 1556 short IDs and the {len(big.vtx[0].serialize())}-byte coinbase, {len(raw)} bytes')

    block = W.MAINNET
    out.append(f'block 514,999, nonce {V.NONCE_514999:016x}')
    raw = payload(block, V.NONCE_514999)
    out.append(f'  cmpctblock: {len(block.vtx) - 1} short IDs and the coinbase, {len(raw)} bytes; the block, {len(block.serialize())} bytes')
    unrelated = V.NON_BLOCK
    pool = [(block.vtx[i].wtxid, block.vtx[i]) for i in range(18, 0, -1) if i not in (4, 9, 15)]
    pool.append((unrelated.wtxid, unrelated))
    extra = [(block.vtx[9].wtxid, block.vtx[9])]
    out.append(f'  mempool: {len(pool)} transactions, 15 of them in the block; extra pool: transaction 9')
    receive(out, raw, block, pool, extra, False)

    out.append('prefilled transactions')
    raw4 = payload(block, V.NONCE_514999, (0, 5, 6, 18))
    p = read_cmpct(raw4)
    out.append('  indexes 0 5 6 18 are sent as ' + ' '.join(str(x.index) for x in p.prefilled_txn)
               + f'; {len(p.shortids)} short IDs, {len(raw4)} bytes')

    out.append('collisions')
    dup = bytearray(raw)
    dup[95:101] = dup[89:95]
    out.append('  one short ID twice in the cmpctblock')
    receive(out, bytes(dup), block, pool, extra, True)
    others = [(block.vtx[j].wtxid, block.vtx[j]) for j in range(1, 19) if j != 4]
    out.append('  transaction 3 in the mempool and another under its wtxid in the extra pool')
    receive(out, raw, block, others, [(block.vtx[3].wtxid, unrelated)], True)
    out.append("  another transaction under transaction 4's wtxid")
    receive(out, raw, block, others, [(block.vtx[4].wtxid, unrelated)], True)
    changed = copy.deepcopy(block.vtx[10])
    item = bytearray(changed.wit.vtxinwit[0].scriptWitness.stack[0])
    item[10] ^= 1
    changed.wit.vtxinwit[0].scriptWitness.stack[0] = bytes(item)
    assert changed.txid == block.vtx[10].txid
    others10 = [(block.vtx[j].wtxid, block.vtx[j]) for j in range(1, 19) if j != 10]
    out.append('  transaction 10 with a witness byte changed, under its wtxid')
    receive(out, raw, block, others10, [(block.vtx[10].wtxid, changed)], True)

    out.append('malformed')
    bad = bytearray(raw)
    bad[89 + 6 * 18 + 1] = 19
    out.append('  a prefilled index past the short IDs')
    receive(out, bytes(bad), block, pool, extra, True)
    out.append(f'  getblocktxn for index 65535: {request_status(V.request(0, [0xffff]))}')
    out.append(f'  getblocktxn with deltas 28672, 36862, 0: {request_status(V.overflow_request(0))}')

    out.append('high-bandwidth peers')
    inbound = {1: False, 2: False, 3: True, 4: True, 5: True}
    lst = []
    for peer in (1, 3, 4, 5, 4, 2):
        on, off = hb_choose(lst, peer, inbound)
        line = f'  peer {peer} ({"inbound" if inbound[peer] else "outbound"}) delivers a block; high-bandwidth: ' + ' '.join(str(x) for x in lst)
        if on is not None:
            line += f'; sendcmpct(1) to {on}'
        if off is not None:
            line += f', sendcmpct(0) to {off}'
        out.append(line)

    out.append('replies, tip at 900000')
    out.append(f'  getdata(MSG_CMPCT_BLOCK) for 899995: {getdata_reply(899995, 900000, True)}, for 899994: {getdata_reply(899994, 900000, True)}')
    out.append(f'  getblocktxn for 899990: {getblocktxn_reply(1, True, 899990, 900000)}, for 899989: '
               f'{getblocktxn_reply(1, True, 899989, 900000)}, with no indexes: {getblocktxn_reply(0, True, 900000, 900000)}')
    return '\n'.join(out) + '\n'


def request_status(raw):
    """DifferenceFormatter::Unser over the deltas: past 0xffff refuses."""
    req = M.BlockTransactionsRequest()
    req.deserialize(BytesIO(raw))
    shift = 0
    for d in req.indexes:
        shift += d
        if shift > 0xffff:
            return 'differential value overflow'
        shift += 1
    return 'ok'


def main():
    want = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'compact-blocks fixture: {want.count(chr(10))} lines agree with Core\'s messages.py, siphash.py and blockencodings.cpp')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
