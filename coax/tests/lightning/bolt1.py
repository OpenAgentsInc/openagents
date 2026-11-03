#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BOLT 1's text and a separate Python reader decide each case outside Coax)
"""The cross-check of Bitcoin chapter 104. It computes every line the
chapter's example prints and compares the transcript with the fixture:

  python3 -I coax/tests/lightning/bolt1.py                      # compare with the fixture
  python3 -I coax/tests/lightning/bolt1.py --coax coax/bin/coax   # also run the example

BigSize, TLV streams, the test namespaces, init, ping, pong and error are
read by this file's own Python, written from BOLT 1's requirements and
sharing no code with Coax. Its verdicts must also agree with the expected
outcome of every vector in the pinned BOLT 1, and the feature names come
from the pinned BOLT 9's table.
"""
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bolts  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/lightning-messages.out'
EXAMPLE = 'bitcoin/examples/lightning-messages.coax'
P = 2**256 - 2**32 - 977
MAINNET = '6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000'


class Refused(Exception):
    pass


def bigsize(b, at):
    if at >= len(b):
        raise Refused('truncated')
    first = b[at]
    if first < 0xfd:
        return first, at + 1
    width = {0xfd: 2, 0xfe: 4, 0xff: 8}[first]
    if len(b) - at - 1 < width:
        raise Refused('truncated')
    v = int.from_bytes(b[at + 1:at + 1 + width], 'big')
    if v < {2: 0xfd, 4: 0x10000, 8: 0x100000000}[width]:
        raise Refused('not minimal')
    return v, at + 1 + width


def write_bigsize(v):
    if v < 0xfd:
        return bytes([v])
    if v < 0x10000:
        return b'\xfd' + v.to_bytes(2, 'big')
    if v < 0x100000000:
        return b'\xfe' + v.to_bytes(4, 'big')
    return b'\xff' + v.to_bytes(8, 'big')


def records(b):
    at, last = 0, None
    while at < len(b):
        t, at = bigsize(b, at)
        if last is not None and t <= last:
            raise Refused('order')
        n, at = bigsize(b, at)
        if n > len(b) - at:
            raise Refused('truncated')
        yield t, b[at:at + n]
        at += n
        last = t


def tu(v, width):
    if len(v) > width:
        raise Refused('length')
    if v and v[0] == 0:
        raise Refused('not minimal')
    return int.from_bytes(v, 'big')


def on_curve(point):
    if len(point) != 33 or point[0] not in (2, 3):
        return False
    x = int.from_bytes(point[1:], 'big')
    if x >= P:
        return False
    y2 = (pow(x, 3, P) + 7) % P
    return pow(y2, (P - 1) // 2, P) in (0, 1)


def decode(b, space):
    """BOLT 1's n1 or n2 namespace: the values, or Refused with a reason."""
    values = {}
    for t, v in records(b):
        if space == 1 and t == 1:
            values['amount_msat'] = tu(v, 8)
        elif space == 1 and t == 2:
            if len(v) != 8:
                raise Refused('length')
            s = int.from_bytes(v, 'big')
            values['scid'] = f'{s >> 40}x{(s >> 16) & 0xffffff}x{s & 0xffff}'
        elif space == 1 and t == 3:
            if len(v) != 49:
                raise Refused('length')
            if not on_curve(v[:33]):
                raise Refused('value')
            values['amount_msat_1'] = int.from_bytes(v[33:41], 'big')
            values['amount_msat_2'] = int.from_bytes(v[41:49], 'big')
        elif space == 1 and t == 254:
            if len(v) != 2:
                raise Refused('length')
            values['cltv_delta'] = int.from_bytes(v, 'big')
        elif space == 2 and t == 0:
            values['amount_msat'] = tu(v, 8)
        elif space == 2 and t == 11:
            values['cltv_expiry'] = tu(v, 4)
        elif t % 2 == 0:
            raise Refused('unknown even')
    return values


def verdict(b, space):
    try:
        return 'ok', decode(b, space)
    except Refused as e:
        return str(e), {}


def read_init(m):
    if len(m) < 2:
        raise Refused('truncated')
    if int.from_bytes(m[:2], 'big') != 16:
        raise Refused('value')
    at = 2
    if len(m) - at < 2:
        raise Refused('truncated')
    gl = int.from_bytes(m[at:at + 2], 'big')
    g = m[at + 2:at + 2 + gl]
    at += 2 + gl
    if at + 2 > len(m):
        raise Refused('truncated')
    fl = int.from_bytes(m[at:at + 2], 'big')
    f = m[at + 2:at + 2 + fl]
    at += 2 + fl
    if at > len(m):
        raise Refused('truncated')
    skipped, networks = 0, []
    for t, v in records(m[at:]):
        if t == 1:
            if len(v) % 32:
                raise Refused('length')
            networks = [v[i:i + 32] for i in range(0, len(v), 32)]
        elif t == 3:
            pass
        elif t % 2 == 0:
            raise Refused('unknown even')
        else:
            skipped += 1
    combined = int.from_bytes(g, 'big') | int.from_bytes(f, 'big')
    return combined, networks, skipped


def transcript():
    names = bolts.features()
    out = ['bigsize']
    encodings = written = 0
    for r in bolts.bigsize():
        b = bytes.fromhex(r['bytes'])
        try:
            v, n = bigsize(b, 0)
            assert 'exp_error' not in r and v == r['value'] and n == len(b)
            out.append(f'  {r["name"]}: {r["bytes"]} -> {v}')
        except Refused as e:
            want = {'decoded bigsize is not canonical': 'not minimal', 'unexpected EOF': 'truncated',
                    'EOF': 'truncated'}[r['exp_error']]
            assert str(e) == want, (r, e)
            out.append(f'  {r["name"]}: {r["bytes"]} -> {e}')
        if r['encode']:
            encodings += 1
            written += write_bigsize(r['value']).hex() == r['bytes']
    out.append(f'  encodings written byte for byte: {written} of {encodings}')

    out.append('tlv streams')
    rows = bolts.tlv()
    agree = 0
    for stream, space, ok, why, record, fields in rows:
        b = bytes.fromhex(stream)
        shown = '(empty)' if not stream else stream[:24] + ('...' if len(stream) > 24 else '')
        line = f'  {["n1/n2", "n1", "n2"][space]} {shown}:'
        good = True
        for ns in ([1, 2] if space == 0 else [space]):
            code, values = verdict(b, ns)
            line += f' {code}'
            good &= (code == 'ok') == ok
            if ok and record:
                for k in ('amount_msat', 'scid', 'amount_msat_1', 'amount_msat_2', 'cltv_delta'):
                    if k in fields:
                        assert str(values[k]) == fields[k], (stream, k)
                        line += f' {k}={values[k]}'
        agree += good
        out.append(line)
    out.append(f'  agree with BOLT 1: {agree} of {len(rows)}')
    pairs = refused = 0
    for g in rows:
        if g[2] and g[1] == 1:
            for bad in rows:
                if not bad[2] and bad[1] != 2:
                    pairs += 1
                    refused += verdict(bytes.fromhex(g[0] + bad[0]), 1)[0] != 'ok'
    out.append(f'  a valid n1 stream followed by a refused one: {refused} of {pairs} refused')

    out.append('messages')
    for message, ok, why in bolts.init_extension():
        try:
            _, _, skipped = read_init(bytes.fromhex(message))
            assert ok
            out.append(f'  init {message}: ok, {skipped} unknown odd records skipped')
        except Refused as e:
            assert not ok
            out.append(f'  init {message}: {e}')
    features = (1 << 8) | (1 << 14) | (1 << 17)
    flen = (features.bit_length() + 7) // 8
    chains = bytes.fromhex(MAINNET)
    init = (16).to_bytes(2, 'big') + b'\0\0' + flen.to_bytes(2, 'big') + features.to_bytes(flen, 'big')
    init += write_bigsize(1) + write_bigsize(len(chains)) + chains
    out.append(f'  our init, {len(init)} bytes: {init.hex()}')
    combined, networks, _ = read_init(init)
    line = f'  read back: ok, {len(networks)} network, features'
    for bit in range(combined.bit_length()):
        if combined >> bit & 1:
            line += f' {bit} ({names.get(bit // 2, "")}, {"required" if bit % 2 == 0 else "supported"})'
    out.append(line)
    known = {8 // 2, 14 // 2, 17 // 2}
    for bit in (101, 100):
        peer = features | (1 << bit)
        close = any(peer >> b & 1 and b % 2 == 0 and b // 2 not in known for b in range(peer.bit_length()))
        out.append(f'  a peer that sets bit {bit}: {"close" if close else "accept"}')
    for t in (18, 32768, 32769, 1):
        action = 'read' if t in (1, 16, 17, 18, 19) else ('ignore' if t % 2 else 'close')
        out.append(f'  message type {t}: {action}')
    ping = (18).to_bytes(2, 'big') + (4).to_bytes(2, 'big') + (2).to_bytes(2, 'big') + bytes(2)
    pong = (19).to_bytes(2, 'big') + (4).to_bytes(2, 'big') + bytes(4)
    out.append(f'  ping: {ping.hex()}, pong: {pong.hex()}')
    out.append('  a ping asking for 65532 bytes: 0 reply bytes')   # num_pong_bytes >= 65532: ignore the ping
    data = b'internal error'
    error = (17).to_bytes(2, 'big') + bytes(32) + len(data).to_bytes(2, 'big') + data
    out.append(f'  error: {error.hex()}')
    printable = all(32 <= c <= 126 for c in data)
    out.append(f'  read: ok, every channel true, {len(data)} bytes of data, printable {"true" if printable else "false"}')
    return out


def main():
    lines = transcript()
    want = FIXTURE.read_text().splitlines()
    assert lines == want, '\n'.join(f'{a!r} != {b!r}' for a, b in zip(lines, want) if a != b) or 'line counts differ'
    print(f'{len(lines)} lines agree with {FIXTURE.name}')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            got = subprocess.run([coax, 'run', EXAMPLE, *mode], cwd=REPO, capture_output=True, text=True, check=True)
            assert got.stdout.splitlines() == lines, f'coax run {" ".join(mode)} differs'
            print(f'coax run {" ".join(mode) or "(debug)"}: {len(lines)} lines agree')


if __name__ == '__main__':
    main()
