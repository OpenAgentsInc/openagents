# Firm reason: 5 (delegation eval fixture: a tiny project the live agent edits in a temp repo)
import unittest
from datetime import datetime
from cron import next_run


class T(unittest.TestCase):
    def test_every_minute(self):
        self.assertEqual(next_run("* * * * *", datetime(2026, 3, 1, 10, 0, 30)), datetime(2026, 3, 1, 10, 1))

    def test_daily(self):
        self.assertEqual(next_run("30 6 * * *", datetime(2026, 3, 1, 7, 0)), datetime(2026, 3, 2, 6, 30))

    def test_step(self):
        self.assertEqual(next_run("*/15 * * * *", datetime(2026, 3, 1, 10, 16)), datetime(2026, 3, 1, 10, 30))


if __name__ == "__main__":
    unittest.main()
