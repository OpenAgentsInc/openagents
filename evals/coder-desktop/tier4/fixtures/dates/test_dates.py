# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
import unittest
from datetime import datetime, timezone
from dates import parse_ts


class T(unittest.TestCase):
    def test_z(self):
        self.assertEqual(parse_ts('2026-03-01T12:00:00Z'), datetime(2026, 3, 1, 12, tzinfo=timezone.utc))

    def test_offset(self):
        self.assertEqual(parse_ts('2026-03-01T14:30:00+02:00'), datetime(2026, 3, 1, 12, 30, tzinfo=timezone.utc))
        self.assertEqual(parse_ts('2026-03-01T14:30:00+02:00').tzinfo, timezone.utc)


if __name__ == '__main__':
    unittest.main()
