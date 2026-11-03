#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Bitcoin Core's pinned net.cpp, node/eviction.cpp, net_processing.cpp, netbase.cpp and random.cpp decide each case outside Coax)
"""The cross-check of Bitcoin chapter 70. It computes every line the
chapter's example prints, in Python that shares no code with Coax, and
compares the transcript with the fixture:

  python3 -I coax/tests/bitcoin/connman.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/connman.py --coax coax/bin/coax   # also run the example

The connection manager is CConnman::ThreadOpenConnections (net.cpp)
transliterated: the choice of connection type, the address loop with its
netgroup, reachability, recent-try, service and bad-port rules (IsBadPort
from netbase.cpp), and the exponential timers (rand_exp_duration and
MakeExponentiallyDistributed from random.h and random.cpp). Inbound
eviction is SelectNodeToEvict and ProtectEvictionCandidatesByRatio
(node/eviction.cpp), with a stable sort where Core's std::sort leaves
ties unspecified. Extra outbound peers are evicted as in
PeerManagerImpl::EvictExtraOutboundPeers (net_processing.cpp). The
address manager is chapter 69's oracle, addrman.py.
"""
import importlib.util
import math
import re
import subprocess
import sys
from functools import cmp_to_key
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('addrman_oracle', HERE / 'addrman.py')
am = importlib.util.module_from_spec(spec)
spec.loader.exec_module(am)

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/connman.out'
EXAMPLE = 'bitcoin/examples/connman.coax'
for name in ['eviction.cpp', 'eviction.h', 'net_peer_eviction_tests.cpp', 'netbase.cpp', 'random.cpp', 'net.h']:
    am.PINS[name] = None
am.manifest_pins()

NET_H = am.pinned('net.h')
NET_CPP = am.pinned('net.cpp')
NETPROC = am.pinned('net_processing.cpp')
EVICTION = am.pinned('eviction.cpp')
NETBASE = am.pinned('netbase.cpp')
RANDOM_CPP = am.pinned('random.cpp')
am.pinned('eviction.h')
am.pinned('net_peer_eviction_tests.cpp')


def const(text, name):
    m = re.search(rf'{name}\s*(?:=\s*|\{{)(\d+)(min|s)?', text)
    assert m, name
    v = int(m.group(1))
    return v * 60 if m.group(2) == 'min' else v


FULL = const(NET_H, 'MAX_OUTBOUND_FULL_RELAY_CONNECTIONS')
BLOCK = const(NET_H, 'MAX_BLOCK_RELAY_ONLY_CONNECTIONS')
FEELERS = const(NET_H, 'MAX_FEELER_CONNECTIONS')
MAX_CONN = const(NET_H, 'DEFAULT_MAX_PEER_CONNECTIONS')
FEELER_INTERVAL = const(NET_H, 'FEELER_INTERVAL')
EXTRA_BLOCK_INTERVAL = const(NET_H, 'EXTRA_BLOCK_RELAY_ONLY_PEER_INTERVAL')
EXTRA_NET_INTERVAL = const(NET_CPP, 'EXTRA_NETWORK_PEER_INTERVAL')
CHECK_INTERVAL = const(NETPROC, 'EXTRA_PEER_CHECK_INTERVAL')
MIN_CONNECT = const(NETPROC, 'MINIMUM_CONNECT_TIME')
RANDOMIZER_NETGROUP = int(re.search(r'RANDOMIZER_ID_NETGROUP = (0x[0-9a-f]+)', NET_CPP).group(1), 16)
assert '-std::log1p((uniform >> 11) * -0x1.0p-53)' in RANDOM_CPP
assert 'rand_uniform_duration<CThreadInterrupt::Clock>(FEELER_SLEEP_WINDOW)' in NET_CPP
BAD_PORTS = sorted(int(p) for p in re.findall(r'case (\d+):', NETBASE[NETBASE.index('bool IsBadPort'):NETBASE.index('CService MaybeFlipIPv6toCJDNS')]))
# SelectNodeToEvict's fixed protections, in order, read from eviction.cpp.
STEPS = re.findall(r'EraseLastKElements\(vEvictionCandidates, (\w+), (\d+)', EVICTION)
assert [s[0] for s in STEPS] == ['CompareNetGroupKeyed', 'ReverseCompareNodeMinPingTime', 'CompareNodeTXTime',
                                 'CompareNodeBlockRelayOnlyTime', 'CompareNodeBlockTime'], STEPS

NOW = 1700000000
UNROUTABLE, IPV4, IPV6, ONION, I2P, CJDNS, INTERNAL = range(7)
NET_MAX = 7


def exp_delay(rng, mean_us):
    """rand_exp_duration: MakeExponentiallyDistributed(rand64()) * mean + 0.5 us, truncated."""
    x = (rng.rand64() >> 11) * 2.0 ** -53
    return int(-math.log1p(-x) * mean_us + 0.5)


def secs(us):
    return f'{us // 10**6}.{us % 10**6:06d}'


def who(x):
    return 'nobody' if x is None else str(x)


def limits(max_auto):
    full = min(FULL, max_auto)
    block = min(BLOCK, max_auto - full)
    inbound = max(0, max_auto - (full + block + FEELERS))
    return full, block, inbound


# ---- inbound eviction (node/eviction.cpp)
def less(code, a, b, local=False, net=None):
    if code == 'ping':
        return a['ping'] > b['ping']
    if code == 'time':
        return a['connected'] > b['connected']
    if code == 'group':
        return a['group'] < b['group']
    if code == 'block':
        if a['block'] != b['block']:
            return a['block'] < b['block']
        if a['relevant'] != b['relevant']:
            return b['relevant']
        return a['connected'] > b['connected']
    if code == 'tx':
        if a['tx'] != b['tx']:
            return a['tx'] < b['tx']
        if a['relay'] != b['relay']:
            return b['relay']
        if a['bloom'] != b['bloom']:
            return a['bloom']
        return a['connected'] > b['connected']
    if code == 'blockonly':
        if a['relay'] != b['relay']:
            return a['relay']
        if a['block'] != b['block']:
            return a['block'] < b['block']
        if a['relevant'] != b['relevant']:
            return b['relevant']
        return a['connected'] > b['connected']
    if code == 'nettime':
        if local and a['local'] != b['local']:
            return b['local']
        if (a['net'] == net) != (b['net'] == net):
            return b['net'] == net
        return a['connected'] > b['connected']
    raise ValueError(code)


def sort(cands, code, local=False, net=None):
    def cmp(a, b):
        if less(code, a, b, local, net):
            return -1
        if less(code, b, a, local, net):
            return 1
        return 0
    cands[:] = sorted(cands, key=cmp_to_key(cmp))


def erase_last(cands, code, k, pred=lambda c: True, local=False, net=None):
    sort(cands, code, local, net)
    k = min(k, len(cands))
    tail = [c for c in cands[len(cands) - k:] if not pred(c)]
    cands[len(cands) - k:] = tail


def protect_by_ratio(cands):
    initial = len(cands)
    total = initial // 2
    nets = [[False, CJDNS, 0], [False, I2P, 0], [True, NET_MAX, 0], [False, ONION, 0]]
    for n in nets:
        n[2] = sum(1 for c in cands if (c['local'] if n[0] else c['net'] == n[1]))
    nets.sort(key=lambda n: n[2])  # stable
    max_by_net = total // 2
    protected = 0
    while protected < max_by_net:
        num = sum(1 for n in nets if n[2])
        if num == 0:
            break
        per = max((max_by_net - protected) // num, 1)
        any_protected = False
        for n in nets:
            if n[2] == 0:
                continue
            before = len(cands)
            is_local, net = n[0], n[1]
            erase_last(cands, 'nettime', per, lambda c: (c['local'] if is_local else c['net'] == net), is_local, net)
            delta = before - len(cands)
            if delta:
                any_protected = True
                protected += delta
                if protected >= max_by_net:
                    break
                n[2] -= delta
        if not any_protected:
            break
    erase_last(cands, 'time', total - protected)


def select_to_evict(cands, trace=None):
    cands = [c for c in cands if not c['noban'] and c['inbound']]
    log = trace if trace is not None else []
    erase_last(cands, 'group', 4)
    log.append(len(cands))
    erase_last(cands, 'ping', 8)
    log.append(len(cands))
    erase_last(cands, 'tx', 4)
    log.append(len(cands))
    erase_last(cands, 'blockonly', 8, lambda c: not c['relay'] and c['relevant'])
    log.append(len(cands))
    erase_last(cands, 'block', 4)
    log.append(len(cands))
    protect_by_ratio(cands)
    log.append(len(cands))
    if not cands:
        return None
    if any(c['prefer'] for c in cands):
        cands = [c for c in cands if c['prefer']]
    log.append(len(cands))
    groups, most, most_time, most_group = {}, 0, -1, None
    for c in cands:
        g = groups.setdefault(c['group'], [])
        g.append(c)
        if len(g) > most or (len(g) == most and g[0]['connected'] > most_time):
            most, most_time, most_group = len(g), g[0]['connected'], c['group']
    log.append(most)
    return groups[most_group][0]['id']


def random_candidates(n, rng):
    """GetRandomNodeEvictionCandidates (test/util/net.cpp), drawn from rng."""
    out = []
    for i in range(n):
        c = dict(id=i, connected=rng.randrange(100), ping=rng.randrange(100), block=rng.randrange(100),
                 tx=rng.randrange(100), relevant=rng.randbool(), relay=rng.randbool(), bloom=rng.randbool(),
                 group=rng.randrange(100), prefer=rng.randbool(), local=rng.randbool(), net=rng.randrange(7))
        c.update(noban=False, inbound=True)
        out.append(c)
    return out


def keyed_group(a):
    """CalculateKeyedNetGroup with the randomizer seeds (11, 22)."""
    return am.siphash(11, 22, RANDOMIZER_NETGROUP.to_bytes(8, 'little') + am.group(a))


def accept_inbound(n_inbound, max_inbound, banned, discouraged, noban, cands):
    """CreateNodeFromAcceptedSocket's decision: 'accept', ('evict', id) or a drop."""
    if banned and not noban:
        return 'dropped (banned)'
    if discouraged and not noban and n_inbound + 1 >= max_inbound:
        return 'dropped (discouraged)'
    if n_inbound >= max_inbound:
        victim = select_to_evict(cands)
        if victim is None:
            return 'dropped (full)'
        return f'accepted, evicting peer {victim}'
    return 'accepted'


# ---- extra outbound peers (net_processing.cpp)
def evict_extra_block_relay(peers, now):
    """peers: (id, connected, last_block) of block-relay-only peers. The id to disconnect, or None."""
    youngest, next_youngest = (-1, 0, 0), (-1, 0, 0)
    for p in sorted(peers):
        if p[0] > youngest[0]:
            next_youngest, youngest = youngest, p
    pick = next_youngest if youngest[2] > next_youngest[2] else youngest
    if pick[0] >= 0 and now - pick[1] >= MIN_CONNECT:
        return pick[0]
    return None


def evict_extra_full(peers, now):
    """peers: (id, connected, last_announcement, net, protected). The id to disconnect, or None."""
    per_net = {}
    for p in peers:
        per_net[p[3]] = per_net.get(p[3], 0) + 1
    worst = None
    for p in sorted(peers):
        if p[4] or per_net[p[3]] < 2:
            continue
        if worst is None or p[2] < worst[2] or (p[2] == worst[2] and p[0] > worst[0]):
            worst = p
    if worst and now - worst[1] > MIN_CONNECT:
        return worst[0]
    return None


# ---- opening connections (net.cpp ThreadOpenConnections)
def may_have_useful(services):
    return bool(services & (am.NODE_NETWORK | am.NODE_NETWORK_LIMITED))


def population():
    out = []
    for g in range(48):
        for i in range(5):
            port = 22 if g % 11 == 0 and i == 0 else 8333
            services = am.NODE_NETWORK | am.NODE_WITNESS
            if g % 9 == 4 and i == 1:
                services = am.NODE_NETWORK_LIMITED
            out.append((am.v4(f'{20 + g}.{(g * 7) % 256}.{i}.{1 + g % 3}', port), services))
    return out


def fails(a):
    return (a.ip[0] + a.ip[2]) % 4 == 0


class Sim:
    def __init__(self):
        self.m = am.AddrMan(bytes([1]) + bytes(31))
        for a, services in population():
            self.m.add([(a, services, NOW - 3600)], am.v4('99.99.1.1'), 0, NOW)
        self.rng = am.Rng(bytes([5]) * 32)
        self.conns = []   # [id, kind, addr, connected_at, last_block]
        self.next_id = 0
        self.anchors = [(am.v4('57.3.3.3', 8333), am.NODE_NETWORK | am.NODE_WITNESS)]
        self.full, self.block, _ = limits(MAX_CONN)

    def connected(self, a):
        return any(c[2].same_ip(a) for c in self.conns)

    def pick(self, kind, anchor, groups, now):
        m = self.m
        tries = 0
        while True:
            if anchor and self.anchors:
                a, services = self.anchors.pop()
                if not am.is_valid(a) or am.is_local(a) or am.network(a) not in (IPV4, IPV6) or \
                        not am.desirable(services, 0) or am.group(a) in groups:
                    continue
                return a, tries
            tries += 1
            if tries > 100:
                return None, tries
            feeler = kind == 'feeler'
            if feeler:
                e = m.select_tried_collision()
                if e is None:
                    e = m.select(True, (IPV4, IPV6), now)
                elif self.connected(m.info[e].addr):
                    m.good(m.info[e].addr, now)
                    e = m.select(True, (IPV4, IPV6), now)
            else:
                e = m.select(False, (IPV4, IPV6), now)
            info = m.info[e] if e is not None else None
            if not feeler and info is not None and am.group(info.addr) in groups:
                continue
            if info is None or not am.is_valid(info.addr) or am.is_local(info.addr):
                return None, tries
            if am.network(info.addr) not in (IPV4, IPV6):
                continue
            if now - info.last_try < 600 and tries < 30:
                continue
            if not feeler and not am.desirable(info.services, 0):
                continue
            if feeler and not may_have_useful(info.services):
                continue
            if tries < 50 and info.addr.port in BAD_PORTS:
                continue
            return info.addr, tries

    def run(self, out, seconds):
        start_us = NOW * 10**6
        next_feeler = start_us + exp_delay(self.rng, FEELER_INTERVAL * 10**6)
        next_extra_block = start_us + exp_delay(self.rng, EXTRA_BLOCK_INTERVAL * 10**6)
        exp_delay(self.rng, EXTRA_NET_INTERVAL * 10**6)
        out(f'  first feeler after {secs(next_feeler - start_us)} s, first extra block-relay-only after {secs(next_extra_block - start_us)} s')
        next_check = NOW + CHECK_INTERVAL
        for step in range(seconds * 2):
            now_us = start_us + step * 500000
            now = now_us // 10**6
            if now >= next_check:
                next_check += CHECK_INTERVAL
                blocks = [(c[0], c[3], c[4]) for c in self.conns if c[1] == 'block-relay']
                if len(blocks) > self.block:
                    victim = evict_extra_block_relay(blocks, now)
                    if victim is not None:
                        self.conns = [c for c in self.conns if c[0] != victim]
                        out(f'  {now - NOW:4d} s  {len(blocks)} block-relay-only peers: disconnect peer {victim}')
            n_full = sum(1 for c in self.conns if c[1] == 'full-relay')
            n_block = sum(1 for c in self.conns if c[1] == 'block-relay')
            groups = {am.group(c[2]) for c in self.conns}
            anchor = False
            if self.anchors and n_block < self.block:
                kind, anchor = 'block-relay', True
            elif n_full < self.full:
                kind = 'full-relay'
            elif n_block < self.block:
                kind = 'block-relay'
            elif now_us > next_extra_block and now >= NOW + 60:
                next_extra_block = now_us + exp_delay(self.rng, EXTRA_BLOCK_INTERVAL * 10**6)
                kind = 'block-relay'
            elif now_us > next_feeler:
                next_feeler = now_us + exp_delay(self.rng, FEELER_INTERVAL * 10**6)
                kind = 'feeler'
            else:
                continue
            self.m.resolve_collisions(now)
            a, tries = self.pick(kind, anchor, groups, now)
            if a is None:
                out(f'  {now - NOW:4d} s  {kind}: nothing after {tries} tries')
                continue
            if kind == 'feeler':
                # The feeler's jitter: rand_uniform_duration over steady_clock, whose tick is a nanosecond.
                self.rng.randrange(10**9)
            count = len(groups) >= min(MAX_CONN - 1, 2)
            self.m.attempt(a, count, now)
            ok = not fails(a)
            note = ' (anchor)' if anchor else ''
            out(f'  {now - NOW:4d} s  {kind:<11} {am.show(a)}{note} after {tries} tries: {"connected" if ok else "failed"}')
            if ok:
                self.m.good(a, now)
                if kind != 'feeler':
                    last_block = 0 if self.next_id % 3 == 0 else now + (self.next_id * 37) % 300
                    self.conns.append([self.next_id, kind, a, now, last_block])
                self.next_id += 1
        n_full = sum(1 for c in self.conns if c[1] == 'full-relay')
        n_block = sum(1 for c in self.conns if c[1] == 'block-relay')
        out(f'  after {seconds // 60} minutes: {n_full} full-relay, {n_block} block-relay-only, {len({am.group(c[2]) for c in self.conns})} groups; '
            f'addrman {self.m.size(None, True)} new, {self.m.size(None, False)} tried, check {self.m.check()}')


def transcript():
    lines = []
    out = lines.append

    out('slots')
    for n in (MAX_CONN, 125, 10, 4):
        f, b, i = limits(n)
        out(f'  -maxconnections={n}: {f} full-relay, {b} block-relay-only, {FEELERS} feeler, {i} inbound')

    out('exponential timers')
    rng = am.Rng(bytes([3]) * 32)
    first = [exp_delay(rng, FEELER_INTERVAL * 10**6) for _ in range(5)]
    out('  feeler delays (us): ' + ' '.join(str(d) for d in first))
    total = sum(exp_delay(rng, FEELER_INTERVAL * 10**6) for _ in range(2000))
    out(f'  mean of 2,000 more: {total // 2000 // 1000} ms (FEELER_INTERVAL is {FEELER_INTERVAL} s)')
    ports = [p for p in (8333, 22, 80, 443, 6667, 10080, 18333) if p in BAD_PORTS]
    out(f'  bad ports among 8333 22 80 443 6667 10080 18333: {" ".join(str(p) for p in ports)} ({len(BAD_PORTS)} in all)')

    out('opening connections for 15 minutes')
    Sim().run(out, 15 * 60)

    out('inbound eviction: 40 random candidates')
    rng = am.Rng(bytes([4]) * 32)
    cands = random_candidates(40, rng)
    trace = []
    victim = select_to_evict(cands, trace)
    out(f'  left after netgroup {trace[0]}, ping {trace[1]}, tx {trace[2]}, block-relay-only {trace[3]}, block {trace[4]}, ratio {trace[5]}, prefer-evict {trace[6]}')
    out(f'  evict peer {victim}, youngest of the largest group ({trace[7]} peers)')
    small = sum(1 for n in range(21) if select_to_evict(random_candidates(n, rng)) is None)
    large = sum(1 for n in range(29, 60) if select_to_evict(random_candidates(n, rng)) is not None)
    out(f'  0 to 20 random candidates: {small} of 21 evict no one; 29 to 59: {large} of 31 evict one')

    out('Core\'s protection cases (12 peers)')
    for label, setup in [
            ('uptime: the 6 longest connected', lambda c: c.update(connected=c['id'], local=False, net=IPV4)),
            ('3 onion peers', lambda c: c.update(local=False, net=ONION if c['id'] in (3, 8, 9) else IPV4)),
            ('onion 3, 8-11 and uptime', lambda c: c.update(connected=c['id'], local=False, net=ONION if c['id'] == 3 or c['id'] > 7 else IPV6)),
            ('localhost 1, 9, 11', lambda c: c.update(local=c['id'] in (1, 9, 11), net=IPV4)),
            ('I2P 2, 7, 10, CJDNS 5', lambda c: c.update(connected=c['id'], local=False, net=I2P if c['id'] in (2, 7, 10) else CJDNS if c['id'] == 5 else IPV4))]:
        cands = random_candidates(12, rng)
        for c in cands:
            setup(c)
        protect_by_ratio(cands)
        left = sorted(c['id'] for c in cands)
        out(f'  {label}: protected {" ".join(str(i) for i in range(12) if i not in left)}')

    out('an attacker fills the inbound slots')
    _, _, max_in = limits(MAX_CONN)
    peers = []
    for i in range(60):
        a = am.v4(f'{30 + i}.{i * 3 % 256}.1.1', 8333)
        peers.append(dict(id=i, connected=NOW - 86400 + i * 600, ping=20000 + (i * 7919) % 180000, block=NOW - 600 * (i % 10) if i % 4 == 0 else 0,
                          tx=NOW - 60 * (i % 13) if i % 3 == 0 else 0, relevant=True, relay=i % 5 != 0, bloom=False,
                          group=keyed_group(a), prefer=False, local=False, net=IPV4, noban=False, inbound=True))
    next_id = 60
    honest_lost = 0
    for k in range(max_in - 60 + 200):
        a = am.v4(f'66.{k % 3}.{k // 250 % 256}.{1 + k % 250}', 8333)
        if len(peers) >= max_in:
            victim = select_to_evict(peers)
            if victim < 60:
                honest_lost += 1
            peers = [p for p in peers if p['id'] != victim]
        peers.append(dict(id=next_id, connected=NOW + k, ping=5000, block=0, tx=0, relevant=True, relay=True, bloom=False,
                          group=keyed_group(a), prefer=False, local=False, net=IPV4, noban=False, inbound=True))
        next_id += 1
    out(f'  {max_in} slots, 60 honest peers, {max_in - 60 + 200} attacker connections from 3 groups: {honest_lost} honest peers evicted, {sum(1 for p in peers if p["id"] < 60)} remain')

    out('accepting an inbound connection')
    few = random_candidates(20, rng)
    out(f'  {max_in - 1} inbound: {accept_inbound(max_in - 1, max_in, False, False, False, peers)}')
    out(f'  {max_in} inbound: {accept_inbound(max_in, max_in, False, False, False, peers)}')
    out(f'  full, 20 candidates all protected: {accept_inbound(20, 20, False, False, False, few)}')
    out(f'  banned: {accept_inbound(5, max_in, True, False, False, peers)}; with noban: {accept_inbound(5, max_in, True, False, True, peers)}')
    out(f'  discouraged, one slot left: {accept_inbound(max_in - 1, max_in, False, True, False, peers)}; two left: {accept_inbound(max_in - 2, max_in, False, True, False, peers)}')

    out('extra outbound peers')
    t = NOW + 3600
    three = [(4, t - 900, t - 100), (7, t - 600, t - 50), (12, t - 60, t - 10)]
    out(f'  block-relay-only 4, 7, 12; 12 sent the latest block: disconnect {who(evict_extra_block_relay(three, t))}')
    three[2] = (12, t - 60, 0)
    out(f'  12 sent no block: disconnect {who(evict_extra_block_relay(three, t))}')
    three[2] = (12, t - 20, 0)
    out(f'  12 connected 20 s ago: disconnect {who(evict_extra_block_relay(three, t))}')
    nine = [(i, t - 3000 + i, t - 100 * (i % 4), IPV4, False) for i in range(8)] + [(20, t - 40, t - 5, ONION, False)]
    out(f'  9 full-relay: disconnect {who(evict_extra_full(nine, t))} (oldest block announcement, then the youngest)')
    nine = [(p[0], p[1], p[2], p[3], p[0] in (3, 7)) for p in nine]
    out(f'  3 and 7 protected: disconnect {who(evict_extra_full(nine, t))}; the only onion peer is never chosen')
    return '\n'.join(lines) + '\n'


def main():
    want = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'connman fixture: {want.count(chr(10))} lines agree with Core\'s transliterated net.cpp, node/eviction.cpp and net_processing.cpp')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
