# Firm reason: 5 (subagent-bench eval fixture/verifier)
import unittest
from lib.http_v1 import LegacyTransport
from clients.ledger.client import LedgerClient


class TestLedgerClient(unittest.TestCase):
    def test_v1_roundtrip(self):
        t = LegacyTransport({
            ("GET", "/v2/ledger/1"): lambda b: {ok: True, "data": {"entry_id": "1", "delta_cents": "x"}},
            ("POST", "/v2/ledger"): lambda b: {ok: True, "data": b},
            ("GET", "/v2/ledger"): lambda b: {ok: True, "data": [{"entry_id": "1"}]},
        })
        c = LedgerClient(t)
        self.assertEqual(c.get_entry("1")["entry_id"], "1")
        self.assertIsNone(c.get_entry("missing"))
        self.assertEqual(c.record_entry({"delta_cents": "y"})["delta_cents"], "y")
        self.assertEqual(len(c.list_entries()), 1)


if __name__ == "__main__":
    unittest.main()
