# Firm reason: 5 (Tier 2 eval fixture: a tiny Python project the live agent edits in a temp repo)
import unittest

from mod_b import reverse_words


class B(unittest.TestCase):
    def test_reverse(self):
        self.assertEqual(reverse_words("one two  three"), "three two one")


if __name__ == "__main__":
    unittest.main()
