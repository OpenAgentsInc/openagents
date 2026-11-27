# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
import unittest
from cache_store import TTLCache


class FakeClock:
    def __init__(self):
        self.t = 0.0

    def __call__(self) -> float:
        return self.t

    def advance(self, dt: float) -> None:
        self.t += dt


class TestTTLCache(unittest.TestCase):
    def test_absolute_ttl_not_extended_by_reads(self):
        clk = FakeClock()
        c = TTLCache(capacity=2, default_ttl=10.0, clock=clk)
        c.put("a", "1")
        clk.advance(6.0)
        self.assertEqual(c.get("a"), "1")
        clk.advance(4.0)  # t == 10.0: must be expired (boundary inclusive)
        self.assertIsNone(c.get("a"))

    def test_lru_eviction_respects_last_used_not_expiry(self):
        clk = FakeClock()
        c = TTLCache(capacity=2, default_ttl=100.0, clock=clk)
        c.put("short_recent", "v1", ttl=20.0)
        clk.advance(1.0)
        c.put("long_old", "v2", ttl=100.0)
        clk.advance(1.0)
        # Touch short_recent so long_old is now LRU even though short_recent expires sooner
        self.assertEqual(c.get("short_recent"), "v1")
        clk.advance(1.0)
        c.put("new", "v3", ttl=50.0)
        self.assertEqual(c.get("short_recent"), "v1")
        self.assertIsNone(c.get("long_old"))
        self.assertEqual(c.get("new"), "v3")


if __name__ == "__main__":
    unittest.main()
