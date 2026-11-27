# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
import unittest
import normalizer


class TestNormalizer(unittest.TestCase):
    def test_max_items(self):
        self.assertEqual(normalizer.MAX_ITEMS, 64)

    def test_clamp_batch(self):
        data = [-5, 0, 3, -1, 7]
        self.assertEqual(normalizer.clamp_batch(data), [0, 0, 3, 0, 7])
        big = list(range(100))
        out = normalizer.clamp_batch(big)
        self.assertEqual(len(out), 64)
        self.assertEqual(out[0], 0)
        self.assertEqual(out[-1], 63)


if __name__ == "__main__":
    unittest.main()
