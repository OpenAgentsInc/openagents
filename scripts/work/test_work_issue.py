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
        for mode in ("pr", "none"):
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


if __name__ == "__main__":
    unittest.main()
