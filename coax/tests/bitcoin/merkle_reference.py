#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a Python transliteration of Bitcoin Core's Merkle code, outside Coax)
"""Bitcoin Core's Merkle trees in Python, for Bitcoin chapter 46.

A transliteration of the pinned Bitcoin Core sources in
coax/tests/vectors/upstream/bitcoin-core/ (revision ddea9196):

- consensus/merkle.cpp: ComputeMerkleRoot with its mutated flag,
  BlockWitnessMerkleRoot, and MerkleComputation, the constant-space path
  calculator behind TransactionMerklePath;
- test/merkle_tests.cpp: ComputeMerkleRootFromBranch, and the older
  BlockBuildMerkleTree and BlockGetMerkleBranch the tests compare against;
- validation.cpp: CheckMerkleRoot's two refusals, in Core's order.

It shares no code with Coax. Transactions inside a block are read with
tx_reference.py (chapter 34's transliteration of Core's reader). Result
codes are the ones coax/lib/bitcoin/merkle.coax uses, defined here
independently.
"""
import hashlib

import tx_reference as R

OK, EMPTY, MUTATED, MISMATCH, CAPACITY, BAD_INDEX, BAD_DEPTH, TX_64, BAD_BLOCK = range(9)
ZERO = bytes(32)


def sha256d(b):
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def compute_root(hashes):
    """ComputeMerkleRoot: (root, mutated). Every hash is 32 bytes, hash order."""
    hashes = list(hashes)
    mutation = False
    while len(hashes) > 1:
        for pos in range(0, len(hashes) - 1, 2):
            if hashes[pos] == hashes[pos + 1]:
                mutation = True
        if len(hashes) & 1:
            hashes.append(hashes[-1])
        hashes = [sha256d(hashes[2 * k] + hashes[2 * k + 1]) for k in range(len(hashes) // 2)]
    if not hashes:
        return ZERO, mutation
    return hashes[0], mutation


def witness_root(wtxids):
    """BlockWitnessMerkleRoot: the coinbase's wtxid is taken as zero."""
    if not wtxids:
        return ZERO
    return compute_root([ZERO] + list(wtxids[1:]))[0]


def merkle_path(leaves, leaf_pos):
    """MerkleComputation: the branch for leaf_pos, deepest first."""
    path = []
    if not leaves:
        return path
    count = 0
    inner = [ZERO] * 32
    matchlevel = -1
    while count < len(leaves):
        h = leaves[count]
        matchh = count == leaf_pos
        count += 1
        level = 0
        while not count & (1 << level):
            if matchh:
                path.append(inner[level])
            elif matchlevel == level:
                path.append(h)
                matchh = True
            h = sha256d(inner[level] + h)
            level += 1
        inner[level] = h
        if matchh:
            matchlevel = level
    level = 0
    while not count & (1 << level):
        level += 1
    h = inner[level]
    matchh = matchlevel == level
    while count != (1 << level):
        if matchh:
            path.append(h)
        h = sha256d(h + h)
        count += 1 << level
        level += 1
        while not count & (1 << level):
            if matchh:
                path.append(inner[level])
            elif matchlevel == level:
                path.append(h)
                matchh = True
            h = sha256d(inner[level] + h)
            level += 1
    return path


def root_from_branch(leaf, branch, index):
    """merkle_tests.cpp's ComputeMerkleRootFromBranch."""
    h = leaf
    for sibling in branch:
        h = sha256d(sibling + h) if index & 1 else sha256d(h + sibling)
        index >>= 1
    return h


def old_tree(leaves):
    """merkle_tests.cpp's BlockBuildMerkleTree: (root, mutated, tree). Its
    mutated flag looks only at the last pair of each level."""
    tree = list(leaves)
    j, mutated, size = 0, False, len(leaves)
    while size > 1:
        for i in range(0, size, 2):
            i2 = min(i + 1, size - 1)
            if i2 == i + 1 and i2 + 1 == size and tree[j + i] == tree[j + i2]:
                mutated = True
            tree.append(sha256d(tree[j + i] + tree[j + i2]))
        j += size
        size = (size + 1) // 2
    return (tree[-1] if tree else ZERO), mutated, tree


def old_branch(count, tree, index):
    """merkle_tests.cpp's BlockGetMerkleBranch."""
    branch, j, size = [], 0, count
    while size > 1:
        i = min(index ^ 1, size - 1)
        branch.append(tree[j + i])
        index >>= 1
        j += size
        size = (size + 1) // 2
    return branch


def depth(count):
    d = 0
    while count > 1:
        count = (count + 1) // 2
        d += 1
    return d


def check(leaves, header_root):
    """CheckMerkleRoot: "bad-txnmrklroot" first, then "bad-txns-duplicate".
    An empty list whose header root is zero passes here; CheckBlock then
    refuses it as "bad-blk-length", which this chapter reports as EMPTY."""
    root, mutated = compute_root(leaves)
    if root != header_root:
        return MISMATCH
    if mutated:
        return MUTATED
    if not leaves:
        return EMPTY
    return OK


def verify(leaf, branch, index, count, root):
    """A Merkle proof against a tree of count leaves."""
    if count < 1 or index < 0 or index >= count:
        return BAD_INDEX
    if len(branch) != depth(count):
        return BAD_DEPTH
    return OK if root_from_branch(leaf, branch, index) == root else MISMATCH


def read_tx(r, room=1 << 62):
    """UnserializeTransaction at the reader's position, with witnesses, as
    a block holds it: (tx, start, end). room is the most inputs, outputs or
    witness items the caller's arrays hold (Coax's TX_CAPACITY)."""
    start = r.at
    version = r.take(4)
    flags = 0
    ins = R.read_inputs(r, room)
    outs = []
    if len(ins) == 0:
        flags = r.take(1)[0]
        if flags != 0:
            ins = R.read_inputs(r, room)
            outs = R.read_outputs(r, room)
    else:
        outs = R.read_outputs(r, room)
    segwit = False
    if flags & 1:
        flags ^= 1
        witness = r.at
        items = 0
        for i in ins:
            for _ in range(r.compact()):
                if items == room:
                    raise R.Refused(R.CAPACITY, r.at)
                i["witness"].append(r.bytes())
                items += 1
        if not any(i["witness"] for i in ins):
            raise R.Refused(R.NO_WITNESS, witness)
        segwit = True
    if flags:
        raise R.Refused(R.UNKNOWN_FLAG, start + 5)
    locktime = r.take(4)
    return dict(version=version, ins=ins, outs=outs, locktime=locktime, segwit=segwit), start, r.at


def block_txs(raw):
    """The transactions of a serialized block: a list of transaction dicts.
    The block must end after the last one."""
    r = R.Reader(raw, None)
    r.take(80)
    n = r.compact()
    txs = [read_tx(r)[0] for _ in range(n)]
    assert r.at == len(raw), "trailing bytes after the last transaction"
    return txs


def block_ids(raw):
    txs = block_txs(raw)
    return [R.txid(t) for t in txs], [R.wtxid(t) for t in txs], txs


def read_block(raw, bip54=False, leaves=1 << 62, room=1 << 62):
    """What merkle_block_ids_vartime reports for a block: code, at,
    tx_code, count, first64, and the txids and wtxids read. leaves is the
    room for IDs, room the room for inputs, outputs and witness items."""
    k = dict(code=OK, at=0, tx_code=R.OK, count=0, first64=-1, txids=[], wtxids=[])
    if len(raw) < 80:
        return dict(k, code=BAD_BLOCK, tx_code=R.TRUNCATED)
    r = R.Reader(raw, None)
    r.at = 80
    try:
        n = r.compact()
    except R.Refused as e:
        return dict(k, code=BAD_BLOCK, at=e.at, tx_code=e.code)
    if n > leaves:
        return dict(k, code=CAPACITY, at=80)
    while k["count"] < n:
        try:
            tx, start, end = read_tx(r, room)
        except R.Refused as e:
            return dict(k, code=CAPACITY if e.code == R.CAPACITY else BAD_BLOCK, at=e.at, tx_code=e.code)
        if len(R.serialize(tx, False)) == 64:
            if bip54:
                return dict(k, code=TX_64, at=start)
            if k["first64"] < 0:
                k["first64"] = k["count"]
        k["txids"].append(R.txid(tx))
        k["wtxids"].append(R.wtxid(tx))
        k["count"] += 1
    if r.at != len(raw):
        k.update(code=BAD_BLOCK, tx_code=R.TRAILING, at=r.at)
    return k
