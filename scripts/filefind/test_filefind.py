"""Tests for filefind's data boundary (DATA-04) and feedback authority (LEARN-04).

    python3 -m unittest scripts/filefind/test_filefind.py
"""

import json
import os
import pickle
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import filefind as ff  # noqa: E402


def sh(cwd, *args):
    return subprocess.run(args, cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


def make_repo(path: Path, remote: str, seed: str) -> str:
    """A repository named path.name whose fix commit names issue #4242."""
    path.mkdir(parents=True)
    sh(path, "git", "init", "-q")
    sh(path, "git", "config", "user.email", "t@example.com")
    sh(path, "git", "config", "user.name", "t")
    (path / "seed.txt").write_text(seed + "\n")
    sh(path, "git", "add", "-A")
    sh(path, "git", "commit", "-q", "-m", f"start {seed}")
    (path / "src.rs").write_text("fn main() {}\n")
    sh(path, "git", "add", "-A")
    sh(path, "git", "commit", "-q", "-m", "fix the thing (#4242)")
    if remote:
        sh(path, "git", "remote", "add", "origin", remote)
    return sh(path, "git", "rev-parse", "HEAD")


def ns(**kw):
    kw.setdefault("cache", None)
    kw.setdefault("workspace", None)
    kw.setdefault("issue_runs", [])
    kw.setdefault("ab", [])
    kw.setdefault("traces", [])
    return type("NS", (), kw)()


def admitted_row(issue, base, role, files=("src.rs",), ok=True, map_digest="sha256:map"):
    return {"trace": {"id": f"issue-run:{issue}-1", "issue": issue, "base": base, "files_changed": list(files),
                      "opened_outside_briefing": ["seed.txt"],
                      "replay": {"verdict": "verified", "checks": {"check:x": ok}}},
            "partition": {"role": role, "map_digest": map_digest}, "items": []}


class Base(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self._cache_root = ff.CACHE_ROOT
        ff.CACHE_ROOT = str(self.root / "cache")
        self._env = {k: os.environ.pop(k, None) for k in ("FILEFIND_WORKSPACE", "OPENAGENTS_WORKSPACE")}
        # Two repositories with the same basename and the same issue numbers.
        self.a = self.root / "one" / "app"
        self.b = self.root / "two" / "app"
        self.head_a = make_repo(self.a, "git@github.com:acme/app.git", "acme")
        self.head_b = make_repo(self.b, "https://github.com/other/app", "other")

    def tearDown(self):
        ff.CACHE_ROOT = self._cache_root
        for k, v in self._env.items():
            if v is not None:
                os.environ[k] = v
        self.tmp.cleanup()

    def traces_file(self, rows):
        path = self.root / f"admitted-{len(list(self.root.glob('admitted-*')))}.jsonl"
        path.write_text("".join(json.dumps(r) + "\n" for r in rows))
        return str(path)


class IdentityTests(Base):
    def test_same_basename_repositories_never_share_a_cache_or_feedback(self):
        ca, cb = ff.default_cache(str(self.a)), ff.default_cache(str(self.b))
        self.assertNotEqual(ca, cb)
        self.assertTrue(os.path.basename(ca).startswith("app-"))
        ia, ib = ff.read_identity(ca), ff.read_identity(cb)
        self.assertEqual(ia["remote"], "github.com/acme/app")
        self.assertNotEqual(ia["root"], ib["root"])
        # One admitted-traces file holding issue #4242 from both repositories.
        traces = self.traces_file([admitted_row(4242, self.head_a, "training"),
                                   admitted_row(4242, self.head_b, "training", files=("other.rs",))])
        ff.cmd_feedback(ns(repo=str(self.a), traces=[traces]))
        ff.cmd_feedback(ns(repo=str(self.b), traces=[traces]))
        self.assertEqual(dict(ff.load_feedback(ca)), {4242: {"src.rs": 1.0}})
        self.assertEqual(dict(ff.load_feedback(cb)), {4242: {"other.rs": 1.0}})
        # B's feedback file copied into A's cache is not A's: nothing is read.
        Path(ca, "feedback.jsonl").write_text(Path(cb, "feedback.jsonl").read_text())
        self.assertEqual(dict(ff.load_feedback(ca)), {})
        self.assertEqual(ff.feedback_rows(ca), [])

    def test_a_cache_stamped_for_another_repository_is_refused(self):
        ca = ff.default_cache(str(self.a))
        with self.assertRaises(ff.ForeignCache):
            ff.cache_for(ns(repo=str(self.b), cache=ca))
        # A fork: same root commit, another remote, is another identity.
        sh(self.a, "git", "remote", "set-url", "origin", "https://github.com/fork/app.git")
        self.assertNotEqual(ff.default_cache(str(self.a)), ca)

    def test_worktrees_share_one_cache_and_workspaces_do_not(self):
        wt = self.root / "wt"
        sh(self.a, "git", "worktree", "add", "-q", "--detach", str(wt))
        self.assertEqual(ff.default_cache(str(wt)), ff.default_cache(str(self.a)))
        self.assertNotEqual(ff.default_cache(str(self.a), "ws_customer"), ff.default_cache(str(self.a)))
        os.environ["OPENAGENTS_WORKSPACE"] = "ws_customer"
        try:
            self.assertEqual(ff.default_cache(str(self.a)), ff.default_cache(str(self.a), "ws_customer"))
        finally:
            del os.environ["OPENAGENTS_WORKSPACE"]

    def test_remote_spellings_normalize_without_credentials(self):
        for url in ("git@github.com:Acme/App.git", "https://github.com/Acme/App",
                    "https://user:tok@github.com/Acme/App.git/", "ssh://git@github.com/Acme/App.git"):
            self.assertEqual(ff.normalize_remote(url), "github.com/Acme/App", url)

    def test_a_legacy_cache_is_adopted_only_by_its_own_repository(self):
        legacy = Path(ff.CACHE_ROOT, "app")
        legacy.mkdir(parents=True)
        with open(legacy / "history.pkl", "wb") as f:
            pickle.dump({"rev": self.head_b}, f)
        (legacy / "feedback.jsonl").write_text('{"issue": 4242, "path": "x", "kind": "changed", "source": "s"}\n')
        ca = ff.default_cache(str(self.a))
        self.assertTrue(legacy.exists(), "repo A must not adopt B's legacy cache")
        self.assertFalse(Path(ca, "history.pkl").exists())
        cb = ff.default_cache(str(self.b))
        self.assertFalse(legacy.exists())
        self.assertTrue(Path(cb, "history.pkl").exists())
        self.assertTrue(Path(cb, "feedback.unbound.jsonl").exists())
        self.assertEqual(ff.feedback_rows(cb), [])


class AuthorityTests(Base):
    def run_dir(self, issue, base, ok=True):
        runs = self.root / "runs"
        d = runs / f"{issue}-1"
        d.mkdir(parents=True)
        (d / "summary.json").write_text(json.dumps({
            "issue": issue, "base": base, "checks": [{"id": "x", "ok": ok}],
            "opened_outside_briefing": ["seed.txt"]}))
        (d / "change.patch").write_text("diff --git a/src.rs b/src.rs\n")
        return str(runs)

    def test_unreplayed_runs_are_observations_and_never_rank(self):
        ca = ff.default_cache(str(self.a))
        ff.cmd_feedback(ns(repo=str(self.a), issue_runs=[self.run_dir(4242, self.head_a)]))
        rows = ff.feedback_rows(ca)
        self.assertEqual({r["authority"] for r in rows}, {"observation"})
        self.assertEqual({(r["path"], r["kind"]) for r in rows}, {("src.rs", "changed"), ("seed.txt", "read_outside")})
        self.assertEqual(dict(ff.load_feedback(ca)), {})

    def test_only_replayed_training_partition_outcomes_are_labels(self):
        ca = ff.default_cache(str(self.a))
        traces = self.traces_file([
            admitted_row(4242, self.head_a, "calibration"),
            admitted_row(4243, self.head_a, "development"),
            admitted_row(4244, self.head_a, None),
            admitted_row(4245, self.head_a, "training", ok=False),
            admitted_row(4246, self.head_a, "training", map_digest=None),  # no corpus map: not eligible
            admitted_row(4247, self.head_a, "training"),
        ])
        ff.cmd_feedback(ns(repo=str(self.a), traces=[traces]))
        self.assertEqual(dict(ff.load_feedback(ca)), {4247: {"src.rs": 1.0}})
        labels = [r for r in ff.feedback_rows(ca) if r["authority"] == "label"]
        self.assertEqual([(r["issue"], r["path"], r["kind"]) for r in labels], [(4247, "src.rs", "changed")])
        # Reads outside the briefing stay observations even on a training trace.
        self.assertIn((4247, "seed.txt", "observation"),
                      {(r["issue"], r["path"], r["authority"]) for r in ff.feedback_rows(ca)})

    def test_feedback_is_idempotent(self):
        ca = ff.default_cache(str(self.a))
        traces = self.traces_file([admitted_row(4247, self.head_a, "training")])
        ff.cmd_feedback(ns(repo=str(self.a), traces=[traces]))
        n = len(ff.feedback_rows(ca))
        ff.cmd_feedback(ns(repo=str(self.a), traces=[traces]))
        self.assertEqual(len(ff.feedback_rows(ca)), n)


if __name__ == "__main__":
    unittest.main()
