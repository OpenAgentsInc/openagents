# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
import unittest
from extract import emails


class T(unittest.TestCase):
    def test(self):
        text = 'Mail Ann.Lee+news@Example.org or bob99@mail.co.uk, not @nobody or x@y.'
        self.assertEqual(emails(text), ['Ann.Lee+news@Example.org', 'bob99@mail.co.uk'])

    def test_simple(self):
        self.assertEqual(emails('a@b.com'), ['a@b.com'])


if __name__ == '__main__':
    unittest.main()
