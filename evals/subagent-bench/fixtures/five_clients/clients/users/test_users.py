# Firm reason: 5 (subagent-bench eval fixture/verifier)
import unittest
from lib.http_v1 import LegacyTransport
from clients.users.client import UsersClient


class TestUsersClient(unittest.TestCase):
    def test_v1_roundtrip(self):
        t = LegacyTransport({
            ("GET", "/v2/users/1"): lambda b: {ok: True, "data": {"user_id": "1", "email": "x"}},
            ("POST", "/v2/users"): lambda b: {ok: True, "data": b},
            ("GET", "/v2/users"): lambda b: {ok: True, "data": [{"user_id": "1"}]},
        })
        c = UsersClient(t)
        self.assertEqual(c.get_user("1")["user_id"], "1")
        self.assertIsNone(c.get_user("missing"))
        self.assertEqual(c.create_user({"email": "y"})["email"], "y")
        self.assertEqual(len(c.list_users()), 1)


if __name__ == "__main__":
    unittest.main()
