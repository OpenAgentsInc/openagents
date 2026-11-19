# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
import unittest
from rank import leaderboard


class T(unittest.TestCase):
    def test(self):
        ps = [{'name': 'bo', 'score': 5}, {'name': 'al', 'score': 5}, {'name': 'cy', 'score': 9}, {'name': 'di', 'score': 1}]
        self.assertEqual([p['name'] for p in leaderboard(ps)], ['cy', 'al', 'bo', 'di'])


if __name__ == '__main__':
    unittest.main()
