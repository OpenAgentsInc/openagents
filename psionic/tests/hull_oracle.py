#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 230's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/hull-memory.coax or chapter 224's and 166's
interpreters. It runs the example's two programs on its own machine, whose
memory is read two ways before every step: by a brute-force hard max over all
stores (chapter 225's rule, latest write winning ties) and by a binary search
over the sorted distinct keys. Every read must agree, and it counts both
methods' comparisons and prints the example's lines.
"""
import hashlib
import struct

CODES = ["push", "get", "set", "add", "sub", "mul", "load", "store", "brif", "out", "ret"]


def score(k, q):
    return 2 * k * q - k * k


def scan_read(stores, q):
    """Chapter 225's read: a hard max over every store, the later one on a tie."""
    best = None
    for i, (k, v) in enumerate(stores):
        if best is None or score(k, q) >= score(stores[best][0], q):
            best = i
    if best is None or stores[best][0] != q:
        return None
    return stores[best][1], best


class Hull:
    def __init__(self):
        self.keys, self.latest, self.writes = [], {}, 0

    def insert(self, k, v):
        lo, hi, c = 0, len(self.keys), 0
        while lo < hi:
            mid = (lo + hi) // 2
            c += 1
            if self.keys[mid] < k:
                lo = mid + 1
            else:
                hi = mid
        if lo == len(self.keys) or self.keys[lo] != k:
            self.keys.insert(lo, k)
        self.latest[k] = (v, self.writes)
        self.writes += 1
        return c

    def read(self, q):
        if not self.keys:
            return None, 0
        lo, hi, c = 0, len(self.keys) - 1, 0
        while lo < hi:
            mid = (lo + hi) // 2
            c += 1
            if score(self.keys[mid + 1], q) > score(self.keys[mid], q):
                lo = mid + 1
            else:
                hi = mid
        if self.keys[lo] != q:
            return None, c
        return self.latest[q], c


def run(program):
    stack, local, ip, rows, out = [], [0] * 8, 0, [], []
    stores, hull = [], Hull()
    reads = hull_c = scan_c = 0
    while True:
        cells = []
        for c in range(64):
            got, n = hull.read(c)
            assert got == scan_read(stores, c), (ip, c)
            cells.append(got[0] if got else 0)
            reads += 1
            hull_c += n
            scan_c += max(len(stores) - 1, 0)
        code, arg = program[ip]
        nxt, x, y = ip + 1, 0, 0
        if code == "push":
            x = arg; stack.append(x); y = len(stack)
        elif code == "get":
            x = local[arg]; stack.append(x); y = arg
        elif code == "set":
            x = stack.pop(); local[arg] = x; y = arg
        elif code in ("add", "sub", "mul"):
            b, a = stack.pop(), stack.pop()
            x = {"add": a + b, "sub": a - b, "mul": a * b}[code]; stack.append(x); y = len(stack)
        elif code == "load":
            y = stack.pop(); x = cells[y]; stack.append(x)
        elif code == "store":
            x, y = stack.pop(), stack.pop()
            stores.append((y, x))
            hull_c += hull.insert(y, x)
        elif code == "brif":
            x = stack.pop(); nxt = arg if x != 0 else ip + 1; y = nxt
        elif code == "out":
            x = stack.pop(); out.append(x); y = len(out)
        else:
            nxt = -1
        rows.append((ip, CODES.index(code), x, y))
        if nxt < 0:
            break
        ip = nxt
    digest = hashlib.sha256(b"".join(struct.pack("<4I", *(t & 0xFFFFFFFF for t in r)) for r in rows)).hexdigest()
    return dict(steps=len(rows), out=out, stores=len(stores), reads=reads, scan=scan_c, hull=hull_c, digest=digest)


SQUARES = [("push", 4), ("set", 0), ("get", 0), ("get", 0), ("get", 0), ("mul", 0), ("store", 0),
           ("get", 0), ("push", 1), ("sub", 0), ("set", 0), ("get", 0), ("brif", 2),
           ("push", 4), ("load", 0), ("push", 1), ("load", 0), ("add", 0), ("out", 0), ("ret", 0)]
FILL = [("push", 8), ("set", 1), ("push", 64), ("set", 0),
        ("get", 0), ("push", 1), ("sub", 0), ("set", 0), ("get", 0), ("get", 1), ("store", 0),
        ("get", 0), ("brif", 4), ("get", 1), ("push", 1), ("sub", 0), ("set", 1), ("get", 1), ("brif", 2),
        ("push", 0), ("load", 0), ("out", 0), ("ret", 0)]


def main():
    for name, p in (("squares", SQUARES), ("fill 64 cells 8 times", FILL)):
        r = run(p)
        if name == "squares":
            # Chapter 224's published digest for this program.
            assert r["digest"].startswith("d7be7b683aee9a03"), r["digest"]
        print("%s: %d steps, output %d, the interpreter's digest: true" % (name, r["steps"], r["out"][0]))
        print("  %d stores, %d reads; comparisons: scan %d, hull %d" % (r["stores"], r["reads"], r["scan"], r["hull"]))
    h, stores = Hull(), []
    for i in range(512):
        h.insert(63 - i % 64, i)
        stores.append((63 - i % 64, i))
    got, c = h.read(0)
    assert got == scan_read(stores, 0)
    print("one read after 512 writes to 64 keys: write %d, scan 511 comparisons, hull %d" % (got[1], c))


if __name__ == "__main__":
    main()
