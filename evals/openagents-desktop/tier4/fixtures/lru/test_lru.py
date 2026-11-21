# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
import unittest
from lru import LRUCache


class T(unittest.TestCase):
    def test_evicts_lru(self):
        c = LRUCache(2)
        c.put('a', 1); c.put('b', 2)
        self.assertEqual(c.get('a'), 1)
        c.put('c', 3)
        self.assertIsNone(c.get('b'))
        self.assertEqual(c.get('a'), 1)
        self.assertEqual(c.get('c'), 3)
        self.assertEqual(len(c), 2)

    def test_update(self):
        c = LRUCache(1)
        c.put('a', 1); c.put('a', 5)
        self.assertEqual(c.get('a'), 5)
        self.assertEqual(len(c), 1)


if __name__ == '__main__':
    unittest.main()
