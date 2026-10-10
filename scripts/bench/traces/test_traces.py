"""Tests for traces.py (#11218): a replay admits an honest trace and
rejects a tampered one, naming the field that diverged.

    python3 -m unittest scripts/bench/traces/test_traces.py
"""

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import traces  # noqa: E402

# A check the replay runs from a clean checkout: greet.txt must say hello.
CHECK = [sys.executable, "-c", "import sys; sys.exit(0 if 'hello' in open('greet.txt').read() else 1)"]


def sh(cwd, *args):
    return subprocess.run(args, cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


class Fixture:
    def __init__(self, tmp: Path):
        self.repo = tmp / "repo"
        self.repo.mkdir()
        sh(self.repo, "git", "init", "-q")
        sh(self.repo, "git", "config", "user.email", "t@example.com")
        sh(self.repo, "git", "config", "user.name", "t")
        (self.repo / "greet.txt").write_text("bye\n")
        (self.repo / "other.txt").write_text("x\n")
        sh(self.repo, "git", "add", "-A")
        sh(self.repo, "git", "commit", "-q", "-m", "base")
        self.base = sh(self.repo, "git", "rev-parse", "HEAD")
        (self.repo / "greet.txt").write_text("hello\n")
        self.patch = subprocess.run(["git", "diff", "--binary"], cwd=self.repo, capture_output=True).stdout
        sh(self.repo, "git", "checkout", "-q", "--", ".")
        self.store = tmp / "store"
        self.runs = tmp / "runs"

    def issue_run(self, name="11218-1", patch=None, checks_ok=True, claimed=None, with_patch=True):
        folder = self.runs / name
        folder.mkdir(parents=True)
        if with_patch:
            (folder / "change.patch").write_bytes(self.patch if patch is None else patch)
        (folder / "briefing.md").write_text("# briefing\n")
        (folder / "summary.json").write_text(json.dumps({
            "issue": 11218, "base": self.base, "briefed": ["greet.txt"],
            "opened_outside_briefing": ["other.txt"],
            "checks": [{"id": "check:greet", "ok": checks_ok}],
            "check_commands": [{"id": "check:greet", "argv": CHECK}],
            "changed": claimed if claimed is not None else ["greet.txt"],
        }))
        return folder

    def capture(self):
        traces.cmd_capture(_ns(store=self.store, repo=self.repo, ab=[], tasks="", issue_runs=[str(self.runs)]))
        return {t["id"]: t for t in traces.read_jsonl(self.store / "traces.jsonl")}

    def replay(self, t):
        return traces.replay(t, self.store, self.repo, "mac", 60)


def _ns(**kw):
    return type("NS", (), kw)()


class ReplayTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.f = Fixture(Path(self.tmp.name))

    def tearDown(self):
        self.tmp.cleanup()

    def test_an_honest_trace_is_verified_and_admitted(self):
        self.f.issue_run()
        t = self.f.capture()["issue-run:11218-1"]
        self.assertEqual(t["files_changed"], ["greet.txt"])
        rec = self.f.replay(t)
        self.assertEqual(rec["verdict"], "verified")
        self.assertEqual(rec["class"], "exact_replay")
        with open(self.f.store / "replays.jsonl", "w") as out:
            out.write(json.dumps(rec) + "\n")
        traces.cmd_admit(_ns(store=self.f.store))
        rows = traces.read_jsonl(self.f.store / "admitted.jsonl")
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["trace"]["evidence_class"], "exact_replay")
        labels = {i["id"].split("#")[1]: i["label"] for i in rows[0]["items"]}
        self.assertEqual(labels, {"outcome": "accepted", "greet.txt": "changed"})
        for item in rows[0]["items"]:
            self.assertEqual(item["label_source"], "measurement")
            self.assertTrue(item["provenance"]["permission"])

    def test_a_tampered_diff_is_rejected_on_its_digest(self):
        self.f.issue_run()
        t = self.f.capture()["issue-run:11218-1"]
        blob = self.f.store / "blobs" / t["diff_digest"].split(":")[1]
        blob.write_bytes(blob.read_bytes().replace(b"+hello", b"+hellO"))
        rec = self.f.replay(t)
        self.assertEqual(rec["verdict"], "rejected")
        self.assertEqual(rec["verification"], "failed")
        self.assertEqual(rec["divergent"]["field"], "diff_digest")
        self.assertEqual(rec["divergent"]["expected"], t["diff_digest"])
        self.assertNotIn("class", rec)

    def test_a_tampered_diff_with_a_forged_digest_is_rejected_on_the_tree(self):
        self.f.issue_run()
        t = self.f.capture()["issue-run:11218-1"]
        forged = self.f.patch.replace(b"+hello", b"+hello world")
        t = dict(t, diff_digest=traces.put_blob(self.f.store, forged))
        rec = self.f.replay(t)
        self.assertEqual(rec["verdict"], "rejected")
        self.assertEqual(rec["divergent"]["field"], "result_tree")

    def test_a_recorded_check_the_diff_does_not_pass_is_rejected(self):
        # The run says check:greet passed, but its diff does not make it pass.
        broken = self.f.patch.replace(b"+hello", b"+howdy")
        self.f.issue_run(patch=broken, checks_ok=True)
        t = self.f.capture()["issue-run:11218-1"]
        rec = self.f.replay(t)
        self.assertEqual(rec["verdict"], "rejected")
        self.assertEqual(rec["divergent"]["field"], "checks.check:greet")

    def test_labels_come_from_the_diff_not_the_summary(self):
        # The summary claims a file the diff does not touch (the bug #11218 names).
        self.f.issue_run(claimed=["greet.txt", "src/never_written.rs"])
        t = self.f.capture()["issue-run:11218-1"]
        self.assertEqual(t["files_changed"], ["greet.txt"])
        rec = self.f.replay(t)
        self.assertEqual(rec["verdict"], "verified")
        self.assertEqual(rec["claim_mismatch"]["claimed"], ["greet.txt", "src/never_written.rs"])
        items = traces.corpus_items(t, rec)
        self.assertNotIn("issue-run:11218-1#src/never_written.rs", {i["id"] for i in items})

    def test_a_run_without_its_diff_is_unverifiable(self):
        self.f.issue_run(with_patch=False)
        t = self.f.capture()["issue-run:11218-1"]
        rec = self.f.replay(t)
        self.assertEqual(rec["verdict"], "unverifiable")

    def test_recapture_never_rewrites_a_recorded_trace(self):
        folder = self.f.issue_run()
        first = self.f.capture()["issue-run:11218-1"]
        (folder / "summary.json").write_text(
            (folder / "summary.json").read_text().replace('"ok": true', '"ok": false'))
        again = self.f.capture()["issue-run:11218-1"]
        self.assertEqual(first["checks_digest"], again["checks_digest"])


if __name__ == "__main__":
    unittest.main()
