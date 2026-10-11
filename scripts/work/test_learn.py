"""Tests for the learning loop (#11312).

    python3 -m unittest scripts/work/test_learn.py
"""

from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import learn  # noqa: E402


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True, text=True).stdout.strip()


def make_repo(d: Path) -> tuple[Path, str]:
    repo = d / "repo"
    repo.mkdir()
    git(repo, "init", "-q")
    git(repo, "config", "user.email", "t@example.com")
    git(repo, "config", "user.name", "t")
    for p in ("a.rs", "b.rs", "c.rs", "d.rs", "e.rs"):
        (repo / p).write_text(p)
    git(repo, "add", ".")
    git(repo, "commit", "-qm", "base")
    return repo, git(repo, "rev-parse", "HEAD")


def write_run(root: Path, name: str, result: dict, summary: dict | None = None):
    d = root / name
    d.mkdir(parents=True)
    (d / "result.json").write_text(json.dumps(result))
    if summary is not None:
        (d / "briefed").mkdir()
        (d / "briefed" / "summary.json").write_text(json.dumps(summary))
    return d


def production(issue, base, ok=True, files=("a.rs",), briefing=("a.rs", "b.rs"), misses=None, cost=0.2,
               engine="briefed", escalated=None):
    agent = {"usage": {"cost_usd": cost}, "verify_fails": 0,
             "summary": {"misses": [{"file": m, "tool": "Read"} for m in (misses or [])],
                         "files_read": list(briefing) + list(misses or [])}}
    return {"type": "result", "run_id": f"r{issue}", "repo": "OpenAgentsInc/openagents", "issue": issue,
            "title": f"issue {issue}", "base": base, "requested_engine": "briefed", "engine": engine,
            "escalated": escalated, "ok": ok, "landed": {"how": "pr", "url": "https://github.com/x/y/pull/1"},
            "cost_usd": cost, "secs": 60.0, "diff": {"files": list(files)},
            "briefing": {"files": list(briefing), "confidence": 0.4, "finder": "filefind (#11210)"},
            "attempts": [{"engine": "briefed", "agent": agent, "ok": ok}]}


def case(n, parent, hand, kind="single"):
    return {"issue": n, "commit": parent, "parent": parent, "commits": [parent], "last_commit": parent,
            "kind": kind, "title": f"t{n}", "body": "", "hand": [{"path": p, "existing": True} for p in hand],
            "derived": []}


class CollectTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def test_production_run_reads_the_diff_and_the_agent_ledger(self):
        runs = self.d / "runs"
        write_run(runs, "r1", production(7, "abc", files=("a.rs", "c.rs"), misses=["c.rs"]))
        rows = learn.collect(type("A", (), {"runs": [str(runs)], "ab": [], "ledger_in": []})())
        self.assertEqual(len(rows), 1)
        r = rows[0]
        self.assertEqual((r["kind"], r["issue"], r["accepted"], r["base"]), ("production", 7, True, "abc"))
        self.assertEqual(r["changed"], ["a.rs", "c.rs"])
        self.assertEqual(r["misses"], ["c.rs"])
        self.assertEqual(r["briefing"]["files"], ["a.rs", "b.rs"])

    def test_without_an_agent_ledger_misses_are_changes_outside_the_briefing(self):
        res = production(8, "abc", files=("a.rs", "d.rs"))
        res["attempts"][0]["agent"].pop("summary")
        runs = self.d / "runs"
        write_run(runs, "r2", res)
        r = learn.collect(type("A", (), {"runs": [str(runs)], "ab": [], "ledger_in": []})())[0]
        self.assertEqual(r["misses"], ["d.rs"])

    def test_work_host_layout_and_unsafe_paths(self):
        res = production(9, "abc", files=("a.rs", "../etc/passwd", "/abs"))
        write_run(self.d / "host-runs" / "x", "out", res)
        r = learn.collect(type("A", (), {"runs": [str(self.d / "host-runs")], "ab": [], "ledger_in": []})())[0]
        self.assertEqual(r["changed"], ["a.rs"])

    def test_bench_trials_skip_the_oracle_and_ungraded(self):
        ab = self.d / "ab" / "tag"
        for name, arm, graded in (("1-B0-0", "B0", True), ("1-C-0", "C", True), ("1-A-0", "A", True),
                                  ("1-B0-1", "B0", False)):
            (ab / name).mkdir(parents=True)
            r = {"issue": 1, "arm": arm, "files_changed": ["a.rs"], "misses": [{"file": "b.rs"}],
                 "briefing_files": ["a.rs"], "cost_usd": 0.3, "wall_secs": 50}
            if graded:
                r["tests_pass"] = True
            (ab / name / "result.json").write_text(json.dumps(r))
        rows = learn.collect(type("A", (), {"runs": [], "ab": [str(self.d / "ab")], "ledger_in": []})())
        self.assertEqual(sorted(r["arm"] for r in rows), ["A", "B0"])
        b0 = next(r for r in rows if r["arm"] == "B0")
        self.assertEqual((b0["kind"], b0["engine"], b0["misses"]), ("bench", "briefed", ["b.rs"]))


class ExamplesTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)
        self.repo, self.base = make_repo(self.d)

    def tearDown(self):
        self.tmp.cleanup()

    def dataset(self):
        # newest first; with n_eval=1 issue 50 is the held-out window
        return {"cases": [case(50, self.base, ["a.rs"]), case(40, self.base, ["a.rs"], "multi"),
                          case(30, self.base, ["a.rs"])]}

    def run_row(self, issue, accepted=True, changed=("a.rs",), misses=(), base=None):
        return {"v": learn.RUN_SCHEMA, "source": f"s{issue}{accepted}", "kind": "production",
                "repo": "OpenAgentsInc/openagents", "issue": issue, "base": base or self.base,
                "accepted": accepted, "changed": list(changed), "misses": list(misses), "title": "t"}

    def test_accepted_runs_extend_cases_and_never_the_held_out_window(self):
        ledger = [self.run_row(30, changed=("a.rs", "b.rs"), misses=("c.rs", "nope.rs")),
                  self.run_row(50, changed=("e.rs",)),
                  self.run_row(40, accepted=False, changed=("d.rs",))]
        merged, info = learn.examples(ledger, self.dataset(), str(self.repo), n_eval=1)
        by = {c["issue"]: c for c in merged["cases"]}
        self.assertEqual(by[30]["extra"], {"b.rs": 1.0, "c.rs": 0.5})  # nope.rs isn't in the tree
        self.assertNotIn("extra", by[50])
        self.assertNotIn("extra", by[40])
        self.assertEqual(info["extended_cases"], [30])
        self.assertEqual(info["skipped"]["held-out issue (never trained on)"], 1)
        self.assertEqual(info["skipped"]["not accepted"], 1)

    def test_an_issue_without_a_case_becomes_a_production_case(self):
        ledger = [self.run_row(60, changed=("d.rs",), misses=("e.rs",))]
        merged, info = learn.examples(ledger, self.dataset(), str(self.repo), n_eval=1,
                                      issue_text=lambda n: {"title": "T", "body": "B"})
        c = merged["cases"][-1]
        self.assertEqual((c["issue"], c["kind"], c["parent"], c["commit"]), (60, "production", self.base, self.base))
        self.assertEqual([h["path"] for h in c["hand"]], ["d.rs"])
        self.assertEqual(c["extra"], {"e.rs": 0.5})
        # a production case is never in the held-out window
        self.assertEqual(learn.eval_issues(merged["cases"], 1), {50})

    def test_a_base_from_another_repository_is_skipped(self):
        ledger = [self.run_row(61, base="0" * 40)]
        _, info = learn.examples(ledger, self.dataset(), str(self.repo), n_eval=1, issue_text=lambda n: {"title": "T"})
        self.assertEqual(info["skipped"], {"no base commit here": 1})


class LeverTests(unittest.TestCase):
    def test_report_by_engine_and_size(self):
        rows = [
            {"kind": "production", "engine": "briefed", "accepted": True, "cost_usd": 0.2, "secs": 50,
             "briefing": {"files": ["a", "b"]}, "changed": ["a", "c"], "misses": ["c"]},
            {"kind": "production", "engine": "bare", "accepted": True, "cost_usd": 0.5, "secs": 60,
             "escalated": "repeated verify failures (4)", "briefing": None, "changed": ["a"], "misses": []},
            {"kind": "production", "engine": "briefed", "accepted": False, "cost_usd": None, "secs": 10,
             "briefing": {"files": ["a"] * 9}, "changed": [], "misses": []},
        ]
        rep = learn.levers(rows)["by_kind"]["production"]
        self.assertEqual(rep["briefing_recall_of_accepted_changes"], 0.5)
        self.assertEqual(rep["by_engine"]["bare"]["cost_per_accepted"], 0.5)
        self.assertIsNone(rep["by_engine"]["briefed"]["cost_per_accepted"])  # one cost unknown: never 0
        self.assertEqual(rep["escalations"], {"repeated verify failures": 1})
        self.assertIn("| <=4 | 1 | 1 |", learn.levers_md({"by_kind": {"production": rep}}))


class RecordTests(unittest.TestCase):
    def report(self, promoted=True):
        m = {"baseline": 0.80, "candidate": 0.83, "diff": 0.03, "se": 0.01}
        return {"at": "2026-10-11T15:00:00Z", "promoted": promoted, "spend_usd": 0.4, "spend_cap_usd": 5,
                "repo_rev": "abcdef1234567", "retrain": {"secs": 3600, "exit": 0},
                "ledger": {"runs": 3, "production": 2, "bench": 1},
                "examples": {"extra_labels": 4, "extended_cases": [1, 2], "production_cases": [3]},
                "gate": {"cases": 100, "eval_issues": [10, 99], "metrics": {"recall@50": m},
                         "baseline": {"digest": "sha256:" + "a" * 64, "overlap_with_eval": 5},
                         "candidate": {"digest": "sha256:" + "b" * 64}}}

    def test_promotions_are_added_newest_first(self):
        with tempfile.TemporaryDirectory() as d:
            doc, rep = Path(d) / "p.md", Path(d) / "r.json"
            rep.write_text(json.dumps(self.report()))
            args = type("A", (), {"report": str(rep), "doc": str(doc)})()
            self.assertEqual(learn.cmd_record(args), 0)
            r2 = self.report()
            r2["at"] = "2026-10-18T15:00:00Z"
            rep.write_text(json.dumps(r2))
            learn.cmd_record(args)
            text = doc.read_text()
            self.assertIn("# filefind ranker promotions", text)
            self.assertLess(text.index("2026-10-18"), text.index("2026-10-11"))
            self.assertIn("| recall@50 | 0.8000 | 0.8300 | +0.0300 | 0.0100 |", text)
            rep.write_text(json.dumps(self.report(promoted=False)))
            self.assertEqual(learn.cmd_record(args), 1)
            self.assertEqual(text, doc.read_text())


class CycleTests(unittest.TestCase):
    """The scheduled cycle against a stand-in retrain.sh and the real gate."""

    def setUp(self):
        sys.path.insert(0, str(HERE.parent / "filefind"))
        import ranker_gate
        self.gate = ranker_gate
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)
        self.repo = self.d / "repo"
        (self.repo / "scripts" / "filefind").mkdir(parents=True)
        self.active = self.repo / "scripts" / "filefind" / "model.json"
        self.active.write_text(json.dumps({"name": "active", "trained_on": {"issue_list": [1]}}))
        self.cand = self.d / "cand.json"
        self.cand.write_text(json.dumps({"name": "cand", "trained_on": {"issue_list": [2]}}))

    def tearDown(self):
        self.tmp.cleanup()

    goldens = {"pass": True, "k": 8, "cases": 20, "active": 0.6, "candidate": 0.61, "diff": 0.01, "se": 0.01}

    def run_cycle(self, gain, promote=True):
        base = [{"issue": 100 + i, "n": 10, "r20": 5, "r50": 7, "r100": 9, "brier_top100": 0.05} for i in range(30)]
        cand = [dict(r, r50=r["r50"] + (gain if i % 3 else 0)) for i, r in enumerate(base)]
        rec = self.gate.receipt(str(self.active), str(self.cand), [r["issue"] for r in base], base, cand, [])
        receipt = self.d / "receipt.json"
        receipt.write_text(json.dumps(rec))
        fake = self.d / "retrain.sh"
        fake.write_text(f"echo training\necho 'candidate: {self.cand}'\necho 'receipt:   {receipt}'\nexit 0\n")
        os.environ["OA_LEARN_RETRAIN"] = str(fake)
        try:
            args = type("A", (), {"runs": [], "ab": [], "ledger_in": [], "repo": str(self.repo),
                                  "work": str(self.d / "work"), "promote": promote, "spend_cap": 1.0,
                                  "goldens": lambda *a: dict(self.goldens)})()
            return learn.cycle(args)
        finally:
            os.environ.pop("OA_LEARN_RETRAIN")

    def test_a_failing_receipt_keeps_the_active_model(self):
        before = self.active.read_bytes()
        rep = self.run_cycle(gain=0)
        self.assertFalse(rep["promoted"])
        self.assertTrue(rep["verdict"].startswith("kept the active model"))
        self.assertEqual(before, self.active.read_bytes())
        self.assertIn("training", (self.d / "work" / "retrain.log").read_text())

    def test_a_passing_receipt_promotes_only_with_promote(self):
        rep = self.run_cycle(gain=2, promote=False)
        self.assertEqual(rep["verdict"], "passed (not promoted)")
        rep = self.run_cycle(gain=2)
        self.assertTrue(rep["promoted"], rep.get("promote"))
        self.assertEqual(self.active.read_bytes(), self.cand.read_bytes())
        self.assertTrue((self.repo / "scripts" / "filefind" / "model.receipt.json").exists())
        self.assertIn("promoted", learn.report_md(rep))

    def test_a_golden_regression_blocks_a_passing_receipt(self):
        self.goldens = {"pass": False, "why": "top-8 recall on the goldens fell -0.0500 (> 2.0 SE)"}
        before = self.active.read_bytes()
        rep = self.run_cycle(gain=2)
        self.assertFalse(rep["promoted"])
        self.assertIn("the goldens did not", rep["verdict"])
        self.assertEqual(before, self.active.read_bytes())

    def test_goldens_check(self):
        a = {n: 0.5 for n in range(10)}
        self.assertTrue(learn.goldens_check(a, {n: 0.5 for n in range(10)})["pass"])
        worse = learn.goldens_check(a, {n: 0.5 - (0.2 if n % 2 else 0.1) for n in range(10)})
        self.assertFalse(worse["pass"])
        rows = learn.golden_rows({1: {"hand": ["a", "b"], "feats": {"a": 1, "b": 2, "c": 3}}}, {"stage2": None}, [1, 2],
                                 k=2, rank=lambda m, f: sorted(f.items(), key=lambda kv: -kv[1]))
        self.assertEqual(rows, {1: 0.5})


class SpendCapTests(unittest.TestCase):
    def test_filefind_refuses_past_the_cap(self):
        spec = importlib.util.spec_from_file_location("ff", HERE.parent / "filefind" / "filefind.py")
        ff = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(ff)
        with tempfile.TemporaryDirectory() as d:
            env = {"FILEFIND_SPEND_CAP_USD": "0.05", "FILEFIND_SPEND_LEDGER": os.path.join(d, "s.jsonl")}
            old = {k: os.environ.get(k) for k in env}
            os.environ.update(env)
            try:
                ff.charge_embedding(1_000_000)  # $0.025
                with self.assertRaises(ff.SpendCapReached):
                    ff.charge_embedding(2_000_000)  # would be $0.075
                self.assertAlmostEqual(learn.spend_so_far(Path(d) / "s.jsonl"), 0.025)
            finally:
                for k, v in old.items():
                    if v is None:
                        os.environ.pop(k, None)
                    else:
                        os.environ[k] = v


class TrainingWeightTests(unittest.TestCase):
    def test_extra_positives_carry_their_weight(self):
        spec = importlib.util.spec_from_file_location("ffb", HERE.parent / "bench" / "file-finding-bench.py")
        ffb = importlib.util.module_from_spec(spec)
        sys.path.insert(0, str(HERE.parent / "filefind"))
        spec.loader.exec_module(ffb)
        feats = {1: {"hand": ["a"], "feats": {"a": {"x": 1.0}, "b": {"x": 0.5}, "c": {"x": 0.1}}}}
        X, y, names, w = ffb.rows_of([{"issue": 1, "extra": {"b": 0.5}}], feats, weights=True)
        got = dict(zip(feats[1]["feats"], zip(y.tolist(), w.tolist())))
        self.assertEqual(got, {"a": (1.0, 1.0), "b": (1.0, 0.5), "c": (0.0, 1.0)})


class JobScriptTests(unittest.TestCase):
    def test_numpy_install_refreshes_the_package_lists_first(self):
        # oa-learn-1's first boot failed: a fresh oa-coder-host image has no apt
        # lists, so `apt-get install python3-numpy` cannot find the package.
        for p in (HERE / "learn-job" / "run.sh", HERE / "host-setup.sh"):
            line = next(ln for ln in p.read_text().splitlines() if "python3-numpy" in ln)
            self.assertLess(line.index("apt-get update"), line.index("apt-get install"), p)


class CompareExitTests(unittest.TestCase):
    def test_a_failing_gate_exits_nonzero(self):
        # retrain.sh printed "to activate a passing candidate: ... promote" after a
        # FAIL, because compare exited 0 either way.
        import pickle
        import types
        spec = importlib.util.spec_from_file_location("ffb2", HERE.parent / "bench" / "file-finding-bench.py")
        ffb = importlib.util.module_from_spec(spec)
        sys.path.insert(0, str(HERE.parent / "filefind"))
        spec.loader.exec_module(ffb)
        with tempfile.TemporaryDirectory() as d:
            for name in ("base.json", "cand.json"):
                Path(d, name).write_text(json.dumps({"stage2": {}}))
            Path(d, "features2.pkl").write_bytes(pickle.dumps({}))
            a = types.SimpleNamespace(work=d, eval=1, baseline=f"{d}/base.json", candidate=f"{d}/cand.json")
            ffb.load = lambda a: [{"issue": 7}]
            ffb.split = lambda cases, n: (cases, [])
            ffb.gate_rows = lambda *x: []
            for ok in (True, False):
                ffb.gate = types.SimpleNamespace(overlap=lambda m, i: [], receipt=lambda *x, ok=ok: {
                    "pass": ok, "why": [] if ok else ["recall@50 gain under 2 SE"], "metrics": {},
                    "candidate": {"digest": "sha256:" + "ab" * 32}})
                if ok:
                    ffb.cmd_compare(a)
                else:
                    with self.assertRaises(SystemExit) as e:
                        ffb.cmd_compare(a)
                    self.assertEqual(e.exception.code, 1)


if __name__ == "__main__":
    unittest.main()
