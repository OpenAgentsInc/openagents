# Firm reason: 5 (delegation eval fixture: a tiny project the live agent edits in a temp repo)
import unittest
from words import split


class T(unittest.TestCase):
    def test_plain(self):
        self.assertEqual(split("ls -la  /tmp"), ["ls", "-la", "/tmp"])

    def test_quotes(self):
        self.assertEqual(split("echo 'a b' \"c d\""), ["echo", "a b", "c d"])


if __name__ == "__main__":
    unittest.main()
