#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BOLT 7's vectors and a separate Python reader and writer decide each case outside Coax)
"""The cross-check of Bitcoin chapter 112. It runs BOLT 7's vectors on
bolt7.py, checks the library's table of forbidden code points against
Python's own Unicode database, computes every line the chapter's example
prints, and compares the transcript with the fixture:

  python3 -I coax/tests/lightning/gossip.py                        # compare with the fixture
  python3 -I coax/tests/lightning/gossip.py --coax coax/bin/coax   # also run the example

bolt7.py is this cross-check's own BOLT 7 reader and writer, CRC32C and
channel graph, written from BOLT 7 and BOLT 1 on ln_crypto.py's secp256k1
and bolt11.py's ECDSA, sharing no code with Coax. Each node_announcement
vector's signature must verify and its alias refuse, and each
extended-queries.json message must read with its stated fields or, for
zlib, refuse, before the transcript is compared.
"""
import re
import subprocess
import sys
import unicodedata
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bolt11 as E  # noqa: E402
import bolt7 as G  # noqa: E402
import bolts  # noqa: E402
import ln_crypto as C  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/lightning-gossip.out'
EXAMPLE = 'bitcoin/examples/lightning-gossip.coax'
LIBRARY = REPO / 'coax/lib/lightning/gossip.coax'
RUN_STEPS = '250000000'
NAMES = ['Alice', 'Bob', 'Carol', 'Dave']


def secret(k, c):
    return int.from_bytes(bytes((k * i + c) & 255 for i in range(32)), 'big')


def check_table():
    """GSP_FORBIDDEN must be exactly the code points of Cc, Cf, Cs, Co, Zl and Zp."""
    body = re.search(r'GSP_FORBIDDEN :: \[52\] u32\.\[(.*?)\];', LIBRARY.read_text(), re.S).group(1)
    v = [int(x, 16) for x in re.findall(r'0x[0-9a-f]+', body)]
    table = list(zip(v[::2], v[1::2]))
    want, start = [], None
    for c in range(0x110000):
        bad = unicodedata.category(chr(c)) in ('Cc', 'Cf', 'Cs', 'Co', 'Zl', 'Zp')
        if bad and start is None:
            start = c
        if not bad and start is not None:
            want.append((start, c - 1))
            start = None
    assert table == want, f'GSP_FORBIDDEN differs from Unicode {unicodedata.unidata_version}'


def check_vectors():
    key, rows = bolts.bolt7_aliases()
    for r in rows:
        m = bytes.fromhex(r['announcement'])
        assert E.verify(G.sha256d(m[66:]), G.public(key), m[2:66]), r['name']
        assert G.code(m) == 'BAD_ALIAS', r['name']
    for q in bolts.bolt7_queries():
        want = 'BAD_ENCODING' if 'COMPRESSED_ZLIB' in str(q['msg']) else 'OK'
        assert G.code(bytes.fromhex(q['hex'])) == want


def announce(chain, sid, na, nb, ba, bb):
    if G.public(nb) < G.public(na):
        na, nb, ba, bb = nb, na, bb, ba
    return G.write_channel_announcement(chain, sid, [na, nb], [ba, bb])


def policies(g, out):
    for sid, c in g.channels.items():
        for d in range(2):
            p = c['updates'][d]
            if p is None:
                continue
            out.append(f'  {G.scid_text(sid)} direction {d}: {p["fee_base"]} msat + {p["fee_ppm"]} ppm, '
                       f'cltv delta {p["cltv"]}, htlcs {p["htlc_min"]} to {p["htlc_max"]} msat'
                       + (', disabled' if p['disabled'] else ''))


def transcript():
    out = ['BOLT 7\'s vectors']
    key, rows = bolts.bolt7_aliases()
    out.append('  node announcements: ' + ' '.join(G.code(bytes.fromhex(r['announcement'])) for r in rows))
    ok = sum(E.verify(G.sha256d(bytes.fromhex(r['announcement'])[66:]), G.public(key),
                      bytes.fromhex(r['announcement'])[2:66]) for r in rows)
    out.append(f'  {ok} of 6 signatures verify under {G.public(key).hex()}')
    out.append('  extended queries: ' + ' '.join(G.code(bytes.fromhex(q['hex'])) for q in bolts.bolt7_queries()))

    out.append('short channel IDs')
    v = G.scid_parse('539268x845x1')
    out.append(f'  539268x845x1 is block {v >> 40}, transaction {(v >> 16) & 0xffffff}, output {v & 0xffff}: '
               f'{v.to_bytes(8, "big").hex()}')
    chain = G.REGTEST
    keys = [secret(3, 1), secret(5, 2), secret(7, 3), secret(9, 4)]
    ids = [G.public(k) for k in keys]
    for k in range(3):
        out.append(f'  {NAMES[k]} {ids[k].hex()}')

    out.append('two channels')
    g = G.Graph(chain)
    fa, fb, fb2, fc = secret(11, 4), secret(13, 5), secret(17, 6), secret(19, 7)
    s1, s2 = G.scid(105, 1, 0), G.scid(106, 2, 1)
    ab = announce(chain, s1, keys[0], keys[1], fa, fb)
    bc = announce(chain, s2, keys[1], keys[2], fb2, fc)
    for label, m in (('Alice and Bob', ab), ('Bob and Carol', bc)):
        a = G.channel_announcement(m)
        out.append(f'  {label} {G.scid_text(a["scid"])}: {len(m)} bytes, OK')
        out.append(f'    node_id_1 is {NAMES[ids.index(a["node1"])]}, the lesser key; signed hash {a["hash"].hex()}')
        out.append(f'    added to the graph: {g.add(m)}')

    out.append('channel updates')
    a1, a2 = G.channel_announcement(ab), G.channel_announcement(bc)
    da = 0 if a1['node1'] == ids[0] else 1
    db2 = 0 if a2['node1'] == ids[1] else 1
    signers = [0, 1, 1, 2]
    scids = [s1, s1, s2, s2]
    dirs = [da, 1 - da, db2, 1 - db2]
    fees = [1000, 0, 500, 2000]
    for k in range(4):
        m = G.write_channel_update(keys[signers[k]], chain, scids[k], 1762156800 + k, dirs[k], 0,
                                   40 + 10 * k, 1000, fees[k], 1 + k, 990000000)
        u = G.channel_update(m)
        out.append(f'  {NAMES[signers[k]]} for {G.scid_text(scids[k])}, direction {u["direction"]}: '
                   f'{g.add(m)}, checksum {u["checksum"]}')
    policies(g, out)

    out.append('node announcements')
    addrs = G.ipv4(127, 0, 0, 1, 9735)
    for k in range(4):
        m = G.write_node_announcement(keys[k], 1762156800, bytes([60 * k, 0x80, 0xff]), NAMES[k].encode(), addrs)
        n = G.node_announcement(m)
        out.append(f'  {NAMES[k]}: {len(m)} bytes, OK, {len(n["addresses"])} address, added: {g.add(m)}')

    out.append('changing them')
    bad = bytearray(ab)
    bad[296] ^= 1
    out.append(f'  one byte of a channel announcement changed: {G.code(bytes(bad))}')
    sw = bytearray(ab)
    sw[300:333], sw[333:366] = ab[333:366], ab[300:333]
    out.append(f'  its node IDs swapped: {G.code(bytes(sw))}')
    other = G.Graph(bytes((0x6f) & 255 for i in range(32)))
    out.append(f'  added to a graph that follows another chain: {other.add(ab)}')
    m = G.write_channel_update(keys[0], chain, s1, 1762160400, 1 - da, 0, 40, 1000, 0, 1, 990000000)
    out.append(f'  Alice signs Bob\'s direction: {g.add(m)}')
    m = G.write_channel_update(keys[0], chain, s1, 1762156800, da, 0, 40, 1000, 1000, 1, 990000000)
    out.append(f'  Alice\'s first update again: {g.add(m)}')
    s = int.from_bytes(m[34:66], 'big')
    m = m[:34] + (C.N - s).to_bytes(32, 'big') + m[66:]
    u = G.channel_update(m)
    verifies = 'true' if E.verify(u['hash'], ids[0], m[2:66]) else 'false'
    out.append(f'  with n - s for s: verifies {verifies}, added: {g.add(m)}')
    m = G.write_channel_update(keys[0], chain, s1, 1762160400, da, 1, 40, 1000, 1000, 1, 990000000)
    out.append(f'  Alice disables 105x1x0 an hour later: {g.add(m)}')
    m = G.write_channel_update(keys[2], chain, G.scid(107, 1, 0), 1762160400, 0, 0, 40, 1000, 1000, 1, 990000000)
    out.append(f'  an update for 107x1x0: {g.add(m)}')
    policies(g, out)

    out.append('catching up')
    f = G.gossip_timestamp_filter(G.write_timestamp_filter(chain, 1762156801, 2))
    passing = [t for t in range(1762156800, 1762156804) if G.filter_passes(f['first'], f['range'], t)]
    out.append(f'  gossip_timestamp_filter from {f["first"]} for {f["range"]}: updates at '
               + ' '.join(str(t) for t in passing) + ' pass')
    qm = G.write_query_channel_range(chain, 100, 10, 3)
    q = G.query_channel_range(qm)
    out.append(f'  query_channel_range blocks {q["first"]} to {q["first"] + q["number"] - 1}, '
               f'options {q["options"]}: {len(qm)} bytes')
    rm = g.reply_range(q['first'], q['number'], q['options'])
    r = G.reply_channel_range(rm)
    out.append(f'  reply_channel_range: {len(rm)} bytes, OK, sync_complete {r["complete"]}')
    for i, sid in enumerate(r['scids']):
        t, c = r['timestamps'][i], r['checksums'][i]
        out.append(f'    {G.scid_text(sid)} timestamps {t[0]} {t[1]}, checksums {c[0]} {c[1]}')
    qm = G.write_query_short_channel_ids(chain, [s1, s2], [1, 6])
    qs = G.query_short_channel_ids(qm)
    out.append(f'  query_short_channel_ids: {len(qm)} bytes, OK, flags {qs["flags"][0]} {qs["flags"][1]}')
    z = bytearray(qm)
    z[36] = 1
    out.append(f'  the same with encoding 1, zlib: {G.code(bytes(z))}')
    return out


def main():
    check_table()
    check_vectors()
    lines = transcript()
    want = FIXTURE.read_text().splitlines()
    assert lines == want, '\n'.join(f'{a!r} != {b!r}' for a, b in zip(lines, want) if a != b) or 'line counts differ'
    print(f'every BOLT 7 vector passes; the forbidden-character table matches Unicode '
          f'{unicodedata.unidata_version}; {len(lines)} lines agree with {FIXTURE.name}')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            got = subprocess.run([coax, 'run', EXAMPLE, *mode, '--run-steps', RUN_STEPS], cwd=REPO,
                                 capture_output=True, text=True, check=True)
            assert got.stdout.splitlines() == lines, f'coax run {" ".join(mode)} differs'
            print(f'coax run {" ".join(mode) or "(debug)"}: {len(lines)} lines agree')


if __name__ == '__main__':
    if '--print' in sys.argv:
        print('\n'.join(transcript()))
    else:
        main()
