"""Landing instructions for briefed and bare issue runs."""

import importlib.util
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


if __name__ == "__main__":
    unittest.main()
