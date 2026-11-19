# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
import unittest
from stats import summarize


class T(unittest.TestCase):
    def test_skips_none(self):
        self.assertEqual(summarize([2, None, 4]), {'count': 2, 'mean': 3.0})

    def test_empty(self):
        self.assertEqual(summarize([]), {'count': 0, 'mean': None})
        self.assertEqual(summarize([None]), {'count': 0, 'mean': None})


if __name__ == '__main__':
    unittest.main()
