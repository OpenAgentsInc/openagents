# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
import unittest
from parser import parse_pairs


class T(unittest.TestCase):
    def test_noise(self):
        for i in range(4000):
            print(f'trace line {i}: warming the cache with a long and very repetitive diagnostic message')

    def test_parse(self):
        self.assertEqual(parse_pairs('a=1;b=2'), {'a': 1, 'b': 2})
        self.assertEqual(parse_pairs(''), {})


if __name__ == '__main__':
    unittest.main()
