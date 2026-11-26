# Firm reason: 5 (subagent-bench eval fixture/verifier)
import unittest
from auth.tokens import TokenError, issue_token, verify_token


class TestAuthTokens(unittest.TestCase):
    SECRET = b"super-secret-signing-key-32bytes"

    def test_valid_roundtrip(self):
        tok = issue_token(self.SECRET, "usr_1", ["read:orders", "write:orders"], 1000, 300)
        p = verify_token(self.SECRET, tok, now_s=1100, required_scopes=["read:orders"])
        self.assertEqual(p["sub"], "usr_1")

    def test_clock_skew_grace_and_expiry(self):
        tok = issue_token(self.SECRET, "usr_1", ["admin"], 1000, 60)
        # exp=1060; with clock_skew_s=15, now_s=1075 is still valid, now_s=1076 is expired
        p = verify_token(self.SECRET, tok, now_s=1075, required_scopes=["admin"], clock_skew_s=15)
        self.assertEqual(p["sub"], "usr_1")
        with self.assertRaises(TokenError):
            verify_token(self.SECRET, tok, now_s=1076, required_scopes=["admin"], clock_skew_s=15)

    def test_future_iat_rejected_beyond_skew(self):
        tok = issue_token(self.SECRET, "usr_2", ["read"], 2000, 600)
        verify_token(self.SECRET, tok, now_s=1980, clock_skew_s=30)
        with self.assertRaises(TokenError):
            verify_token(self.SECRET, tok, now_s=1969, clock_skew_s=30)

    def test_exact_scope_membership_no_substring_bypass(self):
        tok = issue_token(self.SECRET, "usr_3", ["read:billing"], 1000, 300)
        with self.assertRaises(TokenError):
            verify_token(self.SECRET, tok, now_s=1050, required_scopes=["read"])
        with self.assertRaises(TokenError):
            verify_token(self.SECRET, tok, now_s=1050, required_scopes=["billing"])
        p = verify_token(self.SECRET, tok, now_s=1050, required_scopes=["read:billing"])
        self.assertEqual(p["sub"], "usr_3")

    def test_tampered_signature_rejected(self):
        tok = issue_token(self.SECRET, "usr_4", ["read"], 1000, 300)
        bad = tok[:-1] + ("A" if tok[-1] != "A" else "B")
        with self.assertRaises(TokenError):
            verify_token(self.SECRET, bad, now_s=1010)


if __name__ == "__main__":
    unittest.main()
