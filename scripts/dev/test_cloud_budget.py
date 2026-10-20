"""Check the cloud spend guard (#12370) without touching Google Cloud.

Run: python3 -m unittest scripts/dev/test_cloud_budget.py
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("cloud-budget.py").resolve()
PRICES = Path(__file__).with_name("cloud-prices.json").resolve()
T0 = 1_800_000_000  # 2027-01-15T08:00:00Z
A4_SPOT = 39.6336

FAKE_GCLOUD = r'''#!/usr/bin/env python3
import json, os, sys
with open(os.environ["FAKE_LOG"], "a") as fh:
    fh.write(json.dumps(sys.argv[1:]) + "\n")
args = sys.argv[1:]
if args[:3] == ["compute", "instances", "create"]:
    sys.exit(int(os.environ.get("FAKE_CREATE_RC", "0")))
if args[:3] == ["compute", "instances", "describe"]:
    state = os.environ.get("FAKE_DESCRIBE", "RUNNING")
    if state == "GONE":
        sys.stderr.write("ERROR: The resource 'projects/p/zones/z/instances/x' was not found\n")
        sys.exit(1)
    print(json.dumps({"status": state, "creationTimestamp": "2027-01-15T08:00:00Z"}))
    sys.exit(0)
if args[:3] == ["compute", "operations", "list"]:
    print(os.environ.get("FAKE_OPS", "[]"))
    sys.exit(0)
sys.exit(0)
'''

FAKE_AGENT_RUN = r'''#!/usr/bin/env python3
import os, sys
if sys.argv[1] == "labels":
    if os.environ.get("FAKE_NO_RUN"):
        sys.stderr.write("no run\n"); sys.exit(1)
    print("run-id=run-test,run-host=testhost")
elif sys.argv[1] == "add-vm":
    with open(os.environ["FAKE_LOG"], "a") as fh:
        fh.write("add-vm " + " ".join(sys.argv[2:]) + "\n")
'''


def scratch_root() -> str | None:
    try:
        return subprocess.check_output(["openagents", "scratch"], text=True).strip() or None
    except (OSError, subprocess.CalledProcessError):
        return None


class CloudBudgetTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(dir=scratch_root())
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.log = self.root / "calls.log"
        self.log.touch()
        for name, body in (("gcloud", FAKE_GCLOUD), ("agent-run.py", FAKE_AGENT_RUN)):
            path = self.root / name
            path.write_text(body)
            path.chmod(0o755)
        self.ledger = self.root / "ledgers" / "test.jsonl"
        self.env = {
            "PATH": os.environ.get("PATH", ""),
            "HOME": str(self.root),
            "OA_CLOUD_BUDGET_DIR": str(self.root / "ledgers"),
            "OA_CLOUD_PRICES": str(PRICES),
            "OA_GCLOUD": str(self.root / "gcloud"),
            "OA_AGENT_RUN": str(self.root / "agent-run.py"),
            "CLOUDSDK_CONFIG": str(self.root / "gcloud-config"),
            "FAKE_LOG": str(self.log),
            "OA_CLOUD_BUDGET_NOW": str(T0),
        }

    def run_guard(self, *args, at=T0, **env):
        e = dict(self.env, OA_CLOUD_BUDGET_NOW=str(at), **env)
        return subprocess.run([sys.executable, str(SCRIPT), *args], capture_output=True, text=True, env=e)

    def create(self, name, *extra, hours="8", machine="a4-highgpu-8g", spot=True, cap="3000", at=T0, **env):
        args = ["create", name, "--budget", "test", "--cap", cap, "--machine-type", machine,
                "--zone", "us-central1-b", "--hours", hours]
        if spot:
            args.append("--spot")
        return self.run_guard(*args, *extra, at=at, **env)

    def report(self, at):
        r = self.run_guard("report", "--budget", "test", "--offline", "--json", at=at)
        self.assertEqual(r.returncode, 0, r.stderr)
        return json.loads(r.stdout)

    def calls(self):
        return [l for l in self.log.read_text().splitlines() if l]

    def test_dry_run_prints_the_full_command_and_creates_nothing(self):
        r = self.create("oa-dsv41-oracle-1", "--dry-run", "--",
                        "--accelerator=type=nvidia-b200,count=8", "--boot-disk-size=500GB")
        self.assertEqual(r.returncode, 0, r.stderr)
        cmd = r.stdout.strip()
        self.assertTrue(cmd.startswith("gcloud compute instances create oa-dsv41-oracle-1 "))
        self.assertIn("--machine-type=a4-highgpu-8g", cmd)
        self.assertIn("run-id=run-test,run-host=testhost,budget=test,expires-at=2027-01-15t16-00-00z", cmd)
        self.assertIn("--max-run-duration=28800s", cmd)
        self.assertIn("--instance-termination-action=DELETE", cmd)
        self.assertIn("--provisioning-model=SPOT", cmd)
        self.assertTrue(cmd.endswith("--accelerator=type=nvidia-b200,count=8 --boot-disk-size=500GB"))
        self.assertEqual(self.calls(), [])
        self.assertFalse(self.ledger.exists())

    def test_create_records_start_labels_the_run_and_prints_the_vm(self):
        r = self.create("oa-dsv41-oracle-1", "--on-expiry", "stop")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("vm oa-dsv41-oracle-1 zone us-central1-b", r.stdout)
        created = json.loads(self.calls()[0])
        self.assertIn("--instance-termination-action=STOP", created)
        self.assertIn("add-vm run-test oa-dsv41-oracle-1 us-central1-b openagentsgemini", self.calls())
        records = [json.loads(l) for l in self.ledger.read_text().splitlines()]
        self.assertEqual([r["event"] for r in records], ["budget", "start"])
        start = records[1]
        self.assertEqual((start["vm"], start["provisioning"], start["price_per_hour"], start["run_id"]),
                         ("oa-dsv41-oracle-1", "spot", A4_SPOT, "run-test"))

    def test_refuses_past_the_cap(self):
        self.assertEqual(self.create("first", cap="600").returncode, 0)
        # The first machine counts up to its expiry ($317.07), so 8 more hours don't fit in $600.
        r = self.create("second", cap="600", at=T0 + 60)
        self.assertEqual(r.returncode, 2)
        self.assertIn("over the cap", r.stderr)
        self.assertEqual(len([c for c in self.calls() if "create" in c]), 1)
        # Seven hours still fit: 317.07 + 277.44 = 594.51.
        self.assertEqual(self.create("third", hours="7", cap="600", at=T0 + 60).returncode, 0)

    def test_refuses_an_unpriced_machine_type(self):
        r = self.create("odd", machine="n2-standard-4", spot=False)
        self.assertEqual(r.returncode, 2)
        self.assertIn("no on-demand price for n2-standard-4", r.stderr)
        prices = json.loads(PRICES.read_text())
        del prices["machines"]["g4-standard-48"]["spot"]
        custom = self.root / "prices.json"
        custom.write_text(json.dumps(prices))
        r = self.create("g4-one", machine="g4-standard-48", OA_CLOUD_PRICES=str(custom))
        self.assertEqual(r.returncode, 2)
        self.assertIn("no spot price for g4-standard-48", r.stderr)
        self.assertEqual(self.calls(), [])

    def test_refuses_production_machines_and_guard_owned_flags(self):
        r = self.create("clef-decisions-h100", "--dry-run")
        self.assertEqual(r.returncode, 2)
        self.assertIn("production machine", r.stderr)
        r = self.create("ok-name", "--", "--labels=a=b")
        self.assertEqual(r.returncode, 2)
        self.assertIn("--labels is set by the guard", r.stderr)

    def test_refuses_without_a_run(self):
        r = self.create("no-run-vm", FAKE_NO_RUN="1")
        self.assertEqual(r.returncode, 2)
        self.assertIn("no run", r.stderr)

    def test_cost_for_running_and_finished_machines(self):
        self.assertEqual(self.create("vm-a", hours="2").returncode, 0)
        half = self.report(T0 + 1800)
        self.assertAlmostEqual(half["running"], A4_SPOT / 2, places=4)
        self.assertAlmostEqual(half["running_committed"], A4_SPOT * 2, places=4)
        self.assertEqual(half["spent"], 0)
        self.assertEqual(half["running_machines"][0]["age_seconds"], 1800)
        # A running machine never costs more than its expiry allows.
        late = self.report(T0 + 10 * 3600)
        self.assertAlmostEqual(late["running"], A4_SPOT * 2, places=4)
        r = self.run_guard("stop", "vm-a", "--budget", "test", at=T0 + 3600)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("delete", self.calls()[-1])
        done = self.report(T0 + 7200)
        self.assertAlmostEqual(done["spent"], A4_SPOT, places=4)
        self.assertEqual((done["running"], done["finished"], done["running_machines"]), (0, 1, []))
        self.assertAlmostEqual(done["remaining"], 3000 - A4_SPOT, places=4)

    def test_sync_records_a_machine_deleted_at_expiry(self):
        self.assertEqual(self.create("vm-gone", hours="1").returncode, 0)
        ops = json.dumps([{"operationType": "insert", "endTime": "2027-01-15T08:00:30Z"},
                          {"operationType": "compute.instances.deferredDelete",
                           "endTime": "2027-01-15T08:30:00Z"}])
        r = self.run_guard("sync", "--budget", "test", at=T0 + 7200, FAKE_DESCRIBE="GONE", FAKE_OPS=ops)
        self.assertEqual(r.returncode, 0, r.stderr)
        done = self.report(T0 + 7200)
        self.assertAlmostEqual(done["spent"], A4_SPOT / 2, places=4)
        records = [json.loads(l) for l in self.ledger.read_text().splitlines()]
        self.assertEqual(records[-1]["reason"], "gone")

    def test_failed_create_costs_nothing(self):
        r = self.create("vm-fail", FAKE_CREATE_RC="1", FAKE_DESCRIBE="GONE")
        self.assertEqual(r.returncode, 1)
        self.assertEqual(self.report(T0 + 3600)["spent"], 0)
        self.assertEqual(self.report(T0 + 3600)["running_machines"], [])

    def test_failed_create_that_left_a_machine_stays_counted(self):
        r = self.create("vm-maybe", FAKE_CREATE_RC="1")
        self.assertEqual(r.returncode, 1)
        self.assertIn("may exist", r.stderr)
        self.assertEqual([m["vm"] for m in self.report(T0 + 60)["running_machines"]], ["vm-maybe"])

    def test_ledger_replays_after_a_torn_write(self):
        self.assertEqual(self.create("vm-a", hours="2").returncode, 0)
        whole = self.ledger.read_bytes()
        torn = json.dumps({"event": "start", "id": "torn", "vm": "vm-b"}).encode()[:25]
        self.ledger.write_bytes(whole + torn)
        mid = self.report(T0 + 1800)
        self.assertAlmostEqual(mid["running"], A4_SPOT / 2, places=4)
        self.assertEqual([m["vm"] for m in mid["running_machines"]], ["vm-a"])
        # The next record starts on its own line, and every record reads back.
        r = self.run_guard("stop", "vm-a", "--budget", "test", "--keep-machine", at=T0 + 3600)
        self.assertEqual(r.returncode, 0, r.stderr)
        lines = self.ledger.read_bytes().split(b"\n")
        self.assertEqual(lines[-3], torn)
        self.assertEqual(json.loads(lines[-2])["event"], "stop")
        done = self.report(T0 + 7200)
        self.assertAlmostEqual(done["spent"], A4_SPOT, places=4)
        self.assertEqual(done["finished"], 1)


if __name__ == "__main__":
    unittest.main()
