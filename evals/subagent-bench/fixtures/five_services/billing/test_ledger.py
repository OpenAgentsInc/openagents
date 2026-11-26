# Firm reason: 5 (subagent-bench eval fixture/verifier)
import unittest
from billing.ledger import InvoiceLedger, LineItem, allocate_discount


class TestBillingLedger(unittest.TestCase):
    def test_discount_largest_remainder_exact_sum(self):
        items = [LineItem("a", 100), LineItem("b", 100), LineItem("c", 100)]
        alloc = allocate_discount(items, 100)
        self.assertEqual(sum(alloc), 100)
        self.assertEqual(alloc, [34, 33, 33])

    def test_discount_weighted_largest_remainder(self):
        # 10/100 * 5 = 0.5, 20/100 * 5 = 1.0, 70/100 * 5 = 3.5 -> tie on 0.5 goes to idx 0
        items = [LineItem("x", 10), LineItem("y", 20), LineItem("z", 70)]
        alloc = allocate_discount(items, 5)
        self.assertEqual(sum(alloc), 5)
        self.assertEqual(alloc, [1, 1, 3])

    def test_discount_capped_at_total(self):
        items = [LineItem("a", 50), LineItem("b", 30)]
        alloc = allocate_discount(items, 500)
        self.assertEqual(alloc, [50, 30])

    def test_invoice_tax_and_cumulative_refund_cap(self):
        items = [LineItem("srv", 1000, taxable=True), LineItem("ship", 250, taxable=False)]
        inv = InvoiceLedger(items, discount_cents=100, tax_bps=825)
        # alloc: 1000/1250*100 = 80, 250/1250*100 = 20 -> taxable net = 920 -> tax = round(920*825/10000) = 76
        self.assertEqual(inv.subtotal_cents, 1150)
        self.assertEqual(inv.tax_cents, 76)
        self.assertEqual(inv.total_cents, 1226)
        self.assertEqual(inv.record_refund(700), 526)
        self.assertEqual(inv.record_refund(500), 26)
        with self.assertRaises(ValueError):
            inv.record_refund(27)
        self.assertEqual(inv.record_refund(26), 0)


if __name__ == "__main__":
    unittest.main()
