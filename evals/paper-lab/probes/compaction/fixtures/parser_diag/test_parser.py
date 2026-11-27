import unittest
from parser import parse_kv


class TestParser(unittest.TestCase):
    def test_basic(self):
        self.assertEqual(parse_kv("a=1\nb=2\n"), {"a": "1", "b": "2"})

    def test_equals_in_value(self):
        self.assertEqual(parse_kv("token=abc==\nurl=https://x?a=1&b=2\n"), {"token": "abc==", "url": "https://x?a=1&b=2"})


if __name__ == "__main__":
    unittest.main()
