# Firm reason: 5 (Tier 2 eval fixture: a tiny Python project the live agent edits in a temp repo)
import unittest

import lib


class LibTest(unittest.TestCase):
    def test_some(self):
        self.assertEqual(lib.add(2, 3), 5)
        self.assertEqual(lib.diff(2, 7), 5)


if __name__ == "__main__":
    unittest.main()
