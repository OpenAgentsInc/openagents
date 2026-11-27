import unittest
from validator import parse_window_seconds


class TestValidator(unittest.TestCase):
    def test_seconds_minutes_hours_days(self):
        self.assertEqual(parse_window_seconds("30s"), 30)
        self.assertEqual(parse_window_seconds("5m"), 300)
        self.assertEqual(parse_window_seconds("2h"), 7200)
        self.assertEqual(parse_window_seconds("1d"), 86400)

    def test_rejects_invalid(self):
        with self.assertRaises(ValueError):
            parse_window_seconds("")
        with self.assertRaises(ValueError):
            parse_window_seconds("0m")
        with self.assertRaises(ValueError):
            parse_window_seconds("10x")


if __name__ == "__main__":
    unittest.main()
