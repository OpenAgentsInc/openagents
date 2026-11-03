#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Bitcoin Core's pinned chain.h, chain.cpp, validation.cpp, node/blockstorage.cpp and pow.cpp decide each case outside Coax)
"""The cross-check of Bitcoin chapter 59. It computes every line the
chapter's example prints, in Python that shares no code with Coax, and
compares the transcript with the fixture:

  python3 -I coax/tests/bitcoin/blockindex.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/blockindex.py --coax coax/bin/coax   # also run the example

It transliterates CBlockIndex with real object references: GetSkipHeight,
GetAncestor, BuildSkip, LastCommonAncestor, LocatorEntries, CChain's SetTip
and FindFork (chain.cpp); AddToBlockIndex and CBlockIndexWorkComparator
(node/blockstorage.cpp); AcceptBlockHeader, ContextualCheckBlockHeader,
ReceivedBlockTransactions, TryAddBlockIndexCandidate, FindMostWorkChain,
PruneBlockIndexCandidates, InvalidBlockFound, InvalidChainFound,
SetBlockFailureFlags, RecalculateBestHeader and InvalidateBlock's
candidate rules (validation.cpp); and GetNextWorkRequired,
CalculateNextWorkRequired and GetBitsProof (pow.cpp, chain.cpp). Pointer
order, the comparator's last tie-break, is the order entries were
created, as the Coax library's positions are.
"""
import hashlib
import json
import re
import struct
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
UP = HERE.parent / 'vectors/upstream'
FIXTURE = REPO / 'bitcoin/fixtures/block-index.out'
EXAMPLE = 'bitcoin/examples/block-index.coax'
MANIFEST = {e['path']: e['sha256'] for e in json.loads((HERE.parent / 'vectors/manifest.json').read_text())['files']}


def pinned(path):
    data = (UP / path).read_bytes()
    assert hashlib.sha256(data).hexdigest() == MANIFEST['upstream/' + path], path
    return data.decode()


CHAIN_H, CHAIN_CPP = pinned('bitcoin-core/chain.h'), pinned('bitcoin-core/chain.cpp')
VALIDATION, VALIDATION_H = pinned('bitcoin-core/validation.cpp'), pinned('bitcoin-core/validation.h')
STORAGE, POW, PARAMS = pinned('bitcoin-core/blockstorage.cpp'), pinned('bitcoin-core/pow.cpp'), pinned('bitcoin-core/chainparams.cpp')
HEADERS = pinned('mainnet/headers-0-2016.txt')

# Constants and the lines this file transliterates, as the pinned files have them.
STATUS = {name: int(v) for name, v in re.findall(r'(BLOCK_[A-Z_]+)\s*=\s*(\d+),', CHAIN_H)}
assert STATUS['BLOCK_VALID_TREE'] == 2 and STATUS['BLOCK_VALID_TRANSACTIONS'] == 3 and STATUS['BLOCK_HAVE_DATA'] == 8
assert STATUS['BLOCK_FAILED_VALID'] == 32 and STATUS['BLOCK_OPT_WITNESS'] == 128
VALID_MASK = 1 | 2 | 3 | 4 | 5
TREE, TRANSACTIONS = STATUS['BLOCK_VALID_TREE'], STATUS['BLOCK_VALID_TRANSACTIONS']
HAVE_DATA, FAILED, OPT_WITNESS = STATUS['BLOCK_HAVE_DATA'], STATUS['BLOCK_FAILED_VALID'], STATUS['BLOCK_OPT_WITNESS']
SEQ_INIT = int(re.search(r'SEQ_ID_INIT_FROM_DISK = (\d+);', CHAIN_H).group(1))
assert 'int32_t nBlockSequenceId GUARDED_BY(::cs_main) = SEQ_ID_INIT_FROM_DISK + 1;' in VALIDATION_H
MAX_FUTURE = eval(re.search(r'MAX_FUTURE_BLOCK_TIME = ([\d *]+);', CHAIN_H).group(1))
MEDIAN_SPAN = int(re.search(r'nMedianTimeSpan = (\d+);', CHAIN_H).group(1))
assert 'return (height & 1) ? InvertLowestOne(InvertLowestOne(height - 1)) + 1 : InvertLowestOne(height);' in CHAIN_CPP
assert 'if (have.size() > 10) step *= 2;' in CHAIN_CPP
assert '(heightSkip > height && !(heightSkipPrev < heightSkip - 2 &&' in CHAIN_CPP
assert 'if (pa->nSequenceId < pb->nSequenceId) return false;' in STORAGE and 'if (pa < pb) return false;' in STORAGE
assert 'pindexNew->nSequenceId = SEQ_ID_INIT_FROM_DISK;' in STORAGE
assert 'if (best_header == nullptr || best_header->nChainWork < pindexNew->nChainWork) {' in STORAGE
assert 'return state.Invalid(BlockValidationResult::BLOCK_INVALID_PREV, "bad-prevblk");' in VALIDATION
assert '"duplicate-invalid"' in VALIDATION and '"prev-blk-not-found"' in VALIDATION
assert 'if (!(entry.second.nStatus & BLOCK_FAILED_VALID) && m_best_header->nChainWork < entry.second.nChainWork) {' in VALIDATION
assert 'if (state.GetResult() != BlockValidationResult::BLOCK_MUTATED) {' in VALIDATION


def section(name):
    start = PARAMS.index(f'class {name}')
    end = PARAMS.find('\nclass ', start + 1)
    return PARAMS[start:end if end > 0 else len(PARAMS)]


def params(name):
    s = section(name)
    p = {}
    for key in ['BIP34Height', 'BIP65Height', 'BIP66Height', 'SegwitHeight', 'nPowTargetSpacing']:
        p[key] = eval(re.search(rf'consensus\.{key} = ([\d *]+);', s).group(1))
    p['nPowTargetTimespan'] = eval(re.search(r'consensus\.nPowTargetTimespan = ([\d *]+);', s).group(1))
    p['powLimit'] = int(re.search(r'consensus\.powLimit = uint256\{"([0-9a-f]+)"\};', s).group(1), 16)
    for key in ['fPowAllowMinDifficultyBlocks', 'fPowNoRetargeting']:
        p[key] = re.search(rf'consensus\.{key} = (\w+);', s).group(1) == 'true'
    t, nonce, bits, version = re.search(r'genesis = CreateGenesisBlock\((\d+), (\d+), (0x[0-9a-f]+), (\d+), 50 \* COIN\);', s).groups()
    p['genesis'] = (int(t), int(nonce), int(bits, 16), int(version))
    p['hashGenesisBlock'] = re.search(r'consensus\.hashGenesisBlock == uint256\{"([0-9a-f]+)"\}', s).group(1)
    return p


MAIN, REGTEST = params('CMainParams'), params('CRegTestParams')
assert 'consensus.enforce_BIP94 = false;' in section('CMainParams')
assert 'consensus.enforce_BIP94 = opts.enforce_bip94;' in section('CRegTestParams')   # off unless a test turns it on
GENESIS_ROOT = bytes.fromhex('4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b')[::-1]


def sha256d(b):
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def set_compact(bits):
    size, word = bits >> 24, bits & 0x007fffff
    value = word >> 8 * (3 - size) if size <= 3 else word << 8 * (size - 3)
    negative = word != 0 and bits & 0x00800000 != 0
    overflow = word != 0 and (size > 34 or (word > 0xff and size > 33) or (word > 0xffff and size > 32))
    return value, negative, overflow


def get_compact(n):
    size = (n.bit_length() + 7) // 8
    compact = n << 8 * (3 - size) if size <= 3 else n >> 8 * (size - 3)
    if compact & 0x00800000:
        compact >>= 8
        size += 1
    return compact | size << 24


def bits_proof(bits):
    target, negative, overflow = set_compact(bits)
    if negative or overflow or target == 0:
        return 0
    return ((2**256 - 1 - target) // (target + 1)) + 1


def check_pow(h, bits, p):
    target, negative, overflow = set_compact(bits)
    if negative or target == 0 or overflow or target > p['powLimit']:
        return False
    return int.from_bytes(h, 'little') <= target


class Header:
    def __init__(self, raw):
        self.raw = raw
        self.version, = struct.unpack('<i', raw[:4])
        self.prev, self.merkle = raw[4:36], raw[36:68]
        self.time, self.bits, self.nonce = struct.unpack('<III', raw[68:80])
        self.hash = sha256d(raw)


def header(version, prev, merkle, time, bits, nonce):
    return struct.pack('<i', version) + prev + merkle + struct.pack('<III', time, bits, nonce)


class Index:
    def __init__(self, h, pos):
        self.h, self.pos = h, pos
        self.hash = h.hash
        self.pprev = self.pskip = None
        self.height, self.work, self.status, self.seq = 0, 0, 0, SEQ_INIT
        self.ntx = self.chain_tx = 0
        self.time, self.bits, self.time_max = h.time, h.bits, 0

    def ancestor_walk(self, height):
        if height > self.height or height < 0:
            return None, 0
        walk, at, steps = self, self.height, 0
        while at > height:
            skip, skip_prev = skip_height(at), skip_height(at - 1)
            if walk.pskip is not None and (skip == height or (skip > height and not (skip_prev < skip - 2 and skip_prev >= height))):
                walk, at = walk.pskip, skip
            else:
                walk, at = walk.pprev, at - 1
            steps += 1
        return walk, steps

    def ancestor(self, height):
        return self.ancestor_walk(height)[0]

    def build_skip(self):
        if self.pprev:
            self.pskip = self.pprev.ancestor(skip_height(self.height))

    def is_valid(self, level):
        return not self.status & FAILED and self.status & VALID_MASK >= level

    def raise_validity(self, level):
        if self.status & FAILED:
            return False
        if self.status & VALID_MASK < level:
            self.status = self.status & ~VALID_MASK | level
            return True
        return False

    def mtp(self):
        times, p = [], self
        while len(times) < MEDIAN_SPAN and p:
            times.append(p.time)
            p = p.pprev
        times.sort()
        return times[len(times) // 2]


def skip_height(height):
    if height < 2:
        return 0
    lowest = lambda n: n & (n - 1)
    return lowest(lowest(height - 1)) + 1 if height & 1 else lowest(height)


def last_common_ancestor(a, b):
    if a.height > b.height:
        a = a.ancestor(b.height)
    elif b.height > a.height:
        b = b.ancestor(a.height)
    while a is not b:
        while a.pskip is not b.pskip:
            a, b = a.pskip, b.pskip
        a, b = a.pprev, b.pprev
    return a


def locator(index):
    step, have = 1, []
    while index:
        have.append(index)
        if index.height == 0:
            break
        index = index.ancestor(max(index.height - step, 0))
        if len(have) > 10:
            step *= 2
    return have


def worse(a, b):
    """CBlockIndexWorkComparator()(a, b)."""
    if a.work != b.work:
        return a.work < b.work
    if a.seq != b.seq:
        return a.seq > b.seq
    return a.pos > b.pos


class Node:
    """A ChainstateManager with one chainstate: the block index, the active chain and its candidates."""

    def __init__(self, p):
        self.p, self.index, self.chain = p, {}, []
        self.best_header = self.best_invalid = None
        self.seq, self.candidates, self.unlinked = SEQ_INIT + 1, [], []

    def tip(self):
        return self.chain[-1] if self.chain else None

    def contains(self, b):
        return b.height < len(self.chain) and self.chain[b.height] is b

    def set_tip(self, b):
        del self.chain[b.height + 1:]
        self.chain += [None] * (b.height + 1 - len(self.chain))
        while b and self.chain[b.height] is not b:
            self.chain[b.height] = b
            b = b.pprev

    def find_fork(self, b):
        if b.height > len(self.chain) - 1:
            b = b.ancestor(len(self.chain) - 1)
        while b and not self.contains(b):
            b = b.pprev
        return b

    def next_work(self, last, time):
        p = self.p
        interval = p['nPowTargetTimespan'] // p['nPowTargetSpacing']
        limit = get_compact(p['powLimit'])
        if (last.height + 1) % interval != 0:
            if p['fPowAllowMinDifficultyBlocks']:
                if time > last.time + p['nPowTargetSpacing'] * 2:
                    return limit
                b = last
                while b.pprev and b.height % interval != 0 and b.bits == limit:
                    b = b.pprev
                return b.bits
            return last.bits
        first = last.ancestor(last.height - (interval - 1))
        if p['fPowNoRetargeting']:
            return last.bits
        span = min(max(last.time - first.time, p['nPowTargetTimespan'] // 4), p['nPowTargetTimespan'] * 4)
        new = set_compact(last.bits)[0] * span // p['nPowTargetTimespan']
        return get_compact(min(new, p['powLimit']))

    def contextual(self, h, prev, now):
        height = prev.height + 1
        if h.bits != self.next_work(prev, h.time):
            return 'bad-diffbits'
        if h.time <= prev.mtp():
            return 'time-too-old'
        if h.time > now + MAX_FUTURE:
            return 'time-too-new'
        p = self.p
        if (h.version < 2 and height >= p['BIP34Height']) or (h.version < 3 and height >= p['BIP66Height']) or \
           (h.version < 4 and height >= p['BIP65Height']):
            return 'bad-version'
        return None

    def accept_header(self, raw, now):
        h = Header(raw)
        if h.hash[::-1].hex() != self.p['hashGenesisBlock']:
            if h.hash in self.index:
                b = self.index[h.hash]
                return ('duplicate-invalid' if b.status & FAILED else 'ok'), b
            if not check_pow(h.hash, h.bits, self.p):
                return 'high-hash', None
            prev = self.index.get(h.prev)
            if prev is None:
                return 'prev-blk-not-found', None
            if prev.status & FAILED:
                return 'bad-prevblk', None
            reason = self.contextual(h, prev, now)
            if reason:
                return reason, None
        return 'ok', self.add(h)

    def add(self, h):
        if h.hash in self.index:
            return self.index[h.hash]
        b = Index(h, len(self.index))
        self.index[h.hash] = b
        prev = self.index.get(h.prev)
        if prev is not None and prev is not b:
            b.pprev, b.height = prev, prev.height + 1
            b.build_skip()
        b.time_max = max(b.pprev.time_max, b.time) if b.pprev else b.time
        b.work = (b.pprev.work if b.pprev else 0) + bits_proof(b.bits)
        b.raise_validity(TREE)
        if self.best_header is None or self.best_header.work < b.work:
            self.best_header = b
        return b

    def try_add_candidate(self, b):
        if self.tip() is not None and worse(b, self.tip()):
            return
        if b not in self.candidates:
            self.candidates.append(b)

    def received(self, b, ntx):
        b.ntx = ntx
        b.status |= HAVE_DATA
        if b.height >= self.p['SegwitHeight']:
            b.status |= OPT_WITNESS
        b.raise_validity(TRANSACTIONS)
        if b.pprev is None or b.pprev.chain_tx:
            queue = [b]
            while queue:
                x = queue.pop(0)
                x.chain_tx = x.ntx + (x.pprev.chain_tx if x.pprev else 0)
                x.seq = self.seq
                self.seq += 1
                self.try_add_candidate(x)
                for pair in [u for u in self.unlinked if u[0] is x]:
                    queue.append(pair[1])
                    self.unlinked.remove(pair)
        elif b.pprev.is_valid(TREE):
            self.unlinked.append((b.pprev, b))

    def find_most_work_chain(self):
        while True:
            if not self.candidates:
                return None
            best = self.candidates[0]
            for c in self.candidates[1:]:
                if worse(best, c):
                    best = c
            test, bad = best, False
            while test and not self.contains(test):
                failed, missing = bool(test.status & FAILED), not test.status & HAVE_DATA
                if failed or missing:
                    if failed and (self.best_invalid is None or best.work > self.best_invalid.work):
                        self.best_invalid = best
                    x = best
                    while x is not test:
                        if missing and not failed:
                            self.unlinked.append((x.pprev, x))
                        self.candidates.remove(x)
                        x = x.pprev
                    self.candidates.remove(test)
                    bad = True
                    break
                test = test.pprev
            if not bad:
                return best

    def activate(self):
        best = self.find_most_work_chain()
        if best is not None and (self.tip() is None or worse(self.tip(), best)):
            self.set_tip(best)
            self.candidates = [c for c in self.candidates if not worse(c, self.tip())]
        return self.tip()

    def recalculate_best_header(self):
        self.best_header = self.tip()
        for b in self.index.values():
            if not b.status & FAILED and self.best_header.work < b.work:
                self.best_header = b

    def invalid_chain_found(self, b):
        if self.best_invalid is None or b.work > self.best_invalid.work:
            self.best_invalid = b
        for x in self.index.values():
            if x is not b and x.ancestor(b.height) is b:
                x.status |= FAILED
        if self.best_header is not None and self.best_header.ancestor(b.height) is b:
            self.recalculate_best_header()

    def invalid_block_found(self, b):
        b.status |= FAILED
        if b in self.candidates:
            self.candidates.remove(b)
        self.invalid_chain_found(b)

    def invalidate_block(self, b):
        if self.contains(b) and b.pprev:
            self.set_tip(b.pprev)
        b.status |= FAILED
        self.invalid_chain_found(b)
        self.candidates = [x for x in self.index.values()
                           if x.is_valid(TRANSACTIONS) and x.chain_tx and not worse(x, self.tip())]


def transcript():
    lines = []
    out = lines.append

    # Skip pointers and locators.
    out('skip pointers')
    out('  GetSkipHeight of 1 to 20: ' + ' '.join(str(skip_height(h)) for h in range(1, 21)))

    def line(n, base=0, parent=None):
        blocks, prev = [], parent
        for k in range(n):
            b = Index.__new__(Index)
            b.height, b.pprev, b.pskip = base + k, prev, None
            b.build_skip()
            blocks.append(b)
            prev = b
        return blocks

    main = line(300000)
    steps = ' '.join(f'{h}: {main[299999].ancestor_walk(h)[1]}' for h in [0, 1, 150000, 262143, 299998])
    out(f'  300000 entries, steps from 299999 to {steps}')
    most, at = 0, 0
    for h in range(262143):
        s = main[262143].ancestor_walk(h)[1]
        if s > most:
            most, at = s, h
    out(f'  from 262143 (2^18 - 1) to any lower height: at most {most} steps, to {at}; one parent at a time it is up to 262143')
    out('locators')
    main = main[:100000]
    side = line(50000, 50000, main[49999])
    for label, tip in [('main tip 99999', main[99999]), ('branch tip 99999', side[49999])]:
        loc = locator(tip)
        out(f'  {label} ({len(loc)} entries): ' + ' '.join(f'{b.height}{"b" if b.height >= 50000 and b is side[b.height - 50000] else ""}' for b in loc))
    out(f'  the two tips\' last common ancestor: {last_common_ancestor(main[99999], side[49999]).height}')

    # Mainnet's first 2,017 headers.
    out('mainnet headers 0 to 2016')
    raws = [bytes.fromhex(x.split(' ')[2]) for x in HEADERS.splitlines()]
    now = 1793664000
    node = Node(MAIN)
    accepted = 0
    for raw in raws:
        reason, b = node.accept_header(raw, now)
        if reason == 'ok':
            accepted += 1
            node.received(b, 1)
    tip = node.activate()
    out(f'  {accepted} accepted; tip {tip.height} {tip.hash[::-1].hex()}')
    out(f'  chainwork {tip.work:064x}')
    out(f'  best header {node.best_header.height}, transactions {tip.chain_tx}, median time past {tip.mtp()}, time max {tip.time_max}')
    loc = locator(tip)
    out(f'  locator ({len(loc)} entries): ' + ' '.join(str(b.height) for b in loc))
    out(f'  block 2016 again: {node.accept_header(raws[2016], now)[0]}')
    changed = bytearray(raws[2016])
    changed[76] ^= 1
    out(f'  block 2016 with one bit of its nonce changed: {node.accept_header(bytes(changed), now)[0]}')
    small = Node(MAIN)
    for raw in raws[:4]:
        small.accept_header(raw, now)
    out(f'  block 5 before block 4: {small.accept_header(raws[5], now)[0]}')

    # Regtest forks, mined here.
    out('regtest')
    t0, nonce, bits, version = REGTEST['genesis']
    graw = header(version, bytes(32), GENESIS_ROOT, t0, bits, nonce)
    node = Node(REGTEST)
    reason, g = node.accept_header(graw, now)
    node.received(g, 1)
    node.activate()
    assert g.hash[::-1].hex() == REGTEST['hashGenesisBlock']
    out(f'  genesis {g.hash[::-1].hex()}, work {g.work}, status 0x{g.status:02x}')
    names = {g: 'G0'}
    last = []

    def mine(prev, letter, data, version=4, bits=0x207fffff, time=None):
        if time is None:
            time = t0 + 600 * (prev.height + 1) + letter
        nonce = 0
        while True:
            raw = header(version, prev.hash, bytes([letter]) * 32, time, bits, nonce)
            if check_pow(sha256d(raw), bits, REGTEST):
                break
            nonce += 1
        reason, b = node.accept_header(raw, now)
        last[:] = [reason]
        if reason != 'ok':
            return None
        names[b] = 'GABCDEFGHIJ'[letter] + str(b.height)
        if data:
            node.received(b, 1)
        return b

    def run(prev, letter, count, data):
        for _ in range(count):
            prev = mine(prev, letter, data)
        return prev

    def state():
        s = f'tip {names[node.tip()]}, best header {names[node.best_header]}'
        if node.best_invalid is not None:
            s += f', best invalid {names[node.best_invalid]}'
        return s

    def check(prev, version, bits, time):
        mine(prev, 9, False, version, bits, time)
        return last[0]

    a5 = run(g, 1, 5, True)
    node.activate()
    out(f'  A1 to A5 with data: {state()}')
    mtp = a5.mtp()
    out(f'  headers on A5: nBits 0x207ffffe: {check(a5, 4, 0x207ffffe, mtp + 1)}, time at the median time past {mtp}: '
        f'{check(a5, 4, 0x207fffff, mtp)}, version 3: {check(a5, 3, 0x207fffff, mtp + 1)}, '
        f'2 hours and 1 second ahead: {check(a5, 4, 0x207fffff, now + 7201)}')
    a2 = a5.ancestor(2)
    b4 = run(a2, 2, 2, True)
    node.activate()
    out(f'  B3, B4 on A2, work {b4.work} against A5\'s {a5.work}: {state()}')
    b5 = mine(b4, 2, False)
    b6 = mine(b5, 2, False)
    node.activate()
    out(f'  B5, B6 headers only: {state()}')
    node.received(b6, 1)
    node.activate()
    out(f'  B6\'s data first: B6 has {b6.chain_tx} transactions in its chain, sequence {b6.seq}; {state()}')
    node.received(b5, 1)
    node.activate()
    out(f'  then B5\'s: sequence B5 {b5.seq}, B6 {b6.seq}; {state()}')
    out(f'  fork of A5 from the active chain: {names[node.find_fork(a5)]}, last common ancestor of A5 and B6: {names[last_common_ancestor(a5, b6)]}')
    c7 = mine(b6, 3, False)
    d7 = mine(b6, 4, False)
    node.received(d7, 1)
    node.received(c7, 1)
    node.activate()
    out(f'  C7 then D7 headers on B6, D7\'s data first: sequence C7 {c7.seq}, D7 {d7.seq}; {state()}')
    seqs = c7.seq, d7.seq
    c7.seq = d7.seq = SEQ_INIT
    out(f'  with both at sequence 1, as after a restart, the lower position wins: {"C7" if worse(d7, c7) else "D7"}')
    c7.seq, d7.seq = seqs
    f8 = run(a5, 6, 3, True)
    f7 = f8.pprev
    node.invalid_block_found(f7)
    node.activate()
    out(f'  F6 to F8 on A5, and F7 fails ConnectBlock: {state()}')
    mine(f8, 7, False)
    on_f8 = last[0]
    out(f'  a header on F8: {on_f8}; F7 again: {node.accept_header(f7.h.raw, now)[0]}')
    node.invalidate_block(b5)
    first = names[node.tip()]
    node.activate()
    out(f'  invalidateblock B5: tip {first}, then {state()}')
    entries = list(node.index.values())
    out('  failed: ' + ' '.join(names[b] for b in entries if b.status & FAILED))
    out('  entries: ' + ' '.join(f'{names[b]}=0x{b.status:02x}' for b in entries))
    out('  locator from the tip: ' + ' '.join(names[b] for b in locator(node.tip())))
    return '\n'.join(lines) + '\n'


def main():
    want = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'block-index fixture: {want.count(chr(10))} lines agree with Core\'s transliterated chain.cpp, blockstorage.cpp, validation.cpp and pow.cpp')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
