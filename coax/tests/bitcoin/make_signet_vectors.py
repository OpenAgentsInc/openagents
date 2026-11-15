#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: chainparams.py's transliteration of signet.cpp decides every block here outside Coax)
"""Signet blocks that reach the parts of CheckSignetBlockSolution the two
published blocks don't: a solution with a witness, checked by a segwit v0
signature; a first transaction that is not coinbase-shaped; a commitment
in output 0; a 64-byte transaction; and solutions that do not read. Each
block is decided by chainparams.py's signet_check (signet.cpp, with
script_reference.py's interpreter), and the verdicts are written next to
the blocks:

  python3 -I coax/tests/bitcoin/make_signet_vectors.py           # compare with the file
  python3 -I coax/tests/bitcoin/make_signet_vectors.py --write   # rewrite it

The key and the nonce are fixed, so the file is the same on every run.
"""
import hashlib, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import chainparams as C  # noqa: E402
import der_reference as D  # noqa: E402
import script_reference as S  # noqa: E402
import tx_reference as T  # noqa: E402

OUT = HERE / 'signet_vectors.coax'
GENESIS = '00000008819873e925422c1ff0f99f7cc9bbb232af63a077a480a3633bee1ef6'
COMMIT = bytes.fromhex('6a24aa21a9ed') + b'\x44' * 32
KEY = int.from_bytes(hashlib.sha256(b'signet vectors key').digest(), 'big') % D.N
NONCE = int.from_bytes(hashlib.sha256(b'signet vectors nonce').digest(), 'big') % D.N


def sha(b): return hashlib.sha256(b).digest()


def tx(ins, outs):
    return dict(version=(2).to_bytes(4, 'little'), ins=[dict(prevout=p, script=b'', sequence=q, witness=[]) for p, q in ins],
                outs=[dict(value=v.to_bytes(8, 'little'), script=s) for v, s in outs], locktime=bytes(4), segwit=False)


# The first transaction: two inputs with real outpoints, so its bytes are
# nothing like to_spend's.
def first(commitment, more=True):
    outs = [(0, commitment)] + ([(5000, b'\x51')] if more else [])
    return tx([(b'\x22' * 32 + bytes(4), b'\xff' * 4), (b'\x33' * 32 + (1).to_bytes(4, 'little'), b'\xfe\xff\xff\xff')], outs)


# 64 bytes stripped: an empty scriptSig and a 4-byte scriptPubKey.
SIXTY_FOUR = tx([(b'\x55' * 32 + bytes(4), b'\xff' * 4)], [(0, bytes.fromhex('6a020000'))])
assert len(T.serialize(SIXTY_FOUR, False)) == 64


def block(txs):
    leaves = [T.txid(t) for t in txs]
    while len(leaves) > 1:
        if len(leaves) % 2: leaves.append(leaves[-1])
        leaves = [C.sha256d(leaves[i] + leaves[i + 1]) for i in range(0, len(leaves), 2)]
    header = ((0x20000000).to_bytes(4, 'little') + b'\x11' * 32 + leaves[0]
              + (1600000000).to_bytes(4, 'little') + (0x1e0377ae).to_bytes(4, 'little') + (7).to_bytes(4, 'little'))
    return header + T.compact(len(txs)) + b''.join(T.serialize(t, True) for t in txs)


def with_solution(solution):
    return COMMIT + S.push_data(C.HEADER + solution)


def solution(script_sig, witness):
    return T.compact(len(script_sig)) + script_sig + T.compact(len(witness)) + b''.join(T.compact(len(w)) + w for w in witness)


def der(r, s):
    def part(v):
        b = v.to_bytes(32, 'big').lstrip(b'\0')
        return b'\0' + b if b[0] & 0x80 else b
    a, b = part(r), part(s)
    return bytes([0x30, 4 + len(a) + len(b), 2, len(a)]) + a + bytes([2, len(b)]) + b


def signed():
    """A P2WSH challenge of <key> OP_CHECKSIG, and a block whose solution
    signs to_sign. The signature commits to to_spend, so to the block's
    signet Merkle root, which leaves the solution out."""
    q = D.mul(KEY, D.G)
    pub = bytes([2 + (q[1] & 1)]) + q[0].to_bytes(32, 'big')
    ws = bytes([33]) + pub + b'\xac'
    challenge = b'\x00\x20' + sha(ws)
    placeholder = first(with_solution(b''))
    txs = [placeholder, SIXTY_FOUR]
    b = block(txs)
    # to_spend and to_sign as signet.cpp builds them.
    cleared, _ = C.fetch_and_clear(with_solution(b'\x00'))
    cb = first(cleared)
    leaves = [T.txid(cb), T.txid(SIXTY_FOUR)]
    root = C.sha256d(leaves[0] + leaves[1])
    data = b[:36] + root + b[68:72]
    to_spend = dict(version=bytes(4), ins=[dict(prevout=bytes(32) + b'\xff' * 4, script=b'\x00' + S.push_data(data),
                    sequence=bytes(4), witness=[])], outs=[dict(value=bytes(8), script=challenge)], locktime=bytes(4), segwit=False)
    to_sign = dict(version=bytes(4), ins=[dict(prevout=T.txid(to_spend) + bytes(4), script=b'', sequence=bytes(4),
                   witness=[b'', ws])], outs=[dict(value=bytes(8), script=b'\x6a')], locktime=bytes(4), segwit=True)
    z = int.from_bytes(S.sighash_v0(ws, to_sign, 0, 1, 0), 'big') % D.N
    r = D.mul(NONCE, D.G)[0] % D.N
    s = pow(NONCE, -1, D.N) * (z + r * KEY) % D.N
    s = min(s, D.N - s)
    sig = der(r, s) + b'\x01'
    blk = block([first(with_solution(solution(b'', [sig, ws]))), SIXTY_FOUR])
    return challenge, blk


# P2WSH of OP_DROP OP_TRUE: a witness of one item and the script passes.
DROP_TRUE = b'\x75\x51'


def op_true_wsh():
    return b'\x00\x20' + sha(DROP_TRUE)


def vectors():
    rows = []
    challenge, blk = signed()
    rows.append(('SIGNET_SIGNED_CHALLENGE', challenge, None))
    rows.append(('SIGNET_SIGNED', blk, C.signet_check(blk, challenge, GENESIS)))
    # The same block with one bit of the signature changed.
    bad = bytearray(blk); bad[blk.index(b'\x30\x44') + 10] ^= 1
    rows.append(('SIGNET_SIGNED_BAD', bytes(bad), C.signet_check(bytes(bad), challenge, GENESIS)))
    wsh = op_true_wsh()
    rows.append(('SIGNET_WSH_TRUE', wsh, None))
    two = block([first(with_solution(solution(b'', [b'\x01', DROP_TRUE]))), SIXTY_FOUR])
    rows.append(('SIGNET_TWO_ITEMS', two, C.signet_check(two, wsh, GENESIS)))
    for name, sol in [('SIGNET_ITEM_SHORT', bytes.fromhex('00010551')), ('SIGNET_TRAILING', bytes.fromhex('0000ff')),
                      ('SIGNET_EMPTY_ITEM', bytes.fromhex('000100')), ('SIGNET_SIG_LONG', bytes.fromhex('0251'))]:
        blk = block([first(with_solution(sol)), SIXTY_FOUR])
        rows.append((name, blk, C.signet_check(blk, b'\x51', GENESIS)))
    return rows


def render():
    out = ['// Written by coax/tests/bitcoin/make_signet_vectors.py; each verdict is',
           '// what chainparams.py\'s transliteration of signet.cpp decides.']
    for name, data, verdict in vectors():
        if verdict is not None:
            out.append(f'// {name}: {verdict[0]}' + (f' ({verdict[1]})' if verdict[1] else ''))
        out.append(f'{name} :: "{data.hex()}";')
    return '\n'.join(out) + '\n'


def main():
    text = render()
    if '--write' in sys.argv:
        OUT.write_text(text)
        print(f'wrote {OUT.name}')
        return
    assert OUT.read_text() == text, f'{OUT.name} differs: run with --write'
    print(f'{OUT.name} agrees')


if __name__ == '__main__':
    main()
