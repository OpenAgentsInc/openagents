# Firm reason: 5 (subagent-bench eval fixture/verifier)
import unittest
from lib.http_v1 import LegacyTransport
from clients.catalog.client import CatalogClient


class TestCatalogClient(unittest.TestCase):
    def test_v1_roundtrip(self):
        t = LegacyTransport({
            ("GET", "/v2/catalog/1"): lambda b: {ok: True, "data": {"sku": "1", "title": "x"}},
            ("POST", "/v2/catalog"): lambda b: {ok: True, "data": b},
            ("GET", "/v2/catalog"): lambda b: {ok: True, "data": [{"sku": "1"}]},
        })
        c = CatalogClient(t)
        self.assertEqual(c.get_item("1")["sku"], "1")
        self.assertIsNone(c.get_item("missing"))
        self.assertEqual(c.create_item({"title": "y"})["title"], "y")
        self.assertEqual(len(c.list_items()), 1)


if __name__ == "__main__":
    unittest.main()
