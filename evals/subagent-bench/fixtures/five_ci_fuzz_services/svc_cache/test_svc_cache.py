# Firm reason: 5 (subagent-bench eval fixture/verifier)
import random
import unittest
from svc_cache.core import TTLLRUCache


class TestSvcCacheFuzz(unittest.TestCase):
    def test_ttl_lru_fuzz_against_reference(self):
        rng = random.Random(2026)
        for _ in range(2500):
            cap = rng.randint(3, 8)
            c = TTLLRUCache(cap)
            # Simple exact reference model: list of [key, val, exp] ordered LRU -> MRU
            ref = []
            now = 0
            for step in range(180):
                now += rng.randint(0, 3)
                k = f"k{rng.randrange(cap * 2)}"
                if rng.random() < 0.55:
                    v = step
                    ttl = rng.randint(2, 10)
                    c.put(k, v, now, ttl)
                    # ref purge expired
                    ref = [e for e in ref if e[2] > now]
                    ref = [e for e in ref if e[0] != k]
                    if len(ref) >= cap:
                        ref.pop(0)
                    ref.append([k, v, now + ttl])
                else:
                    got = c.get(k, now)
                    ref = [e for e in ref if e[2] > now]
                    found = None
                    for idx, e in enumerate(ref):
                        if e[0] == k:
                            found = ref.pop(idx)
                            ref.append(found)
                            break
                    want = found[1] if found else None
                    self.assertEqual(got, want)


if __name__ == "__main__":
    unittest.main()
