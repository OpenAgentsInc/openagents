#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a BOLT 7 reader and writer in plain Python, sharing no code with Coax)
"""An independent BOLT 7 reader and writer for Bitcoin chapter 112,
written from BOLT 7 and BOLT 1 on ln_crypto.py's secp256k1 and
bolt11.py's ECDSA: short channel IDs, channel_announcement,
node_announcement and channel_update with their double-SHA-256
signatures, the query messages with encoding 0 only, CRC32C checksums,
and a channel graph that accepts gossip in order. It is slow and not
constant-time: an oracle for tests only.
"""
import hashlib
import struct
import sys
import unicodedata
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bolt11 as E  # noqa: E402
import ln_crypto as C  # noqa: E402

CODES = ['OK', 'TRUNCATED', 'WRONG_TYPE', 'BAD_POINT', 'KEY_ORDER', 'NOT_MINIMAL', 'BAD_SIGNATURE',
         'BAD_ALIAS', 'BAD_ADDRESS', 'BAD_ENCODING', 'BAD_LENGTH', 'ORDER', 'UNKNOWN_EVEN', 'BAD_COUNT',
         'UNKNOWN_CHAIN', 'UNKNOWN_CHANNEL', 'UNKNOWN_NODE', 'STALE', 'BAD_TEXT', 'CAPACITY']
REGTEST = bytes.fromhex('0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206')
CHANNEL_ANNOUNCEMENT, NODE_ANNOUNCEMENT, CHANNEL_UPDATE = 256, 257, 258
QUERY_SCIDS, REPLY_SCIDS_END, QUERY_RANGE, REPLY_RANGE, TIMESTAMP_FILTER = 261, 262, 263, 264, 265
ADDRESS_LENGTHS = {1: 6, 2: 18, 3: 12, 4: 37}


class Refused(Exception):
    pass


def sha256d(b):
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


# ---- short_channel_id

def scid(height, tx, out):
    assert height < 1 << 24 and tx < 1 << 24 and out < 1 << 16
    return height << 40 | tx << 16 | out


def scid_text(v):
    return f'{v >> 40}x{(v >> 16) & 0xffffff}x{v & 0xffff}'


def scid_parse(t):
    parts = t.split('x')
    if len(parts) != 3 or not all(p.isdigit() and p.isascii() and (p == '0' or p[0] != '0') for p in parts):
        raise Refused('BAD_TEXT')
    h, i, o = (int(p) for p in parts)
    if h >= 1 << 24 or i >= 1 << 24 or o >= 1 << 16:
        raise Refused('BAD_TEXT')
    return scid(h, i, o)


# ---- BOLT 1's text rule and features

def text_ok(b):
    """BOLT 1's Unicode validation: valid UTF-8 without Cc, Cf, Cs, Co, Zl or Zp."""
    try:
        s = b.decode('utf-8')
    except UnicodeDecodeError:
        return False
    return all(unicodedata.category(c) not in ('Cc', 'Cf', 'Cs', 'Co', 'Zl', 'Zp') for c in s)


def alias_ok(raw):
    return text_ok(raw.rstrip(b'\0'))


def features_minimal(f):
    return len(f) == 0 or f[0] != 0


def unknown_even_feature(f, known=()):
    for bit in range(len(f) * 8):
        if f[len(f) - 1 - bit // 8] >> (bit % 8) & 1 and bit % 2 == 0 and bit not in known:
            return True
    return False


def crc32c(data):
    c = 0xffffffff
    for b in data:
        c ^= b
        for _ in range(8):
            c = (c >> 1) ^ (0x82f63b78 if c & 1 else 0)
    return c ^ 0xffffffff


# ---- reading

class Reader:
    def __init__(self, b, at=0):
        self.b, self.at = b, at

    def take(self, n):
        if self.at + n > len(self.b):
            raise Refused('TRUNCATED')
        v = self.b[self.at:self.at + n]
        self.at += n
        return v

    def u16(self):
        return struct.unpack('>H', self.take(2))[0]

    def u32(self):
        return struct.unpack('>I', self.take(4))[0]

    def u64(self):
        return struct.unpack('>Q', self.take(8))[0]


def bigsize(b, at):
    if at >= len(b):
        raise Refused('TRUNCATED')
    p = b[at]
    if p < 0xfd:
        return p, at + 1
    n = {0xfd: 2, 0xfe: 4, 0xff: 8}[p]
    if at + 1 + n > len(b):
        raise Refused('TRUNCATED')
    v = int.from_bytes(b[at + 1:at + 1 + n], 'big')
    if v < {2: 0xfd, 4: 0x10000, 8: 0x100000000}[n]:
        raise Refused('NOT_MINIMAL')
    return v, at + 1 + n


def tlvs(b, known):
    out, at, prev = {}, 0, -1
    while at < len(b):
        t, at = bigsize(b, at)
        if t <= prev:
            raise Refused('ORDER')
        n, at = bigsize(b, at)
        if at + n > len(b):
            raise Refused('TRUNCATED')
        if t in known:
            out[t] = b[at:at + n]
        elif t % 2 == 0:
            raise Refused('UNKNOWN_EVEN')
        at += n
        prev = t
    return out


def point(b):
    if C.decompress(b) is None:
        raise Refused('BAD_POINT')


def message_type(m):
    if len(m) < 2:
        raise Refused('TRUNCATED')
    return struct.unpack('>H', m[:2])[0]


def expect(m, t):
    if message_type(m) != t:
        raise Refused('WRONG_TYPE')


def channel_announcement(m):
    expect(m, CHANNEL_ANNOUNCEMENT)
    r = Reader(m, 2)
    sigs = [r.take(64) for _ in range(4)]
    features = r.take(r.u16())
    chain = r.take(32)
    sid = r.u64()
    keys = [r.take(33) for _ in range(4)]
    if not features_minimal(features):
        raise Refused('NOT_MINIMAL')
    for k in keys:
        point(k)
    if not keys[0] < keys[1]:
        raise Refused('KEY_ORDER')
    h = sha256d(m[2 + 256:])
    for sig, key in zip(sigs, [keys[0], keys[1], keys[2], keys[3]]):
        if not E.verify(h, key, sig):
            raise Refused('BAD_SIGNATURE')
    return {'features': features, 'chain': chain, 'scid': sid, 'node1': keys[0], 'node2': keys[1],
            'bitcoin1': keys[2], 'bitcoin2': keys[3], 'hash': h,
            'routable': not unknown_even_feature(features)}


def addresses(a):
    """The usable descriptors: (type, data). Unknown types end the list."""
    out, at, dns = [], 0, 0
    while at < len(a):
        t = a[at]
        if t in ADDRESS_LENGTHS:
            n = ADDRESS_LENGTHS[t]
        elif t == 5:
            if at + 1 >= len(a):
                raise Refused('BAD_ADDRESS')
            n = 1 + a[at + 1] + 2
        else:
            break
        if at + 1 + n > len(a):
            raise Refused('BAD_ADDRESS')
        data = a[at + 1:at + 1 + n]
        at += 1 + n
        port = int.from_bytes(data[-2:], 'big')
        if t == 3 or port == 0:
            continue
        if t == 5:
            dns += 1
            if dns > 1 or not all(c < 128 for c in data[1:-2]):
                continue
        out.append((t, data))
    return out


def node_announcement(m):
    expect(m, NODE_ANNOUNCEMENT)
    r = Reader(m, 2)
    sig = r.take(64)
    features = r.take(r.u16())
    ts = r.u32()
    node = r.take(33)
    rgb = r.take(3)
    alias = r.take(32)
    addrs = r.take(r.u16())
    point(node)
    if not E.verify(sha256d(m[2 + 64:]), node, sig):
        raise Refused('BAD_SIGNATURE')
    if not features_minimal(features):
        raise Refused('NOT_MINIMAL')
    if not alias_ok(alias):
        raise Refused('BAD_ALIAS')
    return {'features': features, 'timestamp': ts, 'node': node, 'rgb': rgb,
            'alias': alias.rstrip(b'\0').decode(), 'addresses': addresses(addrs)}


def channel_update(m):
    expect(m, CHANNEL_UPDATE)
    r = Reader(m, 2)
    u = {'signature': r.take(64), 'chain': r.take(32), 'scid': r.u64(), 'timestamp': r.u32(),
         'message_flags': r.take(1)[0], 'channel_flags': r.take(1)[0], 'cltv': r.u16(),
         'htlc_min': r.u64(), 'fee_base': r.u32(), 'fee_ppm': r.u32(), 'htlc_max': r.u64()}
    u['direction'] = u['channel_flags'] & 1
    u['disabled'] = u['channel_flags'] >> 1 & 1
    u['hash'] = sha256d(m[2 + 64:])
    u['checksum'] = crc32c(m[2 + 64:2 + 64 + 40] + m[2 + 64 + 44:])
    return u


def encoded_scids(e):
    if len(e) == 0:
        raise Refused('TRUNCATED')
    if e[0] != 0:
        raise Refused('BAD_ENCODING')
    if (len(e) - 1) % 8:
        raise Refused('BAD_LENGTH')
    ids = [int.from_bytes(e[1 + 8 * i:9 + 8 * i], 'big') for i in range((len(e) - 1) // 8)]
    if any(a >= b for a, b in zip(ids, ids[1:])):
        raise Refused('ORDER')
    return ids


def query_short_channel_ids(m):
    expect(m, QUERY_SCIDS)
    r = Reader(m, 2)
    chain = r.take(32)
    ids = encoded_scids(r.take(r.u16()))
    t = tlvs(m[r.at:], {1})
    flags = None
    if 1 in t:
        v = t[1]
        if len(v) == 0:
            raise Refused('TRUNCATED')
        if v[0] != 0:
            raise Refused('BAD_ENCODING')
        flags, at = [], 1
        while at < len(v):
            f, at = bigsize(v, at)
            flags.append(f)
        if len(flags) != len(ids):
            raise Refused('BAD_COUNT')
    return {'chain': chain, 'scids': ids, 'flags': flags}


def reply_short_channel_ids_end(m):
    expect(m, REPLY_SCIDS_END)
    r = Reader(m, 2)
    return {'chain': r.take(32), 'full': r.take(1)[0]}


def query_channel_range(m):
    expect(m, QUERY_RANGE)
    r = Reader(m, 2)
    q = {'chain': r.take(32), 'first': r.u32(), 'number': r.u32(), 'options': 0}
    t = tlvs(m[r.at:], {1})
    if 1 in t:
        v, at = bigsize(t[1], 0)
        if at != len(t[1]):
            raise Refused('BAD_LENGTH')
        q['options'] = v
    return q


def reply_channel_range(m):
    expect(m, REPLY_RANGE)
    r = Reader(m, 2)
    q = {'chain': r.take(32), 'first': r.u32(), 'number': r.u32(), 'complete': r.take(1)[0]}
    q['scids'] = encoded_scids(r.take(r.u16()))
    n = len(q['scids'])
    t = tlvs(m[r.at:], {1, 3})
    q['timestamps'] = q['checksums'] = None
    if 1 in t:
        v = t[1]
        if len(v) == 0:
            raise Refused('TRUNCATED')
        if v[0] != 0:
            raise Refused('BAD_ENCODING')
        if len(v) - 1 != 8 * n:
            raise Refused('BAD_COUNT')
        q['timestamps'] = [struct.unpack('>II', v[1 + 8 * i:9 + 8 * i]) for i in range(n)]
    if 3 in t:
        v = t[3]
        if len(v) != 8 * n:
            raise Refused('BAD_COUNT')
        q['checksums'] = [struct.unpack('>II', v[8 * i:8 * i + 8]) for i in range(n)]
    return q


def gossip_timestamp_filter(m):
    expect(m, TIMESTAMP_FILTER)
    r = Reader(m, 2)
    return {'chain': r.take(32), 'first': r.u32(), 'range': r.u32()}


def filter_passes(first, rng, ts):
    return first <= ts < first + rng


READERS = {CHANNEL_ANNOUNCEMENT: channel_announcement, NODE_ANNOUNCEMENT: node_announcement,
           CHANNEL_UPDATE: channel_update, QUERY_SCIDS: query_short_channel_ids,
           REPLY_SCIDS_END: reply_short_channel_ids_end, QUERY_RANGE: query_channel_range,
           REPLY_RANGE: reply_channel_range, TIMESTAMP_FILTER: gossip_timestamp_filter}


def read(m):
    t = message_type(m)
    if t not in READERS:
        raise Refused('WRONG_TYPE')
    return READERS[t](m)


def code(m):
    try:
        read(m)
        return 'OK'
    except Refused as e:
        return str(e)


# ---- writing

def public(k):
    return C.compress(C.mul(k))


def sign(k, h):
    return E.sign(k, h)[0]


def write_channel_announcement(chain, sid, node_keys, bitcoin_keys, features=b''):
    """node_keys and bitcoin_keys: secrets, each pair in node ID order."""
    ids = [public(k) for k in node_keys]
    assert ids[0] < ids[1]
    body = struct.pack('>H', len(features)) + features + chain + struct.pack('>Q', sid)
    body += ids[0] + ids[1] + public(bitcoin_keys[0]) + public(bitcoin_keys[1])
    h = sha256d(body)
    sigs = b''.join(sign(k, h) for k in [node_keys[0], node_keys[1], bitcoin_keys[0], bitcoin_keys[1]])
    return struct.pack('>H', CHANNEL_ANNOUNCEMENT) + sigs + body


def write_node_announcement(key, ts, rgb, alias, addrs=b'', features=b''):
    body = struct.pack('>H', len(features)) + features + struct.pack('>I', ts) + public(key) + rgb
    body += alias.ljust(32, b'\0') + struct.pack('>H', len(addrs)) + addrs
    return struct.pack('>H', NODE_ANNOUNCEMENT) + sign(key, sha256d(body)) + body


def write_channel_update(key, chain, sid, ts, direction, disabled, cltv, hmin, base, ppm, hmax):
    body = chain + struct.pack('>QIBBHQIIQ', sid, ts, 1, direction | disabled << 1, cltv, hmin, base, ppm, hmax)
    return struct.pack('>H', CHANNEL_UPDATE) + sign(key, sha256d(body)) + body


def ipv4(a, b, c, d, port):
    return bytes([1, a, b, c, d]) + struct.pack('>H', port)


def hostname(name, port):
    return bytes([5, len(name)]) + name + struct.pack('>H', port)


def write_scids(ids):
    e = b'\0' + b''.join(struct.pack('>Q', i) for i in ids)
    return struct.pack('>H', len(e)) + e


def tlv(t, v):
    assert t < 0xfd and len(v) < 0xfd
    return bytes([t, len(v)]) + v


def write_query_short_channel_ids(chain, ids, flags=None):
    m = struct.pack('>H', QUERY_SCIDS) + chain + write_scids(ids)
    if flags is not None:
        m += tlv(1, b'\0' + bytes(flags))
    return m


def write_query_channel_range(chain, first, number, options=None):
    m = struct.pack('>H', QUERY_RANGE) + chain + struct.pack('>II', first, number)
    if options is not None:
        m += tlv(1, bytes([options]))
    return m


def write_timestamp_filter(chain, first, rng):
    return struct.pack('>H', TIMESTAMP_FILTER) + chain + struct.pack('>II', first, rng)


# ---- the graph

class Graph:
    def __init__(self, chain):
        self.chain = chain
        self.channels = {}   # scid -> announcement fields and updates
        self.nodes = {}      # node ID -> announcement or None

    def add(self, m):
        try:
            return self._add(m)
        except Refused as e:
            return str(e)

    def _add(self, m):
        t = message_type(m)
        if t == CHANNEL_ANNOUNCEMENT:
            a = channel_announcement(m)
            if a['chain'] != self.chain:
                return 'UNKNOWN_CHAIN'
            if a['scid'] in self.channels:
                return 'STALE'
            a['updates'] = [None, None]
            self.channels[a['scid']] = a
            for n in (a['node1'], a['node2']):
                self.nodes.setdefault(n, None)
            return 'OK'
        if t == CHANNEL_UPDATE:
            u = channel_update(m)
            if u['chain'] != self.chain:
                return 'UNKNOWN_CHAIN'
            if u['scid'] not in self.channels:
                return 'UNKNOWN_CHANNEL'
            c = self.channels[u['scid']]
            key = c['node2'] if u['direction'] else c['node1']
            if not E.verify(u['hash'], key, u['signature']):
                return 'BAD_SIGNATURE'
            old = c['updates'][u['direction']]
            if old is not None and u['timestamp'] <= old['timestamp']:
                return 'STALE'
            c['updates'][u['direction']] = u
            return 'OK'
        if t == NODE_ANNOUNCEMENT:
            n = node_announcement(m)
            if n['node'] not in self.nodes:
                return 'UNKNOWN_NODE'
            old = self.nodes[n['node']]
            if old is not None and n['timestamp'] <= old['timestamp']:
                return 'STALE'
            self.nodes[n['node']] = n
            return 'OK'
        return 'WRONG_TYPE'

    def reply_range(self, first, number, options):
        ids = sorted(s for s in self.channels if first <= s >> 40 < first + number)
        m = struct.pack('>H', REPLY_RANGE) + self.chain + struct.pack('>IIB', first, number, 1) + write_scids(ids)
        ups = [self.channels[s]['updates'] for s in ids]
        if options & 1:
            m += tlv(1, b'\0' + b''.join(struct.pack('>II', *[u['timestamp'] if u else 0 for u in p]) for p in ups))
        if options & 2:
            m += tlv(3, b''.join(struct.pack('>II', *[u['checksum'] if u else 0 for u in p]) for p in ups))
        return m
