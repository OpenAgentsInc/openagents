"""Harness for a speedup target over NetworkX's VF2++, second version:
time `vf2pp_is_isomorphic` on fixed-seed random 5-regular graph pairs with
300 nodes, one side per run, building fresh graph objects for every call.

The first version (vf2.py) called the candidate 200 times on the same two
graph objects, and a candidate that caches its answers read as a
million-fold speedup. Here each timed call gets graphs it has never seen,
and only the call is timed.

Usage: python3 vf2-fresh.py candidate|reference. Prints `METRIC <seconds
per call>`; the host divides the reference's by the candidate's.
"""

import random
import sys
import time

import networkx as nx

side = sys.argv[1]
SEEDS = (1, 2, 3)
REPS = 5 if side == "candidate" else 1


def pair(seed):
    g1 = nx.random_regular_graph(5, 300, seed=seed)
    perm = list(range(300))
    random.Random(seed).shuffle(perm)
    return g1, nx.relabel_nodes(g1, dict(zip(range(300), perm)))


if side == "candidate":
    sys.path.insert(0, "/app")
    import fast_networkx as fnx

    def convert(g):
        h = fnx.Graph()
        h.add_nodes_from(g.nodes())
        h.add_edges_from(g.edges())
        return h

    def fresh(seed):
        a, b = pair(seed)
        return convert(a), convert(b)

    check = fnx.vf2pp_is_isomorphic
else:
    fresh = pair
    check = nx.vf2pp_is_isomorphic

check(*fresh(99))
total = 0.0
calls = 0
for _ in range(REPS):
    for seed in SEEDS:
        a, b = fresh(seed)
        start = time.perf_counter()
        assert check(a, b)
        total += time.perf_counter() - start
        calls += 1
print("phase calls:", calls)
print("METRIC", total / calls)
