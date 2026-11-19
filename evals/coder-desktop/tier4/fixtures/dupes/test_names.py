# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
import unittest
from users import username
from tags import tag


class T(unittest.TestCase):
    def test(self):
        self.assertEqual(username('  Ada  Lovelace '), 'ada_lovelace')
        self.assertEqual(tag('Big  News'), '#big-news')


if __name__ == '__main__':
    unittest.main()
