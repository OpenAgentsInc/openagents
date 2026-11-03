#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: transliterates Bitcoin Core's lock-time rules and decides expected results outside Coax)
"""Writes coax/tests/bitcoin/locktime_vectors.coax for Bitcoin chapter 39
from Bitcoin Core files pinned in coax/tests/vectors/upstream/bitcoin-core/:

  python3 -I coax/tests/bitcoin/make_locktime_vectors.py

Core publishes no vectors for IsFinalTx, the median time past or BIP-68
sequence locks; its unit tests build a chain in memory. This script
transliterates IsFinalTx, CalculateSequenceLocks and EvaluateSequenceLocks
(consensus/tx_verify.cpp) and CBlockIndex::GetMedianTimePast (chain.h),
builds a seeded chain of block times, and records cases with the verdicts
the transliteration decides. It shares no code with Coax.

- LT_FINAL: a transaction, a block height and lock-time cutoff, and whether
  IsFinalTx holds.
- LT_MTP: a window of block times (LT_MTP_TIMES[first..first+count]) and
  their median as GetMedianTimePast computes it.
- LT_SEQ: a transaction, whether LOCKTIME_VERIFY_SEQUENCE is set, each
  input's coin height and the median time past of the block before it
  (LT_SEQ_HEIGHTS and LT_SEQ_TIMES from first), the block's height and its
  previous block's median time past, the lock pair CalculateSequenceLocks
  returns, and EvaluateSequenceLocks's verdict.

Every pinned file's SHA-256 must equal its pin below.
"""
import hashlib
from pathlib import Path
import random
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import tx_reference as T  # noqa: E402

UPSTREAM = HERE.parent/'vectors/upstream/bitcoin-core'
PINS = {
    'tx_verify.cpp': '45de31c846d87b202b46f1e314cbd07869c0b3e598cdc7b18342e2fe24de3fc0',
    'chain.h': 'a1b863764bc6e40ed79a447805843eb7b18a09730eb3a849bf9fe43f17274aba',
}
for name, pin in PINS.items():
    assert hashlib.sha256((UPSTREAM/name).read_bytes()).hexdigest() == pin, name

LOCKTIME_THRESHOLD = 500000000
SEQUENCE_FINAL = 0xffffffff
DISABLE = 1 << 31
TYPE = 1 << 22
MASK = 0x0000ffff
GRANULARITY = 9
SPAN = 11


# ---- the transliteration
def is_final_tx(tx, height, time):
    locktime = int.from_bytes(tx['locktime'], 'little')
    if locktime == 0:
        return True
    if locktime < (height if locktime < LOCKTIME_THRESHOLD else time):
        return True
    return all(int.from_bytes(i['sequence'], 'little') == SEQUENCE_FINAL for i in tx['ins'])


class Chain:
    """Block times by height; pprev of height h is h - 1."""
    def __init__(self, times):
        self.times = times

    def median_time_past(self, h):
        window = [self.times[k] for k in range(h, max(h - SPAN, -1), -1)]
        window.sort()
        return window[len(window) // 2]


def calculate_sequence_locks(tx, enforce, prev_heights, chain):
    min_height, min_time = -1, -1
    version = int.from_bytes(tx['version'], 'little')   # uint32_t
    if not (version >= 2 and enforce):
        return min_height, min_time
    for k, i in enumerate(tx['ins']):
        seq = int.from_bytes(i['sequence'], 'little')
        if seq & DISABLE:
            continue
        coin_height = prev_heights[k]
        if seq & TYPE:
            coin_time = chain.median_time_past(max(coin_height - 1, 0))
            min_time = max(min_time, coin_time + ((seq & MASK) << GRANULARITY) - 1)
        else:
            min_height = max(min_height, coin_height + (seq & MASK) - 1)
    return min_height, min_time


def evaluate_sequence_locks(height, prev_mtp, lock):
    return not (lock[0] >= height or lock[1] >= prev_mtp)


# ---- cases
rng = random.Random(39)


def make_tx(version, sequences, locktime):
    ins = [{'prevout': bytes([k + 1]) * 32 + b'\0\0\0\0', 'script': b'', 'sequence': s.to_bytes(4, 'little'), 'witness': []}
           for k, s in enumerate(sequences)]
    return {'version': version.to_bytes(4, 'little'), 'ins': ins,
            'outs': [{'value': bytes(8), 'script': b''}], 'locktime': locktime.to_bytes(4, 'little')}


def hexof(tx):
    data = T.serialize(tx, False)
    assert T.unserialize(data, True) is not None and len(data) <= 512
    return data.hex()


final = []
T_ = LOCKTIME_THRESHOLD
for locktime, height, time in [
        (0, 0, 0), (1, 0, 0), (1, 1, 0), (1, 2, 0), (499999999, 499999999, 0), (499999999, 500000000, 0),
        (T_, 10**9, T_), (T_, 0, T_ + 1), (T_ + 1, 0, T_ + 1), (0xffffffff, 10**9, 0xffffffff),
        (0xffffffff, 0, 0x100000000), (700000, 699999, 2 * 10**9), (700000, 700001, 0)]:
    for seqs in ([0xfffffffe], [SEQUENCE_FINAL], [SEQUENCE_FINAL, SEQUENCE_FINAL], [SEQUENCE_FINAL, 0], [0, 0]):
        tx = make_tx(2, seqs, locktime)
        final.append((hexof(tx), height, time, is_final_tx(tx, height, time)))
for _ in range(40):
    locktime = rng.choice([rng.randrange(1, 1000000), rng.randrange(T_ - 1000, T_ + 1000), rng.randrange(T_, 1 << 32)])
    near = locktime + rng.randrange(-2, 3)
    height, time = (near, rng.randrange(1 << 32)) if locktime < T_ else (rng.randrange(1000000), near)
    seqs = [rng.choice([SEQUENCE_FINAL, SEQUENCE_FINAL, 0xfffffffe, rng.randrange(1 << 32)]) for _ in range(rng.randrange(1, 4))]
    tx = make_tx(rng.choice([1, 2]), seqs, locktime)
    final.append((hexof(tx), height, time, is_final_tx(tx, height, time)))
assert any(f[3] for f in final) and not all(f[3] for f in final)

# A chain of 200 blocks, its times not always rising, as the rules allow.
times = [1231006505]
for _ in range(199):
    times.append(times[-1] + rng.choice([rng.randrange(-3000, 600), rng.randrange(0, 1800), 600]))
chain = Chain(times)
mtp_rows, mtp_times = [], []
for h in [0, 1, 2, 3, 4, 9, 10, 11, 12] + rng.sample(range(13, 200), 15):
    window = [times[k] for k in range(h, max(h - SPAN, -1), -1)]
    rng.shuffle(window)
    mtp_rows.append((len(mtp_times), len(window), chain.median_time_past(h)))
    mtp_times += window

seq_rows, seq_heights, seq_times = [], [], []


def seq_case(version, sequences, enforce, heights, height):
    tx = make_tx(version, sequences, 0)
    lock = calculate_sequence_locks(tx, enforce, heights, chain)
    prev_mtp = chain.median_time_past(height - 1)
    ok = evaluate_sequence_locks(height, prev_mtp, lock)
    seq_rows.append((hexof(tx), enforce, len(seq_heights), height, prev_mtp, lock[0], lock[1], ok))
    seq_heights.extend(heights)
    seq_times.extend(chain.median_time_past(max(h - 1, 0)) for h in heights)
    return lock, ok


# By height: ready exactly at coin height + blocks.
for blocks in (0, 1, 10):
    for height in (100 + blocks - 1, 100 + blocks, 100 + blocks + 1):
        if height > 100:
            seq_case(2, [blocks], True, [100], height)
# The version, the flag and the disable bit switch the lock off.
for version, enforce, seq in ((1, True, 10), (0, True, 10), (2, False, 10), (2, True, DISABLE | 10),
                              (0xffffffff, True, 10), (3, True, 10), (0x80000000, True, 10)):
    seq_case(version, [seq], enforce, [100], 105)
# The ignored bits: only the type flag and the low 16 bits count.
seq_case(2, [0x7fbf0001], True, [100], 101)
seq_case(2, [0x7fbf0001 | TYPE], True, [100], 101)
# By time, at the boundary the previous block's median time past crosses.
for h in (50, 120, 180):
    coin_mtp = chain.median_time_past(h - 1)
    for units in (1, 2, 4):
        for height in range(h + 1, 200):
            if chain.median_time_past(height - 1) > coin_mtp + (units << GRANULARITY) - 1:
                break
        for at in (height - 1, height):
            if at > h:
                seq_case(2, [TYPE | units], True, [h], at)
# Several inputs: the latest lock of each kind wins.
for _ in range(30):
    n = rng.randrange(1, 6)
    seqs, heights = [], []
    for _ in range(n):
        heights.append(rng.randrange(1, 150))
        seqs.append(rng.choice([rng.randrange(0, 40), TYPE | rng.randrange(0, 20), DISABLE | rng.randrange(1 << 31),
                                SEQUENCE_FINAL, rng.randrange(1 << 32)]))
    seq_case(rng.choice([1, 2, 2, 2]), seqs, rng.random() < 0.85, heights, rng.randrange(max(heights) + 1, 200))
assert any(r[7] for r in seq_rows) and not all(r[7] for r in seq_rows)


# ---- write
def boolean(b):
    return 'true' if b else 'false'


def main():
    """Writes locktime_vectors.coax. locktime_examples.py imports the cases
    without writing."""
    out = ['// Generated by make_locktime_vectors.py from a transliteration of Bitcoin',
           "// Core's IsFinalTx, CalculateSequenceLocks and EvaluateSequenceLocks",
           '// (consensus/tx_verify.cpp) and GetMedianTimePast (chain.h), pinned in',
           '// coax/tests/vectors/upstream/bitcoin-core/ (see SOURCES.md there), over a',
           '// seeded chain of block times. Test data only.',
           '//',
           '// LT_FINAL: a transaction, a block height, the lock-time cutoff, IsFinalTx.',
           '// LT_MTP: block times LT_MTP_TIMES[first..first+count] in shuffled order and',
           '// their median time past.',
           '// LT_SEQ: a transaction, LOCKTIME_VERIFY_SEQUENCE, the first index into',
           "// LT_SEQ_HEIGHTS and LT_SEQ_TIMES (each input's coin height and the median",
           "// time past of the block before it), the block's height, its previous",
           "// block's median time past, CalculateSequenceLocks's pair and",
           "// EvaluateSequenceLocks's verdict.",
           'LtFinal :: struct { tx: string; height: int; time: int; final: bool; }',
           'LT_FINAL :: [%d] LtFinal.[' % len(final)]
    out += ['  LtFinal.{tx="%s", height=%d, time=%d, final=%s},' % (t, h, s, boolean(f)) for t, h, s, f in final]
    out += ['];', 'LtMtp :: struct { first: int; count: int; median: int; }', 'LT_MTP :: [%d] LtMtp.[' % len(mtp_rows)]
    out += ['  LtMtp.{first=%d, count=%d, median=%d},' % r for r in mtp_rows]
    out += ['];', 'LT_MTP_TIMES :: [%d] int.[' % len(mtp_times)]
    out += ['  %s,' % ', '.join(str(v) for v in mtp_times[k:k + 8]) for k in range(0, len(mtp_times), 8)]
    out += ['];', 'LtSeq :: struct { tx: string; enforce: bool; first: int; height: int; prev_mtp: int; lock_height: int; lock_time: int; ok: bool; }',
            'LT_SEQ :: [%d] LtSeq.[' % len(seq_rows)]
    out += ['  LtSeq.{tx="%s", enforce=%s, first=%d, height=%d, prev_mtp=%d, lock_height=%d, lock_time=%d, ok=%s},'
            % (t, boolean(e), f, h, m, lh, lt, boolean(ok)) for t, e, f, h, m, lh, lt, ok in seq_rows]
    out += ['];', 'LT_SEQ_HEIGHTS :: [%d] int.[' % len(seq_heights)]
    out += ['  %s,' % ', '.join(str(v) for v in seq_heights[k:k + 16]) for k in range(0, len(seq_heights), 16)]
    out += ['];', 'LT_SEQ_TIMES :: [%d] int.[' % len(seq_times)]
    out += ['  %s,' % ', '.join(str(v) for v in seq_times[k:k + 8]) for k in range(0, len(seq_times), 8)]
    out += ['];']
    (HERE/'locktime_vectors.coax').write_text('\n'.join(out) + '\n')
    print('locktime_vectors.coax: %d final, %d median, %d sequence-lock cases' % (len(final), len(mtp_rows), len(seq_rows)))


if __name__ == '__main__':
    main()
