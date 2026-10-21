"""Check the cloud spend guard (#12370, #12376) without touching Google Cloud.

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
from unittest.mock import patch
import importlib.util

SCRIPT = Path(__file__).with_name("cloud-budget.py").resolve()
PRICES = Path(__file__).with_name("cloud-prices.json").resolve()
T0 = 1_800_000_000  # 2027-01-15T08:00:00Z
A4_SPOT = 39.6336

FAKE_GCLOUD = r'''#!/usr/bin/env python3
import json, os, sys, time
with open(os.environ["FAKE_LOG"], "a") as fh:
    fh.write(json.dumps(sys.argv[1:]) + "\n")
args = sys.argv[1:]
deleted = os.path.join(os.path.dirname(os.environ["FAKE_LOG"]), "deleted")
if args[:3] == ["compute", "instances", "delete"]:
    open(deleted, "w").close()
    sys.exit(0)
after = os.path.exists(deleted)
if after and os.environ.get("FAKE_AFTER_DELETE"):
    for k, v in json.loads(os.environ["FAKE_AFTER_DELETE"]).items():
        os.environ[k] = v
if args[:3] == ["compute", "instances", "create"]:
    time.sleep(float(os.environ.get("FAKE_CREATE_SLEEP", "0")))
    sys.exit(int(os.environ.get("FAKE_CREATE_RC", "0")))
if args[:3] == ["compute", "instances", "list"]:
    print(os.environ.get("FAKE_LIST", "[]"))
    sys.exit(0)
if args[:2] == ["logging", "read"]:
    print(os.environ.get("FAKE_AUDIT", "[]"))
    sys.exit(0)
if args[:3] == ["compute", "instances", "describe"]:
    state = os.environ.get("FAKE_DESCRIBE", "RUNNING")
    if state == "GONE":
        sys.stderr.write("ERROR: The resource 'projects/p/zones/z/instances/x' was not found\n")
        sys.exit(1)
    created = os.environ.get("FAKE_CREATED", "2027-01-15T08:00:00Z")
    info = {"status": state, "creationTimestamp": created}
    if os.environ.get("FAKE_ID"):
        info["id"] = os.environ["FAKE_ID"]
    if os.environ.get("FAKE_STARTED"):
        info["lastStartTimestamp"] = os.environ["FAKE_STARTED"]
    if os.environ.get("FAKE_STOPPED"):
        info["lastStopTimestamp"] = os.environ["FAKE_STOPPED"]
    print(json.dumps(info))
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
            "OA_CLOUD_BUDGET_HOST": "testhost",
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

    def creates(self):
        return [json.loads(c) for c in self.calls() if c.startswith('["compute", "instances", "create"')]

    def records(self):
        out = []
        for line in self.ledger.read_text().splitlines():
            try:
                out.append(json.loads(line))
            except json.JSONDecodeError:
                pass  # a torn or corrupted line the test wrote
        return out

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

    def test_flex_start_queues_and_counts_the_queue_time(self):
        r = self.create("oa-dsv41-flex", "--flex-start", "--queue-hours", "2", hours="3", spot=False)
        self.assertEqual(r.returncode, 0, r.stderr)
        cmd = self.creates()[0]
        self.assertIn("--provisioning-model=FLEX_START", cmd)
        self.assertIn("--request-valid-for-duration=7200s", cmd)
        self.assertIn("--reservation-affinity=none", cmd)
        self.assertIn("--max-run-duration=10800s", cmd)
        start = [x for x in self.records() if x.get("event") == "start"][-1]
        self.assertEqual(start["provisioning"], "flex-start")
        self.assertEqual(start["price_per_hour"], 64.44)
        self.assertEqual(start["expires_epoch"], T0 + 5 * 3600)
        refused = self.create("oa-dsv41-flex-2", "--flex-start", "--spot", hours="1", spot=False)
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn("choose one", refused.stderr)

    def test_flex_start_bills_from_its_start_and_never_for_a_queued_request(self):
        self.assertEqual(self.create("flex-q", "--flex-start", hours="2", spot=False).returncode, 0)
        # The delete cancels the queued insert, which then ends with an error.
        cancelled = json.dumps({"FAKE_OPS": json.dumps([{
            "operationType": "insert", "status": "DONE", "insertTime": "2027-01-15T08:00:01Z",
            "endTime": "2027-01-15T08:30:00Z", "error": {"errors": [{"code": "OPERATION_CANCELED_BY_USER"}]}}])})
        r = self.run_guard("stop", "flex-q", "--budget", "test", at=T0 + 1800, FAKE_DESCRIBE="PENDING",
                           FAKE_AFTER_DELETE=cancelled)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(self.records()[-1]["reason"], "never-started")
        self.assertEqual(self.report(T0 + 3600)["spent"], 0)
        self.assertEqual(self.create("flex-r", "--flex-start", hours="2", spot=False, at=T0 + 3600).returncode, 0)
        r = self.run_guard("stop", "flex-r", "--budget", "test", at=T0 + 3 * 3600,
                           FAKE_DESCRIBE="RUNNING", FAKE_STARTED="2027-01-15T10:00:00Z",
                           FAKE_CREATED="2027-01-15T09:00:00Z")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertAlmostEqual(self.report(T0 + 4 * 3600)["spent"], 64.44, places=4)

    def test_sync_settles_an_unfulfilled_flex_request_at_no_cost(self):
        self.assertEqual(self.create("flex-x", "--flex-start", hours="2", spot=False).returncode, 0)
        ops = json.dumps([{"operationType": "insert", "insertTime": "2027-01-15T08:00:01Z",
                           "error": {"errors": [{"code": "ZONE_RESOURCE_POOL_EXHAUSTED_WITH_DETAILS"}]}}])
        r = self.run_guard("sync", "--budget", "test", at=T0 + 7200, FAKE_DESCRIBE="GONE", FAKE_OPS=ops)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(self.records()[-1]["reason"], "never-started")
        self.assertEqual(self.report(T0 + 7200)["spent"], 0)

    def test_create_records_start_labels_the_run_and_prints_the_vm(self):
        r = self.create("oa-dsv41-oracle-1", "--on-expiry", "stop")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("vm oa-dsv41-oracle-1 zone us-central1-b", r.stdout)
        created = self.creates()[0]
        self.assertIn("--instance-termination-action=STOP", created)
        self.assertIn("add-vm run-test oa-dsv41-oracle-1 us-central1-b openagentsgemini", self.calls())
        records = self.records()
        self.assertEqual([r["event"] for r in records], ["budget", "start"])
        self.assertEqual((records[0]["host"], records[0]["ledger"]), ("testhost", str(self.ledger.resolve())))
        start = records[1]
        self.assertEqual((start["vm"], start["provisioning"], start["price_per_hour"], start["run_id"]),
                         ("oa-dsv41-oracle-1", "spot", A4_SPOT, "run-test"))

    def test_refuses_past_the_cap(self):
        self.assertEqual(self.create("first", cap="600").returncode, 0)
        # The first machine counts up to its expiry ($317.07), so 8 more hours don't fit in $600.
        r = self.create("second", cap="600", at=T0 + 60)
        self.assertEqual(r.returncode, 2)
        self.assertIn("over the cap", r.stderr)
        self.assertEqual(len(self.creates()), 1)
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
        # Past its expiry, a machine nobody has settled from Google's records keeps counting.
        late = self.report(T0 + 10 * 3600)
        self.assertAlmostEqual(late["running"], A4_SPOT * 10, places=4)
        self.assertAlmostEqual(late["running_committed"], A4_SPOT * 10, places=4)
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
        self.assertEqual(lines[-4], torn)
        self.assertEqual(json.loads(lines[-3])["event"], "torn")
        self.assertEqual(json.loads(lines[-2])["event"], "stop")
        done = self.report(T0 + 7200)
        self.assertAlmostEqual(done["spent"], A4_SPOT, places=4)
        self.assertEqual((done["finished"], done["corrupt_lines"]), (1, []))
        # The healed torn line is not corruption: create still admits.
        self.assertEqual(self.create("vm-c", hours="1", at=T0 + 7200).returncode, 0)

    # ------------------------------------------------------------- #12376

    def test_an_interior_malformed_line_refuses_create(self):
        self.assertEqual(self.create("vm-a", hours="2").returncode, 0)
        # A reservation line goes unreadable; skipping it would free $1,900.
        self.assertEqual(self.create("big", hours="48", at=T0 + 60).returncode, 0)
        lines = self.ledger.read_text().splitlines()
        lines[-1] = lines[-1][:60]
        self.ledger.write_text("\n".join(lines) + "\n\n")
        r = self.create("vm-b", hours="1", at=T0 + 120)
        self.assertEqual(r.returncode, 2)
        self.assertIn(f"unreadable records on line {len(lines)}", r.stderr)
        self.assertEqual(len(self.creates()), 2)
        r = self.run_guard("reserve", "10", "--why", "disks", "--budget", "test", at=T0 + 120)
        self.assertEqual(r.returncode, 2)
        # report still prints what it reads, and flags the line.
        rep = self.report(T0 + 120)
        self.assertEqual(rep["corrupt_lines"], [len(lines)])
        self.assertEqual([m["vm"] for m in rep["running_machines"]], ["vm-a"])
        text = self.run_guard("report", "--budget", "test", "--offline", at=T0 + 120)
        self.assertEqual(text.returncode, 0)
        self.assertIn("CORRUPT", text.stdout)

    def test_a_torn_final_line_is_tolerated_by_create(self):
        self.assertEqual(self.create("vm-a", hours="1").returncode, 0)
        self.ledger.write_bytes(self.ledger.read_bytes() + b'{"event": "start", "id": "x", "vm"')
        r = self.create("vm-b", hours="1", at=T0 + 60)
        self.assertEqual(r.returncode, 0, r.stderr)
        events = [r["event"] for r in self.records() if r]
        self.assertEqual(events[-2:], ["torn", "start"])
        self.assertEqual(self.report(T0 + 120)["corrupt_lines"], [])

    def test_sync_finds_a_machine_whose_create_reply_was_lost(self):
        # The machine exists with the budget's label, but the ledger has no running record for it.
        listing = json.dumps([{"name": "vm-lost", "zone": "projects/p/zones/us-central1-b",
                               "machineType": "zones/us-central1-b/machineTypes/a4-highgpu-8g",
                               "status": "RUNNING", "creationTimestamp": "2027-01-15T08:00:00Z",
                               "labels": {"budget": "test", "run-id": "run-x",
                                          "expires-at": "2027-01-15t12-00-00z"},
                               "scheduling": {"provisioningModel": "SPOT"}}])
        r = self.run_guard("sync", "--budget", "test", at=T0 + 600, FAKE_LIST=listing)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("vm-lost carries budget=test", r.stderr)
        rep = self.report(T0 + 600)
        self.assertEqual([m["vm"] for m in rep["running_machines"]], ["vm-lost"])
        self.assertAlmostEqual(rep["running_committed"], A4_SPOT * 4, places=4)
        # A second sync doesn't count it twice.
        self.run_guard("sync", "--budget", "test", at=T0 + 700, FAKE_LIST=listing)
        self.assertEqual(len(self.report(T0 + 700)["running_machines"]), 1)
        # create syncs first, so the lost machine counts against the next request.
        # 73 h is $2,893.25: it fits alone, but not beside the lost machine's $158.53.
        r = self.create("vm-next", hours="73", cap="3000", at=T0 + 800, FAKE_LIST=listing)
        self.assertEqual(r.returncode, 2)
        self.assertIn("over the cap", r.stderr)

    def test_concurrent_admissions_dont_both_pass(self):
        e = dict(self.env, FAKE_CREATE_SLEEP="0.5")
        args = ["--budget", "test", "--cap", "500", "--machine-type", "a4-highgpu-8g",
                "--zone", "us-central1-b", "--hours", "8", "--spot"]
        procs = [subprocess.Popen([sys.executable, str(SCRIPT), "create", f"vm-{i}", *args], env=e,
                                  stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) for i in range(2)]
        codes = sorted(p.wait() for p in procs)
        for p in procs:
            p.stdout.close()
            p.stderr.close()
        self.assertEqual(codes, [0, 2])
        self.assertEqual(len([r for r in self.records() if r["event"] == "start"]), 1)
        self.assertEqual(len(self.creates()), 1)

    def test_a_reused_name_does_not_merge_with_an_older_record(self):
        self.assertEqual(self.create("vm-a", hours="2").returncode, 0)
        # vm-a was deleted after 1 h; someone made a new vm-a at T0 + 3 h with the budget label.
        ops = json.dumps([{"operationType": "delete", "endTime": "2027-01-15T09:00:00Z"}])
        listing = json.dumps([{"name": "vm-a", "zone": "us-central1-b", "machineType": "a4-highgpu-8g",
                               "status": "RUNNING", "creationTimestamp": "2027-01-15T11:00:00Z",
                               "labels": {"budget": "test", "expires-at": "2027-01-15t13-00-00z"},
                               "scheduling": {"provisioningModel": "SPOT"}}])
        r = self.run_guard("sync", "--budget", "test", at=T0 + 4 * 3600, FAKE_LIST=listing,
                           FAKE_CREATED="2027-01-15T11:00:00Z", FAKE_OPS=ops)
        self.assertEqual(r.returncode, 0, r.stderr)
        starts = [r for r in self.records() if r["event"] == "start"]
        self.assertEqual(len(starts), 2)
        rep = self.report(T0 + 4 * 3600)
        self.assertAlmostEqual(rep["spent"], A4_SPOT, places=4)  # the old one ran 1 h
        self.assertEqual(rep["finished"], 1)
        self.assertEqual([m["age_seconds"] for m in rep["running_machines"]], [3600])
        self.assertAlmostEqual(rep["running_committed"], A4_SPOT * 2, places=4)
        # Another sync leaves both alone.
        self.run_guard("sync", "--budget", "test", at=T0 + 4 * 3600 + 60, FAKE_LIST=listing,
                       FAKE_CREATED="2027-01-15T11:00:00Z", FAKE_OPS=ops)
        self.assertEqual(len([r for r in self.records() if r["event"] in ("start", "stop")]), 3)

    def test_a_delayed_expiry_is_costed_to_the_actual_deletion(self):
        self.assertEqual(self.create("vm-late", hours="1").returncode, 0)
        # Google deleted it 10 minutes after its expiry.
        ops = json.dumps([{"operationType": "compute.instances.deferredDelete",
                           "endTime": "2027-01-15T09:10:00Z"}])
        r = self.run_guard("sync", "--budget", "test", at=T0 + 7200, FAKE_DESCRIBE="GONE", FAKE_OPS=ops)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertAlmostEqual(self.report(T0 + 7200)["spent"], A4_SPOT * 70 / 60, places=4)

    def test_audit_log_settles_when_operations_have_aged_out(self):
        self.assertEqual(self.create("vm-old", hours="1").returncode, 0)
        audit = json.dumps([{"timestamp": "2027-01-15T08:45:00Z",
                             "protoPayload": {"methodName": "v1.compute.instances.delete"},
                             "operation": {"last": True}}])
        self.run_guard("sync", "--budget", "test", at=T0 + 7200, FAKE_DESCRIBE="GONE", FAKE_AUDIT=audit)
        self.assertAlmostEqual(self.report(T0 + 7200)["spent"], A4_SPOT * 0.75, places=4)

    def test_a_gone_machine_with_no_provider_record_keeps_its_full_cost(self):
        self.assertEqual(self.create("vm-unknown", hours="2").returncode, 0)
        r = self.run_guard("sync", "--budget", "test", at=T0 + 1800, FAKE_DESCRIBE="GONE")
        self.assertEqual(r.returncode, 0, r.stderr)
        rep = self.report(T0 + 1800)
        self.assertEqual(rep["finished"], 0)
        self.assertAlmostEqual(rep["running_committed"], A4_SPOT * 2, places=4)
        # After its expiry it settles at the expiry: nothing reserved is released.
        self.run_guard("sync", "--budget", "test", at=T0 + 3 * 3600, FAKE_DESCRIBE="GONE")
        rep = self.report(T0 + 3 * 3600)
        self.assertAlmostEqual(rep["spent"], A4_SPOT * 2, places=4)
        self.assertEqual(self.records()[-1]["reason"], "gone-at-expiry")

    def test_a_create_from_a_second_host_is_refused(self):
        self.assertEqual(self.create("vm-a", hours="1").returncode, 0)
        r = self.create("vm-b", hours="1", at=T0 + 60, OA_CLOUD_BUDGET_HOST="otherhost")
        self.assertEqual(r.returncode, 2)
        self.assertIn("host testhost allocates for budget test", r.stderr)
        self.assertEqual(len(self.creates()), 1)
        # --take-over is recorded, and then the old host is the one refused.
        r = self.create("vm-b", "--take-over", hours="1", at=T0 + 60, OA_CLOUD_BUDGET_HOST="otherhost")
        self.assertEqual(r.returncode, 0, r.stderr)
        owner = [r for r in self.records() if r["event"] == "owner"]
        self.assertEqual((owner[-1]["host"], owner[-1]["take_over"]), ("otherhost", True))
        self.assertEqual(self.create("vm-c", hours="1", at=T0 + 120).returncode, 2)

    def test_a_second_ledger_path_on_one_host_is_refused(self):
        self.assertEqual(self.create("vm-a", hours="1").returncode, 0)
        r = self.create("vm-b", hours="1", at=T0 + 60, OA_CLOUD_BUDGET_DIR=str(self.root / "elsewhere"))
        self.assertEqual(r.returncode, 2)
        self.assertIn("on this host, budget test uses the ledger at", r.stderr)

    def test_a_ledger_from_before_the_owner_rule_is_owned_by_the_next_create(self):
        self.ledger.parent.mkdir(parents=True)
        old = {"at": "2027-01-15T07:00:00Z", "budget": "test", "cap": 3000.0, "event": "budget"}
        self.ledger.write_text(json.dumps(old) + "\n")
        before = self.ledger.read_bytes()
        self.assertEqual(self.report(T0)["owner"], None)
        self.assertEqual(self.create("vm-a", hours="1").returncode, 0)
        self.assertTrue(self.ledger.read_bytes().startswith(before))  # history is never rewritten
        self.assertEqual([r["event"] for r in self.records()], ["budget", "owner", "start"])
        self.assertEqual(self.report(T0)["owner"]["host"], "testhost")
        self.assertEqual(self.create("vm-b", hours="1", OA_CLOUD_BUDGET_HOST="otherhost").returncode, 2)

    def test_admission_counts_the_reserve(self):
        r = self.run_guard("reserve", "300", "--why", "disks, bucket, egress", "--budget", "test",
                           "--cap", "600")
        self.assertEqual(r.returncode, 0, r.stderr)
        # 8 h of A4 spot is $317.07; with $300 reserved it no longer fits in $600.
        r = self.create("vm-a", cap="600")
        self.assertEqual(r.returncode, 2)
        self.assertIn("reserve $300.00", r.stderr)
        self.assertEqual(self.create("vm-a", hours="7", cap="600").returncode, 0)
        rep = self.report(T0 + 60)
        self.assertEqual(rep["reserve"], 300)
        self.assertAlmostEqual(rep["remaining"], 600 - 300 - A4_SPOT * 7, places=4)

    def test_a_charge_counts_as_spent_and_against_admission(self):
        # Network transfer isn't a machine; a charge records it as spent (W16, #12403).
        r = self.run_guard("charge", "300", "--why", "weights transfer", "--estimate",
                           "--budget", "test", "--cap", "600")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(self.records()[-1]["event"], "charge")
        self.assertTrue(self.records()[-1]["estimate"])
        rep = self.report(T0)
        self.assertEqual(rep["spent"], 300)
        self.assertEqual(rep["charges"], 300)
        # 8 h of A4 spot is $317.07; after $300 of charges it no longer fits in $600.
        self.assertEqual(self.create("vm-a", cap="600").returncode, 2)
        self.assertEqual(self.create("vm-a", hours="7", cap="600").returncode, 0)
        self.assertEqual(self.run_guard("charge", "0", "--why", "x", "--budget", "test").returncode, 2)
        self.assertEqual(self.run_guard("charge", "5", "--why", " ", "--budget", "test").returncode, 2)

    # ------------------------------------------------------------- #12409

    def flex(self, name, at=T0, hours="2"):
        r = self.create(name, "--flex-start", hours=hours, spot=False, at=at)
        self.assertEqual(r.returncode, 0, r.stderr)
        return [x for x in self.records() if x.get("event") == "start"][-1]

    def test_sync_bills_a_queued_flex_start_from_its_start_at_expiry(self):
        # Requested 08:00, started 09:00, deleted at the end of its run at 10:00: one hour.
        self.flex("flex-exp")
        ops = json.dumps([{"operationType": "compute.instances.deferredDelete", "status": "DONE",
                           "endTime": "2027-01-15T10:00:00Z"}])
        r = self.run_guard("sync", "--budget", "test", at=T0 + 7200, FAKE_DESCRIBE="TERMINATED",
                           FAKE_STARTED="2027-01-15T09:00:00Z", FAKE_OPS=ops)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertAlmostEqual(self.report(T0 + 7200)["spent"], 64.44, places=4)
        self.assertEqual(self.records()[-1]["run_start_epoch"], T0 + 3600)

    def test_sync_bills_a_preempted_flex_start_from_its_insert(self):
        # The machine is gone, so describe has no lastStartTimestamp; the insert's end is its start.
        self.flex("flex-pre")
        ops = json.dumps([{"operationType": "insert", "status": "DONE", "insertTime": "2027-01-15T08:00:01Z",
                           "endTime": "2027-01-15T09:00:00Z", "targetId": "111"},
                          {"operationType": "compute.instances.preempted", "status": "DONE",
                           "endTime": "2027-01-15T09:30:00Z", "targetId": "111"}])
        r = self.run_guard("sync", "--budget", "test", at=T0 + 7200, FAKE_DESCRIBE="GONE", FAKE_OPS=ops)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertAlmostEqual(self.report(T0 + 7200)["spent"], 64.44 / 2, places=4)
        self.assertEqual(self.records()[-1]["instance_id"], "111")

    def test_a_start_between_describe_and_delete_is_not_waived(self):
        # Describe says PENDING; the machine starts before the delete lands. Nothing on record yet
        # says whether it started, so the stop waives nothing and the machine stays counted.
        self.flex("flex-race")
        r = self.run_guard("stop", "flex-race", "--budget", "test", at=T0 + 1800, FAKE_DESCRIBE="PENDING")
        self.assertEqual(r.returncode, 0, r.stderr)
        last = self.records()[-1]
        self.assertEqual((last["event"], last["reason"]), ("pending", "start-unknown"))
        rep = self.report(T0 + 1800)
        self.assertEqual(rep["spent"], 0)
        self.assertEqual([m["vm"] for m in rep["running_machines"]], ["flex-race"])
        self.assertAlmostEqual(rep["running_committed"], 64.44 * 4, places=4)
        # Google's records then show it started at 08:25 and the delete ended it at 08:30.
        ops = json.dumps([{"operationType": "insert", "status": "DONE", "insertTime": "2027-01-15T08:00:01Z",
                           "endTime": "2027-01-15T08:25:00Z"},
                          {"operationType": "delete", "status": "DONE", "endTime": "2027-01-15T08:30:00Z"}])
        r = self.run_guard("sync", "--budget", "test", at=T0 + 2400, FAKE_DESCRIBE="GONE", FAKE_OPS=ops)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertAlmostEqual(self.report(T0 + 2400)["spent"], 64.44 * 300 / 3600, places=4)
        self.assertEqual(self.records()[-1]["reason"], "gone")

    def test_a_start_seen_after_the_delete_settles_the_stop(self):
        self.flex("flex-race2")
        started = json.dumps({"FAKE_OPS": json.dumps([
            {"operationType": "insert", "status": "DONE", "insertTime": "2027-01-15T08:00:01Z",
             "endTime": "2027-01-15T08:20:00Z"},
            {"operationType": "delete", "status": "DONE", "endTime": "2027-01-15T08:29:00Z"}])})
        r = self.run_guard("stop", "flex-race2", "--budget", "test", at=T0 + 1800, FAKE_DESCRIBE="PENDING",
                           FAKE_AFTER_DELETE=started)
        self.assertEqual(r.returncode, 0, r.stderr)
        last = self.records()[-1]
        self.assertEqual((last["event"], last["reason"]), ("stop", "deleted"))
        self.assertAlmostEqual(self.report(T0 + 1800)["spent"], 64.44 * 540 / 3600, places=4)

    def test_missing_timestamps_keep_the_machine_counted_until_expiry(self):
        self.flex("flex-blank")
        r = self.run_guard("sync", "--budget", "test", at=T0 + 3600, FAKE_DESCRIBE="GONE")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(self.report(T0 + 3600)["finished"], 0)
        # An insert with no outcome (still open, or its end aged out) is not evidence either.
        ops = json.dumps([{"operationType": "insert", "status": "RUNNING", "insertTime": "2027-01-15T08:00:01Z"}])
        self.run_guard("sync", "--budget", "test", at=T0 + 3600, FAKE_DESCRIBE="GONE", FAKE_OPS=ops)
        self.assertEqual(self.report(T0 + 3600)["finished"], 0)
        # After its expiry (2 h queue + 2 h run) it settles at the expiry, at the most it could cost.
        self.run_guard("sync", "--budget", "test", at=T0 + 5 * 3600, FAKE_DESCRIBE="GONE", FAKE_OPS=ops)
        rep = self.report(T0 + 5 * 3600)
        self.assertAlmostEqual(rep["spent"], 64.44 * 4, places=4)
        self.assertEqual(self.records()[-1]["reason"], "gone-at-expiry")

    def test_an_unsettled_delete_settles_at_the_delete_after_expiry(self):
        self.flex("flex-gap")
        self.run_guard("stop", "flex-gap", "--budget", "test", at=T0 + 1800, FAKE_DESCRIBE="PENDING")
        self.run_guard("sync", "--budget", "test", at=T0 + 5 * 3600, FAKE_DESCRIBE="GONE")
        self.assertAlmostEqual(self.report(T0 + 5 * 3600)["spent"], 64.44 / 2, places=4)

    def test_a_reused_name_does_not_lend_its_start_to_an_older_flex_request(self):
        # Our request (instance 111) ended with an error at 08:30. A new machine with the same name
        # (instance 222) started at 09:00 and still runs.
        self.flex("flex-reuse")
        ops = json.dumps([
            {"operationType": "insert", "status": "DONE", "insertTime": "2027-01-15T08:00:01Z",
             "endTime": "2027-01-15T08:30:00Z", "targetId": "111",
             "error": {"errors": [{"code": "ZONE_RESOURCE_POOL_EXHAUSTED"}]}},
            {"operationType": "insert", "status": "DONE", "insertTime": "2027-01-15T08:59:00Z",
             "endTime": "2027-01-15T09:00:00Z", "targetId": "222"},
            {"operationType": "delete", "status": "DONE", "endTime": "2027-01-15T10:00:00Z", "targetId": "222"}])
        r = self.run_guard("sync", "--budget", "test", at=T0 + 4 * 3600, FAKE_DESCRIBE="RUNNING",
                           FAKE_CREATED="2027-01-15T09:00:00Z", FAKE_ID="222", FAKE_OPS=ops)
        self.assertEqual(r.returncode, 0, r.stderr)
        stop = [x for x in self.records() if x.get("event") == "stop"][-1]
        self.assertEqual((stop["reason"], stop["cost"], stop["instance_id"]), ("never-started", 0.0, "111"))

    def test_aged_out_operations_settle_from_the_audit_log(self):
        self.flex("flex-old")
        name = "projects/openagentsgemini/zones/us-central1-b/instances/flex-old"
        audit = json.dumps([
            {"timestamp": "2027-01-15T08:00:02Z", "protoPayload": {"methodName": "v1.compute.instances.insert",
             "resourceName": name}, "operation": {"first": True}, "resource": {"labels": {"instance_id": "111"}}},
            {"timestamp": "2027-01-15T09:00:00Z", "protoPayload": {"methodName": "v1.compute.instances.insert",
             "resourceName": name}, "operation": {"last": True}, "resource": {"labels": {"instance_id": "111"}}},
            {"timestamp": "2027-01-15T10:00:00Z", "protoPayload": {"methodName": "v1.compute.instances.delete",
             "resourceName": name}, "operation": {"last": True}, "resource": {"labels": {"instance_id": "111"}}},
            # A longer name that shares the prefix is another machine.
            {"timestamp": "2027-01-15T08:10:00Z", "protoPayload": {"methodName": "v1.compute.instances.insert",
             "resourceName": name + "-2"}, "operation": {"last": True}}])
        r = self.run_guard("sync", "--budget", "test", at=T0 + 4 * 3600, FAKE_DESCRIBE="GONE", FAKE_AUDIT=audit)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertAlmostEqual(self.report(T0 + 4 * 3600)["spent"], 64.44, places=4)

    def test_audit_log_shows_a_cancelled_request_never_ran(self):
        # The disputed record's shape: a delete cancels the queued insert, which ends with an error.
        self.flex("flex-cxl")
        name = "projects/openagentsgemini/zones/us-central1-b/instances/flex-cxl"
        audit = json.dumps([
            {"timestamp": "2027-01-15T08:00:02Z", "protoPayload": {"methodName": "v1.compute.instances.insert",
             "resourceName": name}, "operation": {"first": True}},
            {"timestamp": "2027-01-15T09:50:00Z", "protoPayload": {"methodName": "v1.compute.instances.delete",
             "resourceName": name}, "operation": {"first": True}},
            {"timestamp": "2027-01-15T10:05:00Z", "severity": "ERROR", "protoPayload": {
             "methodName": "v1.compute.instances.insert", "resourceName": name,
             "status": {"code": 1, "message": "OPERATION_CANCELED_BY_USER"}}, "operation": {"last": True}},
            {"timestamp": "2027-01-15T10:05:01Z", "protoPayload": {"methodName": "v1.compute.instances.delete",
             "resourceName": name}, "operation": {"last": True}}])
        self.run_guard("sync", "--budget", "test", at=T0 + 3 * 3600, FAKE_DESCRIBE="GONE", FAKE_AUDIT=audit)
        self.assertEqual(self.records()[-1]["reason"], "never-started")
        self.assertEqual(self.report(T0 + 3 * 3600)["spent"], 0)

    def test_adoption_bills_a_flex_start_machine_from_its_start(self):
        listing = json.dumps([{"id": "333", "name": "flex-lost", "zone": "projects/p/zones/us-central1-b",
                               "machineType": "zones/us-central1-b/machineTypes/a4-highgpu-8g",
                               "status": "RUNNING", "creationTimestamp": "2027-01-15T08:00:00Z",
                               "lastStartTimestamp": "2027-01-15T09:00:00Z",
                               "labels": {"budget": "test", "expires-at": "2027-01-15t12-00-00z"},
                               "scheduling": {"provisioningModel": "FLEX_START"}}])
        r = self.run_guard("sync", "--budget", "test", at=T0 + 2 * 3600, FAKE_LIST=listing)
        self.assertEqual(r.returncode, 0, r.stderr)
        start = [x for x in self.records() if x.get("event") == "start"][-1]
        self.assertEqual((start["run_start_epoch"], start["instance_id"]), (T0 + 3600, "333"))
        self.assertAlmostEqual(self.report(T0 + 2 * 3600)["running"], 64.44, places=4)
        ops = json.dumps([{"operationType": "delete", "status": "DONE", "endTime": "2027-01-15T10:00:00Z",
                           "targetId": "333"}])
        self.run_guard("sync", "--budget", "test", at=T0 + 3 * 3600, FAKE_DESCRIBE="GONE", FAKE_OPS=ops)
        self.assertAlmostEqual(self.report(T0 + 3 * 3600)["spent"], 64.44, places=4)

    def test_correct_appends_a_correction_and_never_rewrites_history(self):
        start = self.flex("flex-old-est")
        # The old sync settled it from the queued request: 08:00 to 10:00, $128.88.
        self.ledger.write_text(self.ledger.read_text() + json.dumps(
            {"event": "stop", "id": start["id"], "vm": "flex-old-est", "stop_epoch": T0 + 7200,
             "stop": "2027-01-15T10:00:00Z", "reason": "gone", "cost": 128.88}) + "\n")
        self.assertAlmostEqual(self.report(T0 + 7200)["spent"], 128.88, places=4)
        before = self.ledger.read_bytes()
        r = self.run_guard("correct", start["id"][:8], "--budget", "test", "--run-start", "2027-01-15T09:00:00Z",
                           "--end", "2027-01-15T10:00:00Z", "--why", "billed from its queued request",
                           at=T0 + 7300)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertTrue(self.ledger.read_bytes().startswith(before))
        last = self.records()[-1]
        self.assertEqual((last["event"], last["previous_cost"], last["cost"]), ("correction", 128.88, 64.44))
        self.assertAlmostEqual(self.report(T0 + 7300)["spent"], 64.44, places=4)
        # The same correction again appends nothing.
        n = len(self.records())
        r = self.run_guard("correct", "flex-old-est", "--budget", "test", "--run-start", "2027-01-15T09:00:00Z",
                           "--end", "2027-01-15T10:00:00Z", "--why", "again", at=T0 + 7400)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(len(self.records()), n)
        # From Google's records: the insert was cancelled, so it never ran.
        ops = json.dumps([{"operationType": "insert", "status": "DONE", "insertTime": "2027-01-15T08:00:01Z",
                           "endTime": "2027-01-15T10:00:00Z", "error": {"errors": [{"code": "X"}]}}])
        r = self.run_guard("correct", start["id"], "--budget", "test", "--from-google", "--why", "audit",
                           at=T0 + 7500, FAKE_DESCRIBE="GONE", FAKE_OPS=ops)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(self.report(T0 + 7500)["spent"], 0)
        # A running machine, or no choice of correction, is refused.
        self.flex("flex-live", at=T0 + 7600)
        self.assertEqual(self.run_guard("correct", "flex-live", "--budget", "test", "--never-started",
                                        "--why", "x", at=T0 + 7600).returncode, 2)
        self.assertEqual(self.run_guard("correct", start["id"], "--budget", "test", "--why", "x").returncode, 2)


    # Instance 111's request failed at 08:05; instance 222 took the name at 08:10, inside the
    # 30-minute start window, and ran until 09:00 (review R2, #12411).
    RAPID_OPS = json.dumps([
        {"operationType": "insert", "status": "DONE", "insertTime": "2027-01-15T08:00:01Z",
         "endTime": "2027-01-15T08:05:00Z", "targetId": "111",
         "error": {"errors": [{"code": "ZONE_RESOURCE_POOL_EXHAUSTED"}]}},
        {"operationType": "insert", "status": "DONE", "insertTime": "2027-01-15T08:09:30Z",
         "endTime": "2027-01-15T08:10:00Z", "targetId": "222"}])
    REPLACEMENT = {"FAKE_ID": "222", "FAKE_CREATED": "2027-01-15T08:10:00Z",
                   "FAKE_STARTED": "2027-01-15T08:10:00Z"}

    def test_a_rapidly_reused_name_lends_no_timestamps(self):
        self.flex("flex-rapid")
        r = self.run_guard("sync", "--budget", "test", at=T0 + 4 * 3600, FAKE_DESCRIBE="TERMINATED",
                           FAKE_STOPPED="2027-01-15T09:00:00Z", FAKE_OPS=self.RAPID_OPS, **self.REPLACEMENT)
        self.assertEqual(r.returncode, 0, r.stderr)
        stop = [x for x in self.records() if x.get("event") == "stop"][-1]
        self.assertEqual((stop["reason"], stop["cost"], stop["instance_id"], stop["stop_epoch"]),
                         ("never-started", 0.0, "111", T0 + 300))
        self.assertEqual(self.report(T0 + 4 * 3600)["spent"], 0)

    def test_settle_rejects_another_instances_snapshot(self):
        # The review's fixture, through the module's own functions: the record knows its ID.
        spec = importlib.util.spec_from_file_location("cloud_budget_r2", SCRIPT)
        budget = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(budget)
        record = {"id": "old-ledger-entry", "instance_id": "111", "vm": "reused", "project": "fixture",
                  "zone": "fixture", "start_epoch": T0, "provisioning": "flex-start",
                  "price_per_hour": 64.44, "expires_epoch": T0 + 7200}
        replacement = {"id": "222", "status": "TERMINATED", "creationTimestamp": "2027-01-15T08:10:00Z",
                       "lastStartTimestamp": "2027-01-15T08:10:00Z",
                       "lastStopTimestamp": "2027-01-15T09:00:00Z"}
        reply = subprocess.CompletedProcess([], 0, json.dumps(replacement), "")
        with patch.object(budget, "gcloud", return_value=reply):
            seen = budget.describe(record)
        self.assertEqual(seen, {"status": "GONE", "other_id": "222"})
        events = [{"kind": "insert", "at": T0 + 300, "begun": T0 + 1, "done": True, "ok": False,
                   "instance_id": "111"}]
        # Settlement checks the snapshot itself, so even the unvalidated one lends nothing.
        raw = {"status": "TERMINATED", "id": "222", "created": T0 + 600, "last_start": T0 + 600,
               "last_stop": T0 + 3600}
        for snapshot in (seen, raw):
            done = budget.settle(record, snapshot, events)
            self.assertEqual((done["reason"], done["instance_id"], done["stop_epoch"]),
                             ("never-started", "111", T0 + 300))
            self.assertEqual(budget.stop_record(record, done)["cost"], 0)

    def test_stop_never_deletes_a_replacement(self):
        self.flex("flex-swap")
        r = self.run_guard("stop", "flex-swap", "--budget", "test", at=T0 + 1800, FAKE_DESCRIBE="RUNNING",
                           FAKE_OPS=self.RAPID_OPS, **self.REPLACEMENT)
        self.assertEqual(r.returncode, 1)
        self.assertIn("didn't delete it", r.stderr)
        self.assertFalse([c for c in self.calls() if c.startswith('["compute", "instances", "delete"')])
        stop = self.records()[-1]
        self.assertEqual((stop["event"], stop["reason"], stop["cost"], stop["instance_id"]),
                         ("stop", "never-started", 0.0, "111"))
        # A standard machine that a delete ended at 08:05 is settled there, and its replacement stays.
        self.assertEqual(self.create("vm-swap", hours="2").returncode, 0)
        ops = json.dumps([
            {"operationType": "insert", "status": "DONE", "insertTime": "2027-01-15T08:00:01Z",
             "endTime": "2027-01-15T08:00:30Z", "targetId": "333"},
            {"operationType": "delete", "status": "DONE", "endTime": "2027-01-15T08:05:00Z", "targetId": "333"},
            {"operationType": "insert", "status": "DONE", "insertTime": "2027-01-15T08:09:30Z",
             "endTime": "2027-01-15T08:10:00Z", "targetId": "444"}])
        r = self.run_guard("stop", "vm-swap", "--budget", "test", at=T0 + 1800, FAKE_DESCRIBE="RUNNING",
                           FAKE_OPS=ops, FAKE_ID="444", FAKE_CREATED="2027-01-15T08:10:00Z")
        self.assertEqual(r.returncode, 1)
        self.assertFalse([c for c in self.calls() if c.startswith('["compute", "instances", "delete"')])
        stop = self.records()[-1]
        self.assertEqual((stop["event"], stop["instance_id"]), ("stop", "333"))
        self.assertAlmostEqual(stop["cost"], A4_SPOT * 5 / 60, places=4)
        # With no record of when it ended, it stays counted and nothing is deleted.
        self.assertEqual(self.create("vm-swap2", hours="2").returncode, 0)
        r = self.run_guard("stop", "vm-swap2", "--budget", "test", at=T0 + 1800, FAKE_DESCRIBE="RUNNING",
                           FAKE_ID="444", FAKE_CREATED="2027-01-15T09:00:00Z")
        self.assertEqual(r.returncode, 1)
        self.assertEqual(self.records()[-1]["event"], "start")
        self.assertFalse([c for c in self.calls() if c.startswith('["compute", "instances", "delete"')])

if __name__ == "__main__":
    unittest.main()
