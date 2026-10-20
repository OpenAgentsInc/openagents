"""Landing instructions for briefed and bare issue runs."""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location("work_issue", Path(__file__).with_name("work_issue.py"))
work = importlib.util.module_from_spec(spec)
spec.loader.exec_module(work)


class LandingBriefTests(unittest.TestCase):
    def test_queue_brief_names_the_drivers_submit_command(self):
        text = work.landing_instructions(12038, "queue")
        self.assertIn("Do not commit, push", text)
        self.assertIn("the driver finishes with `openagents land submit --issue 12038`", text)
        self.assertTrue(text.endswith("Leave that step to the driver; never push to `main`."))

    def test_other_modes_leave_landing_to_the_driver(self):
        for mode in ("pr", "push", "none"):
            with self.subTest(mode=mode):
                self.assertEqual(work.landing_instructions(12038, mode), work.NO_LANDING)

    def test_both_engine_briefs_end_with_the_queue_instructions(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            run = Mock()
            proc = Mock(returncode=0, stdout=[])
            proc.poll.return_value = 0
            briefing = {
                "issue": 12038, "title": "Queue changes", "base": "base", "packages": [],
                "files": [], "plan": [], "markdown": "Issue briefing.",
            }
            with patch.object(work.subprocess, "Popen", return_value=proc) as launch, \
                    patch.object(work, "agent_env", return_value={}):
                work.run_briefed(run, briefing, root, root / "briefed", "", 1, land_mode="queue")
                system = (root / "briefed" / "system.md").read_text()
                self.assertTrue(system.endswith(work.landing_instructions(12038, "queue")))
                work.run_bare(run, {"number": 12038, "title": "Queue changes"}, root,
                              root / "bare", "", 1, land_mode="queue")
                prompt = launch.call_args.args[0][2]
                self.assertTrue(prompt.endswith(work.landing_instructions(12038, "queue")))


class PushLandingTests(unittest.TestCase):
    def test_push_only_pushes_the_named_branch(self):
        run = Mock()
        with patch.object(work, "sh") as sh, \
                patch.dict(work.os.environ, {"OA_WORK_PUSH_BRANCH": "land/archil-12136-ab"}):
            landed = work.land(run, Path("/w"), "o/r", {"number": 12136}, "push", "abcdef123456", "briefed", [])
        self.assertEqual(landed, {"how": "push", "branch": "land/archil-12136-ab"})
        self.assertEqual(sh.call_count, 1)
        self.assertEqual(sh.call_args.args[0][-1], "HEAD:refs/heads/land/archil-12136-ab")


class DirectLandingTests(unittest.TestCase):
    """`--land direct` (#12165) against a real origin: the change lands on
    main, or goes to the queue with the reason."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        top = Path(self.tmp.name)
        self.origin = top / "origin.git"
        work.sh(["git", "init", "-q", "--bare", "-b", "main", str(self.origin)])
        self.root = top / "work"
        work.sh(["git", "clone", "-q", str(self.origin), str(self.root)])
        self.git("config", "user.email", "t@example.com")
        self.git("config", "user.name", "t")
        self.write("a.txt", "one\n")
        self.git("add", "-A")
        self.git("commit", "-qm", "base")
        self.git("push", "-q", "origin", "HEAD:main")
        self.env = patch.dict(work.os.environ, {"OA_WORK_HOST_COMMENTS": "1", "OA_WORK_FALLBACK": "push",
                                                "OA_WORK_PUSH_BRANCH": "land/archil-1-x"})
        self.env.start()

    def tearDown(self):
        self.env.stop()
        self.tmp.cleanup()

    def git(self, *args):
        return work.sh(["git", "-C", str(self.root), *args]).stdout.strip()

    def write(self, name, text):
        (self.root / name).write_text(text)

    def main_moves(self, name, text):
        """Another change lands on main from a second clone."""
        other = Path(self.tmp.name) / "other"
        if not other.exists():
            work.sh(["git", "clone", "-q", str(self.origin), str(other)])
        work.sh(["git", "-C", str(other), "pull", "-q", "origin", "main"])
        (other / name).write_text(text)
        work.sh(["git", "-C", str(other), "add", "-A"])
        work.sh(["git", "-C", str(other), "-c", "user.email=o@example.com", "-c", "user.name=o",
                 "commit", "-qm", "other"])
        work.sh(["git", "-C", str(other), "push", "-q", "origin", "HEAD:main"])

    def change(self, name, text):
        self.write(name, text)
        self.git("add", "-A")
        self.git("commit", "-qm", "change")

    def land(self, checks=None):
        passing = checks or [{"name": "cargo test -p x", "ok": True}]
        with patch.object(work, "replay_checks", return_value=passing):
            return work.land(Mock(), self.root, "o/r", {"number": 1}, "direct", "run1", "briefed", [])

    def origin_main(self):
        return work.sh(["git", "--git-dir", str(self.origin), "rev-parse", "main"]).stdout.strip()

    def test_a_passing_change_rebases_and_lands_on_main(self):
        self.change("b.txt", "mine\n")
        self.main_moves("c.txt", "theirs\n")
        landed = self.land()
        self.assertEqual(landed["how"], "direct")
        self.assertEqual(landed["commit"], self.origin_main())
        self.assertEqual(work.sh(["git", "--git-dir", str(self.origin), "show", "main:b.txt"]).stdout, "mine\n")

    def test_a_conflict_goes_to_the_queue_and_says_why(self):
        self.change("a.txt", "mine\n")
        self.main_moves("a.txt", "theirs\n")
        before = self.origin_main()
        landed = self.land()
        self.assertEqual(landed["how"], "push")
        self.assertEqual(landed["branch"], "land/archil-1-x")
        self.assertIn("conflicts", landed["fallback"])
        self.assertEqual(self.origin_main(), before)

    def test_a_check_failing_after_the_rebase_goes_to_the_queue(self):
        self.change("b.txt", "mine\n")
        landed = self.land([{"name": "cargo test -p x", "ok": False}])
        self.assertEqual(landed["how"], "push")
        self.assertIn("`cargo test -p x`", landed["fallback"])

    def test_files_the_checks_rewrite_go_to_the_queue(self):
        self.change("b.txt", "mine\n")

        def rewrites(*_):
            self.write("b.txt", "formatted\n")
            return [{"name": "cargo fmt -p x", "ok": True}]
        with patch.object(work, "replay_checks", side_effect=rewrites):
            landed = work.land(Mock(), self.root, "o/r", {"number": 1}, "direct", "run1", "briefed", [])
        self.assertEqual(landed["how"], "push")
        self.assertIn("rewrote files", landed["fallback"])


LIBTEST_OUT = """
running 3 tests
test pages::a::tests::fine ... ok
test pages::sdk_docs::tests::every_contract_operation_has_a_page ... FAILED
test pages::b::tests::other_thing_breaks ... FAILED

failures:

---- pages::sdk_docs::tests::every_contract_operation_has_a_page stdout ----
thread 'x' panicked at src/pages/sdk_docs/tests.rs:19:5:
missing a page for land.submit

failures:
    pages::sdk_docs::tests::every_contract_operation_has_a_page
    pages::b::tests::other_thing_breaks

test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
"""
CARGO_ERR = "   Compiling x v1.0.0\nwarning: unused\n" * 80 + "error: test failed, to rerun pass `-p x --lib`\n"


class TestReportTests(unittest.TestCase):
    """The replayed test check names what failed (#12242's runs showed only
    compiler warnings)."""

    def test_the_tail_names_the_failing_tests_and_their_panics(self):
        tail, failed = work.test_report(LIBTEST_OUT, CARGO_ERR)
        self.assertEqual(failed, ["pages::sdk_docs::tests::every_contract_operation_has_a_page",
                                  "pages::b::tests::other_thing_breaks"])
        self.assertTrue(tail.startswith("failing tests: pages::sdk_docs"))
        self.assertIn("missing a page for land.submit", tail)
        self.assertIn("error: test failed", tail)
        self.assertNotIn("Compiling", tail)

    def test_a_build_failure_keeps_the_compiler_errors(self):
        tail, failed = work.test_report("", "src/lib.rs:1:1: error[E0308]: mismatched types\nerror: could not compile `x`\n")
        self.assertEqual(failed, [])
        self.assertIn("error[E0308]", tail)

    def test_an_issue_that_names_a_test_keeps_it_counted(self):
        text = "sdk_docs::every_contract_operation_has_a_page fails on main"
        self.assertTrue(work.named_in("pages::sdk_docs::tests::every_contract_operation_has_a_page", text))
        self.assertFalse(work.named_in("pages::b::tests::other_thing_breaks", text))
        self.assertFalse(work.named_in("x::tests::a", ""))


class PreexistingFailureTests(unittest.TestCase):
    """A test main already fails doesn't fail the change, unless the issue
    asks for it to be fixed."""

    def replay(self, issue_text, at_base):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "worktree"
            (root / "crates/x/src").mkdir(parents=True)
            (root / "crates/x/src/lib.rs").write_text("")
            kinds = Mock()
            kinds.plan.return_value = []
            kinds.nested_root.return_value = None

            def cargo(_root, args, _timeout):
                if args[0] in ("fmt", "check"):
                    return Mock(returncode=0, stdout="", stderr="")
                return Mock(returncode=101, stdout=LIBTEST_OUT, stderr=CARGO_ERR)
            with patch.object(work, "changed_files", return_value=["crates/x/src/lib.rs"]), \
                    patch.object(work, "package_of", return_value="x"), \
                    patch.object(work, "kinds_mod", return_value=kinds), \
                    patch.object(work, "cargo", side_effect=cargo), \
                    patch.object(work, "base_failures", return_value=at_base) as base:
                checks = work.replay_checks(Mock(), root, "base", issue_text)
        return checks[-1], base

    def test_tests_main_fails_too_are_not_counted(self):
        both = ["pages::sdk_docs::tests::every_contract_operation_has_a_page", "pages::b::tests::other_thing_breaks"]
        check, base = self.replay("Speed up the pool", both)
        self.assertTrue(check["ok"])
        self.assertEqual(check["preexisting"], both)
        self.assertTrue(base.called)

    def test_a_test_only_the_change_breaks_still_fails(self):
        check, _ = self.replay("Speed up the pool", ["pages::b::tests::other_thing_breaks"])
        self.assertFalse(check["ok"])
        self.assertIn("failing tests:", check["tail"])

    def test_a_test_the_issue_names_is_never_excused(self):
        check, base = self.replay("every_contract_operation_has_a_page fails on main", ["anything"])
        self.assertFalse(check["ok"])
        self.assertFalse(base.called)


class FallbackTests(unittest.TestCase):
    """Bare Claude Code out of its usage limit is said, and the briefed
    attempt's checks stay the run's (the runs reported `checks 0/0`)."""

    LIMIT = {"type": "result", "is_error": True, "api_error_status": 429, "api_error": "usage_limit_reached",
             "result": "You've hit your weekly limit · resets Oct 24, 8am (UTC)", "total_cost_usd": 0}

    def test_a_usage_limit_result_is_the_attempts_error(self):
        self.assertIn("weekly limit", work.bare_error(self.LIMIT, ""))
        self.assertIsNone(work.bare_error({"type": "result", "is_error": False}, ""))
        self.assertEqual(work.bare_error(None, "boom"), "boom")

    def test_a_fallback_that_did_nothing_did_not_run(self):
        nothing = {"diff": {"files": []}, "agent": {"error": "You've hit your weekly limit"}}
        self.assertFalse(work.fallback_ran(nothing))
        self.assertTrue(work.fallback_ran({"diff": {"files": []}, "agent": {"error": None}}))
        self.assertTrue(work.fallback_ran({"diff": {"files": ["a.rs"]}, "agent": {"error": "late"}}))

    def test_the_run_keeps_the_briefed_attempts_checks(self):
        briefed = {"engine": "briefed:codex", "ok": False, "outcome": "checks failed", "worktree": "/nowhere/w",
                   "out": "/nowhere", "agent": {"usage": {"cost_usd": None}, "verify_fails": 0,
                                                "verify_passed": True, "error": None},
                   "checks": [{"name": "cargo test -p x", "ok": False}, {"name": "cargo fmt -p x", "ok": True}],
                   "diff": {"files": ["crates/x/src/lib.rs"], "added": 1, "removed": 0}}
        bare = {"engine": "bare", "ok": False, "outcome": "no change", "worktree": "/nowhere/b", "out": "/nowhere",
                "agent": {"usage": {"cost_usd": 0}, "error": "You've hit your weekly limit"},
                "checks": [], "diff": {"files": [], "added": 0, "removed": 0}}
        issue = {"number": 1, "title": "t", "body": "", "state": "OPEN", "url": "u"}
        with tempfile.TemporaryDirectory() as temporary:
            out = Path(temporary) / "run"
            argv = ["work_issue.py", "--repo", "o/r", "--issue", "1", "--engine", "briefed:codex",
                    "--out", str(out), "--land", "none", "--no-rebrief"]
            briefing = {"files": [{"path": "a", "score": 1}], "finder": "lite", "checks": [], "markdown": "m"}
            with patch.object(work.sys, "argv", argv), \
                    patch.object(work, "signed_in", return_value=None), \
                    patch.object(work, "sh", return_value=Mock(stdout=__import__("json").dumps(issue))), \
                    patch.object(work, "checkout_for", return_value=Path(temporary)), \
                    patch.object(work, "default_branch_sha", return_value="base"), \
                    patch.object(work, "briefing_for", return_value=briefing), \
                    patch.dict(work.os.environ, {"OA_BRIEFED_AGENT": __file__}), \
                    patch.object(work, "attempt", side_effect=[briefed, bare]), \
                    patch.object(work, "drop_worktree"), \
                    patch("builtins.print"):
                self.assertEqual(work.main(), 1)
            result = __import__("json").loads((out / "result.json").read_text())
        self.assertEqual(result["engine"], "briefed:codex")
        self.assertEqual(len(result["checks"]), 2)
        self.assertIn("Claude Code could not take over: You've hit your weekly limit", result["escalated"])


class BinaryFileBriefingTests(unittest.TestCase):
    """A briefing that lists a binary file (#12241: a `.wasm` asset) shows
    it as binary instead of stopping on its bytes."""

    def test_a_binary_file_is_excerpted_as_binary(self):
        work.sys.path.insert(0, str(work.BENCH))
        import briefing
        import common
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            work.sh(["git", "init", "-q", str(repo)])
            (repo / "lander_gl_bg.wasm").write_bytes(b"\x00asm\x01\x00\x00\x00\xfb\xfe\xff" * 40)
            (repo / "lib.rs").write_text("fn main() {}\n")
            work.sh(["git", "-C", str(repo), "add", "-A"])
            work.sh(["git", "-C", str(repo), "-c", "user.email=t@example.com", "-c", "user.name=t",
                     "commit", "-qm", "base"])
            levers = dict(common.LEVERS)
            with patch.object(common, "REPO", repo), patch.object(briefing, "REPO", repo):
                self.assertEqual(briefing.excerpt("HEAD", "lander_gl_bg.wasm", [], levers), "(binary file)")
                self.assertIn("fn main", briefing.excerpt("HEAD", "lib.rs", [], levers))


class ReplayChecksTests(unittest.TestCase):
    def replay(self, build_code):
        calls = []

        def cargo(root, args, timeout):
            calls.append(args[0])
            code = build_code if args[0] == "check" else 0
            return Mock(returncode=code, stdout="", stderr="error[E0425]: helper" if code else "")

        kinds = Mock()
        kinds.plan.return_value = []
        kinds.nested_root.return_value = None
        with patch.object(work, "kinds_mod", return_value=kinds), \
                patch.object(work, "changed_files", return_value=["crates/x/src/lib.rs"]), \
                patch.object(work, "package_of", return_value="x"), \
                patch.object(work, "cargo", side_effect=cargo), \
                patch.object(Path, "exists", return_value=True):
            checks = work.replay_checks(Mock(), Path("/repo"), "base")
        return calls, checks

    def test_the_shipped_build_runs_before_the_tests(self):
        calls, checks = self.replay(0)
        self.assertEqual(calls, ["fmt", "check", "test"])
        self.assertEqual([c["name"] for c in checks],
                         ["cargo fmt -p x", "cargo check -p x", "cargo test -p x"])

    def test_a_failed_build_fails_the_replay_and_skips_the_tests(self):
        calls, checks = self.replay(101)
        self.assertEqual(calls, ["fmt", "check"])
        self.assertFalse(work.checks_pass(checks))
        self.assertIn("E0425", checks[1]["tail"])


class QueueSubmitTests(unittest.TestCase):
    def test_a_stopped_integrator_is_said_in_the_run_result(self):
        down = "The integrator (oa-dev-env-1) was last seen 2h ago and could not be started."
        out = Mock(returncode=0, stdout=json.dumps({"entry": {"id": "e1"}, "integrator_down": down}),
                   stderr="")
        run = Mock()
        with patch.object(work, "sh", return_value=out), patch.object(work, "comment") as said:
            landed = work.land(run, Path("/repo"), "o/r", {"number": 5}, "queue", "run1", "briefed", [])
        self.assertEqual(landed["integrator_down"], down)
        self.assertIn(down, said.call_args.args[2])
        self.assertTrue(any("WARNING" in c.args[1] for c in run.say.call_args_list))
class AttributionTests(unittest.TestCase):
    """The commit credits the engine that did the work: a Codex change
    never carries Claude's trailer (43ca758ed3 did)."""

    def commit_message(self, engine):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            work.sh(["git", "init", "-q", str(root)])
            for key, value in (("user.email", "t@example.com"), ("user.name", "t")):
                work.sh(["git", "-C", str(root), "config", key, value])
            (root / "a.txt").write_text("one\n")
            work.sh(["git", "-C", str(root), "add", "-A"])
            work.sh(["git", "-C", str(root), "commit", "-qm", "base"])
            base = work.sh(["git", "-C", str(root), "rev-parse", "HEAD"]).stdout.strip()
            (root / "a.txt").write_text("two\n")
            work.commit(root, {"number": 7, "title": "Fix a"}, engine, base)
            return work.sh(["git", "-C", str(root), "log", "-1", "--format=%B"]).stdout

    def test_each_engine_gets_its_own_trailer(self):
        self.assertIn("Co-Authored-By: Codex <noreply@openai.com>", self.commit_message("briefed:codex"))
        self.assertNotIn("Claude", self.commit_message("briefed:codex"))
        for engine in ("briefed", "bare"):
            self.assertIn(work.TRAILER, self.commit_message(engine))
        self.assertNotIn("Co-Authored-By", self.commit_message("briefed:gemini"))


if __name__ == "__main__":
    unittest.main()
