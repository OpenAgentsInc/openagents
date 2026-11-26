# Firm reason: 5 (subagent-bench eval fixture/verifier)
import random
import unittest
from svc_sched.core import merge_intervals


class TestSvcSchedFuzz(unittest.TestCase):
    def test_interval_merge_fuzz(self):
        rng = random.Random(77)
        for _ in range(8500):
            pts = []
            for _ in range(40):
                a = rng.randint(0, 120)
                b = a + rng.randint(0, 15)
                pts.append((a, b))
            merged = merge_intervals(pts)
            # Check exact point-set equivalence on integer domain [0..140]
            ref = set()
            for a, b in pts:
                for x in range(a, b + 1):
                    ref.add(x)
            got = set()
            for i, (a, b) in enumerate(merged):
                self.assertLessEqual(a, b)
                if i > 0:
                    self.assertGreater(a, merged[i - 1][1])
                for x in range(a, b + 1):
                    got.add(x)
            self.assertEqual(got, ref)


if __name__ == "__main__":
    unittest.main()
