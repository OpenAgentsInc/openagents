"""Check web traffic handoff without Cloud Run, credentials, or real waits.

Run with OPENAGENTS_SCRATCH set to the session's scratch directory.
"""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(os.environ.get("HANDOFF_SCRIPT", Path(__file__).with_name("web.sh")))

GCLOUD = """#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
path = Path(os.environ["HANDOFF_STATE"])
state = json.loads(path.read_text())
args = sys.argv[1:]
state["calls"].append(args)
def save():
    path.write_text(json.dumps(state))
def value(flag):
    return args[args.index(flag) + 1]
if args[:3] == ["run", "services", "describe"]:
    save()
    if os.environ.get("HANDOFF_MODE") == "read-failure":
        sys.exit(9)
    print(json.dumps({"status": {"traffic": state["traffic"]}}))
elif args[:3] == ["run", "services", "update-traffic"]:
    if "--update-tags" in args:
        for pair in value("--update-tags").split(","):
            tag, revision = pair.split("=")
            state["traffic"].append({"tag": tag, "revisionName": revision})
        save()
        if os.environ.get("HANDOFF_MODE") == "shift-failure":
            print("Traffic update failed.", file=sys.stderr)
            sys.exit(9)
    if "--to-revisions" in args:
        revision, percent = value("--to-revisions").split("=")
        for entry in state["traffic"]:
            entry.pop("percent", None)
        state["traffic"].append({"revisionName": revision, "percent": int(percent)})
    if "--remove-tags" in args:
        if os.environ.get("HANDOFF_MODE") == "concurrent-tag":
            state["traffic"].append({"tag": "concurrent", "revisionName": "another"})
        if os.environ.get("HANDOFF_MODE") == "cleanup-failure":
            save()
            print("Tag removal failed.", file=sys.stderr)
            sys.exit(9)
        tags = value("--remove-tags").split(",")
        state["traffic"] = [entry for entry in state["traffic"] if entry.get("tag") not in tags]
    save()
elif args[:3] == ["run", "revisions", "describe"]:
    save()
    minimum = "0" if os.environ.get("HANDOFF_MODE") == "cold-outgoing" and args[3] == "old-b" else "1"
    print(json.dumps({"metadata": {"annotations": {"autoscaling.knative.dev/minScale": minimum}}}))
elif args[:3] == ["run", "services", "replace"]:
    if os.environ.get("HANDOFF_MODE") == "replace-failure":
        save()
        print("Replacement failed.", file=sys.stderr)
        sys.exit(9)
    state["replaced_spec"] = json.loads(Path(args[3]).read_text())
    state["traffic"] = state["replaced_spec"]["spec"]["traffic"]
    save()
else:
    save()
    sys.exit("Unexpected gcloud command")
"""

SLEEP = """#!/usr/bin/env python3
import json, os, signal, sys
from pathlib import Path
path = Path(os.environ["HANDOFF_STATE"])
state = json.loads(path.read_text())
state["sleeps"].append(int(sys.argv[1]))
state["clock"] += int(sys.argv[1])
path.write_text(json.dumps(state))
mode = os.environ.get("HANDOFF_MODE")
if mode in ("interrupt", "terminate") or mode == "process-relative-clock" and state["clock"] > 300:
    os.kill(int(os.environ["HANDOFF_MAIN_PID"]), signal.SIGINT if mode == "interrupt" else signal.SIGTERM)
"""

PYTHON = """#!{python}
import json, os, sys, time
from pathlib import Path
if len(sys.argv) == 3 and sys.argv[1] == "-c" and sys.argv[2].startswith("import time;"):
    path = Path(os.environ["HANDOFF_STATE"])
    state = json.loads(path.read_text())
    def local_clock():
        state["monotonic_reads"] += 1
        path.write_text(json.dumps(state))
        return 0 if os.environ.get("HANDOFF_MODE") == "process-relative-clock" else state["clock"] * 1000000000
    def shared_clock(clock):
        assert clock == time.CLOCK_MONOTONIC
        state["os_clock_reads"] += 1
        path.write_text(json.dumps(state))
        return state["clock"] * 1000000000
    time.monotonic_ns = local_clock
    time.clock_gettime_ns = shared_clock
    exec(sys.argv[2])
else:
    os.execv({python!r}, [{python!r}] + sys.argv[1:])
""".format(python=sys.executable)

CURL = """#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
path = Path(os.environ["HANDOFF_STATE"])
state = json.loads(path.read_text())
state["probes"].append(sys.argv[1:])
mode = os.environ.get("HANDOFF_MODE")
state["clock"] += 9 if mode == "slow-probes" else 0
path.write_text(json.dumps(state))
if mode == "probe-timeout":
    sys.exit(28)
print("503" if mode == "public-failure" and sys.argv[-1].endswith("/docs") else "200")
"""


class WebHandoff(unittest.TestCase):
    def run_shift(self, mode="", traffic=None, action="shift", replace=False, dry_run=False):
        with tempfile.TemporaryDirectory(
            prefix="web-handoff-", dir=os.environ["OPENAGENTS_SCRATCH"]
        ) as directory:
            root = Path(directory)
            tools = root / "bin"
            tools.mkdir()
            for name, source in [("gcloud", GCLOUD), ("sleep", SLEEP), ("python3", PYTHON), ("curl", CURL)]:
                tool = tools / name
                tool.write_text(source)
                tool.chmod(0o755)
            state_path = root / "state.json"
            state_path.write_text(json.dumps({
                "traffic": traffic if traffic is not None else [
                    {"revisionName": "old-a", "percent": 70},
                    {"revisionName": "old-b", "percent": 30},
                    {"revisionName": "candidate", "tag": "new"},
                    {"revisionName": "old-a", "tag": "existing"},
                    {"revisionName": "inactive", "tag": "unrelated"},
                ],
                "calls": [], "sleeps": [], "probes": [], "clock": 0,
                "monotonic_reads": 0, "os_clock_reads": 0,
            }))
            environment = {
                **os.environ, "HOME": str(root), "XDG_STATE_HOME": str(root / "xdg"),
                "CLOUDSDK_CONFIG": str(root / "gcloud"), "SMOKE": "0",
                "PATH": str(tools) + os.pathsep + os.environ["PATH"],
                "HANDOFF_STATE": str(state_path), "HANDOFF_MODE": mode,
            }
            environment.pop("DRY_RUN", None)
            if dry_run:
                environment["DRY_RUN"] = "1"
            # exec preserves this PID, so the injected signal targets the
            # outer script, rather than only its handoff subprocess.
            command = ["/bin/sh", "-c",
                       'HANDOFF_MAIN_PID=$$; export HANDOFF_MAIN_PID; exec /bin/sh "$@"',
                       "web-handoff-test", str(SCRIPT), action, "candidate"]
            if replace:
                spec = root / "replacement.json"
                spec.write_text(json.dumps({
                    "metadata": {"name": "wrong-service" if mode == "wrong-spec" else "openagents-web-1-staging"},
                    "spec": {"template": {"metadata": {"name": "candidate"},
                                          "spec": {"containers": [{"name": "web", "image": "latest-image"}]}},
                             "traffic": [{"latestRevision": True, "percent": 100}]},
                }))
                command.append(str(spec))
            result = subprocess.run(
                command,
                env=environment, text=True, capture_output=True, timeout=30,
            )
            return result, json.loads(state_path.read_text())

    def assert_existing_tags(self, state):
        self.assertEqual(
            {entry["tag"]: entry["revisionName"] for entry in state["traffic"] if "tag" in entry},
            {"new": "candidate", "existing": "old-a", "unrelated": "inactive"},
        )

    def test_only_outgoing_positive_revisions_are_tagged_and_cleanup_preserves_tags(self):
        for action in ["shift", "rollback"]:
            with self.subTest(action=action):
                result, state = self.run_shift(action=action)
                self.assertEqual(result.returncode, 0, result.stderr)
                updates = [call for call in state["calls"] if "--update-tags" in call]
                self.assertEqual(len(updates), 1)
                update = updates[0]
                pairs = update[update.index("--update-tags") + 1].split(",")
                self.assertEqual({pair.split("=")[1] for pair in pairs}, {"old-a", "old-b"})
                self.assertTrue(all(pair.startswith("oa-drain-") for pair in pairs))
                self.assertEqual(update[update.index("--to-revisions") + 1], "candidate=100")
                self.assertEqual(state["sleeps"], [30] * 10)
                self.assert_existing_tags(state)

    def test_same_destination_needs_no_tag_or_wait(self):
        result, state = self.run_shift(traffic=[{"revisionName": "candidate", "percent": 100}])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any("--update-tags" in call or "--remove-tags" in call for call in state["calls"]))
        self.assertEqual(state["sleeps"], [])

    def test_staging_shift_uses_the_shared_handoff_and_staging_origin(self):
        result, state = self.run_shift(action="shift-staging")
        self.assertEqual(result.returncode, 0, result.stderr)
        services = [call[3] for call in state["calls"] if call[:2] == ["run", "services"]]
        self.assertTrue(all(service == "openagents-web-1-staging" for service in services))
        self.assertTrue(all(args[-1].startswith("https://staging.openagents.com/") for args in state["probes"]))
        self.assert_existing_tags(state)

    def test_staging_replace_installs_tags_first_and_keeps_the_spec_and_all_tags(self):
        result, state = self.run_shift(action="shift-staging", replace=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        writes = [call for call in state["calls"] if call[:2] == ["run", "services"] and call[2] != "describe"]
        self.assertIn("--update-tags", writes[0])
        self.assertNotIn("--to-revisions", writes[0])
        self.assertEqual(writes[1][2], "replace")
        spec = state["replaced_spec"]
        self.assertEqual(spec["spec"]["template"]["spec"]["containers"], [{"name": "web", "image": "latest-image"}])
        self.assertTrue(any(entry.get("tag", "").startswith("oa-drain-") for entry in spec["spec"]["traffic"]))
        self.assert_existing_tags(state)

    def test_failed_replacement_cleans_tags_without_changing_the_old_traffic(self):
        result, state = self.run_shift(action="shift-staging", replace=True, mode="replace-failure")
        self.assertNotEqual(result.returncode, 0)
        positive = {entry["revisionName"]: entry["percent"] for entry in state["traffic"] if entry.get("percent")}
        self.assertEqual(positive, {"old-a": 70, "old-b": 30})
        self.assertEqual(state["sleeps"], [])
        self.assert_existing_tags(state)

    def test_wrong_replacement_service_is_refused_before_any_write(self):
        result, state = self.run_shift(action="shift-staging", replace=True, mode="wrong-spec")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(call[2] in ("update-traffic", "replace") for call in state["calls"] if call[:2] == ["run", "services"]))
        self.assert_existing_tags(state)

    def test_dry_run_does_not_write_probe_or_sleep(self):
        for replace in [False, True]:
            with self.subTest(replace=replace):
                result, state = self.run_shift(action="shift-staging", replace=replace, dry_run=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertFalse(any(call[2] in ("update-traffic", "replace") for call in state["calls"] if call[:2] == ["run", "services"]))
                self.assertEqual(state["probes"], [])
                self.assertEqual(state["sleeps"], [])
                self.assert_existing_tags(state)

    def test_failed_read_never_changes_traffic(self):
        result, state = self.run_shift(mode="read-failure")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any("update-traffic" in call for call in state["calls"]))

    def test_a_cold_outgoing_revision_is_refused_before_changing_traffic(self):
        result, state = self.run_shift(mode="cold-outgoing")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("outgoing revision old-b", result.stderr)
        self.assertFalse(any("update-traffic" in call for call in state["calls"]))

    def test_cleanup_preserves_a_concurrent_unrelated_tag(self):
        result, state = self.run_shift(mode="concurrent-tag")
        self.assertEqual(result.returncode, 0, result.stderr)
        tags = {entry["tag"]: entry["revisionName"] for entry in state["traffic"] if "tag" in entry}
        self.assertEqual(tags.pop("concurrent"), "another")
        self.assertEqual(tags, {"new": "candidate", "existing": "old-a", "unrelated": "inactive"})

    def test_failed_shift_still_removes_tags_that_may_have_been_created(self):
        result, state = self.run_shift(mode="shift-failure")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(state["sleeps"], [])
        self.assert_existing_tags(state)

    def test_interrupts_remove_tags_and_preserve_failure(self):
        for mode, status in [("interrupt", 130), ("terminate", 143)]:
            with self.subTest(mode=mode):
                result, state = self.run_shift(mode=mode)
                self.assertEqual(result.returncode, status, result.stderr)
                self.assert_existing_tags(state)

    def test_failed_cleanup_is_not_reported_as_success(self):
        result, state = self.run_shift(mode="cleanup-failure")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Could not remove temporary web tags:", result.stderr)
        self.assertIn("--remove-tags oa-drain-", result.stderr)
        self.assertTrue(any(entry.get("tag", "").startswith("oa-drain-") for entry in state["traffic"]))

    def test_a_failed_public_read_remains_a_failure_and_a_later_run_can_recover(self):
        failed, state = self.run_shift(mode="public-failure")
        self.assertNotEqual(failed.returncode, 0)
        self.assertIn("/docs returned 503", failed.stderr)
        self.assertEqual(state["clock"], 300)
        self.assert_existing_tags(state)
        recovered, state = self.run_shift()
        self.assertEqual(recovered.returncode, 0, recovered.stderr)
        self.assertEqual(len(state["probes"]), 3)
        for args in state["probes"]:
            self.assertEqual(args[args.index("--connect-timeout") + 1], "5")
            self.assertEqual(args[args.index("--max-time") + 1], "10")
            self.assertNotIn("--retry", args)
            self.assertNotIn("-X", args)
            self.assertNotIn("Authorization", " ".join(args))
        self.assert_existing_tags(state)

    def test_probe_time_counts_toward_the_drain_deadline(self):
        result, state = self.run_shift(mode="slow-probes")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(state["clock"], 300)
        self.assertEqual(sum(state["sleeps"]), 273)

    def test_separate_process_clock_origins_cannot_extend_the_drain(self):
        result, state = self.run_shift(mode="process-relative-clock")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(state["clock"], 300)
        self.assertEqual(state["monotonic_reads"], 0)
        self.assertGreaterEqual(state["os_clock_reads"], 11)
        self.assert_existing_tags(state)

    def test_probe_timeouts_do_not_skip_cleanup_or_hide_failure(self):
        result, state = self.run_shift(mode="probe-timeout")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("returned 000", result.stderr)
        self.assertEqual(state["clock"], 300)
        self.assert_existing_tags(state)


if __name__ == "__main__":
    unittest.main()
