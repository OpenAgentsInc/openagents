#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Bitcoin Core's pinned net_processing.cpp, net.cpp, banman.cpp, bloom.cpp, hash.cpp, headerssync.cpp and txdownloadman_impl.cpp decide each case outside Coax)
"""The cross-check of Bitcoin chapter 71. It computes every line the
chapter's example prints, in Python that shares no code with Coax, and
compares the transcript with the fixture:

  python3 -I coax/tests/bitcoin/dos.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/dos.py --coax coax/bin/coax   # also run the example

It transliterates Bitcoin Core's message limits (net.cpp's header check,
the size checks in PeerManagerImpl::ProcessMessage), Misbehaving and
MaybeDiscourageAndDisconnect, MaybePunishNodeForBlock, MurmurHash3
(hash.cpp), CRollingBloomFilter (common/bloom.cpp) behind BanMan's
discouragement filter (banman.cpp), the anti-DoS work threshold and
low-work headers decision (net_processing.cpp) with HeadersSyncState's
memory bound (headerssync.cpp), the getheaders rate limit, the receive
and send flood control (net.cpp) and the per-peer transaction
announcement limits (node/txdownloadman_impl.cpp). Chain work comes
from GetBlockProof over compact targets, as in pow.cpp.
"""
import importlib.util
import math
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('addrman_oracle', HERE / 'addrman.py')
am = importlib.util.module_from_spec(spec)
spec.loader.exec_module(am)

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/dos.out'
EXAMPLE = 'bitcoin/examples/dos.coax'
for name in ['net.h', 'banman.h', 'banman.cpp', 'bloom.cpp', 'bloom.h', 'hash.cpp', 'hash_tests.cpp', 'headerssync.cpp',
             'headerssync.h', 'net_processing.h', 'txdownloadman.h', 'txdownloadman_impl.cpp', 'chainparams.cpp', 'chain.h',
             'serialize.h', 'script.h', 'consensus_validation.h', 'fastrange.h', 'denialofservice_tests.cpp', 'bloom_tests.cpp',
             'net_permissions.h']:
    am.PINS[name] = None
am.manifest_pins()
src = {name: am.pinned(name) for name in am.PINS}


def num(text, name):
    m = re.search(rf'{name}\s*(?:=\s*|\{{)([\d\' *]+)', text)
    assert m, name
    return eval(m.group(1).replace("'", ''))


NET_H, NETP, NETCPP = src['net.h'], src['net_processing.cpp'], src['net.cpp']
MAX_PROTOCOL_MESSAGE_LENGTH = num(NET_H, 'MAX_PROTOCOL_MESSAGE_LENGTH')
MAX_SIZE = int(re.search(r'MAX_SIZE = (0x[0-9a-fA-F]+)', src['serialize.h']).group(1), 16)
V2_MAX_CONTENTS = 1 + 12 + min(MAX_SIZE, MAX_PROTOCOL_MESSAGE_LENGTH)
assert 'MAX_CONTENTS_LEN =' in NETCPP and 'std::min<size_t>(MAX_SIZE, MAX_PROTOCOL_MESSAGE_LENGTH)' in NETCPP
MAX_INV_SZ = num(NETP, 'MAX_INV_SZ')
MAX_LOCATOR_SZ = num(NETP, 'MAX_LOCATOR_SZ')
MAX_ADDR_TO_SEND = num(NETP, 'MAX_ADDR_TO_SEND')
MAX_HEADERS_RESULTS = num(src['net_processing.h'], 'MAX_HEADERS_RESULTS')
MAX_BLOOM_FILTER_SIZE = num(src['bloom.h'], 'MAX_BLOOM_FILTER_SIZE')
MAX_HASH_FUNCS = num(src['bloom.h'], 'MAX_HASH_FUNCS')
MAX_SCRIPT_ELEMENT_SIZE = num(src['script.h'], 'MAX_SCRIPT_ELEMENT_SIZE')
RECV_FLOOD = num(NET_H, 'DEFAULT_MAXRECEIVEBUFFER') * 1000
SEND_MAX = num(NET_H, 'DEFAULT_MAXSENDBUFFER') * 1000
READ_CHUNK = 0x10000
assert 'pchBuf[0x10000]' in NETCPP
MAX_PEER_TX_ANNOUNCEMENTS = num(src['txdownloadman.h'], 'MAX_PEER_TX_ANNOUNCEMENTS')
MAX_PEER_TX_REQUEST_IN_FLIGHT = num(src['txdownloadman.h'], 'MAX_PEER_TX_REQUEST_IN_FLIGHT')
TX_DELAY = 2
for d in ['TXID_RELAY_DELAY{2s}', 'NONPREF_PEER_TX_DELAY{2s}', 'OVERLOADED_PEER_TX_DELAY{2s}']:
    assert d in src['txdownloadman.h'], d
HEADERS_RESPONSE_TIME = 120
assert 'HEADERS_RESPONSE_TIME{2min}' in NETP
MAX_FUTURE_BLOCK_TIME = 2 * 60 * 60
assert 'MAX_FUTURE_BLOCK_TIME = 2 * 60 * 60' in src['chain.h']
MAIN = src['chainparams.cpp'][src['chainparams.cpp'].index('class CMainParams'):]
MIN_WORK_MAIN = int(re.search(r'nMinimumChainWork = uint256\{"([0-9a-f]+)"\}', MAIN).group(1), 16)
PERIOD = int(re.search(r'\.commitment_period = (\d+)', MAIN).group(1))
REDOWNLOAD = int(re.search(r'\.redownload_buffer_size = (\d+)', MAIN).group(1))
GENESIS_TIME = int(re.search(r'genesis = CreateGenesisBlock\((\d+),', MAIN).group(1))
COMPRESSED_HEADER = 4 + 32 + 4 + 4 + 4
assert re.search(r'struct CompressedHeader \{\s*//[^\n]*\n\s*int32_t nVersion\{0\};\s*uint256 hashMerkleRoot;\s*uint32_t nTime\{0\};\s*uint32_t nBits\{0\};\s*uint32_t nNonce\{0\};',
                 src['headerssync.h'])
assert 'm_max_commitments = 6 * max_seconds_since_start / m_params.commitment_period;' in src['headerssync.cpp']
assert 'm_discouraged GUARDED_BY(m_banned_mutex) {50000, 0.000001}' in src['banman.h']
assert 'm_tx_inventory_known_filter GUARDED_BY(m_tx_inventory_mutex){50000, 0.000001}' in NETP
assert 'std::make_unique<CRollingBloomFilter>(5000, 0.001)' in NETP
BLOCK_RESULTS = re.findall(r'^\s+(BLOCK_\w+)', src['consensus_validation.h'][src['consensus_validation.h'].index('enum class BlockValidationResult'):
                                                                          src['consensus_validation.h'].index('};', src['consensus_validation.h'].index('enum class BlockValidationResult'))], re.M)
assert BLOCK_RESULTS[0] == 'BLOCK_RESULT_UNSET' and len(BLOCK_RESULTS) == 9, BLOCK_RESULTS

NOW = 1793664000   # 2026-11-03 00:00 UTC
MASK32 = 2**32 - 1


# ---- limits (net.cpp, net_processing.cpp ProcessMessage)
def check(kind, n, extra=0):
    """(verdict, Core's reason)."""
    if kind == 'frame':
        return ('disconnect', 'oversized message') if n > MAX_SIZE or n > MAX_PROTOCOL_MESSAGE_LENGTH else ('ok', '')
    if kind == 'frame_v2':
        return ('disconnect', 'oversized message') if n > V2_MAX_CONTENTS else ('ok', '')
    if kind in ('inv', 'getdata'):
        return ('misbehaving', f'{kind} message size = {n}') if n > MAX_INV_SZ else ('ok', '')
    if kind in ('addr', 'addrv2'):
        return ('misbehaving', f'{kind} message size = {n}') if n > MAX_ADDR_TO_SEND else ('ok', '')
    if kind == 'headers':
        return ('misbehaving', f'headers message size = {n}') if n > MAX_HEADERS_RESULTS else ('ok', '')
    if kind in ('getheaders', 'getblocks'):
        return ('disconnect', f'{kind} locator size {n} > {MAX_LOCATOR_SZ}') if n > MAX_LOCATOR_SZ else ('ok', '')
    if kind == 'filterload':
        return ('misbehaving', 'too-large bloom filter') if n > MAX_BLOOM_FILTER_SIZE or extra > MAX_HASH_FUNCS else ('ok', '')
    if kind == 'filteradd':
        return ('misbehaving', 'bad filteradd message') if n > MAX_SCRIPT_ELEMENT_SIZE else ('ok', '')
    raise ValueError(kind)


for line in ['if (vInv.size() > MAX_INV_SZ)', 'locator.vHave.size() > MAX_LOCATOR_SZ', 'if (vAddr.size() > MAX_ADDR_TO_SEND)',
             'nCount > m_opts.max_headers_result', 'vData.size() > MAX_SCRIPT_ELEMENT_SIZE', 'Misbehaving(peer, "too-large bloom filter")',
             'hdr.nMessageSize > MAX_SIZE || hdr.nMessageSize > MAX_PROTOCOL_MESSAGE_LENGTH']:
    assert line in NETP or line in NETCPP, line


# ---- punishment (net_processing.cpp)
def punish(noban=False, manual=False, private=False, local=False):
    if noban or manual:
        return 'kept'
    if private:
        return 'disconnected'
    if local:
        return 'disconnected, not discouraged'
    return 'discouraged and disconnected'


def punish_block(result, via_compact, inbound):
    if result in ('BLOCK_CONSENSUS', 'BLOCK_MUTATED'):
        return not via_compact
    if result == 'BLOCK_CACHED_INVALID':
        return not via_compact and not inbound
    return result in ('BLOCK_INVALID_HEADER', 'BLOCK_INVALID_PREV', 'BLOCK_MISSING_PREV')


# ---- MurmurHash3 and CRollingBloomFilter (hash.cpp, common/bloom.cpp)
def rotl32(x, r):
    return ((x << r) | (x >> (32 - r))) & MASK32


def murmur3(seed, data):
    h1, c1, c2 = seed, 0xcc9e2d51, 0x1b873593
    n = len(data) // 4
    for i in range(n):
        k1 = int.from_bytes(data[4 * i:4 * i + 4], 'little')
        k1 = rotl32(k1 * c1 & MASK32, 15) * c2 & MASK32
        h1 = (rotl32(h1 ^ k1, 13) * 5 + 0xe6546b64) & MASK32
    tail = data[4 * n:]
    k1 = 0
    if len(tail) >= 3:
        k1 ^= tail[2] << 16
    if len(tail) >= 2:
        k1 ^= tail[1] << 8
    if len(tail) >= 1:
        k1 ^= tail[0]
        k1 = rotl32(k1 * c1 & MASK32, 15) * c2 & MASK32
        h1 ^= k1
    h1 ^= len(data)
    h1 ^= h1 >> 16
    h1 = h1 * 0x85ebca6b & MASK32
    h1 ^= h1 >> 13
    h1 = h1 * 0xc2b2ae35 & MASK32
    h1 ^= h1 >> 16
    return h1


VECTORS = [(int(e, 16), int(s, 16), bytes.fromhex(d)) for e, s, d in
           re.findall(r'T\((0x[0-9a-fA-F]+)U, (0x[0-9a-fA-F]+), "([0-9a-f]*)"\)', src['hash_tests.cpp'])]
assert len(VECTORS) == 14 and all(murmur3(s, d) == e for e, s, d in VECTORS)


class Rolling:
    def __init__(self, n, fp, tweak):
        log_fp = math.log(fp)
        self.k = max(1, min(int(math.floor(log_fp / math.log(0.5) + 0.5)), 50))
        self.per_gen = (n + 1) // 2
        max_elements = self.per_gen * 3
        bits = math.ceil(-1.0 * self.k * max_elements / math.log(1.0 - math.exp(log_fp / self.k)))
        self.data = [0] * (((bits + 63) // 64) << 1)
        self.reset(tweak)

    def reset(self, tweak):
        self.tweak, self.this_gen, self.gen = tweak, 0, 1
        self.data = [0] * len(self.data)

    def positions(self, key):
        for n in range(self.k):
            h = murmur3((n * 0xFBA4C795 + self.tweak) & MASK32, key)
            yield h & 0x3f, (h * len(self.data)) >> 32

    def insert(self, key):
        if self.this_gen == self.per_gen:
            self.this_gen = 0
            self.gen += 1
            if self.gen == 4:
                self.gen = 1
            m1 = (0 - (self.gen & 1)) & (2**64 - 1)
            m2 = (0 - (self.gen >> 1)) & (2**64 - 1)
            for p in range(0, len(self.data), 2):
                p1, p2 = self.data[p], self.data[p + 1]
                mask = (p1 ^ m1) | (p2 ^ m2)
                self.data[p], self.data[p + 1] = p1 & mask, p2 & mask
        self.this_gen += 1
        for bit, pos in self.positions(key):
            a, b = pos & ~1, pos | 1
            self.data[a] = (self.data[a] & ~(1 << bit)) | ((self.gen & 1) << bit)
            self.data[b] = (self.data[b] & ~(1 << bit)) | ((self.gen >> 1) << bit)

    def contains(self, key):
        for bit, pos in self.positions(key):
            if not ((self.data[pos & ~1] | self.data[pos | 1]) >> bit) & 1:
                return False
        return True


assert 'nHashFuncs = std::max(1, std::min((int)round(logFpRate / log(0.5)), 50));' in src['bloom.cpp']
assert 'return MurmurHash3(nHashNum * 0xFBA4C795 + nTweak, vDataToHash);' in src['bloom.cpp']
assert 'return (uint64_t{x} * n) >> 32;' in src['fastrange.h']


def addr_bytes(a):
    return bytes(10) + b'\xff\xff' + a.ip if a.net == am.IPV4 else a.ip


class BanMan:
    def __init__(self, tweak):
        self.discouraged = Rolling(50000, 0.000001, tweak)
        self.bans = {}

    def discourage(self, a):
        self.discouraged.insert(addr_bytes(a))

    def is_discouraged(self, a):
        return self.discouraged.contains(addr_bytes(a))

    def ban(self, a, until):
        self.bans[(a.net, a.ip)] = until

    def is_banned(self, a, now):
        until = self.bans.get((a.net, a.ip))
        return until is not None and now < until


# ---- headers (net_processing.cpp, headerssync.cpp, pow.cpp)
def proof(bits):
    size, word = bits >> 24, bits & 0x007fffff
    target = word >> 8 * (3 - size) if size <= 3 else word << 8 * (size - 3)
    if target == 0 or bits & 0x00800000:
        return 0
    return (2**256 - 1 - target) // (target + 1) + 1


def threshold(tip_work, tip_bits, min_work):
    near = tip_work - min(144 * proof(tip_bits), tip_work)
    return max(near, min_work)


def route(start_work, bits, count, thresh):
    total = start_work + count * proof(bits)
    if total < thresh:
        return 'presync' if count == MAX_HEADERS_RESULTS else 'ignored'
    return 'accepted'


def getheaders_times(times):
    last, sent = 0, []
    for t in times:
        if (NOW + t) - last > HEADERS_RESPONSE_TIME:
            last = NOW + t
            sent.append(t)
    return sent


# ---- flood control (net.cpp)
class Buf:
    def __init__(self, msg):
        self.msg, self.partial, self.queued, self.send = msg, 0, 0, 0
        self.pause_recv = self.pause_send = False
        self.peak = self.peak_queue = 0

    def note(self):
        self.peak = max(self.peak, self.partial + self.queued * self.msg + self.send)
        self.peak_queue = max(self.peak_queue, self.queued * self.msg)

    def read(self, n):
        if self.pause_recv:
            return 0
        self.partial += n
        done, self.partial = divmod(self.partial, self.msg)
        self.queued += done
        self.pause_recv = self.queued * self.msg > RECV_FLOOD
        self.note()
        return done

    def take(self):
        if self.pause_send or self.queued == 0:
            return False
        self.queued -= 1
        self.pause_recv = self.queued * self.msg > RECV_FLOOD
        return True

    def push(self, n):
        self.send += n
        self.pause_send = self.send > SEND_MAX
        self.note()

    def sent(self, n):
        self.send -= min(n, self.send)
        self.pause_send = self.send > SEND_MAX


MAX_MESSAGE = MAX_PROTOCOL_MESSAGE_LENGTH + 24


def tx_delay(tracked, in_flight, preferred, wtxid, wtxid_peers, relay):
    if not relay and tracked >= MAX_PEER_TX_ANNOUNCEMENTS:
        return None
    d = 0
    if not preferred:
        d += TX_DELAY
    if not wtxid and wtxid_peers > 0:
        d += TX_DELAY
    if not relay and in_flight >= MAX_PEER_TX_REQUEST_IN_FLIGHT:
        d += TX_DELAY
    return d


def key32(rng):
    return b''.join(rng.rand64().to_bytes(8, 'little') for _ in range(4))


def transcript():
    lines = []
    out = lines.append

    out('message limits')
    for label, kind, n, extra in [
            ('v1 payload of 4000000 bytes', 'frame', 4000000, 0), ('v1 payload of 4000001 bytes', 'frame', 4000001, 0),
            ('v2 contents of 4000014 bytes', 'frame_v2', 4000014, 0),
            ('inv with 50000 entries', 'inv', 50000, 0), ('inv with 50001 entries', 'inv', 50001, 0),
            ('getdata with 50001 entries', 'getdata', 50001, 0),
            ('addr with 1001 addresses', 'addr', 1001, 0), ('addrv2 with 1001 addresses', 'addrv2', 1001, 0),
            ('headers with 2000 headers', 'headers', 2000, 0), ('headers with 2001 headers', 'headers', 2001, 0),
            ('getheaders with a 102-hash locator', 'getheaders', 102, 0), ('getblocks with a 101-hash locator', 'getblocks', 101, 0),
            ('filterload of 36001 bytes', 'filterload', 36001, 10), ('filterload with 51 hash functions', 'filterload', 100, 51),
            ('filteradd of 521 bytes', 'filteradd', 521, 0)]:
        verdict, reason = check(kind, n, extra)
        out(f'  {label}: {verdict}' + (f' ({reason})' if reason else ''))

    out('what misbehaving costs')
    out(f'  inbound or outbound peer: {punish()}')
    out(f'  noban: {punish(noban=True)}; manual: {punish(manual=True)}; private broadcast: {punish(private=True)}; localhost: {punish(local=True)}')
    bm = BanMan(0x2b7e1516)
    a0, a1, other = am.v4('160.176.192.1'), am.v4('160.176.192.2'), am.v4('160.176.255.1')
    tor = am.Addr(am.ONION, bytes.fromhex('79bcc625184b05194975c28b66b66b0469f7f6556fb1ac3189a79b40dda32f1f'))
    for a in (a0, a1, tor):
        bm.discourage(a)
    out(f'  after 3 discouragements: 160.176.192.1 {bm.is_discouraged(a0)}, the onion address {bm.is_discouraged(tor)}, 160.176.255.1 {bm.is_discouraged(other)}')
    bm.ban(other, NOW + 86400)
    out(f'  banned 160.176.255.1 for a day: {bm.is_banned(other, NOW)} now, {bm.is_banned(other, NOW + 86400)} after 24 hours')

    out('invalid blocks')
    for r in BLOCK_RESULTS[1:]:
        cases = [('', False, True), ('compact', True, True)] if r in ('BLOCK_CONSENSUS', 'BLOCK_MUTATED') else \
            [('outbound', False, False), ('inbound', False, True)] if r == 'BLOCK_CACHED_INVALID' else [('', False, True)]
        out(f'  {r}: ' + '; '.join((f'{label} ' if label else '') + ('misbehaving' if punish_block(r, c, i) else 'not punished') for label, c, i in cases))

    out('MurmurHash3 and rolling bloom filters')
    for e, s, d in VECTORS[1:3] + VECTORS[4:5] + VECTORS[-1:]:
        out(f'  MurmurHash3({s:#010x}, "{d.hex()}") = {murmur3(s, d):08x}')
    for label, n, fp in [('discouraged addresses', 50000, 0.000001), ('known transactions, per peer', 50000, 0.000001),
                         ('known addresses, per peer', 5000, 0.001), ('Core\'s test filter', 100, 0.01)]:
        f = Rolling(n, fp, 0)
        out(f'  ({n}, {fp:g}) {label}: {f.k} hash functions, {f.per_gen} a generation, {len(f.data) * 8} bytes')
    rng = am.Rng(bytes([7]) * 32)
    data = [key32(rng) for _ in range(399)]
    rb1 = Rolling(100, 0.01, 0x9e3779b9)
    for d in data:
        rb1.insert(d)
    last = sum(rb1.contains(d) for d in data[299:])
    fp = sum(rb1.contains(key32(rng)) for _ in range(10000))
    out(f'  (100, 0.01) after 399 inserts: last 100 remembered {last}, false positives in 10000 others {fp}')
    rb1.reset(0x9e3779b9)
    rolling = True
    for i, d in enumerate(data):
        if i >= 100 and not rb1.contains(data[i - 100]):
            rolling = False
        rb1.insert(d)
        rolling = rolling and rb1.contains(d)
    out(f'  after reset, rolling through them: every one of the last 100 remembered {rolling}')
    bm = BanMan(0x243f6a88)
    for i in range(100000):
        bm.discourage(am.v4(f'10.{i >> 16}.{(i >> 8) & 255}.{i & 255}'))
    kept = sum(bm.is_discouraged(am.v4(f'10.{i >> 16}.{(i >> 8) & 255}.{i & 255}')) for i in range(50000, 100000, 10))
    old = sum(bm.is_discouraged(am.v4(f'10.{i >> 16}.{(i >> 8) & 255}.{i & 255}')) for i in range(0, 10000, 10))
    fresh = sum(bm.is_discouraged(am.v4(f'11.{i >> 16}.{(i >> 8) & 255}.{i & 255}')) for i in range(20000))
    out(f'  100000 discouraged: of the last 50000, {kept} of 5000 sampled remembered; of the first 10000, {old} of 1000; {fresh} of 20000 others match')

    out('headers')
    sent = getheaders_times([0, 30, 120, 121, 200, 242])
    out(f'  unconnecting headers at 0 30 120 121 200 242 s: getheaders sent at {" ".join(str(t) for t in sent)}')
    gw = proof(0x1d00ffff)
    th = threshold(gw, 0x1d00ffff, MIN_WORK_MAIN)
    out(f'  mainnet at genesis: threshold {th:064x}')
    tip = MIN_WORK_MAIN + 10000 * proof(0x17023a04)
    th2 = threshold(tip, 0x17023a04, MIN_WORK_MAIN)
    out(f'  10000 blocks past the minimum: threshold is the tip less 144 blocks: {th2 == tip - 144 * proof(0x17023a04)}')
    out(f'  2000 genesis-difficulty headers from genesis: {route(gw, 0x1d00ffff, 2000, th)}; 1999: {route(gw, 0x1d00ffff, 1999, th)}')
    out(f'  regtest, 1 header from genesis: {route(proof(0x207fffff), 0x207fffff, 1, threshold(proof(0x207fffff), 0x207fffff, 0))}')
    commitments = 6 * (NOW - GENESIS_TIME + MAX_FUTURE_BLOCK_TIME) // PERIOD
    total = (commitments + 7) // 8 + REDOWNLOAD * COMPRESSED_HEADER
    out(f'  presync from genesis on 2026-11-03: at most {commitments} commitment bits ({(commitments + 7) // 8} bytes) and {REDOWNLOAD} headers of {COMPRESSED_HEADER} bytes: {total} bytes')

    out('floods')
    b = Buf(MAX_PROTOCOL_MESSAGE_LENGTH)
    paused = 0
    for tick in range(400):
        for _ in range(16):
            b.read(READ_CHUNK)
        paused += b.pause_recv
        if tick % 8 == 7:
            b.take()
    recv_bound = RECV_FLOOD + MAX_MESSAGE + READ_CHUNK
    out(f'  4 MB messages at 1 MiB a tick, one processed every 8 ticks: peak {b.peak} bytes, reading paused {paused} of 400 ticks; receive bound {recv_bound}')
    b = Buf(24 + 37)
    paused = 0
    for tick in range(50):
        for _ in range(16):
            b.read(READ_CHUNK)
        paused += b.pause_recv
        for _ in range(100):
            if b.take():
                b.push(1000024)
    send_bound = SEND_MAX + MAX_MESSAGE
    out(f'  getdata for 1 MB blocks from a peer that never reads: {b.send} bytes waiting to send, peak {b.peak}, reading paused {paused} of 50 ticks')
    fk = Rolling(50000, 0.000001, 0)
    fa = Rolling(5000, 0.001, 0)
    out(f'  per peer: receive {recv_bound} + send {send_bound} + filters {len(fk.data) * 8 + len(fa.data) * 8} = {recv_bound + send_bound + len(fk.data) * 8 + len(fa.data) * 8} bytes')
    tracked = dropped = 0
    for _ in range(10 * 50000):
        if tx_delay(tracked, 0, False, True, 0, False) is None:
            dropped += 1
        else:
            tracked += 1
    out(f'  10 inv messages of 50000 wtxids: {tracked} announcements tracked, {dropped} dropped; with the relay permission: {tx_delay(5000, 0, False, True, 0, True) is not None}')
    out(f'  request delay: outbound {tx_delay(0, 0, True, True, 1, False)} s, inbound {tx_delay(0, 0, False, True, 1, False)} s, '
        f'inbound txid with wtxid peers {tx_delay(0, 0, False, False, 1, False)} s, inbound with 100 in flight {tx_delay(0, 100, False, True, 1, False)} s')
    return '\n'.join(lines) + '\n'


def main():
    want = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'dos fixture: {want.count(chr(10))} lines agree with Core\'s transliterated net_processing.cpp, net.cpp, bloom.cpp and headerssync.cpp')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
