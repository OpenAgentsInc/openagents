#!/usr/bin/env python3
"""Offline publication provenance regressions; no credentials or GitBook access."""
import contextlib
import copy
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock


SPEC = importlib.util.spec_from_file_location(
    "book_publisher", Path(__file__).with_name("publish-coax-book.py"))
publisher = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publisher)


class Response:
    def __init__(self, value):
        self.raw = json.dumps(value).encode()

    def __enter__(self):
        return self

    def __exit__(self, *args):
        return False

    def read(self):
        return self.raw


class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.book = self.root / "coax/book"
        self.book.mkdir(parents=True)
        (self.book / "SUMMARY.md").write_text("# Summary\n\n- [Welcome](README.md)\n")
        (self.book / "README.md").write_text("# Welcome\n\nExact source body.\n")
        subprocess.run(["git", "init", "-q", str(self.root)], check=True)
        self.repo_git("add", ".")
        self.repo_git("-c", "user.name=Publication Test", "-c", "user.email=test@example.invalid",
                      "commit", "-qm", "source fixture")
        self.commit = self.repo_git("rev-parse", "HEAD").strip()
        self.live = {"id": "published-revision", "pages": [
            {"id": "page-1", "title": "Welcome", "slug": "readme", "path": "welcome"}]}
        self.receipt = self.root / "receipt.json"
        self.addCleanup(mock.patch.stopall)
        mock.patch.object(publisher, "ROOT", self.root).start()
        # Every test fails on accidental authentication or network access.
        self.token = mock.patch.object(publisher, "token_from_file",
                                       side_effect=AssertionError("credential read")).start()
        self.network = mock.patch.object(publisher.urllib.request, "build_opener",
                                         side_effect=AssertionError("network")).start()
        self.urlopen = mock.patch.object(publisher.urllib.request, "urlopen",
                                         side_effect=AssertionError("network")).start()

    def repo_git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root, text=True)

    def snapshot(self):
        return publisher.source_snapshot("coax", self.commit)

    def save_receipt(self):
        submitted = [dict(publisher.file_identity("README.md", b"Exact source body.\n"),
                          file="README.md", page_id="page-1", title="Welcome")]
        receipt = publisher.publication_receipt("coax", self.snapshot(), "draft-1", self.live, submitted)
        publisher.write_receipt(self.receipt, receipt)
        return receipt

    def invoke(self, *args):
        with mock.patch("sys.argv", ["publisher", *args]), contextlib.redirect_stdout(io.StringIO()), \
                contextlib.redirect_stderr(io.StringIO()):
            publisher.main()

    def test_snapshot_binds_exact_commit_inventory_and_file_bytes(self):
        snap = self.snapshot()
        self.assertEqual(snap["commit"], self.commit)
        self.assertEqual(snap["pages"][0]["path"], "coax/book/README.md")
        self.assertEqual(snap["pages"][0]["bytes"], len((self.book / "README.md").read_bytes()))
        (self.book / "README.md").write_text("# Welcome\n\nChanged body, same title.\n")
        with self.assertRaisesRegex(SystemExit, "commit|source"):
            self.snapshot()

    def test_snapshot_refuses_untracked_or_escaping_source(self):
        (self.book / "other.md").write_text("untracked")
        (self.book / "SUMMARY.md").write_text("- [Welcome](other.md)\n")
        self.repo_git("add", "coax/book/SUMMARY.md")
        self.repo_git("-c", "user.name=Publication Test", "-c", "user.email=test@example.invalid",
                      "commit", "-qm", "untracked target fixture")
        self.commit = self.repo_git("rev-parse", "HEAD").strip()
        with self.assertRaisesRegex(SystemExit, "missing.*commit"):
            self.snapshot()
        (self.book / "SUMMARY.md").write_text("- [Escape](../../../outside.md)\n")
        self.repo_git("add", "coax/book/SUMMARY.md")
        self.repo_git("-c", "user.name=Publication Test", "-c", "user.email=test@example.invalid",
                      "commit", "-qm", "escaping target fixture")
        self.commit = self.repo_git("rev-parse", "HEAD").strip()
        with self.assertRaisesRegex(SystemExit, "escapes"):
            self.snapshot()

    def test_snapshot_refuses_symlink_source_even_inside_repository(self):
        (self.book / "alias.md").symlink_to("README.md")
        (self.book / "SUMMARY.md").write_text("- [Welcome](alias.md)\n")
        self.repo_git("add", "coax/book")
        self.repo_git("-c", "user.name=Publication Test", "-c", "user.email=test@example.invalid",
                      "commit", "-qm", "symlink target fixture")
        self.commit = self.repo_git("rev-parse", "HEAD").strip()
        with self.assertRaisesRegex(SystemExit, "symlinks"):
            self.snapshot()

    def test_local_receipt_check_needs_no_credentials_or_network(self):
        receipt = self.save_receipt()
        self.assertEqual(receipt["book"], "coax")
        self.assertEqual(receipt["source"]["commit"], self.commit)
        self.assertEqual(receipt["change_request"], "draft-1")
        self.assertEqual(receipt["verified_live"]["revision"], self.live["id"])
        self.assertEqual(receipt["public_url"], publisher.PUBLIC)
        self.assertIn("not", receipt["verification_scope"])
        self.invoke("--check-receipt", "--receipt", str(self.receipt), "--token-file", "forbidden")
        self.token.assert_not_called()
        self.network.assert_not_called()
        self.urlopen.assert_not_called()

    def test_receipt_survives_unrelated_later_commit_but_not_body_drift(self):
        self.save_receipt()
        (self.root / "unrelated").write_text("later metadata")
        self.repo_git("add", "unrelated")
        self.repo_git("-c", "user.name=Publication Test", "-c", "user.email=test@example.invalid",
                      "commit", "-qm", "later unrelated change")
        publisher.check_receipt("coax", self.receipt)
        (self.book / "README.md").write_text("# Welcome\n\nUnpublished body.\n")
        with self.assertRaises(SystemExit):
            publisher.check_receipt("coax", self.receipt)

    def test_receipt_rejects_identity_digest_and_timestamp_tampering(self):
        receipt = self.save_receipt()
        for path, bad in [(('book',), 'nostr'), (('source', 'commit'), 'a' * 40),
                          (('source', 'sha256'), '0' * 64),
                          (('verified_live', 'sha256'), '0' * 64),
                          (('submitted_markdown_sha256',), '0' * 64),
                          (('published_at',), 'yesterday'), (('change_request',), '')]:
            with self.subTest(path=path):
                changed = copy.deepcopy(receipt)
                at = changed
                for key in path[:-1]:
                    at = at[key]
                at[path[-1]] = bad
                self.receipt.write_text(json.dumps(changed))
                with self.assertRaises(SystemExit):
                    publisher.check_receipt("coax", self.receipt)

    def test_receipt_validates_submitted_document_mapping_and_hash_shape(self):
        receipt = self.save_receipt()
        for key, value in [("page_id", "unknown-page"), ("file", "elsewhere.md"),
                           ("sha256", "not-a-digest"), ("bytes", -1)]:
            with self.subTest(key=key):
                changed = copy.deepcopy(receipt)
                changed["submitted_markdown"][0][key] = value
                changed["submitted_markdown_sha256"] = publisher.digest(changed["submitted_markdown"])
                self.receipt.write_text(json.dumps(changed))
                with self.assertRaises(SystemExit):
                    publisher.check_receipt("coax", self.receipt)

    def test_live_receipt_requires_exact_revision_and_page_identity(self):
        self.save_receipt()
        publisher.check_receipt("coax", self.receipt, self.live)
        for key, value in [("id", "another-revision"), ("pages", [
                {"id": "other-page", "title": "Welcome", "path": "welcome"}])]:
            changed = copy.deepcopy(self.live)
            changed[key] = value
            with self.assertRaises(SystemExit):
                publisher.check_receipt("coax", self.receipt, changed)

    def test_strict_live_missing_or_stale_receipt_fails_before_credentials(self):
        with self.assertRaises(SystemExit):
            self.invoke("--check-live", "--require-receipt", "--receipt", str(self.receipt),
                        "--token-file", "forbidden")
        self.save_receipt()
        (self.book / "README.md").write_text("stale")
        with self.assertRaises(SystemExit):
            self.invoke("--check-live", "--require-receipt", "--receipt", str(self.receipt),
                        "--token-file", "forbidden")
        self.token.assert_not_called()
        self.urlopen.assert_not_called()

    def test_check_live_without_receipt_is_explicitly_inventory_only(self):
        self.urlopen.side_effect = None
        self.urlopen.return_value = Response(self.live)
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            publisher.check_live("coax", "stub-token")
        self.assertIn("inventory only", output.getvalue())
        self.assertFalse(self.receipt.exists())

    def test_strict_live_check_is_read_only_and_rejects_new_revision(self):
        self.save_receipt()
        previous = self.receipt.read_bytes()
        self.urlopen.side_effect = None
        self.urlopen.return_value = Response(self.live)
        with mock.patch.dict(publisher.os.environ, {"GITBOOK_TOKEN": "stub-token", "PATH": publisher.os.environ.get("PATH", "")}, clear=True):
            self.invoke("--check-live", "--require-receipt", "--receipt", str(self.receipt))
            changed = copy.deepcopy(self.live)
            changed["id"] = "new-revision-with-identical-titles"
            self.urlopen.return_value = Response(changed)
            with self.assertRaises(SystemExit):
                self.invoke("--check-live", "--require-receipt", "--receipt", str(self.receipt))
        self.assertEqual(self.receipt.read_bytes(), previous)

    def test_atomic_failure_preserves_previous_receipt(self):
        receipt = self.save_receipt()
        previous = self.receipt.read_bytes()
        with mock.patch.object(publisher.os, "replace", side_effect=OSError("disk failure")):
            with self.assertRaises(OSError):
                publisher.write_receipt(self.receipt, receipt)
        self.assertEqual(self.receipt.read_bytes(), previous)
        self.assertEqual(list(self.root.glob(".receipt.json.*")), [])

    def publish_stub(self, *, merge="merge", wrong_inventory=False):
        tree = copy.deepcopy(self.live)
        final = copy.deepcopy(self.live)
        if wrong_inventory:
            final["pages"][0]["title"] = "Unreviewed title"
        base = "/spaces/" + publisher.SPACE
        draft = base + "/change-requests/draft-1"
        requests = []

        def open_request(req, timeout):
            path = req.full_url.removeprefix("https://api.gitbook.com/v1")
            body = json.loads(req.data) if req.data else None
            requests.append((path, body))
            if path == base:
                value = {"title": publisher.BOOKS["coax"][2], "urls": {"published": publisher.PUBLIC}}
            elif path == base + "/change-requests":
                value = {"id": "draft-1"}
            elif path == draft:
                value = {"revision": "draft-revision"}
            elif path == draft + "/merge":
                value = {"result": merge, "revision": self.live["id"]}
            elif path == base + "/content":
                value = final if any(p == draft + "/merge" for p, _ in requests) else tree
            elif path == draft + "/content":
                value = tree
            elif path == draft + "/content?compat=false":
                value = {}
            else:
                raise AssertionError(path)
            return Response(value)

        opener = mock.Mock()
        opener.open.side_effect = open_request
        self.network.side_effect = None
        self.network.return_value = opener
        original_git = publisher.git

        def git(*args):
            if args[:2] == ("remote", "get-url"):
                return publisher.REPOSITORY
            if args[0] == "status":
                return ""
            if args[0] == "ls-remote":
                return self.commit + "\trefs/heads/main"
            return original_git(*args)

        books = dict(publisher.BOOKS)
        row = list(books["coax"])
        row[4] = lambda: None
        books["coax"] = tuple(row)
        with mock.patch.object(publisher, "BOOKS", books), mock.patch.object(publisher, "git", git), \
                mock.patch.dict(publisher.os.environ, {"GITBOOK_TOKEN": "stub-token", "PATH": publisher.os.environ.get("PATH", "")}, clear=True):
            self.invoke("--receipt", str(self.receipt))
        return requests

    def test_success_receipt_written_after_merge_and_verified_live_inventory(self):
        requests = self.publish_stub()
        receipt = publisher.check_receipt("coax", self.receipt, self.live)
        self.assertTrue(any(path.endswith("/merge") for path, _ in requests))
        self.assertEqual(receipt["submitted_markdown"][0]["file"], "README.md")
        self.assertEqual(receipt["submitted_markdown"][0]["bytes"], len(b"Exact source body.\n"))
        self.assertEqual(receipt["verified_live"]["inventory"][0]["id"], "page-1")

    def test_unsuccessful_merge_or_live_mismatch_cannot_replace_receipt(self):
        self.save_receipt()
        previous = self.receipt.read_bytes()
        for options in [{"merge": "conflict"}, {"wrong_inventory": True}]:
            with self.subTest(options=options), self.assertRaises(SystemExit):
                self.publish_stub(**options)
            self.assertEqual(self.receipt.read_bytes(), previous)

    def test_release_gate_still_precedes_credentials_and_network(self):
        failure = subprocess.CalledProcessError(1, ["release-drift"])
        with mock.patch.object(publisher.subprocess, "run", side_effect=failure):
            with self.assertRaises(subprocess.CalledProcessError):
                self.invoke("--token-file", "forbidden")
        self.token.assert_not_called()
        self.network.assert_not_called()


if __name__ == "__main__":
    unittest.main()
