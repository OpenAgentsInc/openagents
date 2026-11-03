#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Bitcoin Core's pinned addrman.cpp, netgroup.cpp, netaddress.cpp, random.h and chacha20.py decide each case outside Coax)
"""The cross-check of Bitcoin chapter 69. It computes every line the
chapter's example prints, in Python that shares no code with Coax, and
compares the transcript with the fixture:

  python3 -I coax/tests/bitcoin/addrman.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/addrman.py --coax coax/bin/coax   # also run the example

The address manager is Bitcoin Core's AddrManImpl (addrman.cpp) and
AddrInfo transliterated: bucket selection, AddSingle, Good_, Attempt_,
Select_, GetAddr_, ResolveCollisions_, SelectTriedCollision_, the
serialized format and CheckAddrman. Groups are NetGroupManager::GetGroup
(netgroup.cpp) without an asmap, over CNetAddr's classification
(netaddress.cpp). Randomness is FastRandomContext (random.h): ChaCha20's
keystream from the pinned test_framework/crypto/chacha20.py, read 8
bytes at a time, with RandomMixin's randbits and randrange. Address
relay is ProcessAddrs, RelayAddress and GetAddresses (net_processing.cpp,
net.cpp), and every constant is read from the pinned sources.
"""
import hashlib
import re
import struct
import subprocess
import sys
import types
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
CORE = HERE.parent / 'vectors/upstream/bitcoin-core'
FIXTURE = REPO / 'bitcoin/fixtures/addrman.out'
EXAMPLE = 'bitcoin/examples/addrman.coax'
PINS = {
    'addrman.cpp': None, 'addrman.h': None, 'addrman_impl.h': None, 'netgroup.cpp': None,
    'addrman_tests.cpp': None, 'random.h': None, 'chacha20.py': None,
    'net_processing.cpp': 'bcd69aba37869cd68dfe03cce37b1400d1488b59ef1faafdc319ddcb51ebea36',
    'net.cpp': '05e8ce20762e9c23e060d428fb5218cb32fea599afbd40b2417b9bd2a236d7a6',
}


def manifest_pins():
    import json
    m = json.loads((HERE.parent / 'vectors/manifest.json').read_text())
    for f in m['files']:
        name = f['path'].rsplit('/', 1)[-1]
        if f['path'].startswith('upstream/bitcoin-core/') and name in PINS and PINS[name] is None:
            PINS[name] = f['sha256']


def pinned(name):
    data = (CORE / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == PINS[name], f'{name} differs from its pin'
    return data.decode()


manifest_pins()


def constants():
    h, impl, cpp = pinned('addrman.h'), pinned('addrman_impl.h'), pinned('addrman.cpp')
    c = {}
    for name in ['ADDRMAN_TRIED_BUCKET_COUNT_LOG2', 'ADDRMAN_NEW_BUCKET_COUNT_LOG2', 'ADDRMAN_BUCKET_SIZE_LOG2']:
        c[name[:-5]] = 1 << int(re.search(rf'{name}\{{(\d+)\}}', impl).group(1))
    for name in ['ADDRMAN_TRIED_BUCKETS_PER_GROUP', 'ADDRMAN_NEW_BUCKETS_PER_SOURCE_GROUP', 'ADDRMAN_NEW_BUCKETS_PER_ADDRESS',
                 'ADDRMAN_RETRIES', 'ADDRMAN_MAX_FAILURES', 'ADDRMAN_SET_TRIED_COLLISION_SIZE']:
        c[name] = int(re.search(rf'{name}\{{(\d+)\}}', h).group(1))
    hours = {'ADDRMAN_HORIZON': (r'\{30 \* 24h\}', 30 * 86400), 'ADDRMAN_MIN_FAIL': (r'\{7 \* 24h\}', 7 * 86400),
             'ADDRMAN_REPLACEMENT': (r'\{4h\}', 4 * 3600), 'ADDRMAN_TEST_WINDOW': (r'\{40min\}', 40 * 60)}
    for name, (pattern, value) in hours.items():
        assert re.search(name + pattern, h), name
        c[name] = value
    assert 'nKey{deterministic ? uint256{1} : insecure_rand.rand256()}' in cpp
    assert 'fChance *= pow(0.66, std::min(nAttempts, 8));' in cpp
    assert 'chance_factor *= 1.2;' in cpp
    assert 'static constexpr uint8_t INCOMPATIBILITY_BASE = 32;' in impl
    proc = pinned('net_processing.cpp')
    assert 'MAX_ADDR_RATE_PER_SECOND{0.1}' in proc and 'MAX_ADDR_TO_SEND{1000}' in proc
    assert 'MAX_PCT_ADDR_TO_SEND = 23;' in proc and 'vAddr.size() <= 10 && addr.IsRoutable()' in proc
    c['RANDOMIZER_ID_ADDRESS_RELAY'] = int(re.search(r'RANDOMIZER_ID_ADDRESS_RELAY = (0x[0-9a-f]+)ULL', proc).group(1), 16)
    net = pinned('net.cpp')
    for name in ['DNSSEEDS_TO_QUERY_AT_ONCE', 'SEED_OUTBOUND_CONNECTION_THRESHOLD', 'DNSSEEDS_DELAY_PEER_THRESHOLD']:
        c[name] = int(re.search(rf'{name} = (\d+);', net).group(1))
    assert 'DNSSEEDS_DELAY_FEW_PEERS{11}' in net and 'DNSSEEDS_DELAY_MANY_PEERS{5}' in net
    assert '21h + FastRandomContext().randrange<std::chrono::microseconds>(6h)' in net
    return c


C = constants()
TRIED, NEW, SIZE = C['ADDRMAN_TRIED_BUCKET_COUNT'], C['ADDRMAN_NEW_BUCKET_COUNT'], C['ADDRMAN_BUCKET_SIZE']


def load_chacha():
    text = pinned('chacha20.py')
    module = types.ModuleType('chacha20_pinned')
    exec(compile(text, str(CORE / 'chacha20.py'), 'exec'), module.__dict__)
    return module.chacha20_block


CHACHA = load_chacha()


# ---- FastRandomContext (random.h): ChaCha20 keystream, nonce 0, block 0 first
class Rng:
    def __init__(self, key=bytes(32)):
        self.key, self.block, self.buf, self.bits, self.nbits = key, 0, b'', 0, 0

    def rand64(self):
        if not self.buf:
            self.buf = CHACHA(self.key, bytes(12), self.block)
            self.block += 1
        v, self.buf = int.from_bytes(self.buf[:8], 'little'), self.buf[8:]
        return v

    def randbits(self, bits):
        if bits == 64:
            return self.rand64()
        if bits <= self.nbits:
            ret = self.bits
            self.bits >>= bits
            self.nbits -= bits
        else:
            gen = self.rand64()
            ret = ((gen << self.nbits) | self.bits) & (2**64 - 1)
            self.bits = gen >> (bits - self.nbits)
            self.nbits = 64 + self.nbits - bits
        return ret & ((1 << bits) - 1)

    def randrange(self, n):
        maxval = n - 1
        bits = maxval.bit_length()
        while True:
            r = self.randbits(bits)
            if r <= maxval:
                return r

    def randbool(self):
        return self.randbits(1) == 1


# ---- CNetAddr (netaddress.cpp). Networks as Core's enum Network.
UNROUTABLE, IPV4, IPV6, ONION, I2P, CJDNS, INTERNAL = range(7)
V4_PREFIX = bytes(10) + b'\xff\xff'
TORV2_PREFIX = bytes.fromhex('fd87d87eeb43')
INTERNAL_PREFIX = bytes.fromhex('fd6b88c08724')


class Addr:
    def __init__(self, net, ip, port=0):
        self.net, self.ip, self.port = net, bytes(ip), port

    def key_tuple(self):
        return (self.net, self.ip, self.port)

    def same_ip(self, o):
        return self.net == o.net and self.ip == o.ip


def v4(text, port=0):
    return Addr(IPV4, bytes(int(x) for x in text.split('.')), port)


def legacy(b16, port=0):
    if b16.startswith(V4_PREFIX):
        return Addr(IPV4, b16[12:], port)
    if b16.startswith(TORV2_PREFIX):
        return Addr(IPV6, bytes(16), port)
    if b16.startswith(INTERNAL_PREFIX):
        return Addr(INTERNAL, b16[6:], port)
    return Addr(IPV6, b16, port)


def v6(text, port=0):
    import ipaddress
    return legacy(ipaddress.IPv6Address(text).packed, port)


def is_rfc(a):
    m, ipv4, ipv6 = a.ip, a.net == IPV4, a.net == IPV6
    return {
        '1918': ipv4 and (m[0] == 10 or (m[0] == 192 and m[1] == 168) or (m[0] == 172 and 16 <= m[1] <= 31)),
        '2544': ipv4 and m[0] == 198 and m[1] in (18, 19),
        '3927': ipv4 and m[:2] == bytes([169, 254]),
        '6598': ipv4 and m[0] == 100 and 64 <= m[1] <= 127,
        '5737': ipv4 and m[:3] in (bytes([192, 0, 2]), bytes([198, 51, 100]), bytes([203, 0, 113])),
        '3849': ipv6 and m[:4] == bytes([0x20, 0x01, 0x0d, 0xb8]),
        '9637': ipv6 and m[:2] == bytes([0x3f, 0xff]) and (m[2] & 0xf0) == 0,
        '3964': ipv6 and m[:2] == bytes([0x20, 0x02]),
        '6052': ipv6 and m[:12] == bytes([0, 0x64, 0xff, 0x9b]) + bytes(8),
        '4380': ipv6 and m[:4] == bytes([0x20, 0x01, 0, 0]),
        '4862': ipv6 and m[:8] == bytes([0xfe, 0x80]) + bytes(6),
        '4193': ipv6 and (m[0] & 0xfe) == 0xfc,
        '6145': ipv6 and m[:12] == bytes(8) + bytes([0xff, 0xff, 0, 0]),
        '4843': ipv6 and m[:3] == bytes([0x20, 0x01, 0]) and (m[3] & 0xf0) == 0x10,
        '7343': ipv6 and m[:3] == bytes([0x20, 0x01, 0]) and (m[3] & 0xf0) == 0x20,
        'henet': ipv6 and m[:4] == bytes([0x20, 0x01, 0x04, 0x70]),
    }


def is_local(a):
    return (a.net == IPV4 and a.ip[0] in (127, 0)) or (a.net == IPV6 and a.ip == bytes(15) + b'\x01')


def is_valid(a):
    r = is_rfc(a)
    if a.net == IPV6 and a.ip == bytes(16):
        return False
    if a.net == CJDNS and a.ip[0] != 0xfc:
        return False
    if r['3849'] or r['9637'] or a.net == INTERNAL:
        return False
    if a.net == IPV4 and a.ip in (bytes(4), b'\xff' * 4):
        return False
    return True


def is_routable(a):
    r = is_rfc(a)
    return is_valid(a) and not (r['1918'] or r['2544'] or r['3927'] or r['4862'] or r['6598'] or r['5737'] or r['4193']
                                or r['4843'] or r['7343'] or is_local(a) or a.net == INTERNAL)


def network(a):
    if a.net == INTERNAL:
        return INTERNAL
    return a.net if is_routable(a) else UNROUTABLE


def has_linked_ipv4(a):
    r = is_rfc(a)
    return is_routable(a) and (a.net == IPV4 or r['6145'] or r['6052'] or r['3964'] or r['4380'])


def linked_ipv4(a):
    r = is_rfc(a)
    if a.net == IPV4:
        return int.from_bytes(a.ip, 'big')
    if r['6052'] or r['6145']:
        return int.from_bytes(a.ip[12:], 'big')
    if r['3964']:
        return int.from_bytes(a.ip[2:6], 'big')
    return (~int.from_bytes(a.ip[12:], 'big')) & 0xffffffff


def net_class(a):
    if a.net == INTERNAL:
        return INTERNAL
    if not is_routable(a):
        return UNROUTABLE
    if has_linked_ipv4(a):
        return IPV4
    return a.net


def addr_bytes(a):
    if a.net == IPV4:
        return V4_PREFIX + a.ip
    if a.net == INTERNAL:
        return INTERNAL_PREFIX + a.ip
    return a.ip


def service_key(a):
    return addr_bytes(a) + bytes([a.port // 256, a.port & 255])


def group(a):
    """NetGroupManager::GetGroup without an asmap."""
    out = bytes([net_class(a)])
    start, bits = 0, 0
    if is_local(a):
        pass
    elif a.net == INTERNAL:
        start, bits = len(INTERNAL_PREFIX), 10 * 8
    elif not is_routable(a):
        pass
    elif has_linked_ipv4(a):
        v = linked_ipv4(a)
        return out + bytes([(v >> 24) & 255, (v >> 16) & 255])
    elif a.net in (ONION, I2P):
        bits = 4
    elif a.net == CJDNS:
        bits = 12
    elif is_rfc(a)['henet']:
        bits = 36
    else:
        bits = 32
    b = addr_bytes(a)
    n = bits // 8
    out += b[start:start + n]
    if bits % 8:
        out += bytes([b[n + start] | ((1 << (8 - bits % 8)) - 1)])
    return out


def compact(n):
    if n < 253:
        return bytes([n])
    if n <= 0xffff:
        return b'\xfd' + n.to_bytes(2, 'little')
    if n <= 0xffffffff:
        return b'\xfe' + n.to_bytes(4, 'little')
    return b'\xff' + n.to_bytes(8, 'little')


def vec(b):
    return compact(len(b)) + b


def cheap(data):
    return int.from_bytes(hashlib.sha256(hashlib.sha256(data).digest()).digest()[:8], 'little')


def tried_bucket(key, a):
    h1 = cheap(key + vec(service_key(a)))
    h2 = cheap(key + vec(group(a)) + (h1 % C['ADDRMAN_TRIED_BUCKETS_PER_GROUP']).to_bytes(8, 'little'))
    return h2 % TRIED


def new_bucket(key, a, src):
    sg = group(src)
    h1 = cheap(key + vec(group(a)) + vec(sg))
    h2 = cheap(key + vec(sg) + (h1 % C['ADDRMAN_NEW_BUCKETS_PER_SOURCE_GROUP']).to_bytes(8, 'little'))
    return h2 % NEW


def position(key, fnew, bucket, a):
    return cheap(key + (b'N' if fnew else b'K') + struct.pack('<i', bucket) + vec(service_key(a))) % SIZE


TIME_INIT = 100000000


class Info:
    def __init__(self, addr, source, services=0, time=TIME_INIT):
        self.addr, self.source, self.services, self.time = addr, source, services, time
        self.last_try = self.last_count_attempt = self.last_success = 0
        self.attempts = self.refs = 0
        self.tried = False
        self.pos = -1

    def terrible(self, now):
        if now - self.last_try <= 60:
            return False
        if self.time > now + 600:
            return True
        if now - self.time > C['ADDRMAN_HORIZON']:
            return True
        if self.last_success == 0 and self.attempts >= C['ADDRMAN_RETRIES']:
            return True
        if now - self.last_success > C['ADDRMAN_MIN_FAIL'] and self.attempts >= C['ADDRMAN_MAX_FAILURES']:
            return True
        return False

    def chance(self, now):
        f = 1.0
        if now - self.last_try < 600:
            f *= 0.01
        f *= pow(0.66, min(self.attempts, 8))
        return f


class AddrMan:
    def __init__(self, key, rng=None):
        self.key = key
        self.rng = rng or Rng()
        self.id_count = 0
        self.info, self.map = {}, {}
        self.random = []
        self.n_tried = self.n_new = 0
        self.vv_tried = [[-1] * SIZE for _ in range(TRIED)]
        self.vv_new = [[-1] * SIZE for _ in range(NEW)]
        self.last_good = 1
        self.collisions = set()
        self.counts = {}

    def count(self, net, which, d):
        c = self.counts.setdefault(net, [0, 0])
        c[which] += d

    def find(self, a):
        return self.map.get(a.key_tuple(), None)

    def create(self, a, services, time, src):
        nid = self.id_count
        self.id_count += 1
        info = Info(a, src, services, time)
        self.info[nid] = info
        self.map[a.key_tuple()] = nid
        info.pos = len(self.random)
        self.random.append(nid)
        self.n_new += 1
        self.count(network(a), 0, 1)
        return nid

    def swap(self, p1, p2):
        if p1 == p2:
            return
        i1, i2 = self.random[p1], self.random[p2]
        self.info[i1].pos, self.info[i2].pos = p2, p1
        self.random[p1], self.random[p2] = i2, i1

    def delete(self, nid):
        info = self.info[nid]
        assert not info.tried and info.refs == 0
        self.swap(info.pos, len(self.random) - 1)
        self.count(network(info.addr), 0, -1)
        self.random.pop()
        del self.map[info.addr.key_tuple()]
        del self.info[nid]
        self.n_new -= 1

    def clear_new(self, b, p):
        if self.vv_new[b][p] != -1:
            nid = self.vv_new[b][p]
            info = self.info[nid]
            info.refs -= 1
            self.vv_new[b][p] = -1
            if info.refs == 0:
                self.delete(nid)

    def make_tried(self, info, nid):
        start = new_bucket(self.key, info.addr, info.source)
        for n in range(NEW):
            b = (start + n) % NEW
            p = position(self.key, True, b, info.addr)
            if self.vv_new[b][p] == nid:
                self.vv_new[b][p] = -1
                info.refs -= 1
                if info.refs == 0:
                    break
        self.n_new -= 1
        self.count(network(info.addr), 0, -1)
        kb = tried_bucket(self.key, info.addr)
        kp = position(self.key, False, kb, info.addr)
        if self.vv_tried[kb][kp] != -1:
            evict = self.vv_tried[kb][kp]
            old = self.info[evict]
            old.tried = False
            self.vv_tried[kb][kp] = -1
            self.n_tried -= 1
            self.count(network(old.addr), 1, -1)
            ub = new_bucket(self.key, old.addr, old.source)
            up = position(self.key, True, ub, old.addr)
            self.clear_new(ub, up)
            old.refs = 1
            self.vv_new[ub][up] = evict
            self.n_new += 1
            self.count(network(old.addr), 0, 1)
        self.vv_tried[kb][kp] = nid
        self.n_tried += 1
        info.tried = True
        self.count(network(info.addr), 1, 1)

    def add_single(self, a, services, time, src, penalty, now):
        if not is_routable(a):
            return False
        nid = self.find(a)
        if a.same_ip(src):
            penalty = 0
        if nid is not None:
            info = self.info[nid]
            online = now - time < 24 * 3600
            interval = 3600 if online else 24 * 3600
            if info.time < time - interval - penalty:
                info.time = max(0, time - penalty)
            info.services |= services
            if time <= info.time:
                return False
            if info.tried:
                return False
            if info.refs == C['ADDRMAN_NEW_BUCKETS_PER_ADDRESS']:
                return False
            if info.refs > 0:
                if self.rng.randrange(1 << info.refs) != 0:
                    return False
        else:
            nid = self.create(a, services, time, src)
            info = self.info[nid]
            info.time = max(0, info.time - penalty)
        ub = new_bucket(self.key, a, src)
        up = position(self.key, True, ub, a)
        insert = self.vv_new[ub][up] == -1
        if self.vv_new[ub][up] != nid:
            if not insert:
                existing = self.info[self.vv_new[ub][up]]
                if existing.terrible(now) or (existing.refs > 1 and info.refs == 0):
                    insert = True
            if insert:
                self.clear_new(ub, up)
                info.refs += 1
                self.vv_new[ub][up] = nid
            elif info.refs == 0:
                self.delete(nid)
        return insert

    def add(self, entries, src, penalty, now):
        return sum(1 for (a, s, t) in entries if self.add_single(a, s, t, src, penalty, now))

    def good_(self, a, test_before_evict, time):
        self.last_good = time
        nid = self.find(a)
        if nid is None:
            return False
        info = self.info[nid]
        info.last_success = info.last_try = time
        info.attempts = 0
        if info.tried:
            return False
        if info.refs <= 0:
            return False
        tb = tried_bucket(self.key, info.addr)
        tp = position(self.key, False, tb, info.addr)
        if test_before_evict and self.vv_tried[tb][tp] != -1:
            if len(self.collisions) < C['ADDRMAN_SET_TRIED_COLLISION_SIZE']:
                self.collisions.add(nid)
            return False
        self.make_tried(info, nid)
        return True

    def good(self, a, time):
        return self.good_(a, True, time)

    def attempt(self, a, count_failure, time):
        nid = self.find(a)
        if nid is None:
            return
        info = self.info[nid]
        info.last_try = time
        if count_failure and info.last_count_attempt < self.last_good:
            info.last_count_attempt = time
            info.attempts += 1

    def connected(self, a, time):
        nid = self.find(a)
        if nid is not None and time - self.info[nid].time > 20 * 60:
            self.info[nid].time = time

    def set_services(self, a, services):
        nid = self.find(a)
        if nid is not None:
            self.info[nid].services = services

    def select(self, new_only, networks, now):
        if not self.random:
            return None
        new_count, tried_count = self.n_new, self.n_tried
        if networks:
            new_count = sum(self.counts.get(n, [0, 0])[0] for n in networks)
            tried_count = sum(self.counts.get(n, [0, 0])[1] for n in networks)
        if new_only and new_count == 0:
            return None
        if new_count + tried_count == 0:
            return None
        if new_only or tried_count == 0:
            search_tried = False
        elif new_count == 0:
            search_tried = True
        else:
            search_tried = self.rng.randbool()
        buckets = TRIED if search_tried else NEW
        factor = 1.0
        while True:
            b = self.rng.randrange(buckets)
            table = self.vv_tried if search_tried else self.vv_new
            copied = [x for x in table[b] if x != -1]
            node = -1
            while copied:
                p = self.rng.randrange(len(copied))
                node = copied[p]
                if not networks:
                    break
                if network(self.info[node].addr) in networks:
                    break
                copied[p] = copied[-1]
                copied.pop()
            if not copied:
                continue
            info = self.info[node]
            if self.rng.randbits(30) < factor * info.chance(now) * (1 << 30):
                return node
            factor *= 1.2

    def get_addr(self, max_addresses, max_pct, net, filtered, now):
        n = len(self.random)
        if max_pct != 0:
            n = min(max_pct, 100) * n // 100
        if max_addresses != 0:
            n = min(n, max_addresses)
        out = []
        for i in range(len(self.random)):
            if len(out) >= n:
                break
            r = self.rng.randrange(len(self.random) - i) + i
            self.swap(i, r)
            info = self.info[self.random[i]]
            if net is not None and net_class(info.addr) != net:
                continue
            if filtered and info.terrible(now):
                continue
            out.append(self.random[i])
        return out

    def resolve_collisions(self, now):
        for nid in sorted(self.collisions):
            erase = False
            if nid not in self.info:
                erase = True
            else:
                new = self.info[nid]
                tb = tried_bucket(self.key, new.addr)
                tp = position(self.key, False, tb, new.addr)
                if not is_valid(new.addr):
                    erase = True
                else:
                    old = self.info[self.vv_tried[tb][tp]]
                    if now - old.last_success < C['ADDRMAN_REPLACEMENT']:
                        erase = True
                    elif now - old.last_try < C['ADDRMAN_REPLACEMENT']:
                        if now - old.last_try > 60:
                            self.good_(new.addr, False, now)
                            erase = True
                    elif now - new.last_success > C['ADDRMAN_TEST_WINDOW']:
                        self.good_(new.addr, False, now)
                        erase = True
            if erase:
                self.collisions.discard(nid)

    def select_tried_collision(self):
        if not self.collisions:
            return None
        ids = sorted(self.collisions)
        nid = ids[self.rng.randrange(len(ids))]
        if nid not in self.info:
            self.collisions.discard(nid)
            return None
        new = self.info[nid]
        tb = tried_bucket(self.key, new.addr)
        tp = position(self.key, False, tb, new.addr)
        return self.vv_tried[tb][tp]

    def find_entry(self, a):
        nid = self.find(a)
        if nid is None:
            return None
        info = self.info[nid]
        if info.tried:
            b = tried_bucket(self.key, info.addr)
            return (True, 1, b, position(self.key, False, b, info.addr))
        b = new_bucket(self.key, info.addr, info.source)
        return (False, info.refs, b, position(self.key, True, b, info.addr))

    def size(self, net=None, in_new=None):
        if net is None:
            if in_new is None:
                return len(self.random)
            return self.n_new if in_new else self.n_tried
        c = self.counts.get(net, [0, 0])
        if in_new is None:
            return c[0] + c[1]
        return c[0] if in_new else c[1]

    def check(self):
        """CheckAddrman: 0, or Core's error code."""
        if len(self.random) != self.n_tried + self.n_new:
            return -7
        tried, new = set(), {}
        for n, info in self.info.items():
            if info.tried:
                if not info.last_success:
                    return -1
                if info.refs:
                    return -2
                tried.add(n)
            else:
                if info.refs < 0 or info.refs > C['ADDRMAN_NEW_BUCKETS_PER_ADDRESS']:
                    return -3
                if not info.refs:
                    return -4
                new[n] = info.refs
            if self.map.get(info.addr.key_tuple()) != n:
                return -5
            if info.pos < 0 or info.pos >= len(self.random) or self.random[info.pos] != n:
                return -14
        if len(tried) != self.n_tried:
            return -9
        if len(new) != self.n_new:
            return -10
        for b in range(TRIED):
            for i in range(SIZE):
                x = self.vv_tried[b][i]
                if x != -1:
                    if x not in tried:
                        return -11
                    if tried_bucket(self.key, self.info[x].addr) != b:
                        return -17
                    if position(self.key, False, b, self.info[x].addr) != i:
                        return -18
                    tried.discard(x)
        for b in range(NEW):
            for i in range(SIZE):
                x = self.vv_new[b][i]
                if x != -1:
                    if x not in new:
                        return -12
                    if position(self.key, True, b, self.info[x].addr) != i:
                        return -19
                    new[x] -= 1
                    if new[x] == 0:
                        del new[x]
        if tried:
            return -13
        if new:
            return -15
        if self.key == bytes(32):
            return -16
        return 0

    # ---- peers.dat: Serialize, as Core writes format 4 (V4_MULTIPORT)
    def serialize(self, magic):
        bip155 = {IPV4: 1, IPV6: 2, ONION: 4, I2P: 5, CJDNS: 6, INTERNAL: 2}

        def netaddr(a):
            return bytes([bip155[a.net]]) + vec(addr_bytes(a) if a.net == INTERNAL else a.ip)

        def entry(info):
            return (struct.pack('<I', 220000 | (1 << 29)) + struct.pack('<I', info.time) + compact(info.services)
                    + netaddr(info.addr) + info.addr.port.to_bytes(2, 'big') + netaddr(info.source)
                    + struct.pack('<q', info.last_success) + struct.pack('<i', info.attempts))
        out = bytes([4, 32 + 4]) + self.key + struct.pack('<iii', self.n_new, self.n_tried, NEW ^ (1 << 30))
        # Core walks its unordered_map, an order C++ leaves unspecified; this
        # walks vRandom, which both implementations keep in the same order.
        index = {}
        for nid in self.random:
            info = self.info[nid]
            if info.refs:
                index[nid] = len(index)
                out += entry(info)
        for nid in self.random:
            if self.info[nid].tried:
                out += entry(self.info[nid])
        for b in range(NEW):
            ids = [x for x in self.vv_new[b] if x != -1]
            out += struct.pack('<i', len(ids)) + b''.join(struct.pack('<i', index[x]) for x in ids)
        out += bytes(32)
        body = magic + out
        return body + hashlib.sha256(hashlib.sha256(body).digest()).digest()


class Corrupt(Exception):
    pass


def unserialize(data, magic, key_unused=None):
    """DeserializeDB then AddrManImpl::Unserialize: the address manager, or Corrupt."""
    if len(data) < 36 or data[:4] != magic:
        raise Corrupt('magic')
    if hashlib.sha256(hashlib.sha256(data[:-32]).digest()).digest() != data[-32:]:
        raise Corrupt('checksum')
    s = data[4:-32]
    at = [0]

    def take(n):
        if at[0] + n > len(s):
            raise Corrupt('end of data')
        r = s[at[0]:at[0] + n]
        at[0] += n
        return r

    def u(n, signed=False):
        return int.from_bytes(take(n), 'little', signed=signed)

    def compact_read():
        x = u(1)
        if x < 253:
            return x
        return u({253: 2, 254: 4, 255: 8}[x])

    nets = {1: (IPV4, 4), 2: (IPV6, 16), 4: (ONION, 32), 5: (I2P, 32), 6: (CJDNS, 16)}

    def netaddr(v2):
        if not v2:
            return legacy(take(16))
        nid, n = u(1), compact_read()
        if n > 512:
            raise Corrupt('address too long')
        if nid in nets and nets[nid][1] == n:
            raw = take(n)
            net = nets[nid][0]
            if net != IPV6:
                return Addr(net, raw)
            if raw.startswith(INTERNAL_PREFIX):
                return Addr(INTERNAL, raw[6:])
            if raw.startswith(V4_PREFIX) or raw.startswith(TORV2_PREFIX):
                return Addr(IPV6, bytes(16))
            return Addr(IPV6, raw)
        if nid in nets:
            raise Corrupt('wrong length')
        take(n)
        return Addr(IPV6, bytes(16))

    def entry(fmt):
        version = u(4) & ~0x7ffff
        if version == 0:
            v2 = False
        elif version == (1 << 29) and fmt >= 3:
            v2 = True
        else:
            raise Corrupt('Unsupported CAddress disk format version')
        time = u(4)
        services = compact_read() if v2 else u(8)
        a = netaddr(v2)
        a.port = int.from_bytes(take(2), 'big')
        src = netaddr(v2)
        info = Info(a, src, services, time)
        info.last_success = u(8, True)
        info.attempts = u(4, True)
        return info

    fmt = u(1)
    compat = u(1)
    if compat < 32:
        raise Corrupt('compat')
    if compat - 32 > 4:
        raise Corrupt('version')
    m = AddrMan(take(32), Rng())
    n_new, n_tried, n_buckets = u(4, True), u(4, True), u(4, True)
    if fmt >= 1:
        n_buckets ^= 1 << 30
    if not 0 <= n_new <= NEW * SIZE or not 0 <= n_tried <= TRIED * SIZE:
        raise Corrupt('counts')
    for n in range(n_new):
        info = entry(fmt)
        m.info[n] = info
        m.map[info.addr.key_tuple()] = n
        info.pos = len(m.random)
        m.random.append(n)
        m.count(network(info.addr), 0, 1)
    m.id_count = n_new
    m.n_new = n_new
    lost = 0
    for _ in range(n_tried):
        info = entry(fmt)
        kb = tried_bucket(m.key, info.addr)
        kp = position(m.key, False, kb, info.addr)
        if is_valid(info.addr) and m.vv_tried[kb][kp] == -1:
            info.pos = len(m.random)
            info.tried = True
            m.random.append(m.id_count)
            m.info[m.id_count] = info
            m.map[info.addr.key_tuple()] = m.id_count
            m.vv_tried[kb][kp] = m.id_count
            m.id_count += 1
            m.count(network(info.addr), 1, 1)
        else:
            lost += 1
    m.n_tried = n_tried - lost
    pairs = []
    for b in range(n_buckets):
        for _ in range(u(4, True)):
            e = u(4, True)
            if 0 <= e < n_new:
                pairs.append((b, e))
    version = take(32) if fmt >= 2 else bytes(32)
    restore = n_buckets == NEW and version == bytes(32)
    for b, e in pairs:
        info = m.info[e]
        if not is_valid(info.addr):
            continue
        if info.refs >= C['ADDRMAN_NEW_BUCKETS_PER_ADDRESS']:
            continue
        p = position(m.key, True, b, info.addr)
        if restore and m.vv_new[b][p] == -1:
            m.vv_new[b][p] = e
            info.refs += 1
        else:
            b = new_bucket(m.key, info.addr, info.source)
            p = position(m.key, True, b, info.addr)
            if m.vv_new[b][p] == -1:
                m.vv_new[b][p] = e
                info.refs += 1
    for nid in sorted(m.info):
        if not m.info[nid].tried and m.info[nid].refs == 0:
            m.delete(nid)
    if m.check() != 0:
        raise Corrupt('check')
    return m


# ---- address relay (net_processing.cpp ProcessAddrs, RelayAddress; net.cpp GetAddresses)
NODE_NETWORK, NODE_WITNESS, NODE_NETWORK_LIMITED, NODE_P2P_V2 = 1, 8, 1024, 2048


def desirable(services, tip_depth):
    if services & NODE_NETWORK_LIMITED and tip_depth < 144:
        want = NODE_NETWORK_LIMITED | NODE_WITNESS
    else:
        want = NODE_NETWORK | NODE_WITNESS
    return services & want == want


class Peer:
    def __init__(self, now):
        self.tokens, self.stamp = 1.0, now
        self.getaddr_recvd = False


def process_addrs(m, peer, entries, src, now, rng, permission=False, tip_depth=0, reachable=(IPV4, IPV6)):
    if len(entries) > 1000:
        return ('misbehaving', 0, 0, 0, [])
    if peer.tokens < 1000:
        peer.tokens = min(peer.tokens + max(now - peer.stamp, 0) * 0.1, 1000)
    peer.stamp = now
    entries = list(entries)
    for i in range(len(entries) - 1, 0, -1):
        j = rng.randrange(i + 1)
        entries[i], entries[j] = entries[j], entries[i]
    processed = limited = 0
    ok, relay = [], []
    for (a, services, time) in entries:
        if peer.tokens < 1.0:
            if not permission:
                limited += 1
                continue
        else:
            peer.tokens -= 1.0
        if not (services & (NODE_NETWORK | NODE_NETWORK_LIMITED)) and not desirable(services, tip_depth):
            continue
        if time <= TIME_INIT or time > now + 600:
            time = now - 5 * 86400
        processed += 1
        if time > now - 600 and len(entries) <= 10 and is_routable(a):
            relay.append(a)
        if network(a) in reachable:
            ok.append((a, services, time))
    added = m.add(ok, src, 2 * 3600, now)
    return ('ok', processed, limited, added, relay)


def siphash(k0, k1, data):
    mask = 2**64 - 1

    def rotl(x, b):
        return ((x << b) | (x >> (64 - b))) & mask
    v = [k0 ^ 0x736f6d6570736575, k1 ^ 0x646f72616e646f6d, k0 ^ 0x6c7967656e657261, k1 ^ 0x7465646279746573]

    def rnd():
        v[0] = (v[0] + v[1]) & mask; v[1] = rotl(v[1], 13); v[1] ^= v[0]; v[0] = rotl(v[0], 32)
        v[2] = (v[2] + v[3]) & mask; v[3] = rotl(v[3], 16); v[3] ^= v[2]
        v[0] = (v[0] + v[3]) & mask; v[3] = rotl(v[3], 21); v[3] ^= v[0]
        v[2] = (v[2] + v[1]) & mask; v[1] = rotl(v[1], 17); v[1] ^= v[2]; v[2] = rotl(v[2], 32)
    tail = len(data) % 8
    for i in range(0, len(data) - tail, 8):
        w = int.from_bytes(data[i:i + 8], 'little')
        v[3] ^= w; rnd(); rnd(); v[0] ^= w
    w = (len(data) & 255) << 56 | int.from_bytes(data[len(data) - tail:], 'little')
    v[3] ^= w; rnd(); rnd(); v[0] ^= w
    v[2] ^= 255
    rnd(); rnd(); rnd(); rnd()
    return v[0] ^ v[1] ^ v[2] ^ v[3]


def service_hash(a):
    """CServiceHash(0, 0): SipHash of m_net, port and m_addr."""
    return siphash(0, 0, a.net.to_bytes(8, 'little') + a.port.to_bytes(8, 'little') + a.ip)


def relay_targets(seed0, seed1, a, reachable, now, peers):
    """RelayAddress: the peers an address goes to, best first. peers are (id, eligible)."""
    h = service_hash(a)
    t = (now + h) // (24 * 3600) % 2**64
    base = C['RANDOMIZER_ID_ADDRESS_RELAY'].to_bytes(8, 'little') + h.to_bytes(8, 'little') + t.to_bytes(8, 'little')
    count = 2 if reachable or siphash(seed0, seed1, base) & 1 else 1
    best = [(0, None), (0, None)]
    for pid, ok in peers:
        if not ok:
            continue
        k = siphash(seed0, seed1, base + pid.to_bytes(8, 'little'))
        for i in range(count):
            if k > best[i][0]:
                best[i + 1:count] = best[i:count - 1]
                best[i] = (k, pid)
                break
    return [pid for k, pid in best[:count] if k != 0]


class Cache:
    def __init__(self):
        self.expires, self.ids = 0, []


def getaddr_reply(m, peer, inbound, cache, now_us, rng):
    if not inbound:
        return 'ignored: outbound'
    if peer.getaddr_recvd:
        return 'ignored: repeated'
    peer.getaddr_recvd = True
    if cache.expires < now_us:
        cache.ids = [m.info[i].addr for i in m.get_addr(1000, 23, None, True, now_us // 1000000)]
        cache.expires = now_us + 21 * 3600 * 10**6 + rng.randrange(6 * 3600 * 10**6)
    return cache.ids


def seed_plan(size, force, seeds):
    """ThreadDNSAddressSeed's first decision: how many seeds to query now and how long to wait first."""
    if force or size == 0:
        return seeds, 0
    wait = 300 if size >= C['DNSSEEDS_DELAY_PEER_THRESHOLD'] else 11
    return C['DNSSEEDS_TO_QUERY_AT_ONCE'], wait


# ---- the transcript
NOW = 1700000000
MAINNET = bytes.fromhex('f9beb4d9')


def show(a):
    if a is None:
        return '[::]:0'
    if a.net == IPV4:
        return '.'.join(str(x) for x in a.ip) + f':{a.port}'
    return f'net{a.net}:{a.ip.hex()}:{a.port}'


def transcript():
    lines = []
    out = lines.append
    k1 = hashlib.sha256(hashlib.sha256((1).to_bytes(4, 'little')).digest()).digest()
    k2 = hashlib.sha256(hashlib.sha256((2).to_bytes(4, 'little')).digest()).digest()
    one = bytes([1]) + bytes(31)

    out('groups')
    I2P_A = bytes.fromhex('a0ce38ce2224d2cecaf9929388f73379259c0c27e0debdbd7ca4cd085b55e25a')
    for label, a in [('250.1.1.1', v4('250.1.1.1')), ('10.0.0.1', v4('10.0.0.1')), ('127.0.0.1', v4('127.0.0.1')),
                     ('2001:470:1234::1 (he.net)', v6('2001:470:1234::1')), ('2a01:e::1', v6('2a01:e::1')),
                     ('2002:fa01:0101::1 (6to4)', v6('2002:fa01:0101::1')), ('2001:0::0506:fefe (Teredo)', v6('2001:0::0506:fefe')),
                     ('::ffff:250.1.1.1 (v1 form)', legacy(V4_PREFIX + bytes([250, 1, 1, 1]))),
                     ('fc00::1 (CJDNS)', Addr(CJDNS, bytes.fromhex('fc000000000000000000000000000001'))),
                     ('I2P', Addr(I2P, I2P_A)), ('Tor v3', Addr(ONION, bytes.fromhex('f2' + '00' * 31)))]:
        out(f'  {label}: {group(a).hex()}' + ('' if is_routable(a) else ' (unroutable)'))

    out('buckets with SHA-256d(1) as the key')
    a1, a2 = v4('250.1.1.1', 8333), v4('250.1.1.1', 9999)
    out(f'  250.1.1.1:8333 tried bucket {tried_bucket(k1, a1)}, with key 2: {tried_bucket(k2, a1)}, port 9999: {tried_bucket(k1, a2)}')
    b1, s1 = v4('250.1.2.1', 8333), v4('250.1.2.1')
    out(f'  250.1.2.1:8333 from itself: new bucket {new_bucket(k1, b1, s1)}, with key 2: {new_bucket(k2, b1, s1)}, port 9999: {new_bucket(k1, v4("250.1.2.1", 9999), s1)}')
    same16 = {tried_bucket(k1, v4(f'250.1.1.{i}')) for i in range(255)}
    diff16 = {tried_bucket(k1, v4(f'250.{j}.1.1')) for j in range(255)}
    out(f'  tried buckets for 255 addresses in one /16: {len(same16)}; in 255 /16s: {len(diff16)}')
    one_src = {new_bucket(k1, v4(f'{250 + j // 255}.{j % 256}.1.1'), v4('251.4.1.1')) for j in range(4 * 255)}
    many_src = {new_bucket(k1, v4('250.1.1.1'), v4(f'250.{p}.1.1')) for p in range(255)}
    same_grp = {new_bucket(k1, v4(f'250.1.1.{i}'), v4(f'250.1.1.{i}')) for i in range(255)}
    out(f'  new buckets for 1,020 addresses from one source group: {len(one_src)}; one address from 255 source groups: {len(many_src)}; one /16 from itself: {len(same_grp)}')

    src = v4('252.2.2.2')
    out('new table collisions (deterministic key 1)')
    m = AddrMan(one)
    sizes = []
    for i in range(1, 25):
        m.add([(v4(f'250.1.1.{i}'), 0, TIME_INIT)], src, 0, NOW)
        sizes.append(m.size())
    out(f'  sizes after adding 250.1.1.1 to .24: {" ".join(str(s) for s in sizes)}')
    out(f'  check: {m.check()}')

    out('tried table collisions')
    m = AddrMan(one)
    goods = []
    for i in range(1, 38):
        a = v4(f'250.1.1.{i}')
        m.add([(a, 0, TIME_INIT)], src, 0, NOW)
        goods.append('1' if m.good(a, NOW) else '0')
    out(f'  Good for 250.1.1.1 to .37: {"".join(goods)}')
    out(f'  collision with: {show(m.info[m.select_tried_collision()].addr)}')
    m.resolve_collisions(NOW)
    c = m.select_tried_collision()
    out(f'  after ResolveCollisions, 19 still recent: {show(None if c is None else m.info[c].addr)}')
    pos = m.find_entry(v4('250.1.1.36'))
    out(f'  250.1.1.36 stays in new: tried={pos[0]}, multiplicity {pos[1]}')

    out('test-before-evict')
    m = AddrMan(one)
    for i in range(1, 36):
        a = v4(f'250.1.1.{i}')
        m.add([(a, 0, TIME_INIT)], src, 0, NOW)
        m.good(a, NOW)
    a36, a19 = v4('250.1.1.36'), v4('250.1.1.19')
    m.add([(a36, 0, TIME_INIT)], src, 0, NOW)
    out(f'  Good(250.1.1.36): {m.good(a36, NOW)}, colliding with {show(m.info[m.select_tried_collision()].addr)}')
    m.good(a19, 1)
    m.attempt(a19, False, NOW - 61)
    m.resolve_collisions(NOW)
    out(f'  19 last succeeded at 1 s and was tried 61 s ago: 36 tried={m.find_entry(a36)[0]}, 19 tried={m.find_entry(a19)[0]}')
    out(f'  Good(250.1.1.19): {m.good(a19, NOW)}, colliding with {show(m.info[m.select_tried_collision()].addr)}')
    m.resolve_collisions(NOW + 4 * 3600)
    out(f'  4 hours later, untested: 19 tried={m.find_entry(a19)[0]}, 36 tried={m.find_entry(a36)[0]}, check {m.check()}')

    out('multiplicity')
    m = AddrMan(one)
    a = v4('253.3.3.3', 8333)
    for i in range(1, 20):
        m.add([(a, 0, NOW)], v4(f'{i % 256}.{i >> 8}.1.1'), 0, NOW)
    first = m.find_entry(a)[1]
    for i in range(1, 400):
        m.add([(a, 0, NOW + i)], v4(f'{i % 256}.{i >> 8}.1.1'), 0, NOW + 400)
    out(f'  19 sources, same time: {first}; 399 sources, newer each time: {m.find_entry(a)[1]}; size {m.size()}')

    out('getaddr')
    m = AddrMan(one)
    first = [(v4('250.250.2.1', 8333), 0, NOW), (v4('250.251.2.2', 9999), 0, NOW), (v4('251.252.2.3', 8333), 0, NOW),
             (v4('252.253.3.4', 8333), 0, NOW), (v4('252.254.4.5', 8333), 0, NOW)]
    m.add([first[0], first[2], first[4]], v4('250.1.2.1'), 0, NOW)
    m.add([first[1], first[3]], v4('250.2.3.3'), 0, NOW)
    out(f'  five addresses: {len(m.get_addr(0, 0, None, True, NOW))} returned, 23% gives {len(m.get_addr(2500, 23, None, True, NOW))}')
    m.good(first[0][0], NOW)
    m.good(first[1][0], NOW)
    for i in range(1, 8 * 256):
        text = f'{i % 256}.{i >> 8}.1.23'
        a = v4(text)
        m.add([(a, 0, NOW)], v4(text), 0, NOW)
        if i % 8 == 0:
            m.good(a, NOW)
    got = m.get_addr(2500, 23, None, True, NOW)
    out(f'  size {m.size()} ({m.size(None, True)} new, {m.size(None, False)} tried), 23% gives {len(got)}, check {m.check()}')
    out(f'  first five: {" ".join(show(m.info[i].addr) for i in got[:5])}')

    out('select')
    m = AddrMan(one)
    for text, port, source in [('250.1.1.1', 8333, '252.2.2.2'), ('250.3.1.1', 8333, '250.3.1.1'), ('250.3.2.2', 9999, '250.3.1.1'),
                               ('250.3.3.3', 9999, '250.4.1.1'), ('250.4.4.4', 8333, '250.3.2.2'), ('250.4.5.5', 7777, '250.3.2.2'),
                               ('250.4.6.6', 8333, '250.1.1.3')]:
        m.add([(v4(text, port), 0, NOW)], v4(source), 0, NOW)
    for text, port in [('250.1.1.1', 8333), ('250.4.4.4', 8333), ('250.4.5.5', 7777), ('250.4.6.6', 8333)]:
        m.good(v4(text, port), NOW - 3600)
    out(f'  {m.size(None, True)} new, {m.size(None, False)} tried')
    picks = [m.select(False, (), NOW) for _ in range(12)]
    out('  any: ' + ' '.join(show(m.info[p].addr) for p in picks))
    picks = [m.select(True, (), NOW) for _ in range(6)]
    out('  new only: ' + ' '.join(show(m.info[p].addr) for p in picks))
    i2p = Addr(I2P, I2P_A, 0)
    m.add([(i2p, 0, NOW)], src, 0, NOW)
    p = m.select(False, (I2P,), NOW)
    q = m.select(False, (CJDNS,), NOW)
    out(f'  I2P only: {show(m.info[p].addr)}; CJDNS only: {show(None if q is None else m.info[q].addr)}')
    m.attempt(v4('250.1.1.1', 8333), True, NOW - 30)
    out(f'  chance of 250.1.1.1 after a failed attempt 30 s ago: 1 in {round(1 / m.info[m.find(v4("250.1.1.1", 8333))].chance(NOW))}')

    out('terrible')
    for label, info in [('tried 30 s ago', dict(last_try=NOW - 30, time=0)), ('11 minutes in the future', dict(time=NOW + 660)),
                        ('31 days old', dict(time=NOW - 31 * 86400)), ('3 attempts, never a success', dict(time=NOW, attempts=3)),
                        ('10 failures in 8 days', dict(time=NOW, attempts=10, last_success=NOW - 8 * 86400)),
                        ('9 failures in 8 days', dict(time=NOW, attempts=9, last_success=NOW - 8 * 86400)),
                        ('fresh', dict(time=NOW))]:
        x = Info(v4('250.1.1.1'), src)
        for k, v in info.items():
            setattr(x, k, v)
        out(f'  {label}: {x.terrible(NOW)}')

    out('eclipse: one source group sends 20,000 addresses')
    m = AddrMan(one)
    attacker = v4('66.66.1.1')
    for i in range(20000):
        a = v4(f'{1 + i % 200}.{(i // 200) % 250}.{i % 7}.{9 + i % 11}', 8333)
        m.add([(a, 0, NOW)], v4(f'66.66.{i % 250}.{1 + i % 5}'), 0, NOW)
    buckets = sum(1 for b in range(NEW) if any(x != -1 for x in m.vv_new[b]))
    out(f'  kept {m.size()} of 20000 in {buckets} of {NEW} new buckets')
    honest = 0
    for i in range(1000):
        a = v4(f'{100 + i % 50}.{i // 50}.7.7', 8333)
        if m.add([(a, 0, NOW)], v4(f'{150 + i % 50}.{i // 50 + 30}.1.1'), 0, NOW):
            honest += 1
    picks = [m.select(True, (), NOW) for _ in range(400)]
    from_attacker = sum(1 for p in picks if m.info[p].addr.ip[2] != 7)
    out(f'  1,000 honest addresses from 1,000 groups: {honest} kept; of 400 selections {from_attacker} are the attacker\'s, check {m.check()}')

    out('peers.dat')
    m = AddrMan(one)
    for i in range(1, 9):
        m.add([(v4(f'250.7.{i}.1', 8333), 9, NOW - i)], v4('252.5.1.1'), 0, NOW)
    m.good(v4('250.7.1.1', 8333), NOW)
    m.good(v4('250.7.2.1', 8333), NOW)
    m.add([(i2p, 1024 | 8, NOW)], v4('252.5.1.1'), 0, NOW)
    data = m.serialize(MAINNET)
    out(f'  {len(data)} bytes, SHA-256 {hashlib.sha256(data).hexdigest()}')
    back = unserialize(data, MAINNET)
    out(f'  read back: {back.size(None, True)} new, {back.size(None, False)} tried, same places: {all(back.find_entry(m.info[i].addr) == m.find_entry(m.info[i].addr) for i in m.info)}')
    for label, cut in [('a flipped byte', 'flip'), ('the last byte missing', 'cut'), ('another network', 'magic')]:
        bad = bytearray(data)
        if cut == 'flip':
            bad[60] ^= 1
        elif cut == 'cut':
            bad = bad[:-1]
        try:
            unserialize(bytes(bad), bytes.fromhex('0b110907') if cut == 'magic' else MAINNET)
            out(f'  {label}: read')
        except Corrupt as e:
            out(f'  {label}: refused')
    # Core's load_addrman_corrupted: 10 new and 10 tried claimed, one entry present.
    body = bytes([1, 32]) + one + struct.pack('<iii', 10, 10, NEW ^ (1 << 30)) + struct.pack('<II', 0, TIME_INIT) + bytes(8) \
        + V4_PREFIX + bytes([252, 1, 1, 1]) + (7777).to_bytes(2, 'big') + V4_PREFIX + bytes([252, 2, 2, 2]) + bytes(12)
    corrupt = MAINNET + body
    corrupt += hashlib.sha256(hashlib.sha256(corrupt).digest()).digest()
    try:
        unserialize(corrupt, MAINNET)
        out('  20 entries claimed, one present: read')
    except Corrupt as e:
        out(f'  20 entries claimed, one present: refused ({e})')
    # Core's remove_invalid: two entries rewritten as 0.0.0.0 and 255.255.255.255 are dropped.
    m = AddrMan(one)
    m.add([(v4('5.5.5.5'), 0, TIME_INIT), (v4('7.7.7.7'), 0, TIME_INIT), (v4('6.6.6.6'), 0, TIME_INIT), (v4('8.8.8.8'), 0, TIME_INIT)], Addr(IPV6, bytes(16)), 0, NOW)
    m.good(v4('7.7.7.7'), NOW)
    m.good(v4('8.8.8.8'), NOW)
    data = bytearray(m.serialize(MAINNET)[:-32])
    data = data.replace(bytes([6, 6, 6, 6]), bytes(4), 1).replace(bytes([8, 8, 8, 8]), b'\xff' * 4, 1)
    data = bytes(data) + hashlib.sha256(hashlib.sha256(bytes(data)).digest()).digest()
    out(f'  four entries, two made invalid: {unserialize(data, MAINNET).size()} remain')

    out('addr relay')
    m = AddrMan(one)
    rng = Rng(bytes([7]) * 32)
    peer = Peer(NOW)

    def batch(n, base, time, services=NODE_NETWORK | NODE_WITNESS):
        return [(v4(f'{base}.{i // 250}.{i % 250}.1', 8333), services, time) for i in range(n)]
    src_peer = v4('99.1.1.1', 8333)
    r = process_addrs(m, peer, batch(1001, 20, NOW), src_peer, NOW, rng)
    out(f'  1,001 addresses: {r[0]}')
    peer.tokens += 1000
    r = process_addrs(m, peer, batch(1000, 21, NOW - 3600), src_peer, NOW + 1, rng)
    out(f'  1,000 after our getaddr: {r[1]} processed, {r[2]} rate-limited, {r[3]} added, {len(r[4])} to relay')
    r = process_addrs(m, peer, batch(1000, 22, NOW - 3600), src_peer, NOW + 11, rng)
    out(f'  1,000 more 10 s later: {r[1]} processed, {r[2]} rate-limited')
    r = process_addrs(m, peer, batch(1000, 23, NOW - 3600), src_peer, NOW + 11, rng, permission=True)
    out(f'  1,000 from a peer with the addr permission: {r[1]} processed, {r[2]} rate-limited')
    peer = Peer(NOW)
    r = process_addrs(m, peer, batch(10, 30, NOW + 3540), src_peer, NOW + 3600, rng)
    out(f'  10 fresh addresses an hour later: {r[1]} processed, {len(r[4])} to relay')
    r = process_addrs(m, peer, batch(11, 31, NOW + 3540), src_peer, NOW + 3600, rng)
    out(f'  11 fresh addresses: {r[1]} processed, {len(r[4])} to relay, {r[2]} rate-limited')
    peer = Peer(NOW - 100)
    entries = [(v4('40.1.1.1', 8333), 0, NOW), (v4('40.1.1.2', 8333), NODE_NETWORK_LIMITED, 50), (v4('10.1.1.3', 8333), NODE_NETWORK, NOW),
               (Addr(ONION, bytes(32), 9050), NODE_NETWORK, NOW)]
    r = process_addrs(m, peer, entries, src_peer, NOW, rng)
    t = m.info[m.find(v4('40.1.1.2', 8333))].time
    out(f'  no services, an ancient time, a private address, Tor: {r[1]} processed, {r[3]} added; the ancient time became now - {(NOW - t) // 3600} h')
    peers = [(i, i != 3) for i in range(1, 9)]
    a = v4('250.1.1.1', 8333)
    out(f'  relay 250.1.1.1:8333 to: {relay_targets(11, 22, a, True, NOW, peers)}, next day: {relay_targets(11, 22, a, True, NOW + 86400, peers)}')
    out(f'  unreachable Tor address to: {relay_targets(11, 22, Addr(ONION, bytes(32), 9050), False, NOW, peers)}')

    out('getaddr replies')
    m = AddrMan(one)
    for i in range(1, 200):
        m.add([(v4(f'250.{i}.1.1', 8333), 9, NOW)], v4(f'251.{i}.1.1'), 0, NOW)
    cache = Cache()
    rng = Rng(bytes([9]) * 32)
    p1, p2 = Peer(NOW), Peer(NOW)
    r1 = getaddr_reply(m, p1, True, cache, NOW * 10**6, rng)
    out(f'  inbound: {len(r1)} addresses, cache until +{(cache.expires - NOW * 10**6) // 10**6} s')
    out(f'  again on that connection: {getaddr_reply(m, p1, True, cache, NOW * 10**6, rng)}')
    out(f'  from an outbound peer: {getaddr_reply(m, Peer(NOW), False, cache, NOW * 10**6, rng)}')
    r2 = getaddr_reply(m, p2, True, cache, (NOW + 3600) * 10**6, rng)
    out(f'  another inbound an hour later: same answer {r2 == r1}')
    r3 = getaddr_reply(m, Peer(NOW), True, cache, cache.expires + 1, rng)
    out(f'  after the cache expires: same answer {[x.key_tuple() for x in r3] == [x.key_tuple() for x in r1]}')

    out('DNS seeds')
    for size, force in [(0, False), (500, False), (5000, False), (5000, True)]:
        n, wait = seed_plan(size, force, 9)
        out(f'  {size} known addresses' + (', -forcednsseed' if force else '') + f': wait {wait} s, then query {n} of 9')
    return '\n'.join(lines) + '\n'


def main():
    want = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'addrman fixture: {want.count(chr(10))} lines agree with Core\'s transliterated addrman.cpp, netgroup.cpp and random.h')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
