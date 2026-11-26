# Firm reason: 5 (subagent-bench eval fixture/verifier)
import random
import unittest
from svc_graph.core import shortest_path


class TestSvcGraphFuzz(unittest.TestCase):
    def test_dijkstra_fuzz_against_bellman_ford(self):
        rng = random.Random(101)
        for trial in range(1800):
            n = 18
            adj = {i: [] for i in range(n)}
            edges = []
            for u in range(n):
                for _ in range(4):
                    v = rng.randrange(n)
                    w = rng.randint(1, 25)
                    adj[u].append((v, w))
                    edges.append((u, v, w))
            # Reference Bellman-Ford from 0
            bf = [10**9] * n
            bf[0] = 0
            for _ in range(n - 1):
                for u, v, w in edges:
                    if bf[u] + w < bf[v]:
                        bf[v] = bf[u] + w
            dst = n - 1
            if bf[dst] >= 10**9:
                with self.assertRaises(ValueError):
                    shortest_path(adj, 0, dst)
            else:
                d, path = shortest_path(adj, 0, dst)
                self.assertEqual(d, bf[dst])
                self.assertEqual(path[0], 0)
                self.assertEqual(path[-1], dst)


if __name__ == "__main__":
    unittest.main()
