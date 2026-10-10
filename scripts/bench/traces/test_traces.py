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

    def issue_run(self, name="11218-1", patch=None, checks_ok=True, claimed=None, with_patch=True, **extra):
        folder = self.runs / name
        folder.mkdir(parents=True)
        if with_patch:
            (folder / "change.patch").write_bytes(self.patch if patch is None else patch)
        (folder / "briefing.md").write_text("# briefing\n")
        summary = {
            "issue": 11218, "base": self.base, "briefed": ["greet.txt"],
            "opened_outside_briefing": ["other.txt"],
            "checks": [{"id": "check:greet", "ok": checks_ok}],
            "check_commands": [{"id": "check:greet", "argv": CHECK}],
            "changed": claimed if claimed is not None else ["greet.txt"],
        }
        summary.update(extra)
        (folder / "summary.json").write_text(json.dumps(summary))
        return folder

    def capture(self):
        traces.cmd_capture(_ns(store=self.store, repo=self.repo, ab=[], tasks="", issue_runs=[str(self.runs)]))
        return {t["id"]: t for t in traces.read_jsonl(self.store / "traces.jsonl")}

    def replay(self, t):
        return traces.replay(t, self.store, self.repo, "mac", 60)


def _ns(**kw):
    return type("NS", (), kw)()


def corpus_map(tmp: Path, roles: dict) -> Path:
    """An issues.tsv in the #11215 corpus's shape."""
    path = tmp / "issues.tsv"
    path.write_text("issue\tpartition\tfix\tparent\tissue_sha256\n" +
                    "".join(f"{n}\t{r}\tf\tp\tsha256:x\n" for n, r in roles.items()))
    return path


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
        traces.cmd_admit(_ns(store=self.f.store,
                             corpus_map=corpus_map(Path(self.tmp.name), {11218: "training"})))
        rows = traces.read_jsonl(self.f.store / "admitted.jsonl")
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["partition"]["role"], "training")
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



class InventoryTests(unittest.TestCase):
    """#11230: every attempt survives export, and missing cost stays unknown."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.f = Fixture(Path(self.tmp.name))

    def tearDown(self):
        self.tmp.cleanup()

    def manifest(self):
        out = Path(self.tmp.name) / "manifest.json"
        traces.cmd_manifest(_ns(store=self.f.store, out=str(out)))
        return json.loads(out.read_text())

    def test_missing_agent_cost_is_unknown_not_zero(self):
        cost = {"denomination": "USD", "components": {
            "agent": {"usd": None, "unknown_reason": "Claude Code reported no cost for the session"},
            "decisions": {"usd": 0.002, "unknown_reason": None}}}
        self.f.issue_run(agent_usd=None, decision_usd=0.002, cost=cost)
        t = self.f.capture()["issue-run:11218-1"]
        self.assertIsNone(t["cost_usd"])
        self.assertFalse(t["cost"]["complete"])
        self.assertIsNone(t["cost"]["total_usd"])
        self.assertEqual(t["cost"]["known_subtotal_usd"], 0.002)
        self.assertIn("no cost", t["cost"]["components"]["agent"]["unknown_reason"])

    def test_a_legacy_summary_without_costs_is_unknown_not_zero(self):
        self.f.issue_run()  # no agent_usd or decision_usd at all
        t = self.f.capture()["issue-run:11218-1"]
        self.assertIsNone(t["cost_usd"])
        self.assertEqual(t["outcome"]["status"], "unknown")
        self.assertIsNone(t["wall_secs"])

    def test_a_fully_priced_run_has_a_total(self):
        self.f.issue_run(agent_usd=1.25, decision_usd=0.5)
        t = self.f.capture()["issue-run:11218-1"]
        self.assertEqual(t["cost_usd"], 1.75)
        self.assertTrue(t["cost"]["complete"])

    def test_failed_setup_cancel_and_retry_survive_export(self):
        # Attempt 1: setup failed, no base, no diff.
        self.f.issue_run(name="11218-1-aa", with_patch=False, base=None, run_id="11218-1-aa",
                         attempt={"index": 1, "prior_runs": []},
                         outcome={"status": "setup_failed", "delivers": False, "reason": "no worktree"})
        # Attempt 2: cancelled after the agent started.
        self.f.issue_run(name="11218-2-bb", run_id="11218-2-bb", agent_usd=None,
                         attempt={"index": 2, "prior_runs": ["11218-1-aa"]},
                         outcome={"status": "cancelled", "delivers": False, "reason": "stopped"})
        # Attempt 3: killed before writing its summary (its process is gone).
        killed = self.f.runs / "11218-3-cc"
        killed.mkdir(parents=True)
        dead = subprocess.Popen([sys.executable, "-c", "pass"])
        dead.wait()
        (killed / "run.json").write_text(json.dumps({
            "run_id": "11218-3-cc", "issue": 11218, "pid": dead.pid,
            "attempt": {"index": 3, "prior_runs": ["11218-1-aa", "11218-2-bb"]}}))
        # A run still working (this process) is not captured yet.
        live = self.f.runs / "11218-4-dd"
        live.mkdir(parents=True)
        (live / "run.json").write_text(json.dumps({"run_id": "11218-4-dd", "issue": 11218, "pid": os.getpid()}))
        got = self.f.capture()
        self.assertEqual(got["issue-run:11218-1-aa"]["outcome"]["status"], "setup_failed")
        self.assertTrue(got["issue-run:11218-1-aa"]["capture_error"])
        self.assertEqual(got["issue-run:11218-2-bb"]["outcome"]["status"], "cancelled")
        self.assertEqual(got["issue-run:11218-3-cc"]["outcome"]["status"], "incomplete")
        self.assertEqual(got["issue-run:11218-3-cc"]["attempt"]["index"], 3)
        self.assertNotIn("issue-run:11218-4-dd", got)
        # Unreplayed and unverifiable attempts are all in the export.
        attempts = {r["trace"]: r for r in self.manifest()["attempts"]}
        self.assertEqual(set(attempts), {"issue-run:11218-1-aa", "issue-run:11218-2-bb", "issue-run:11218-3-cc"})
        self.assertIsNone(attempts["issue-run:11218-2-bb"]["cost_usd"])
        self.assertFalse(attempts["issue-run:11218-2-bb"]["cost_complete"])
        self.assertTrue(attempts["issue-run:11218-2-bb"]["cost_unknown"])
        self.assertIsNone(attempts["issue-run:11218-1-aa"]["replay_verdict"])

    def test_capture_marks_the_folder_with_the_stored_diff(self):
        folder = self.f.issue_run()
        traces.cmd_capture(_ns(store=self.f.store, repo=self.f.repo, ab=[], tasks="", issue_runs=[],
                               issue_run_folders=[str(folder)]))
        mark = json.loads((folder / "trace-captured.json").read_text())
        self.assertEqual(mark["trace"], "issue-run:11218-1")
        self.assertEqual(mark["diff_digest"], traces.sha256(self.f.patch))


class PartitionTests(unittest.TestCase):
    """LEARN-02: an admitted trace takes its partition from the corpus's
    issue-group map. A held-out group never lands in training."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.f = Fixture(Path(self.tmp.name))
        self.f.issue_run()
        self.t = self.f.capture()["issue-run:11218-1"]
        self.rec = self.f.replay(self.t)
        self.assertEqual(self.rec["verdict"], "verified")
        with open(self.f.store / "replays.jsonl", "w") as out:
            out.write(json.dumps(self.rec) + "\n")

    def tearDown(self):
        self.tmp.cleanup()

    def admit(self, roles):
        traces.cmd_admit(_ns(store=self.f.store, corpus_map=corpus_map(Path(self.tmp.name), roles)))
        return traces.read_jsonl(self.f.store / "admitted.jsonl")

    def test_a_held_out_group_never_lands_in_training(self):
        for role in ("calibration", "development", "locked"):
            rows = self.admit({11218: role, 1: "training"})
            self.assertEqual(rows[0]["partition"]["role"], role)
            self.assertTrue(rows[0]["items"])
            for item in rows[0]["items"]:
                self.assertEqual(item["partition"], role)
                self.assertNotEqual(item["partition"], "training")
                self.assertEqual(item["group"], "issue-11218")

    def test_an_issue_outside_the_map_yields_no_items(self):
        rows = self.admit({1: "training"})
        self.assertEqual(len(rows), 1)
        self.assertIsNone(rows[0]["partition"]["role"])
        self.assertEqual(rows[0]["items"], [])

    def test_the_committed_map_keeps_the_41_traces_out_of_training(self):
        # The 41 admitted traces (docs/coder/traces/2026-10-10-manifest.json)
        # are issues #10074, #10228 and #10273: calibration and development.
        pmap = traces.load_partition_map()
        manifest = json.loads((traces.HERE.parents[2] / "docs" / "coder" / "traces" /
                               "2026-10-10-manifest.json").read_text())
        admitted = [r for r in manifest["rows"] if r["verdict"] == "verified"]
        self.assertEqual(len(admitted), 41)
        for r in admitted:
            role = pmap["issues"].get(r["issue"])
            self.assertIn(role, ("calibration", "development"), r["trace"])
            self.assertEqual(r["partition"], role, r["trace"])
            t = dict(self.t, issue=r["issue"])
            for item in traces.corpus_items(t, self.rec, pmap):
                self.assertEqual(item["partition"], role)

    def test_an_unknown_partition_name_is_refused(self):
        with self.assertRaises(ValueError):
            traces.load_partition_map(corpus_map(Path(self.tmp.name), {11218: "train"}))


if __name__ == "__main__":
    unittest.main()
