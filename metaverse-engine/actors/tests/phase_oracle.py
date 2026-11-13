#!/usr/bin/env python3
# Firm reason: 2 (independent exhaustive execution-order oracle).
"""Check supplied native observations by enumerating all 24 four-task orders."""
import hashlib
import itertools
from pathlib import Path
import sys


def expected(graph, profile):
    ids = (40, 10, 30, 20)
    phases = (3, 0, 2, 1) if profile == 3 else (0, 0, 0, 0)
    edges = [(i, j) for i in range(4) for j in range(4) if i != j]
    dependencies = {(i, j) for bit, (i, j) in enumerate(edges) if graph & (1 << bit)}
    dependencies |= {(i, j) for i in range(4) for j in range(4) if phases[j] < phases[i]}
    orders = []
    for order in itertools.permutations(range(4)):
        if all(order.index(j) < order.index(i) for i, j in dependencies):
            orders.append(order)
    if not orders:
        return (7, 0, 0, 0, 0, 0, 0)  # cycle, no published plan
    reads = [set() for _ in range(4)]
    writes = [set() for _ in range(4)]
    if profile == 1:
        writes = [{0} for _ in range(4)]
    if profile == 2:
        writes[0], reads[1], writes[2], reads[3] = {0}, {0}, {1}, {1}
    for i, j in itertools.combinations(range(4), 2):
        touches = writes[i] & (reads[j] | writes[j]) or writes[j] & reads[i]
        if touches and len({order.index(i) < order.index(j) for order in orders}) > 1:
            return (8, 0, 0, 0, 0, 0, 0)  # conflict, no published plan
    canonical = min(tuple(ids[i] for i in order) for order in orders)
    return (0, 1, 4, *canonical)


def main():
    data = Path(sys.argv[1]).read_bytes()
    seen = set()
    counts = {"ok": 0, "cycle": 0, "conflict": 0}
    for line in data.splitlines():
        graph, profile, *actual = map(int, line.split())
        assert 0 <= graph < 4096 and 0 <= profile < 4
        assert (graph, profile) not in seen, (graph, profile, "duplicate")
        seen.add((graph, profile))
        assert tuple(actual) == expected(graph, profile), (graph, profile, actual, expected(graph, profile))
        counts[{0: "ok", 7: "cycle", 8: "conflict"}[actual[0]]] += 1
    assert len(seen) == 4096 * 4, ("missing observations", len(seen))
    print(f"PASS: {len(seen)} exact plans/refusals; {counts}; sha256={hashlib.sha256(data).hexdigest()}")


if __name__ == "__main__":
    main()
