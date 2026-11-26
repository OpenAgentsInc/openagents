# Firm reason: 5 (subagent-bench eval fixture/verifier)
import unittest
from inventory.allocator import InsufficientStockError, WarehouseAllocator


class TestWarehouseAllocator(unittest.TestCase):
    def test_fifo_multi_lot_allocation(self):
        wh = WarehouseAllocator()
        wh.add_lot("widget", "L2", 5, received_at=200)
        wh.add_lot("widget", "L1", 3, received_at=100)
        wh.add_lot("widget", "L3", 10, received_at=300)
        picks = wh.reserve_fifo("R1", "widget", 9, now_s=500, ttl_s=60)
        self.assertEqual(picks, [("L1", 3), ("L2", 5), ("L3", 1)])
        self.assertEqual(wh.available("widget", now_s=510), 9)

    def test_expired_reservation_restored_automatically(self):
        wh = WarehouseAllocator()
        wh.add_lot("bolt", "B1", 10, received_at=10)
        wh.reserve_fifo("R1", "bolt", 8, now_s=100, ttl_s=30)
        self.assertEqual(wh.available("bolt", now_s=129), 2)
        self.assertEqual(wh.available("bolt", now_s=130), 10)
        picks = wh.reserve_fifo("R2", "bolt", 9, now_s=130, ttl_s=60)
        self.assertEqual(picks, [("B1", 9)])

    def test_shortfall_rolls_back_atomically(self):
        wh = WarehouseAllocator()
        wh.add_lot("gear", "G1", 4, received_at=10)
        wh.add_lot("gear", "G2", 3, received_at=20)
        with self.assertRaises(InsufficientStockError):
            wh.reserve_fifo("R_BIG", "gear", 10, now_s=100, ttl_s=60)
        self.assertEqual(wh.available("gear", now_s=101), 7)
        picks = wh.reserve_fifo("R_OK", "gear", 6, now_s=101, ttl_s=60)
        self.assertEqual(picks, [("G1", 4), ("G2", 2)])


if __name__ == "__main__":
    unittest.main()
