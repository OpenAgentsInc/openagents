#!/usr/bin/env python3
"""Regression checks for reviewed book-claim invalidation (tooling only)."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("book_claims", ROOT / "coax/book/capability-claims.py")
claims = importlib.util.module_from_spec(spec)
spec.loader.exec_module(claims)


class BookClaims(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="coax-book-claims-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        (self.root / "coax/book").mkdir(parents=True)
        self.feature = {"id": "generics.instantiation", "status": "partial", "meaning": "bounded instances"}
        self.catalogue = {"schema_version": 1, "implementation_version": "0.36.0", "features": [self.feature]}
        self.text = "Generic procedure calls work; constraints remain planned."
        self.doc = self.root / "coax/book/ch08.md"
        self.doc.write_text("<!-- capability-claim: generics.current -->\n" + self.text + "\n<!-- /capability-claim -->\n")
        (self.root / "evidence.txt").write_text("retained scoped evidence\n")
        self.ledger = {"schema_version": 1, "catalogue": "coax/release-status.json", "claims": [{
            "id": "generics.current", "document": "coax/book/ch08.md",
            "scope": "Reviewed current generic procedure subset, not general generics.",
            "text_sha256": claims.digest(self.text.encode()),
            "features": [{"id": self.feature["id"], "sha256": claims.feature_digest(self.feature)}],
            "evidence": [{"path": "evidence.txt", "sha256": claims.digest((self.root / "evidence.txt").read_bytes())}],
        }]}
        self.write()

    def write(self):
        (self.root / "coax/release-status.json").write_text(json.dumps(self.catalogue))
        (self.root / "coax/book/capability-claims.json").write_text(json.dumps(self.ledger))

    def errors(self):
        self.write()
        return claims.problems(self.root)

    def test_reviewed_claim_and_unrelated_version_change_remain_valid(self):
        self.assertEqual(self.errors(), [])
        self.catalogue["implementation_version"] = "0.37.0"
        self.catalogue["features"].append({"id": "unrelated", "status": "works"})
        self.assertEqual(self.errors(), [])

    def test_changed_prose_requires_review(self):
        self.doc.write_text(self.doc.read_text().replace("calls work", "calls do not work"))
        self.assertTrue(any("text changed" in x for x in self.errors()))

    def test_capability_scope_change_requires_review_even_when_status_is_same(self):
        self.feature["meaning"] = "constraints now work too"
        self.assertTrue(any("feature changed" in x for x in self.errors()))

    def test_evidence_change_requires_review(self):
        (self.root / "evidence.txt").write_text("new evidence\n")
        self.assertTrue(any("evidence changed" in x for x in self.errors()))

    def test_unknown_feature_and_missing_evidence_refuse(self):
        self.ledger["claims"][0]["features"][0]["id"] = "absent"
        self.ledger["claims"][0]["evidence"][0]["path"] = "missing.txt"
        self.assertGreaterEqual(len(self.errors()), 2)

    def test_missing_duplicate_unknown_and_malformed_markers_refuse(self):
        original = self.doc.read_text()
        for text in ["old prose without a claim\n", original + original,
                     original + original.replace("generics.current", "unregistered"),
                     original.replace("<!-- /capability-claim -->", "")]:
            self.doc.write_text(text)
            self.assertTrue(self.errors(), text)

    def test_duplicate_claim_empty_claim_and_unsafe_paths_refuse(self):
        original = json.loads(json.dumps(self.ledger))
        self.ledger["claims"].append(self.ledger["claims"][0].copy())
        self.assertTrue(self.errors())
        self.ledger = json.loads(json.dumps(original)); self.ledger["claims"] = []
        self.assertTrue(self.errors())
        self.ledger = json.loads(json.dumps(original)); self.ledger["claims"][0]["features"] = []; self.ledger["claims"][0]["evidence"] = []
        self.assertTrue(self.errors())
        for field in ["document", "catalogue"]:
            self.ledger = json.loads(json.dumps(original))
            target = self.ledger["claims"][0] if field == "document" else self.ledger
            target[field] = "../escape"
            self.assertTrue(self.errors())

    def test_malformed_ledger_refuses_without_writes(self):
        p = self.root / "coax/book/capability-claims.json"
        p.write_text("{")
        before = {x: x.read_bytes() for x in self.root.rglob("*") if x.is_file()}
        self.assertTrue(claims.problems(self.root))
        self.assertEqual(before, {x: x.read_bytes() for x in self.root.rglob("*") if x.is_file()})

    def test_boolean_schema_and_unregistered_readme_claim_refuse(self):
        self.ledger["schema_version"] = True
        self.assertTrue(self.errors())
        self.ledger["schema_version"] = 1
        self.catalogue["schema_version"] = True
        self.assertTrue(self.errors())
        self.catalogue["schema_version"] = 1
        (self.root / "psionic").mkdir()
        (self.root / "psionic/README.md").write_text(self.doc.read_text().replace("generics.current", "unregistered"))
        self.assertTrue(self.errors())

    def test_book_symlink_cannot_read_outside_repository(self):
        with tempfile.TemporaryDirectory() as other:
            p = Path(other) / "outside.md"
            p.write_text("outside the book repository\n")
            (self.root / "coax/book/outside.md").symlink_to(p)
            self.assertTrue(self.errors())


if __name__ == "__main__":
    unittest.main()
