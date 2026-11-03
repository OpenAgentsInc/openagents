#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Bitcoin Core's pinned headerssync.cpp, net_processing.cpp, headers_sync_chainwork_tests.cpp and p2p_ibd_stalling.py decide each case outside Coax)
"""The cross-check of Bitcoin chapter 72. It computes every line the
chapter's example prints, in Python that shares no code with Coax, and
compares the transcript with the fixture:

  python3 -I coax/tests/bitcoin/sync.py                        # compare with the fixture
  python3 -I coax/tests/bitcoin/sync.py --coax coax/bin/coax   # also run the example

It transliterates Bitcoin Core's HeadersSyncState (headerssync.cpp), the
headers path of ProcessHeadersMessage with IsContinuationOfLowWorkHeadersSync,
TryLowWorkHeadersSync and GetAntiDoSWorkThreshold, and the block download
of net_processing.cpp: FindNextBlocksToDownload and FindNextBlocks,
BlockRequested, RemoveBlockRequest, the stall and download-timeout checks
in SendMessages and the stalling timeout's decay in BlockConnected. Headers
are mined as Core's tests mine them (GenerateHeaders, FinalizeHeader), the
regtest genesis block is built by chainparams.py, and blocks are checked
with merkle_reference.py's ComputeMerkleRoot.
"""
import collections
import hashlib
import importlib.util
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
spec = importlib.util.spec_from_file_location('addrman_oracle', HERE / 'addrman.py')
am = importlib.util.module_from_spec(spec)
spec.loader.exec_module(am)
import chainparams as CP  # noqa: E402
import merkle_reference as MR  # noqa: E402
import tx_reference as T  # noqa: E402
spec = importlib.util.spec_from_file_location('dos_oracle', HERE / 'dos.py')
dos = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dos)

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/headers-sync.out'
EXAMPLE = 'bitcoin/examples/headers-sync.coax'
for name in ['headerssync.cpp', 'headerssync.h', 'net_processing.h', 'chainparams.cpp', 'chain.h', 'hash_tests.cpp',
             'headers_sync_chainwork_tests.cpp', 'p2p_ibd_stalling.py']:
    am.PINS[name] = None
am.manifest_pins()
src = {name: am.pinned(name) for name in am.PINS}
NETP, HSS_CPP, TESTS, STALL = src['net_processing.cpp'], src['headerssync.cpp'], src['headers_sync_chainwork_tests.cpp'], src['p2p_ibd_stalling.py']


def const(text, pattern):
    m = re.search(pattern, text)
    assert m, pattern
    return int(m.group(1).replace("'", ''))


MAX_HEADERS_RESULTS = const(src['net_processing.h'], r'MAX_HEADERS_RESULTS = (\d+);')
BLOCK_DOWNLOAD_WINDOW = const(NETP, r'BLOCK_DOWNLOAD_WINDOW = (\d+);')
MAX_BLOCKS_IN_TRANSIT_PER_PEER = const(NETP, r'MAX_BLOCKS_IN_TRANSIT_PER_PEER = (\d+);')
STALL_DEFAULT = const(NETP, r'BLOCK_STALLING_TIMEOUT_DEFAULT\{(\d+)s\}')
STALL_MAX = const(NETP, r'BLOCK_STALLING_TIMEOUT_MAX\{(\d+)s\}')
MANUAL_COOLDOWN = 60 * const(NETP, r'MANUAL_PEER_BLOCK_DOWNLOAD_COOLDOWN\{(\d+)min\}')
TIMEOUT_BASE = float(re.search(r'BLOCK_DOWNLOAD_TIMEOUT_BASE = ([\d.]+);', NETP).group(1))
TIMEOUT_PER_PEER = float(re.search(r'BLOCK_DOWNLOAD_TIMEOUT_PER_PEER = ([\d.]+);', NETP).group(1))
MAX_FUTURE_BLOCK_TIME = 2 * 60 * 60
assert 'MAX_FUTURE_BLOCK_TIME = 2 * 60 * 60' in src['chain.h']
for line in ['std::min(2 * stalling_timeout, BLOCK_STALLING_TIMEOUT_MAX)',
             'std::max(std::chrono::duration_cast<std::chrono::seconds>(stalling_timeout * 0.85), BLOCK_STALLING_TIMEOUT_DEFAULT)',
             'near_chaintip_work = tip->nChainWork - std::min<arith_uint256>(144*GetBlockProof(*tip), tip->nChainWork);',
             'if (state.vBlocksInFlight.empty() && staller != -1) {',
             'state.m_downloading_since = std::max(state.m_downloading_since, GetTime<std::chrono::microseconds>());',
             'int nWindowEnd = state->pindexLastCommonBlock->nHeight + BLOCK_DOWNLOAD_WINDOW;',
             'int nMaxHeight = std::min<int>(state->pindexBestKnownBlock->nHeight, nWindowEnd + 1);',
             'if (vBlocks.size() == 0 && waitingfor != peer.m_id) {']:
    assert line in NETP, line
for line in ['m_max_commitments = 6 * max_seconds_since_start / m_params.commitment_period;',
             'm_redownloaded_headers.size() > m_params.redownload_buffer_size',
             'm_process_all_remaining_headers = true;']:
    assert line in HSS_CPP, line
CP_TEXT = src['chainparams.cpp']
REGTEST = CP_TEXT[CP_TEXT.index('class CRegTestParams'):]
PERIOD = const(REGTEST, r'\.commitment_period = (\d+)')
BUFFER = const(REGTEST, r'\.redownload_buffer_size = (\d+)')
TARGET_BLOCKS = const(TESTS, r'TARGET_BLOCKS\{([\d\']+)\}')
TEST_PERIOD = const(TESTS, r'COMMITMENT_PERIOD\{(\d+)\}')
TEST_BUFFER = TARGET_BLOCKS - (MAX_HEADERS_RESULTS + const(TESTS, r'REDOWNLOAD_BUFFER_SIZE\{TARGET_BLOCKS - \(MAX_HEADERS_RESULTS \+ (\d+)\)\}'))
assert 'CHAIN_WORK{TARGET_BLOCKS * 2}' in TESTS
# p2p_ibd_stalling.py: 1025 blocks, 5 peers, block 1 withheld from all.
assert 'NUM_BLOCKS = 1025' in STALL and 'NUM_PEERS = 5' in STALL

NOW = 1793664000   # 2026-11-03
SALT = (0x0123456789abcdef, 0xfedcba9876543210, 7)
NETS = {n['name']: n for n in CP.networks()}
RT = NETS['regtest']
assert RT['min_difficulty']          # PermittedDifficultyTransition is always true on regtest
GENESIS = CP.genesis_block(RT['genesis'])
POW_LIMIT = RT['powLimit']


def sha256d(b):
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def proof(bits):
    """GetBlockProof."""
    t = CP.target(bits)
    return (2**256) // (t + 1)


def short(h):
    return h[::-1].hex()[:8]


# ---- headers, as Core's tests make them
def header(version, prev, root, time, bits, nonce):
    return (version.to_bytes(4, 'little') + prev + root + time.to_bytes(4, 'little') + bits.to_bytes(4, 'little')
            + nonce.to_bytes(4, 'little'))


def fields(h):
    return dict(version=int.from_bytes(h[0:4], 'little'), prev=h[4:36], root=h[36:68], time=int.from_bytes(h[68:72], 'little'),
                bits=int.from_bytes(h[72:76], 'little'), nonce=int.from_bytes(h[76:80], 'little'))


def check_pow(h):
    bits = fields(h)['bits']
    t = CP.target(bits)
    return 0 < t <= POW_LIMIT and int.from_bytes(sha256d(h), 'little') <= t


def generate(count, prev, version, prev_time, root_byte, bits=0x207fffff):
    """GenerateHeaders: each header one second after the last, nonce raised
    from 0 until it meets its target (FinalizeHeader)."""
    out = []
    for _ in range(count):
        prev_time += 1
        nonce = 0
        while True:
            h = header(version, prev, bytes([root_byte]) + bytes(31), prev_time, bits, nonce)
            if check_pow(h):
                break
            nonce += 1
        out.append(h)
        prev = sha256d(h)
    return out


# ---- the block index, as far as headers sync needs it
class Tree:
    def __init__(self, genesis_header):
        g = fields(genesis_header)
        self.e = [dict(hash=sha256d(genesis_header), prev=-1, height=0, work=proof(g['bits']), bits=g['bits'], time=g['time'],
                       version=g['version'])]
        self.index = {self.e[0]['hash']: 0}
        self.best_header = 0
        self.active = [0]      # nothing is connected: the active chain is genesis

    def mtp(self, i):
        times = []
        while i >= 0 and len(times) < 11:
            times.append(self.e[i]['time'])
            i = self.e[i]['prev']
        return sorted(times)[len(times) // 2]

    def ancestor(self, i, height):
        while self.e[i]['height'] > height:
            i = self.e[i]['prev']
        return i

    def locator(self, i):
        """LocatorEntries: ten back one at a time, then doubling, then genesis."""
        out, step = [], 1
        while True:
            out.append(self.e[i]['hash'])
            if self.e[i]['height'] == 0:
                return out
            height = max(self.e[i]['height'] - step, 0)
            i = self.ancestor(i, height)
            if len(out) > 10:
                step *= 2

    def accept(self, h, now):
        """AcceptBlockHeader for a regtest header that passes."""
        f = fields(h)
        hash_ = sha256d(h)
        if hash_ in self.index:
            return self.index[hash_]
        prev = self.index[f['prev']]
        height = self.e[prev]['height'] + 1
        assert check_pow(h) and f['time'] > self.mtp(prev) and f['time'] <= now + MAX_FUTURE_BLOCK_TIME
        assert f['version'] >= 4 or height < min(RT['BIP34Height'], RT['BIP65Height'], RT['BIP66Height'])
        self.e.append(dict(hash=hash_, prev=prev, height=height, work=self.e[prev]['work'] + proof(f['bits']), bits=f['bits'],
                           time=f['time'], version=f['version']))
        i = len(self.e) - 1
        self.index[hash_] = i
        if self.e[self.best_header]['work'] < self.e[i]['work']:
            self.best_header = i
        return i


# ---- HeadersSyncState (headerssync.cpp)
PRESYNC, REDOWNLOAD, FINAL = 'PRESYNC', 'REDOWNLOAD', 'FINAL'


class SystemClockError(Exception):
    pass


class HeadersSyncState:
    def __init__(self, tree, start, period, buffer_size, minimum_required_work, now, offset, k0, k1):
        self.offset, self.k0, self.k1 = offset, k0, k1
        self.period, self.buffer_size = period, buffer_size
        self.tree, self.start = tree, start
        self.minimum_required_work = minimum_required_work
        e = tree.e[start]
        self.current_chain_work = e['work']
        self.last_hash = e['hash']
        self.current_height = e['height']
        max_seconds_since_start = now - tree.mtp(start) + MAX_FUTURE_BLOCK_TIME
        if max_seconds_since_start < 0:
            raise SystemClockError
        self.max_commitments = 6 * max_seconds_since_start // period
        self.commitments = collections.deque()
        self.redownloaded = collections.deque()
        self.state = PRESYNC
        self.process_all = False

    def commit(self, h):
        return am.siphash(self.k0, self.k1, sha256d(h)) & 1

    def finalize(self):
        self.commitments.clear()
        self.redownloaded.clear()
        self.process_all = False
        self.current_height = 0
        self.state = FINAL

    def process(self, headers, full):
        """ProcessNextHeaders: (success, request_more, released headers)."""
        success, more, released = False, False, []
        if not headers or self.state == FINAL:
            return success, more, released
        if self.state == PRESYNC:
            success = self.store_commitments(headers)
            if success and (full or self.state == REDOWNLOAD):
                more = True
        elif self.state == REDOWNLOAD:
            success = all(self.store_redownloaded(h) for h in headers)
            if success:
                released = self.pop_ready()
                if not (not self.redownloaded and self.process_all) and full:
                    more = True
        if not (success and more):
            self.finalize()
        return success, more, released

    def store_commitments(self, headers):
        if fields(headers[0])['prev'] != self.last_hash:
            return False
        for h in headers:
            next_height = self.current_height + 1
            if next_height % self.period == self.offset:
                self.commitments.append(self.commit(h))
                if len(self.commitments) > self.max_commitments:
                    return False
            self.current_chain_work += proof(fields(h)['bits'])
            self.last_hash = sha256d(h)
            self.current_height = next_height
        if self.current_chain_work >= self.minimum_required_work:
            s = self.tree.e[self.start]
            self.redownloaded.clear()
            self.buffer_last_height = s['height']
            self.buffer_first_prev = s['hash']
            self.buffer_last_hash = s['hash']
            self.redownload_work = s['work']
            self.state = REDOWNLOAD
        return True

    def store_redownloaded(self, h):
        f = fields(h)
        next_height = self.buffer_last_height + 1
        if f['prev'] != self.buffer_last_hash:
            return False
        self.redownload_work += proof(f['bits'])
        if self.redownload_work >= self.minimum_required_work:
            self.process_all = True
        if not self.process_all and next_height % self.period == self.offset:
            if not self.commitments:
                return False
            if self.commit(h) != self.commitments.popleft():
                return False
        self.redownloaded.append(h[0:4] + h[36:80])      # CompressedHeader: no previous hash
        self.buffer_last_hash = sha256d(h)
        self.buffer_last_height = next_height
        return True

    def pop_ready(self):
        out = []
        while len(self.redownloaded) > self.buffer_size or (self.redownloaded and self.process_all):
            c = self.redownloaded.popleft()
            full = c[0:4] + self.buffer_first_prev + c[4:]
            out.append(full)
            self.buffer_first_prev = sha256d(full)
        return out

    def locator(self):
        if self.state == FINAL:
            return None
        first = self.last_hash if self.state == PRESYNC else self.buffer_last_hash
        return [first] + self.tree.locator(self.start)


# ---- ProcessHeadersMessage (net_processing.cpp), the paths the example takes
class HeadersPeer:
    def __init__(self):
        self.sync = None


def threshold(tree, min_work):
    tip = tree.e[tree.active[-1]]
    near = tip['work'] - min(144 * proof(tip['bits']), tip['work'])
    return max(near, min_work)


def on_headers(peer, tree, headers, now, min_work):
    """(what happened, headers stored, presync failed, locator to send)."""
    full = len(headers) == MAX_HEADERS_RESULTS
    for k, h in enumerate(headers):
        assert check_pow(h) and (k == 0 or fields(h)['prev'] == sha256d(headers[k - 1]))
    locator, failed, validated = None, False, False
    if peer.sync:      # IsContinuationOfLowWorkHeadersSync
        success, more, released = peer.sync.process(headers, full)
        if more:
            locator = peer.sync.locator()
        if peer.sync.state == FINAL:
            peer.sync = None
        if success:
            if not released:
                return 'presync', 0, failed, locator
            headers, validated = released, True
        else:
            failed = True
    have_sync = peer.sync is not None
    start = tree.index[fields(headers[0])['prev']]
    last = tree.index.get(sha256d(headers[-1]))
    if last is not None and tree.ancestor(tree.best_header, tree.e[last]['height']) == last:
        validated = True
    if not validated:  # TryLowWorkHeadersSync
        total = tree.e[start]['work'] + sum(proof(fields(h)['bits']) for h in headers)
        minimum = threshold(tree, min_work)
        if total < minimum:
            if len(headers) != MAX_HEADERS_RESULTS:
                return 'low work', 0, failed, locator
            peer.sync = HeadersSyncState(tree, start, PERIOD, BUFFER, minimum, now, SALT[2] % PERIOD, SALT[0], SALT[1])
            success, more, released = peer.sync.process(headers, True)
            assert not released
            if more:
                locator = peer.sync.locator()
            if peer.sync.state == FINAL:
                peer.sync = None
            return 'presync', 0, failed, locator
    for h in headers:
        tree.accept(h, now)
    if full and not have_sync:
        locator = tree.locator(tree.index[sha256d(headers[-1])])
    return 'stored', len(headers), failed, locator


def find_fork(chain, locator):
    """FindForkInGlobalIndex over the peer's chain of hashes."""
    for h in locator:
        if h in chain:
            return chain.index(h)
    return 0


def sync_from(out, label, tree, chain_hashes, raws, min_work, swap=None):
    out(f'  {label}')
    peer = HeadersPeer()
    locator = tree.locator(0)
    rnd, swapped = 0, False
    while locator:
        rnd += 1
        if swap and peer.sync and peer.sync.state == REDOWNLOAD:
            swapped = True
        hashes, raw = swap if swapped else (chain_hashes, raws)
        fork = find_fork(hashes, locator)
        sent = raw[fork:fork + MAX_HEADERS_RESULTS]
        what, stored, failed, locator = on_headers(peer, tree, sent, NOW, min_work)
        line = f'    {rnd}. {len(sent)} headers after {fork}: {what}'
        if failed:
            line += ' (the presync failed; these headers start another)'
        if what == 'presync':
            s = peer.sync
            if s is None:
                line += ', FINAL'
            elif s.state == PRESYNC:
                line += f', PRESYNC at {s.current_height}, {len(s.commitments)} commitments'
            else:
                line += f', REDOWNLOAD, {len(s.redownloaded)} waiting, {len(s.commitments)} commitments left'
        if what == 'stored':
            b = tree.e[tree.best_header]
            line += f' {stored}, best header {b["height"]} ({short(b["hash"])})'
        out(line)
    out(f'    block index: {len(tree.e)} entries')


def peer_chain(genesis_hash, n, root):
    raw = generate(n, genesis_hash, 4, RT['genesis']['time'], root)
    return [genesis_hash] + [sha256d(h) for h in raw], raw


def headers_sync(out):
    out('headers sync from genesis (regtest, minimum chain work of 2200 blocks)')
    tree = Tree(GENESIS[:80])
    g = tree.e[0]['hash']
    min_work = 2 * 2200
    assert proof(RT['genesis']['bits']) == 2
    honest = peer_chain(g, 2500, 0)
    sync_from(out, 'an honest peer with 2500 blocks', tree, *honest, min_work)
    low = peer_chain(g, 2100, 1)
    sync_from(out, 'a peer with 2100 blocks, too little work', tree, *low, min_work)
    b, c = peer_chain(g, 2500, 2), peer_chain(g, 2100, 3)
    sync_from(out, 'a peer that presyncs 2500 blocks, then serves 2100 others', tree, *b, min_work, swap=c)


# ---- headers_sync_chainwork_tests.cpp
def core_tests(out):
    out(f"Core's headers_sync_chainwork_tests ({TARGET_BLOCKS} blocks, a commitment every {TEST_PERIOD}, a buffer of {TEST_BUFFER})")
    tree = Tree(GENESIS[:80])
    g = tree.e[0]
    work = TARGET_BLOCKS * 2
    first = generate(TARGET_BLOCKS - 1, g['hash'], 1, g['time'], 0)
    second = generate(TARGET_BLOCKS - 2, g['hash'], 1, g['time'], 1)

    def init(now=NOW):
        return HeadersSyncState(tree, 0, TEST_PERIOD, TEST_BUFFER, work, now, SALT[2], SALT[0], SALT[1])

    def step(s, r):
        success, more, released = r
        text = f' {s.state}, {"ok" if success else "failed"}'
        if more:
            text += ', more'
        if released:
            text += f', {len(released)} released'
        return text + ';'
    s = init()
    line = '  sneaky_redownload:'
    line += step(s, s.process(first[:1], True))
    line += step(s, s.process(first[1:], True))
    line += step(s, s.process(second, True))
    out(line)
    for full in (False, True):
        s = init()
        line = f'  happy_path, {"full" if full else "short"} messages:'
        line += step(s, s.process(first, full))
        line += step(s, s.process(first[:TEST_BUFFER], True))
        line += step(s, s.process(first[TEST_BUFFER:TEST_BUFFER + 1], True))
        r = s.process(first[TEST_BUFFER + 1:], full)
        line += step(s, r)
        assert sha256d(r[2][-1]) == sha256d(first[-1])
        out(line + f' the last released is {short(sha256d(r[2][-1]))}')
    s = init()
    line = '  too_little_work:'
    line += step(s, s.process(second[:1], True))
    line += step(s, s.process(second[1:], False))
    out(line)
    at = g['time'] - MAX_FUTURE_BLOCK_TIME

    def starts(now):
        try:
            init(now)
            return True
        except SystemClockError:
            return False
    out(f'  system_clock_lagging_behind_chain_start: 2 hours before genesis {"starts" if starts(at) else "refuses"}, '
        f'a second more {"starts" if starts(at - 1) else "is SystemClockError"}')


# ---- block download (net_processing.cpp)
class Peer:
    def __init__(self):
        self.live, self.manual = True, False
        self.best = None            # pindexBestKnownBlock, a height on the one chain
        self.common = None          # pindexLastCommonBlock
        self.flight = []            # vBlocksInFlight
        self.stalling_since = 0     # ms
        self.downloading_since = 0
        self.paused_until = 0


class Download:
    """One chain of n blocks above genesis; every entry is its height."""

    def __init__(self, n, peers):
        self.n = n
        self.have = [False] * (n + 1)
        self.chain_tx = [False] * (n + 1)    # HaveNumChainTxs
        self.have[0] = self.chain_tx[0] = True
        self.requested = {}                  # mapBlocksInFlight: height -> peer
        self.tip = 0
        self.timeout = STALL_DEFAULT
        self.peers = peers
        self.downloading_from = 0
        self.spacing = RT['spacing']

    def find_next(self, p, count):
        """FindNextBlocksToDownload and FindNextBlocks: (blocks, staller)."""
        peer = self.peers[p]
        if count == 0 or peer.best is None or peer.best < self.tip:
            return [], -1
        fork = min(peer.best, self.tip)       # LastCommonAncestor on one chain
        if peer.common is None or fork > peer.common:
            peer.common = fork
        if peer.common == peer.best:
            return [], -1
        window_end = peer.common + BLOCK_DOWNLOAD_WINDOW
        max_height = min(peer.best, window_end + 1)
        blocks, waiting = [], -1
        for h in range(peer.common + 1, max_height + 1):
            if self.have[h]:
                if self.chain_tx[h]:
                    peer.common = h
                continue
            if h in self.requested:
                if waiting == -1:
                    waiting = self.requested[h]
                continue
            if h > window_end:
                return blocks, (waiting if not blocks and waiting != p else -1)
            blocks.append(h)
            if len(blocks) == count:
                return blocks, -1
        return blocks, -1

    def requested_from(self, p, h, now):
        peer = self.peers[p]
        peer.flight.append(h)
        self.requested[h] = p
        if len(peer.flight) == 1:
            peer.downloading_since = now
            self.downloading_from += 1

    def remove_request(self, h, p, now):
        if self.requested.get(h) != p:
            return
        peer = self.peers[p]
        if peer.flight[0] == h:
            peer.downloading_since = max(peer.downloading_since, now)
        peer.flight.remove(h)
        if not peer.flight:
            self.downloading_from -= 1
        peer.stalling_since = 0
        del self.requested[h]

    def disconnect(self, p):
        peer = self.peers[p]
        for h in peer.flight:
            del self.requested[h]
        if peer.flight:
            self.downloading_from -= 1
        peer.flight, peer.live = [], False

    def send_messages(self, p, now):
        """(verdict, requested, staller, stall started)."""
        peer = self.peers[p]
        if not peer.live:
            return 'ok', [], -1, False
        if peer.stalling_since and peer.stalling_since < now - 1000 * self.timeout:
            if peer.manual:
                peer.paused_until = now + 1000 * MANUAL_COOLDOWN
                while peer.flight:
                    self.remove_request(peer.flight[0], p, now)
                verdict = 'paused'
            else:
                self.disconnect(p)
                verdict = 'stalled'
            self.timeout = min(2 * self.timeout, STALL_MAX)
            return verdict, [], -1, False
        if peer.flight:
            others = self.downloading_from - 1
            if now > peer.downloading_since + 1000 * self.spacing * (TIMEOUT_BASE + TIMEOUT_PER_PEER * others):
                self.disconnect(p)
                return 'timed out', [], -1, False
        if now < peer.paused_until or len(peer.flight) >= MAX_BLOCKS_IN_TRANSIT_PER_PEER:
            return 'ok', [], -1, False
        blocks, staller = self.find_next(p, MAX_BLOCKS_IN_TRANSIT_PER_PEER - len(peer.flight))
        for h in blocks:
            self.requested_from(p, h, now)
        started = False
        if not peer.flight and staller != -1 and self.peers[staller].stalling_since == 0:
            self.peers[staller].stalling_since = now
            started = True
        return 'ok', blocks, staller, started

    def block_received(self, p, h, now):
        """A requested block arrives: stored, then connected along with every
        stored child (each BlockConnected lowers the stalling timeout)."""
        if self.requested.get(h) != p:
            return -1
        self.remove_request(h, p, now)
        self.have[h] = True
        if not self.chain_tx[h - 1]:
            return 0
        n = 0
        while h <= self.n and self.have[h]:
            self.chain_tx[h] = True
            self.tip = h
            n += 1
            if self.timeout != STALL_DEFAULT:
                self.timeout = max(int(self.timeout * 0.85), STALL_DEFAULT)
            h += 1
        return n


def settle(d, now, withheld):
    requested = stalls = dropped = 0
    last = -1
    moved = True
    while moved:
        moved = False
        for p, peer in enumerate(d.peers):
            if not peer.live:
                continue
            verdict, blocks, _, started = d.send_messages(p, now)
            if verdict in ('stalled', 'timed out'):
                dropped += 1
                last = p
                moved = True
            if verdict == 'paused':
                moved = True
            stalls += started
            for h in blocks:
                requested += 1
                moved = True
                if h not in withheld:
                    d.block_received(p, h, now)
    return requested, stalls, dropped, last


def in_flight(d):
    return sum(len(p.flight) for p in d.peers)


def stalling(out):
    out("Core's p2p_ibd_stalling: 1025 blocks, 5 peers, blocks 1 and 501 withheld")
    peers = [Peer() for _ in range(5)]
    d = Download(1025, peers)
    stall = (1, 501)
    now = 1000000
    asked = 0
    for p in range(5):
        peers[p].best = 1024
        asked += settle(d, now, stall)[0]
    out(f'    headers to 1024 from each peer in turn: {asked} requested, tip {d.tip}, {in_flight(d)} in flight '
        f'(blocks 1 and 501, both from peer {d.requested[stall[1]]})')

    def line(label):
        requested, stalls, dropped, last = settle(d, now, stall)
        text = f'    {label}: {requested} requested, tip {d.tip}, {in_flight(d)} in flight, timeout {d.timeout} s'
        if stalls:
            text += ', stall started'
        if dropped:
            text += f', peer {last} disconnected for stalling'
        out(text)
    now += 3000
    line('3 s later')
    for p in range(5):
        peers[p].best = 1025
    line('headers to 1025')
    for wait in (3000, 3000, 2000, 7000, 2000):
        now += wait
        line(f'{wait // 1000} s later')
    holder = d.requested[stall[0]]
    linked = d.block_received(holder, stall[0], now)
    out(f'    block 1 arrives from peer {holder}: {linked} blocks link, tip {d.tip}, timeout {d.timeout} s')


def parallel(out):
    out('3000 blocks from 4 peers serving a block every 4, 6, 9 and 3000 ms')
    peers = [Peer() for _ in range(4)]
    d = Download(3000, peers)
    for p in peers:
        p.best = 3000
    pace = [4, 6, 9, 3000]
    nxt, served = [0] * 4, [0] * 4
    lead = t = starts = 0
    shown = -1
    while d.tip != 3000 and t < 600000:
        for p in range(4):
            if peers[p].live and peers[p].flight and nxt[p] <= t:
                d.block_received(p, peers[p].flight[0], t)
                served[p] += 1
                nxt[p] = t + pace[p]
        for p in range(4):
            was = len(peers[p].flight)
            verdict, blocks, staller, started = d.send_messages(p, t)
            if verdict == 'stalled':
                out(f'    at {t} ms: peer {p} disconnected for stalling at tip {d.tip}; timeout now {d.timeout} s')
            if started:
                starts += 1
                if staller != shown:
                    shown = staller
                    out(f'    at {t} ms: the window is full; peer {staller} holds block {peers[staller].flight[0]}, stall started')
            if was == 0 and peers[p].flight:
                nxt[p] = t + pace[p]
            for h in blocks:
                lead = max(lead, h - d.tip)
        t += 1
    out(f'    done at {t} ms, tip {d.tip}; blocks from each peer: {served[0]} {served[1]} {served[2]} {served[3]}; '
        f'{starts} stalls started; the furthest request was {lead} blocks past the tip')


# ---- a block against the header asked for
def read_txs(b):
    r = T.Reader(b, (1 << 62, 1 << 62, 1 << 62))
    r.at = 80
    txs = []
    for _ in range(r.compact()):
        start = r.at
        r.take(4)
        ins = T.read_inputs(r, 1 << 62)
        assert ins, 'segwit blocks are not needed here'
        T.read_outputs(r, 1 << 62)
        r.take(4)
        txs.append(b[start:r.at])
    assert r.at == len(b)
    return txs


def check_block(b, want):
    """The header is the one asked for, and CheckMerkleRoot: Core's
    BLOCK_INVALID_HEADER for the first, BLOCK_MUTATED for the second."""
    if sha256d(b[:80]) != want:
        return 'BLOCK_INVALID_HEADER'
    root, mutated = MR.compute_root([sha256d(tx) for tx in read_txs(b)])
    if root != b[36:68] or mutated:
        return 'BLOCK_MUTATED'
    return 'ok'


def checks(out):
    out('blocks against their headers')
    g = GENESIS
    out(f'  regtest genesis, {len(g)} bytes: {check_block(g, sha256d(g[:80]))}')
    b = bytearray(g)
    b[36] ^= 1
    b = bytes(b)
    out(f'  its Merkle root changed, against the header asked for: {check_block(b, sha256d(g[:80]))}')
    out(f'  the same, asked for by its own hash: {check_block(b, sha256d(b[:80]))}')
    tx = g[81:]
    root = sha256d(g[36:68] + g[36:68])
    b = g[:36] + root + g[68:80] + b'\x02' + tx + tx
    out(f"  its coinbase twice, under the pair's root: {check_block(b, sha256d(b[:80]))}")
    # MaybePunishNodeForBlock, as dos.py transliterates it: a full block,
    # not a compact one, from an outbound peer.
    word = lambda r: 'misbehaving' if dos.punish_block(r, False, False) else 'no'
    out(f'  punished: {word("BLOCK_MUTATED")} for a mutated block, {word("BLOCK_INVALID_HEADER")} for a wrong header')


def transcript():
    lines = []
    out = lines.append
    headers_sync(out)
    core_tests(out)
    out('block download')
    stalling(out)
    parallel(out)
    checks(out)
    return '\n'.join(lines) + '\n'


def main():
    want = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'sync fixture: {want.count(chr(10))} lines agree with Core\'s transliterated headerssync.cpp and net_processing.cpp')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
