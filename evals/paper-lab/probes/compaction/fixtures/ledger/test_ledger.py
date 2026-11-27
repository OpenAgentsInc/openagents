import unittest
from ledger import apply_fee, reconcile_batch


class TestLedger(unittest.TestCase):
    def test_apply_fee(self):
        self.assertEqual(apply_fee(10000, 250), 9750)
        self.assertEqual(apply_fee(500, 100), 495)

    def test_reconcile_batch(self):
        self.assertEqual(reconcile_batch([100, -40, None, 25]), 85)
        self.assertEqual(reconcile_batch([-10, -20]), -30)


if __name__ == "__main__":
    unittest.main()
