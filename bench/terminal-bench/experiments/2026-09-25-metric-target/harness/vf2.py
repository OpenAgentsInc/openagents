"""Harness for a speedup target over NetworkX's VF2++, written from the
stated definition only: time `vf2pp_is_isomorphic` on fixed-seed random
5-regular graph pairs with 300 nodes, on one side per run.

Usage: python3 vf2.py candidate|reference. Prints `METRIC <seconds per
call>`; the host divides the reference's by the candidate's.
"""

import random
import sys
import time

import networkx as nx

side = sys.argv[1]
pairs = []
for seed in (1, 2, 3):
    g1 = nx.random_regular_graph(5, 300, seed=seed)
    perm = list(range(300))
    random.Random(seed).shuffle(perm)
    g2 = nx.relabel_nodes(g1, dict(zip(range(300), perm)))
    pairs.append((g1, g2))
if side == "candidate":
    sys.path.insert(0, "/app")
    import fast_networkx as fnx

    def convert(g):
        h = fnx.Graph()
        h.add_nodes_from(g.nodes())
        h.add_edges_from(g.edges())
        return h

    pairs = [(convert(a), convert(b)) for a, b in pairs]
    check = fnx.vf2pp_is_isomorphic
    reps = 200
else:
    check = nx.vf2pp_is_isomorphic
    reps = 1
check(*pairs[0])
start = time.perf_counter()
for _ in range(reps):
    for a, b in pairs:
        assert check(a, b)
print("phase calls:", reps * len(pairs))
print("METRIC", (time.perf_counter() - start) / (reps * len(pairs)))
