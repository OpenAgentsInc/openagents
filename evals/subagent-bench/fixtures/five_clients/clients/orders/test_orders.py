# Firm reason: 5 (subagent-bench eval fixture/verifier)
import unittest
from lib.http_v1 import LegacyTransport
from clients.orders.client import OrdersClient


class TestOrdersClient(unittest.TestCase):
    def test_v1_roundtrip(self):
        t = LegacyTransport({
            ("GET", "/v2/orders/1"): lambda b: {ok: True, "data": {"order_id": "1", "amount": "x"}},
            ("POST", "/v2/orders"): lambda b: {ok: True, "data": b},
            ("GET", "/v2/orders"): lambda b: {ok: True, "data": [{"order_id": "1"}]},
        })
        c = OrdersClient(t)
        self.assertEqual(c.get_order("1")["order_id"], "1")
        self.assertIsNone(c.get_order("missing"))
        self.assertEqual(c.create_order({"amount": "y"})["amount"], "y")
        self.assertEqual(len(c.list_orders()), 1)


if __name__ == "__main__":
    unittest.main()
