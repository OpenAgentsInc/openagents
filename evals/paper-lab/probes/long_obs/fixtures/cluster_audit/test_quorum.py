import unittest
import quorum


class TestQuorum(unittest.TestCase):
    def test_retry_cap_matches_log(self):
        self.assertEqual(quorum.RETRY_CAP, 19)

    def test_quorum_min_matches_log(self):
        self.assertEqual(quorum.QUORUM_MIN, 3)

    def test_strict_majority_prevents_split_brain(self):
        self.assertFalse(quorum.has_quorum(2, 4))
        self.assertTrue(quorum.has_quorum(3, 4))
        self.assertFalse(quorum.has_quorum(3, 6))
        self.assertTrue(quorum.has_quorum(4, 6))
        self.assertFalse(quorum.has_quorum(2, 3))


if __name__ == "__main__":
    unittest.main()
