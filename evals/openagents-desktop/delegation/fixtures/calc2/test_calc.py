# Firm reason: 5 (delegation eval fixture: a tiny project the live agent edits in a temp repo)
import unittest
from calc import evaluate


class T(unittest.TestCase):
    def test_add(self):
        self.assertEqual(evaluate("1 + 2"), 3)

    def test_precedence(self):
        self.assertEqual(evaluate("2 + 3 * 4"), 14)

    def test_parens(self):
        self.assertEqual(evaluate("(2 + 3) * 4"), 20)


if __name__ == "__main__":
    unittest.main()
