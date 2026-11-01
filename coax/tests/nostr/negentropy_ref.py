#!/usr/bin/env python3
"""A Python transcription of Negentropy protocol V1 (NIP-77's appendix) and
of the reference implementation's choices (hoytech/negentropy, js/Negentropy.js,
read on 2026-11-01, not run): 16 buckets, an IdList below 32 items, and the
frame limit check `n > limit - 200`. It shares no code with
coax/lib/nostr/nip77.coax. Nostr From Scratch, chapter 43, compares the Coax
code with the values this prints (nostr/examples/checks/negentropy-sync.coax).

Run: python3 -I coax/tests/nostr/negentropy_ref.py
"""
import hashlib

V1 = 0x61
INF = 2**64 - 1
SKIP, FINGERPRINT, IDLIST = 0, 1, 2


def varint(n):
    out = [n & 127]
    n >>= 7
    while n:
        out.append((n & 127) | 128)
        n >>= 7
    return bytes(reversed(out))


def fingerprint(ids):
    total = sum(int.from_bytes(i, "little") for i in ids) % 2**256
    return hashlib.sha256(total.to_bytes(32, "little") + varint(len(ids))).digest()[:16]


class Reader:
    def __init__(self, b):
        self.b, self.at, self.last = b, 0, 0

    def take(self, n):
        if self.at + n > len(self.b):
            raise ValueError("short")
        s = self.b[self.at:self.at + n]
        self.at += n
        return s

    def varint(self):
        v = 0
        while True:
            c = self.take(1)[0]
            v = (v << 7) | (c & 127)
            if c < 128:
                return v

    def bound(self):
        t = self.varint()
        t = INF if t == 0 else t - 1
        if self.last == INF or t == INF:
            self.last = INF
        else:
            t += self.last
            self.last = t
        n = self.varint()
        if n > 32:
            raise ValueError("bound")
        return (t if self.last != INF else INF, self.take(n))


class Side:
    def __init__(self, items, initiator, limit=0):
        self.items = sorted(items)  # (timestamp, id)
        self.initiator, self.limit = initiator, limit

    def bound(self, b, out):
        t, pre = b
        if t == INF:
            out += varint(0)
            self.last = INF
        else:
            out += varint(t - self.last + 1)
            self.last = t
        out += varint(len(pre)) + pre

    def over(self, n):
        return self.limit and n > self.limit - 200

    def lower(self, frm, b):
        t, pre = b
        key = (t, pre + bytes(32 - len(pre)))
        i = frm
        while i < len(self.items) and self.items[i] < key:
            i += 1
        return i

    def split(self, lo, hi, ub, out):
        n = hi - lo
        if n < 32:
            self.bound(ub, out)
            out += varint(IDLIST) + varint(n)
            for t, i in self.items[lo:hi]:
                out += i
            return
        per, extra, cur = n // 16, n % 16, lo
        for k in range(16):
            size = per + (1 if k < extra else 0)
            fp = fingerprint([i for _, i in self.items[cur:cur + size]])
            cur += size
            if cur == hi:
                nb = ub
            else:
                (pt, pi), (ct, ci) = self.items[cur - 1], self.items[cur]
                if pt != ct:
                    nb = (ct, b"")
                else:
                    s = 0
                    while ci[s] == pi[s]:
                        s += 1
                    nb = (ct, ci[:s + 1])
            self.bound(nb, out)
            out += varint(FINGERPRINT) + fp

    def initiate(self):
        self.last = 0
        out = bytearray([V1])
        self.split(0, len(self.items), (INF, b""), out)
        return bytes(out)

    def reconcile(self, q):
        have, need = [], []
        r = Reader(q)
        self.last = 0
        full = bytearray([V1])
        v = r.take(1)[0]
        if v < 0x60 or v > 0x6F:
            raise ValueError("version byte")
        if v != V1:
            if self.initiator:
                raise ValueError("unsupported version")
            return bytes(full), have, need
        prev, pidx, skip, sent = (0, b""), 0, False, 0
        while r.at < len(q):
            o = bytearray()

            def do_skip():
                nonlocal skip
                if skip:
                    skip = False
                    self.bound(prev, o)
                    o.extend(varint(SKIP))

            cur = r.bound()
            mode = r.varint()
            lo, hi = pidx, self.lower(pidx, cur)
            if mode == SKIP:
                skip = True
            elif mode == FINGERPRINT:
                theirs = r.take(16)
                if theirs != fingerprint([i for _, i in self.items[lo:hi]]):
                    do_skip()
                    self.split(lo, hi, cur, o)
                else:
                    skip = True
            elif mode == IDLIST:
                n = r.varint()
                theirs = [r.take(32) for _ in range(n)]
                if self.initiator:
                    skip = True
                    left = dict.fromkeys(theirs)
                    for _, i in self.items[lo:hi]:
                        if i in left:
                            del left[i]
                        else:
                            have.append(i)
                    need.extend(left)
                else:
                    do_skip()
                    ids, k, end = bytearray(), 0, cur
                    for x in range(lo, hi):
                        if self.over(len(full) + len(ids)):
                            end, hi = self.items[x], x
                            break
                        ids += self.items[x][1]
                        k += 1
                    self.bound(end, o)
                    o.extend(varint(IDLIST) + varint(k) + ids)
                    full += o
                    o = bytearray()
                    sent = hi
            else:
                raise ValueError("mode")
            if self.over(len(full) + len(o)):
                # The reference fingerprints items[hi:], but the range
                # this closes starts at the last bound sent (sent).
                fp = fingerprint([i for _, i in self.items[sent:]])
                self.bound((INF, b""), full)
                full += varint(FINGERPRINT) + fp
                break
            if o:
                sent = hi
            full += o
            pidx, prev = hi, cur
        if len(full) == 1 and self.initiator:
            return None, have, need
        return bytes(full), have, need


def sync(a, b, limit=0):
    c, s = Side(a, True, limit), Side(b, False, limit)
    msgs = [c.initiate()]
    have, need = [], []
    while True:
        reply, _, _ = s.reconcile(msgs[-1])
        msgs.append(reply)
        nxt, h, n = c.reconcile(reply)
        have += h
        need += n
        if nxt is None:
            return msgs, have, need
        msgs.append(nxt)


def gen_id(i):
    return hashlib.sha256(b"negentropy %d" % i).digest()


def gen_item(i):
    return (1793491200 - (i * 37) % 500, gen_id(i))


def digest(parts):
    return hashlib.sha256(b"".join(parts)).hexdigest()


def report(name, a, b, limit):
    msgs, have, need = sync(a, b, limit)
    ia, ib = {i for _, i in a}, {i for _, i in b}
    assert set(have) == ia - ib and set(need) == ib - ia
    assert len(have) == len(set(have)) and len(need) == len(set(need))
    print(f"{name}: client {len(a)} server {len(b)} limit {limit}: messages {len(msgs)},"
          f" sizes {[len(m) for m in msgs]}")
    print(f"  have {len(have)} sha256(sorted) {digest(sorted(have))}")
    print(f"  need {len(need)} sha256(sorted) {digest(sorted(need))}")
    print(f"  sha256(messages in order) {digest(msgs)}")
    print(f"  first message sha256 {hashlib.sha256(msgs[0]).hexdigest()}")


def main():
    for n in (0, 1, 127, 128, 300, 16383, 16384, 1793491200, 2**63 - 1):
        print(f"varint({n}) = {varint(n).hex()}")
    a = bytes([0xFF, 0xFF]) + bytes(30)
    b = bytes([0x01]) + bytes(31)
    total = (int.from_bytes(a, "little") + int.from_bytes(b, "little")) % 2**256
    print(f"sum = {total.to_bytes(32, 'little').hex()}")
    print(f"fingerprint(a, b) = {fingerprint([a, b]).hex()}")
    ones = bytes([0xFF] * 32)
    print(f"fingerprint(ff..ff, 01 00..00) = {fingerprint([ones, b]).hex()} (sum wraps to 0)")
    print(f"fingerprint() = {fingerprint([]).hex()}")
    small = [(1000, a), (1000, b), (1005, gen_id(0))]
    print(f"initiate(small) = {Side(small, True).initiate().hex()}")
    # Random sets: the client and server each keep an item when a byte of
    # its id says so, from one universe of 1,200 items.
    uni = [gen_item(i) for i in range(1200)]
    client = [x for x in uni if x[1][0] % 5 != 0]
    server = [x for x in uni if x[1][1] % 5 != 0]
    report("random", client, server, 0)
    report("random limit 4096", client, server, 4096)
    # Nearly equal: 3 only on the client, 4 only on the server.
    near_c = [x for k, x in enumerate(uni) if k not in (5, 600, 1100)]
    near_s = [x for k, x in enumerate(uni) if k not in (7, 8, 333, 1199)]
    report("near", near_c, near_s, 0)
    report("equal", uni, uni, 0)
    report("empty client", [], uni[:100], 0)
    # Smaller cases for nip77.coax's own checks.
    u6 = uni[:600]
    report("near 600", [x for k, x in enumerate(u6) if k not in (5, 300)], [x for k, x in enumerate(u6) if k not in (7, 8, 599)], 0)
    report("cut IdList", [], uni[:300], 4096)
    # The server lacks one item in each of 13 of the 16 first buckets; its
    # answer is cut just after a skipped bucket.
    gone = (27, 48, 59, 65, 157, 201, 222, 320, 364, 412, 423, 483, 543)
    report("cut after a skip", u6, [x for k, x in enumerate(u6) if k not in gone], 4096)
    report("cut fingerprints", [x for x in u6 if x[1][0] % 5 != 0], [x for x in u6 if x[1][1] % 5 != 0], 4096)
    # The example's two relays: the first 1,000 items, kept by the same rule.
    report("relays", [x for x in client if x in uni[:1000]], [x for x in server if x in uni[:1000]], 0)
    report("relays limit 4096", [x for x in client if x in uni[:1000]], [x for x in server if x in uni[:1000]], 4096)


if __name__ == "__main__":
    main()
