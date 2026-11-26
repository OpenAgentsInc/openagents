# Firm reason: 5 (subagent-bench eval fixture/verifier)
import hashlib
import random
import unittest
from svc_crypto.core import pkcs7_pad, pkcs7_unpad


class TestSvcCryptoFuzz(unittest.TestCase):
    def test_pkcs7_roundtrip_and_fuzz(self):
        # Realistic CPU-bound crypto/fuzz property verification (~6-8s)
        rng = random.Random(42)
        for i in range(45000):
            bs = (i % 16) + 1
            raw = rng.randbytes(i % 48)
            padded = pkcs7_pad(raw, bs)
            self.assertEqual(len(padded) % bs, 0)
            self.assertGreater(len(padded), len(raw))
            self.assertEqual(pkcs7_unpad(padded, bs), raw)
            # Hash mix to emulate real test workload
            _ = hashlib.sha256(padded).digest()
            if i % 100 == 0 and bs > 1:
                bad = bytearray(padded)
                bad[-1] = bs
                if bs >= 2:
                    bad[-2] ^= 0xFF
                    with self.assertRaises(ValueError):
                        pkcs7_unpad(bytes(bad), bs)


if __name__ == "__main__":
    unittest.main()
