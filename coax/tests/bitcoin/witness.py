#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Bitcoin Core's messages.py, blocktools.py and transliterated rules decide each case outside Coax)
"""The cross-check of Bitcoin chapter 49. It computes every line the
chapter's example prints, in Python that shares no code with Coax, and
compares the transcript with the fixture:

  python3 -I coax/tests/bitcoin/witness.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/witness.py --coax coax/bin/coax   # also run the example

Blocks are read with Bitcoin Core's test_framework/messages.py, and the
commitment script is blocktools.py's get_witness_script. The verdicts are
make_witness_vectors.py's transliterations of GetWitnessCommitmentIndex,
CheckWitnessMalleation and IsBlockMutated, run again here on blocks read
back from the vectors' bytes.
"""
import subprocess
import sys
from io import BytesIO
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import make_witness_vectors as W  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/witness-commitment.out'
EXAMPLE = 'bitcoin/examples/witness-commitment.coax'
M = W.M


def read(raw):
    b = M.CBlock()
    b.deserialize(BytesIO(raw))
    return b


def shown(digest):
    return digest[::-1].hex()


def where(pos):
    return 'no commitment' if pos == W.NO_COMMITMENT else f'commitment at output {pos}'


def named(name, segwit):
    return f'  {name}' + ('' if segwit else ' (segwit not active)')


def verdict(reason):
    return reason or 'ok'


def transcript():
    out = []
    b = read(W.MAINNET.serialize())
    pos = W.commitment_index(b)
    out.append(f'block 514,999: {len(b.vtx)} transactions')
    out.append('  with witnesses:' + ''.join(f' {n}' for n, tx in enumerate(b.vtx) if W.has_witness(tx)))
    cb = b.vtx[0]
    out.append(f'  coinbase outputs: {len(cb.vout)}, {where(pos)}')
    script = cb.vout[pos].scriptPubKey
    out.append(f'  its script: {script.hex()}')
    stack = cb.wit.vtxinwit[0].scriptWitness.stack
    out.append(f'  coinbase witness: {len(stack)} item of {len(stack[0])} bytes')
    out.append(f'  reserved value: {stack[0].hex()}')
    root = M.ser_uint256(b.calc_witness_merkle_root())
    out.append(f'  witness root: {shown(root)}')
    out.append(f'  SHA-256d(root || reserved): {M.hash256(root + stack[0]).hex()}')
    out.append(f'  check: {verdict(W.check_witness_malleation(b, True))}')
    same = W.BT.get_witness_script(b.calc_witness_merkle_root(), 0) == script
    out.append(f'  the script GenerateCoinbaseCommitment writes is the block\'s: {str(same).lower()}')

    out.append('CheckWitnessMalleation')
    for name, block, segwit in W.CASES:
        x = read(block.serialize())
        out.append(f'{named(name, segwit)}: {where(W.commitment_index(x))}, {verdict(W.check_witness_malleation(x, segwit))}')

    out.append('a witness byte changed')
    before = read(W.CASES[3][1].serialize())
    after = read(W.CASES[6][1].serialize())
    same_ids = [t.txid for t in before.vtx] == [t.txid for t in after.vtx]
    same_hash = M.hash256(before.serialize()[:80]) == M.hash256(after.serialize()[:80])
    out.append(f'  same txids: {str(same_ids).lower()}, same block hash: {str(same_hash).lower()}')
    out.append(f'  wtxid of transaction 1 before: {before.vtx[1].wtxid_hex}')
    out.append(f'  wtxid of transaction 1 after: {after.vtx[1].wtxid_hex}')

    out.append('IsBlockMutated')
    for name, block, segwit in W.MUTATED:
        x = read(block.serialize())
        x.hashMerkleRoot = block.hashMerkleRoot
        reason = W.is_block_mutated(x, segwit)
        out.append(f'{named(name, segwit)}: ' + (f'mutated, {reason}' if reason else 'not mutated'))
    return '\n'.join(out) + '\n'


def main():
    want = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'witness-commitment fixture: {want.count(chr(10))} lines agree with Core\'s messages.py, blocktools.py and rules')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
