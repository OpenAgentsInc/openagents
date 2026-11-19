# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
import unittest
from app import double


class T(unittest.TestCase):
    def test(self):
        self.assertEqual(double(4), 8)


if __name__ == '__main__':
    unittest.main()
