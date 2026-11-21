# Firm reason: 5 (Tier 2 eval fixture: a tiny Python project the live agent edits in a temp repo)
import unittest

from textutil import slugify


class SlugTest(unittest.TestCase):
    def test_simple(self):
        self.assertEqual(slugify("Hello World"), "hello-world")

    def test_punctuation_and_spaces(self):
        self.assertEqual(slugify("  Ship it, now!  "), "ship-it-now")


if __name__ == "__main__":
    unittest.main()
