# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
import unittest
from emitter import emit_event


class TestEmitter(unittest.TestCase):
    def test_emit_basic(self):
        ev = emit_event("ping", "ok")
        self.assertEqual(ev.kind, "ping")
        self.assertEqual(ev.payload, "ok")


if __name__ == "__main__":
    unittest.main()
