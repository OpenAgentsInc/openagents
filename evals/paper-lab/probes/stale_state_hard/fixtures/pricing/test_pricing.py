import unittest
from pricing import discounted_total


class TestPricing(unittest.TestCase):
    def test_discounts(self):
        self.assertEqual(discounted_total(1000, 15), 850)
        self.assertEqual(discounted_total(250, 0), 250)
        self.assertEqual(discounted_total(250, 100), 0)

    def test_invalid_pct(self):
        with self.assertRaises(ValueError):
            discounted_total(100, -5)


if __name__ == "__main__":
    unittest.main()
