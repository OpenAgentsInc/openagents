# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
import unittest
from codec import encode_frame, checksum
from validator import decode_and_verify
from router import roundtrip_batch


class TestPipeline(unittest.TestCase):
    def test_plain_messages(self):
        self.assertEqual(roundtrip_batch(["hello", "world"]), ["hello", "world"])

    def test_colons_and_backslashes(self):
        msgs = ["a:b:c", "path\\to\\file", "edge\\:case:", "trailing\\"]
        self.assertEqual(roundtrip_batch(msgs), msgs)

    def test_wire_contract(self):
        # Canonical wire contract: len == len(escaped), crc == checksum(raw_payload)
        f = encode_frame("x:y")
        self.assertEqual(f, f"4:x\\:y:{checksum('x:y')}")
        self.assertEqual(decode_and_verify(f), "x:y")

    def test_corrupt_crc_rejected(self):
        f = encode_frame("hello")
        bad = f[:-1] + ("0" if f[-1] != "0" else "1")
        with self.assertRaises(ValueError):
            decode_and_verify(bad)


if __name__ == "__main__":
    unittest.main()
