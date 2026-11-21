# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
import unittest
from cart import checkout
from report import line


class T(unittest.TestCase):
    def test(self):
        items = [{'price': 3, 'qty': 2}, {'price': 1, 'qty': 4}]
        self.assertEqual(checkout(items, 1), 9)
        self.assertEqual(line(items), 'total=10')


if __name__ == '__main__':
    unittest.main()
