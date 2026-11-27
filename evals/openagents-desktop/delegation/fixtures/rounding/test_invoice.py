# Firm reason: 5 (delegation eval fixture: a tiny project the live agent edits in a temp repo)
import unittest
from shop.invoice import invoice_total


class T(unittest.TestCase):
    def test_half_off_item(self):
        # 0.25 at half off is 0.125, which is 0.13; no tax
        self.assertEqual(invoice_total([(0.25, 1, 0.5)], 0.0), 0.13)

    def test_plain(self):
        self.assertEqual(invoice_total([(10.00, 2, 0.0)], 0.10), 22.00)


if __name__ == "__main__":
    unittest.main()
