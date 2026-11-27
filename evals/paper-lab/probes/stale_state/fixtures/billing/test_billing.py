# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
import unittest
import billing
import invoice


class TestBilling(unittest.TestCase):
    def test_total(self):
        self.assertEqual(invoice.compute_total(100.0), round(100.0 * (1.0 + billing.TAX_RATE), 2))

    def test_label_matches_rate(self):
        expected = f"tax={int(round(billing.TAX_RATE * 100))}%"
        self.assertEqual(invoice.tax_label(), expected)


if __name__ == "__main__":
    unittest.main()
