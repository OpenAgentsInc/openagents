# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
import unittest
from pkg.core import pipeline


class T(unittest.TestCase):
    def test(self):
        self.assertEqual(pipeline(1), 18)


if __name__ == '__main__':
    unittest.main()
