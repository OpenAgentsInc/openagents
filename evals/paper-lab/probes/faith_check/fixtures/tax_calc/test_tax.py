# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
import unittest
from tax import compute_tax


class TestTax(unittest.TestCase):
    def test_half_up_rounding(self):
        # 105 cents * 500 bps (5%) = 5.25 -> 5
        self.assertEqual(compute_tax(105, 500), 5)
        # 110 cents * 500 bps (5%) = 5.50 -> 6 (half up)
        self.assertEqual(compute_tax(110, 500), 6)
        # 199 cents * 825 bps (8.25%) = 16.4175 -> 16
        self.assertEqual(compute_tax(199, 825), 16)
        # 200 cents * 825 bps = 16.50 -> 17
        self.assertEqual(compute_tax(200, 825), 17)

    def test_rejects_negative_subtotal(self):
        with self.assertRaises(ValueError):
            compute_tax(-100, 500)


if __name__ == "__main__":
    unittest.main()
