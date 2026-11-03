#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes the example's output with Python transliterations of Core, outside Coax)
"""Checks the output of Bitcoin chapter 39's example, timelocks.coax,
against make_locktime_vectors.py's transliteration of Bitcoin Core's
IsFinalTx, GetMedianTimePast, CalculateSequenceLocks and
EvaluateSequenceLocks, and against script_reference.py's VerifyScript.
Neither shares code with Coax.

  python3 -I coax/tests/bitcoin/locktime_examples.py bitcoin/fixtures/timelocks.out

Every row the example copies must be that row of the pinned tx_valid.json
or tx_invalid.json, every verdict it prints must be the reference's, and
its counts of Core rows that reach OP_CHECKLOCKTIMEVERIFY or
OP_CHECKSEQUENCEVERIFY must be the reference's counts."""
import json
from pathlib import Path
import re
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import script_reference as S  # noqa: E402
import tx_reference as T  # noqa: E402

# The transliteration and the seeded cases; importing it writes nothing.
import make_locktime_vectors as L  # noqa: E402

ROOT = HERE.parents[1].parent
UPSTREAM = HERE.parent/'vectors/upstream/bitcoin-core'
EXAMPLE = ROOT/'bitcoin/examples/timelocks.coax'


def read(hex_tx):
    return T.unserialize(bytes.fromhex(hex_tx), True)


def uses_lock(s):
    pc = 0
    while pc < len(s):
        ok, op, _, pc = S.get_op(s, pc)
        if not ok: return False
        if op in (0xb1, 0xb2): return True
    return False


def spend_uses_lock(tx, k, spk):
    i = tx['ins'][k]
    return uses_lock(spk) or uses_lock(i['script']) or any(uses_lock(w) for w in i['witness'])


def final_word(f):
    return 'final' if f else 'not final'


def main():
    out = Path(sys.argv[1]).read_text()
    agreed = 0

    # IsFinalTx.
    tx = read('020000000100000000000000000000000000000000000000000000000000000000000000000000000000feffffff01000000000000000000ff64cd1d')
    lines = [('lock time 499999999 in block 499999999', L.is_final_tx(tx, 499999999, 0)),
             ('lock time 499999999 in block 500000000', L.is_final_tx(tx, 500000000, 0))]
    tx['locktime'] = (500000000).to_bytes(4, 'little')
    lines += [('lock time 500000000, cutoff 500000000', L.is_final_tx(tx, 1000000000, 500000000)),
              ('lock time 500000000, cutoff 500000001', L.is_final_tx(tx, 0, 500000001))]
    tx['ins'][0]['sequence'] = b'\xff' * 4
    lines += [('lock time 500000000, sequence 0xffffffff, cutoff 0', L.is_final_tx(tx, 0, 0))]
    for text, f in lines:
        assert '  %s: %s\n' % (text, final_word(f)) in out, text
        agreed += 1
    assert '  reference cases: %d of %d agree\n' % (len(L.final), len(L.final)) in out

    # The median time past.
    times = [int(v) for v in re.search(r'times := \[11\] int\.\[([^\]]*)\]', EXAMPLE.read_text()).group(1).replace('\n', ' ').split(',')]
    for n in (1, 2, 6, 11):
        median = L.Chain(times[:n][::-1]).median_time_past(n - 1)
        assert '  the first %d: %d\n' % (n, median) in out, n
        agreed += 1
    assert '  reference windows: %d of %d agree\n' % (len(L.mtp_rows), len(L.mtp_rows)) in out

    # BIP-68 encodings, as the BIP describes them.
    for blocks in (10, 144, 65535):
        assert '  %d blocks: 0x%08x\n' % (blocks, blocks) in out
        agreed += 1
    for seconds in (1, 512, 3600, 33553920):
        units = -(-seconds // 512)
        assert '  %d seconds: 0x%08x (%d seconds)\n' % (seconds, L.TYPE | units, units * 512) in out
        agreed += 1
    for q in (0x7fbf0001, 0x7fff0001, 0x80000000, 0xfffffffe):
        if q & L.DISABLE: want = 'no relative lock'
        elif q & L.TYPE: want = '%d seconds' % ((q & L.MASK) << L.GRANULARITY)
        else: want = '%d blocks' % (q & L.MASK)
        assert '  0x%08x: %s\n' % (q, want) in out, hex(q)
        agreed += 1

    # Sequence locks on a coin in block 100, the block before it with
    # median time past 1,700,000,000.
    class Fixed:
        def median_time_past(self, h):
            assert h == 99
            return 1700000000
    for q in (10, 0x00400001, 0x8000000a, 0):
        for version in (2, 1):
            t = L.make_tx(version, [q], 0)
            lock = L.calculate_sequence_locks(t, True, [100], Fixed())
            word = lambda mtp: 'ok' if L.evaluate_sequence_locks(110, mtp, lock) else 'locked'
            line = '  version %d, sequence 0x%08x: last locked height %d, time %d; block 110 with MTP 1700000511: %s, 1700000512: %s\n' % (
                version, q, lock[0], lock[1], word(1700000511), word(1700000512))
            assert line in out, line
            agreed += 1
            if version == 2 and q == 10:
                assert '    block 109: %s\n' % ('ok' if L.evaluate_sequence_locks(109, 1800000000, lock) else 'locked') in out
    assert '  reference cases: %d of %d agree\n' % (len(L.seq_rows), len(L.seq_rows)) in out

    # The copied rows: each is its file's row, and each verdict the reference's.
    text = EXAMPLE.read_text()
    files = {n: json.loads((UPSTREAM/(n + '.json')).read_text()) for n in ('tx_valid', 'tx_invalid')}
    locks = S.F['P2SH'] | S.F['CHECKLOCKTIMEVERIFY'] | S.F['CHECKSEQUENCEVERIFY']
    rows = re.findall(r'Row\.\{file="(\w+)", line=(\d+), what="([^"]+)",\s*tx="([0-9a-f]+)", spk="([0-9a-f]+)"\}', text)
    assert len(rows) == 12
    for name, line, what, hex_tx, spk in rows:
        row = files[name][int(line)]
        assert row[1] == hex_tx and len(row[0]) == 1 and S.parse_script(row[0][0][2]).hex() == spk, (name, line)
        tx = read(hex_tx)
        verdicts = [S.ERRORS[S.verify_script(tx['ins'][0]['script'], bytes.fromhex(spk), [], f, S.Checker(tx, 0, 0))]
                    for f in (S.F['P2SH'], locks)]
        assert '  %s %s, %s: %s / %s\n' % (name, line, what, verdicts[0], verdicts[1]) in out, (name, line)
        agreed += 1

    # script_tests.json rows that reach the opcodes.
    n = same = 0
    for r in S.script_rows():
        if not r['runnable']: continue
        tx = T.unserialize(r['tx'], True)
        if not spend_uses_lock(tx, 0, r['spk']): continue
        n += 1
        same += S.ERRORS[S.run_row(r)] == r['expected']
    assert n == same and '  script_tests.json: %d of %d rows give Core\'s result\n' % (n, n) in out
    agreed += n

    # tx_valid.json and tx_invalid.json flag sets, as chapter 36's vectors
    # list them, run through the reference.
    vectors = (HERE/'script_vectors.coax').read_text()
    txs = re.findall(r'ScriptTx\.\{tx="([0-9a-f]+)", valid=(true|false), first=(\d+), count=(\d+)\}', vectors)
    prevs = re.findall(r'ScriptPrev\.\{spk="([0-9a-f]*)", amount=(\d+)\}', vectors)
    cases = re.findall(r'ScriptTxCase\.\{tx=(\d+), flags=(\d+), valid=(true|false)\}', vectors)
    assert len(txs) == 206 and len(prevs) == 276 and len(cases) == 4532
    counts = {True: [set(), 0], False: [set(), 0]}
    for k, flags, valid in cases:
        hex_tx, tx_valid, first, count = txs[int(k)]
        tx = read(hex_tx)
        spks = [bytes.fromhex(prevs[int(first) + j][0]) for j in range(int(count))]
        if not any(spend_uses_lock(tx, j, spks[j]) for j in range(int(count))): continue
        verdict = all(S.verify_script(tx['ins'][j]['script'], spks[j], tx['ins'][j]['witness'], int(flags),
                                      S.Checker(tx, j, int(prevs[int(first) + j][1]))) == 0 for j in range(int(count)))
        assert verdict == (valid == 'true')
        c = counts[tx_valid == 'true']
        c[0].add(k)
        c[1] += 1
    for name, v in (('tx_valid.json', True), ('tx_invalid.json', False)):
        line = '  %s: %d transactions, %d of %d flag sets give Core\'s verdict\n' % (name, len(counts[v][0]), counts[v][1], counts[v][1])
        assert line in out, line
        agreed += counts[v][1]
    print('%s: %d values agree with the reference' % (sys.argv[1], agreed))


if __name__ == '__main__':
    main()
