# Firm reason: 5 (Tier 2 eval fixture: a tiny Python project the live agent edits in a temp repo)
import unittest

from calc import clamp, mean


class CalcTest(unittest.TestCase):
    def test_mean(self):
        self.assertEqual(mean([2, 4, 6]), 4)
        self.assertEqual(mean([5]), 5)

    def test_clamp(self):
        self.assertEqual(clamp(5, 0, 3), 3)
        self.assertEqual(clamp(-1, 0, 3), 0)


if __name__ == "__main__":
    unittest.main()
