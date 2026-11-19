# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
import unittest
from roman import to_roman, from_roman


class T(unittest.TestCase):
    def test_to(self):
        self.assertEqual(to_roman(1994), 'MCMXCIV')
        self.assertEqual(to_roman(4), 'IV')
        self.assertEqual(to_roman(3999), 'MMMCMXCIX')

    def test_round_trip(self):
        for n in range(1, 4000):
            self.assertEqual(from_roman(to_roman(n)), n)


if __name__ == '__main__':
    unittest.main()
