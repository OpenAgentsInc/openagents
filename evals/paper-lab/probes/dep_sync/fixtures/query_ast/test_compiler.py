# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
import unittest
from nodes import AggNode
from compiler import compile_agg


class TestCompiler(unittest.TestCase):
    def test_basic_agg(self):
        n = AggNode(func="first_value", expr="amount")
        self.assertEqual(compile_agg(n), "first_value(amount)")


if __name__ == "__main__":
    unittest.main()
