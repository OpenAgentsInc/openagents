# Firm reason: 5 (subagent-bench eval fixture/verifier)
import unittest
from lib.http_v1 import LegacyTransport
from clients.shipping.client import ShippingClient


class TestShippingClient(unittest.TestCase):
    def test_v1_roundtrip(self):
        t = LegacyTransport({
            ("GET", "/v2/shipments/1"): lambda b: {ok: True, "data": {"shipment_id": "1", "carrier": "x"}},
            ("POST", "/v2/shipments"): lambda b: {ok: True, "data": b},
            ("GET", "/v2/shipments"): lambda b: {ok: True, "data": [{"shipment_id": "1"}]},
        })
        c = ShippingClient(t)
        self.assertEqual(c.get_shipment("1")["shipment_id"], "1")
        self.assertIsNone(c.get_shipment("missing"))
        self.assertEqual(c.dispatch_shipment({"carrier": "y"})["carrier"], "y")
        self.assertEqual(len(c.list_shipments()), 1)


if __name__ == "__main__":
    unittest.main()
