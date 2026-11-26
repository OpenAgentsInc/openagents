# Firm reason: 5 (subagent-bench eval fixture/verifier)
import random
import unittest
from svc_pack.core import rle_decode, rle_encode


class TestSvcPackFuzz(unittest.TestCase):
    def test_rle_long_runs_and_fuzz(self):
        # Long runs > 255
        big = b"A" * 600 + b"B" * 255 + b"C" * 256
        self.assertEqual(rle_decode(rle_encode(big)), big)
        with self.assertRaises(ValueError):
            rle_decode(b"\x00A")
        rng = random.Random(999)
        for _ in range(18000):
            chunks = [bytes([rng.randrange(8)]) * rng.randint(1, 35) for _ in range(12)]
            raw = b"".join(chunks)
            enc = rle_encode(raw)
            self.assertEqual(rle_decode(enc), raw)


if __name__ == "__main__":
    unittest.main()
