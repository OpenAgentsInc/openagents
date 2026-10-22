"""Offline controls for the two Maya services; no cloud or model calls.

Run: python3 scripts/pylon/test_maya_supervisor.py
"""
import argparse
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import Mock, patch


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


HERE = Path(__file__).parent
m = module("maya_supervisor", HERE / "maya-supervisor.py")
g = module("maya_budget_tests", HERE.parent / "dev/cloud-budget.py")
NOW = 1800000000.0


def slot(number):
    return {"vm": m.NAMES[number - 1], "allocation": str(number) * 32,
            "instance_id": str(number * 111), "disk_id": str(number * 777),
            "producer": str(number) * 64, "expires": NOW + 86400,
            "run_id": "old-run", "installed": True, "pending": None}


def instance(s, status="RUNNING"):
    return {"id": s["instance_id"], "name": s["vm"], "status": status,
            "creationTimestamp": "2027-01-15T08:00:00Z",
            "labels": {"run-id": s["run_id"], "budget": m.BUDGET, "issue": "12517",
                       "pylon-group": "maya-consumer-q-" + s["vm"][-1]},
            "disks": [{"boot": True, "autoDelete": True, "source": "p/z/disks/" + s["vm"]}]}


def ledger(*extra):
    return g.Ledger([{"event": "budget", "cap": 3000}, {"event": "reserve", "amount": 300}, *extra])


def start(name="oa-maya-old", ident="a", hours=1, price=1, run="new-run"):
    return {"event": "start", "id": ident, "vm": name, "run_id": run,
            "start_epoch": NOW, "expires_epoch": NOW + hours * 3600, "price_per_hour": price}


class Lifecycle:
    def __init__(self):
        self.state = {"slots": [slot(1), slot(2)], "owners": ["old-run", "new-run"], "run_id": "new-run"}
        self.done = False
        self.seen = {s["vm"]: instance(s) for s in self.state["slots"]}
        self.calls = []
        self.fail = None

    def completion(self):
        return self.done

    def save(self):
        pass

    def initial_storage(self):
        self.calls.append("initial-storage")

    def describe(self, s):
        return self.seen[s["vm"]]

    def cleanup(self, s):
        self.calls.append(("cleanup", s["vm"]))
        if self.fail == s["vm"]:
            raise m.Refused("cleanup unavailable")
        self.seen[s["vm"]] = None
        s.update(allocation=None, instance_id=None, disk_id=None, installed=False)

    def create(self, s):
        self.calls.append(("create", s["vm"]))
        assert sum(v is not None for v in self.seen.values()) < 2
        s.update(allocation="new-allocation", instance_id="999", disk_id="888",
                 expires=NOW + 86400, run_id="new-run")
        self.seen[s["vm"]] = instance(s)

    def install(self, s, number):
        self.calls.append(("install", s["vm"], number))
        s["installed"] = True

    def adopt(self, s):
        self.calls.append(("adopt", s["vm"]))

    def health(self, s, now):
        self.calls.append(("health", s["vm"]))

    def recover(self, s):
        raise m.Refused("pending reservation unresolved")


class Controls(unittest.TestCase):
    def setUp(self):
        scratch = os.environ.get("OPENAGENTS_SCRATCH")
        if not scratch:
            scratch = subprocess.check_output(["openagents", "scratch"], text=True).strip()
        self.temp = tempfile.TemporaryDirectory(dir=scratch)
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name)
        self.config = {"storage_per_hour": 0.5}
        self.backend = object.__new__(m.Backend)
        self.backend.home = self.home
        self.backend.tools = self.home / "tools"
        self.backend.guard = g
        self.backend.config = {"gcloud": "/gcloud", "gh": "/gh", "ssh_user": "operator", "ssh_key": "/operator-key", **self.config}
        self.backend.state = Lifecycle().state
        self.backend.env = {}
        self.backend.save = Mock()

    def test_healthy_services_adopt_without_restarting_or_creating(self):
        b = Lifecycle()
        self.assertFalse(m.tick(b, b.state, NOW))
        self.assertEqual(b.calls, ["initial-storage", ("adopt", m.NAMES[0]), ("health", m.NAMES[0]),
                                   ("adopt", m.NAMES[1]), ("health", m.NAMES[1])])

    def test_spot_loss_recreates_only_missing_slot(self):
        b = Lifecycle()
        b.seen[m.NAMES[0]] = None
        m.tick(b, b.state, NOW)
        self.assertIn(("cleanup", m.NAMES[0]), b.calls)
        self.assertIn(("create", m.NAMES[0]), b.calls)
        self.assertNotIn(("create", m.NAMES[1]), b.calls)

    def test_stopped_spot_is_deleted_before_create(self):
        b = Lifecycle()
        b.seen[m.NAMES[0]]["status"] = "TERMINATED"
        m.tick(b, b.state, NOW)
        self.assertLess(b.calls.index(("cleanup", m.NAMES[0])), b.calls.index(("create", m.NAMES[0])))

    def test_renewal_starts_two_hours_before_expiry(self):
        b = Lifecycle()
        b.state["slots"][0]["expires"] = NOW + 7200
        m.tick(b, b.state, NOW)
        self.assertIn(("create", m.NAMES[0]), b.calls)

    def test_unknown_github_freezes_allocations_and_adoption_but_cleans_lost_slot(self):
        b = Lifecycle()
        b.done = None
        b.seen[m.NAMES[0]] = None
        self.assertFalse(m.tick(b, b.state, NOW))
        self.assertEqual(b.calls, [("cleanup", m.NAMES[0])])

    def test_unknown_github_cleans_stopped_and_expired_slots_without_new_spend(self):
        b = Lifecycle()
        b.done = None
        b.seen[m.NAMES[0]]["status"] = "TERMINATED"
        b.state["slots"][1]["expires"] = NOW
        self.assertFalse(m.tick(b, b.state, NOW))
        self.assertEqual(b.calls, [("cleanup", n) for n in m.NAMES])

    def test_unknown_github_does_not_renew_before_current_deadline(self):
        b = Lifecycle()
        b.done = None
        b.state["slots"][0]["expires"] = NOW + 60
        self.assertFalse(m.tick(b, b.state, NOW))
        self.assertEqual(b.calls, [])

    def test_completion_deletes_both_without_creating(self):
        b = Lifecycle()
        b.done = True
        self.assertTrue(m.tick(b, b.state, NOW))
        self.assertEqual(b.calls, [("cleanup", n) for n in m.NAMES])
        self.assertEqual(b.state["phase"], "completed")

    def test_stop_works_when_github_unavailable(self):
        b = Lifecycle()
        b.done = None
        self.assertTrue(m.tick(b, b.state, NOW, stopping=True))
        self.assertEqual(b.state["phase"], "stopped")

    def test_completion_tries_both_even_if_first_cleanup_fails(self):
        b = Lifecycle()
        b.done, b.fail = True, m.NAMES[0]
        with self.assertRaises(m.Refused):
            m.tick(b, b.state, NOW)
        self.assertIn(("cleanup", m.NAMES[1]), b.calls)
        self.assertNotEqual(b.state.get("phase"), "completed")

    def test_reused_name_or_foreign_run_blocks_mutation(self):
        for field in ("id", "run", "group", "autoDelete"):
            b = Lifecycle()
            info = b.seen[m.NAMES[0]]
            if field == "id":
                info["id"] = "replacement"
            elif field == "run":
                info["labels"]["run-id"] = "foreign"
            elif field == "group":
                info["labels"]["pylon-group"] = "qualification"
            else:
                info["disks"][0]["autoDelete"] = False
            with self.subTest(field=field), self.assertRaises(m.Refused):
                m.tick(b, b.state, NOW)
            self.assertFalse(any(c[0] in ("create", "cleanup") for c in b.calls if isinstance(c, tuple)))

    def test_pending_unknown_never_reserves_again(self):
        b = Lifecycle()
        b.state["slots"][0]["pending"] = {"at": NOW}
        with self.assertRaises(m.Refused):
            m.tick(b, b.state, NOW)
        self.assertFalse(any(c[0] == "create" for c in b.calls if isinstance(c, tuple)))

    def test_maya_does_not_pay_global_reserve_or_qualification_spend(self):
        records = ledger(start(price=10), start("qualification", "q", price=200),
                         {"event": "charge", "amount": 50, "why": "#12403 W16 transfer"},
                         {"event": "charge", "amount": 12, "why": "Maya storage budget estimate"})
        self.assertEqual(m.maya_commitment(g, records, NOW), 22)

    def test_historical_settlements_and_live_commitments_both_count(self):
        records = ledger(start(ident="old", price=5), {"event": "stop", "id": "old", "stop_epoch": NOW + 3600},
                         start(ident="live", hours=24, price=2))
        self.assertEqual(m.maya_commitment(g, records, NOW), 53)

    def test_ambiguous_charge_or_corruption_blocks_new_admission(self):
        with self.assertRaises(m.Refused):
            m.safe_hours(self.config, g, ledger({"event": "charge", "amount": 5, "why": "unknown transfer"}), NOW)
        with self.assertRaises(g.Refused):
            m.safe_hours(self.config, g, g.Ledger(corrupt=[1]), NOW)

    def test_both_caps_include_new_compute_and_storage(self):
        rate = g.price_for(g.load_prices(), "g4-standard-48", "spot")[0] + 0.5
        for name, price in (("oa-maya-old", 970), ("qualification", 2670)):
            records = ledger(start(name, hours=1, price=price))
            hours = m.safe_hours(self.config, g, records, NOW)
            self.assertLess(hours, 24)
            self.assertLessEqual(hours * rate, 30)
            self.assertGreaterEqual(hours, 1)
        with self.assertRaises(m.Refused):
            m.safe_hours(self.config, g, ledger(start(price=999)), NOW)

    def test_changed_global_reserve_is_not_silently_accepted(self):
        with self.assertRaises(m.Refused):
            m.safe_hours(self.config, g, ledger({"event": "reserve", "amount": 200}), NOW)

    def test_budget_argument_precedes_gcloud_extra_separator(self):
        self.backend.command = Mock(return_value="")
        self.backend.guard_call("create", m.NAMES[0], "--", "--boot-disk-size=1200GB")
        argv = self.backend.command.call_args.args[0]
        self.assertLess(argv.index("--budget"), argv.index("--"))

    def test_open_issue_null_reason_is_valid_but_not_complete(self):
        rows = {"i" + str(n): {"state": "CLOSED", "stateReason": "COMPLETED"} for n in m.COMPLETION}
        self.backend.command = Mock(side_effect=lambda _: json.dumps({"data": {"repository": rows}}))
        self.assertTrue(self.backend.completion())
        rows["i12502"] = {"state": "OPEN", "stateReason": None}
        self.assertIs(self.backend.completion(), False)
        rows["i12502"] = {"state": "CLOSED", "stateReason": "NOT_PLANNED"}
        self.assertIs(self.backend.completion(), False)
        rows["i12502"] = None
        self.assertIsNone(self.backend.completion())

    def test_storage_lost_reply_and_restart_use_same_charge(self):
        records = ledger()
        self.backend.records = lambda: records
        def charge(*args):
            records.append({"event": "charge", "amount": float(args[1]), "why": args[3], "estimate": True})
            raise subprocess.TimeoutExpired("guard", 300)
        self.backend.guard_call = Mock(side_effect=charge)
        with self.assertRaises(subprocess.TimeoutExpired):
            self.backend.storage("reservation=stable", 24)
        self.backend.storage("reservation=stable", 24)
        self.assertEqual(self.backend.guard_call.call_count, 1)
        self.assertEqual(g.ledger_charges(records), 12)

    def test_initial_storage_allowance_is_booked_once_per_allocation(self):
        records = ledger()
        self.backend.records = lambda: records
        self.backend.config["slots"] = [slot(1), slot(2)]
        def charge(*args):
            records.append({"event": "charge", "amount": float(args[1]), "why": args[3], "estimate": True})
        self.backend.guard_call = Mock(side_effect=charge)
        self.backend.initial_storage()
        self.backend.initial_storage()
        self.assertEqual(self.backend.guard_call.call_count, 2)
        self.assertEqual(g.ledger_charges(records), 24)

    def test_definite_failed_create_preserves_intended_storage_reservation(self):
        s = slot(1)
        s.update(pending={"at": NOW, "run_id": "new-run"}, reservation={"storage": "same", "hours": 24})
        failed = start(s["vm"], run="new-run")
        records = ledger(failed, {"event": "stop", "id": "a", "stop_epoch": NOW, "reason": "create-failed"})
        self.backend.records = lambda: records
        with self.assertRaises(m.Refused):
            self.backend.recover(s)
        self.assertIsNone(s["pending"])
        self.assertEqual(s["reservation"], {"storage": "same", "hours": 24})
        s["pending"] = {"at": NOW + 60, "run_id": "new-run"}
        with self.assertRaises(m.Refused):
            self.backend.recover(s)
        self.assertEqual(s["reservation"]["storage"], "same")

    def test_failed_create_then_controller_restart_reuses_storage_booking(self):
        s = slot(1)
        s.update(allocation=None, instance_id=None, disk_id=None, installed=False)
        self.backend.state["slots"][0] = s
        records, at, attempts = ledger(), [NOW], []
        self.backend.records = lambda: records
        self.backend.cloud = Mock(side_effect=lambda *args: "[]" if args[2] == "list" else '{"id":"777"}')
        found = instance(slot(1))
        found["labels"]["run-id"] = "new-run"
        self.backend.describe = Mock(return_value=found)
        def guard(*args, **kwargs):
            if args[0] == "charge":
                records.append({"event": "charge", "amount": float(args[1]), "why": args[3], "estimate": True})
            elif args[0] == "create":
                ident = "attempt" + str(len(attempts))
                attempts.append(args)
                row = start(s["vm"], ident=ident, hours=24)
                row["start_epoch"], row["expires_epoch"] = at[0], at[0] + 86400
                records.append(row)
                if len(attempts) == 1:
                    records.append({"event": "stop", "id": ident, "stop_epoch": at[0], "reason": "create-failed"})
                    raise m.Refused("no spot capacity")
        self.backend.guard_call = Mock(side_effect=guard)
        with patch.object(m.time, "time", side_effect=lambda: at[0]):
            with self.assertRaises(m.Refused):
                self.backend.create(s)
            intended = copy.deepcopy(s["reservation"])
            self.backend.state = json.loads(json.dumps(self.backend.state))
            s = self.backend.state["slots"][0]
            self.assertEqual(s["reservation"], intended)
            at[0] += 60
            found["creationTimestamp"] = "2027-01-15T08:01:00Z"
            self.backend.create(s)
        self.assertEqual(g.ledger_charges(records), 12)
        self.assertEqual(len(attempts), 2)
        self.assertEqual(s["allocation"], "attempt1")
        self.assertIsNone(s["reservation"])

    def test_unrecognized_third_maya_blocks_even_guard_sync(self):
        self.backend.cloud = Mock(return_value='[{"id":"foreign","name":"oa-maya-other"}]')
        self.backend.guard_call = Mock()
        with self.assertRaises(m.Refused):
            self.backend.create(slot(1))
        self.backend.guard_call.assert_not_called()

    def test_lost_create_reply_recovers_one_generation(self):
        s = slot(1)
        s.update(pending={"at": NOW, "run_id": "old-run"}, reservation={"storage": "same", "hours": 24})
        self.backend.records = lambda: ledger(start(s["vm"], run="old-run", hours=24))
        self.backend.describe = Mock(return_value=instance(s))
        self.backend.cloud = Mock(return_value=json.dumps({"id": s["disk_id"]}))
        self.backend.recover(s)
        self.assertIsNone(s["pending"])
        self.assertIsNone(s["reservation"])
        self.assertEqual(s["allocation"], "a")
        self.assertEqual(s["instance_id"], "111")

    def test_create_reclaimed_before_reply_settles_before_next_lifetime(self):
        s = slot(1)
        s.update(allocation=None, instance_id=None, disk_id=None,
                 pending={"at": NOW, "run_id": "new-run"}, reservation={"storage": "paid-lifetime", "hours": 24})
        records = ledger(start(s["vm"], run="new-run", hours=24))
        self.backend.records = lambda: records
        self.backend.describe = Mock(return_value=None)
        def sync(*args):
            if args[0] == "sync":
                records.append({"event": "stop", "id": "a", "stop_epoch": NOW + 60, "reason": "preempted"})
        self.backend.guard_call = Mock(side_effect=sync)
        self.backend.disk_absent = Mock()
        with self.assertRaises(m.Refused):
            self.backend.recover(s)
        self.assertIsNone(s["pending"])
        self.assertIsNone(s["reservation"])
        self.backend.disk_absent.assert_called_once_with(s)
        self.assertIn(unittest.mock.call("stop", s["vm"], "--record", "a"), self.backend.guard_call.call_args_list)

    def test_missing_unsettled_create_does_not_free_booked_lifetime(self):
        s = slot(1)
        pending = {"at": NOW, "run_id": "new-run"}
        s.update(allocation=None, instance_id=None, disk_id=None, pending=pending,
                 reservation={"storage": "same", "hours": 24})
        self.backend.records = lambda: ledger(start(s["vm"], run="new-run", hours=24))
        self.backend.describe = Mock(return_value=None)
        self.backend.guard_call = Mock()
        with self.assertRaises(m.Refused):
            self.backend.recover(s)
        self.assertEqual(s["pending"], pending)
        self.assertEqual(s["reservation"]["storage"], "same")

    def test_cleanup_foreign_disk_does_not_call_guard(self):
        s = slot(1)
        self.backend.describe = Mock(return_value=instance(s))
        self.backend.cloud = Mock(return_value=json.dumps({"id": "999", "name": s["vm"]}))
        self.backend.guard_call = Mock()
        with self.assertRaises(m.Refused):
            self.backend.cleanup(s)
        self.backend.guard_call.assert_not_called()

    def test_unconfirmed_auto_deleted_disk_keeps_slot_blocked(self):
        s = slot(1)
        self.backend.describe = Mock(return_value=None)
        self.backend.guard_call = Mock()
        with patch.object(m.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "777", "")):
            with self.assertRaises(m.Refused):
                self.backend.cleanup(s)
        self.assertIsNotNone(s["allocation"])

    def test_trusted_guest_hostkeys_bind_exact_native_id_and_reject_injection(self):
        attrs = {"queryValue": {"items": [{"namespace": "hostkeys", "key": "ssh-ed25519", "value": "aG9zdGtleQ=="}]}}
        self.assertEqual(m.host_keys(attrs, "111"), "compute.111 ssh-ed25519 aG9zdGtleQ==\n")
        attrs["queryValue"]["items"][0]["value"] += "\nforeign hostkey"
        with self.assertRaises(m.Refused):
            m.host_keys(attrs, "111")
        with self.assertRaises(m.Refused):
            m.host_keys({}, "111")

    def test_busy_or_loading_does_not_restart_but_terminal_failure_is_bounded(self):
        s = slot(1)
        live = {"at": NOW, "group": {"build": m.WORKER_HASH, "pinned": True, "ready": False, "busy": True,
                                     "generation": 2, "failed": None}}
        self.backend.transport = Mock(return_value=(["ssh"], [], "identity && "))
        (self.home / "bundle").mkdir()
        m.atomic(self.home / "bundle/launch-1.json", {"settings": {"allow": []}})
        observed = {"live": live, "settings": {"allow": []}, "identity": {"ok": True, "data": {"pubkey": s["producer"]}}}
        self.backend.command = Mock(side_effect=lambda argv: json.dumps(observed) if "sudo python3" in argv[-1] else "")
        self.backend.health(s, NOW)
        self.assertEqual(self.backend.command.call_count, 1)
        live["group"]["failed"] = "terminal"
        self.backend.health(s, NOW)
        self.assertEqual(s["restarts"], 1)
        with self.assertRaises(m.Refused):
            self.backend.health(s, NOW)
        s["restarts"] = 3
        with self.assertRaises(m.Refused):
            self.backend.health(s, NOW + 3601)

    def test_stale_health_never_restarts(self):
        s = slot(1)
        self.backend.transport = Mock(return_value=(["ssh"], [], ""))
        (self.home / "bundle").mkdir()
        m.atomic(self.home / "bundle/launch-1.json", {"settings": {"allow": []}})
        self.backend.command = Mock(return_value=json.dumps({"live": {"at": NOW - 3600}, "settings": {"allow": []},
                                                            "identity": {"ok": True, "data": {"pubkey": s["producer"]}}}))
        with self.assertRaises(m.Refused):
            self.backend.health(s, NOW)
        self.assertEqual(self.backend.command.call_count, 1)

    def test_replacement_ssh_pins_native_host_and_checks_metadata_before_key_copy(self):
        s = slot(1)
        self.backend.describe = Mock(return_value=instance(s))
        self.backend.cloud = Mock(return_value=json.dumps({"queryValue": {"items": [
            {"namespace": "hostkeys", "key": "ssh-ed25519", "value": "aG9zdGtleQ=="}]}}))
        self.backend.command = Mock(return_value="")
        self.backend.install(s, 1)
        calls = [c.args[0] for c in self.backend.command.call_args_list]
        self.assertIn("operator@" + s["vm"], calls[0])
        self.assertIn("--plain", calls[0])
        self.assertIn("--ssh-flag=-oHostKeyAlias=compute.111", calls[0])
        self.assertIn("--ssh-flag=-oStrictHostKeyChecking=yes", calls[0])
        self.assertIn("instance/id)", calls[0][-1])
        self.assertIn(' = 111 && install -m600', calls[0][-1])
        self.assertIn("slot-1.provider.key", " ".join(calls[1]))
        self.assertIn("--scp-flag=-oStrictHostKeyChecking=yes", calls[1])
        self.assertIn("rm ~/maya-provider.key", calls[-1][-1])
        self.assertTrue(s["installed"])

    def test_provider_vault_refuses_public_mode_or_symlink(self):
        key = self.home / "key"
        key.write_text("test-only")
        key.chmod(0o644)
        with self.assertRaises(m.Refused):
            m.private(key)
        key.chmod(0o600)
        m.private(key)
        link = self.home / "linked"
        link.symlink_to(key)
        with self.assertRaises(m.Refused):
            m.private(link)

    def bundle(self):
        source = self.home / "source"
        source.mkdir()
        members = {name: b"public" for name in m.PUBLIC_MEMBERS}
        for n in (1, 2):
            launch = {"settings": {"group": {"artifact": "sha256:" + m.Q_HASH, "build": m.WORKER_HASH,
                      "profile": "consumer-q", "id": f"maya-consumer-q-{n}", "startup_timeout_ms": 3600000}}}
            members[f"launch-{n}.json"] = json.dumps(launch).encode()
        for name, data in members.items():
            (source / name).write_bytes(data)
        with tarfile.open(source / "service-files.tar.gz", "w:gz") as archive:
            for name, data in members.items():
                info = tarfile.TarInfo(name)
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
        for name in ("startup.sh", "bootstrap.sh"):
            (source / name).write_text("public bootstrap")
        (source / "install-config.sh").write_text("public install")
        (source / "install-head.sh").write_text("systemctl enable --now --no-block maya-pylon.service\n")
        controller = self.home / "controller"
        vault = controller / "vault"
        vault.mkdir(parents=True, mode=0o700)
        for n in (1, 2):
            key = vault / f"slot-{n}.provider.key"
            key.write_text("TEST-ONLY-PRIVATE-MATERIAL")
            key.chmod(0o600)
        ssh = self.home / ".ssh"
        ssh.mkdir()
        (ssh / "google_compute_engine").write_text("TEST-ONLY-SSH-KEY")
        (ssh / "google_compute_engine").chmod(0o600)
        (ssh / "google_compute_engine.pub").write_text("ssh-ed25519 aG9zdA== test-only")
        (ssh / "google_compute_known_hosts").write_text("compute.111 ssh-ed25519 aG9zdA==\ncompute.222 ssh-ed25519 aG9zdA==\n")
        deployment = self.home / "deployment.json"
        m.atomic(deployment, {"run_id": "old-run", "ssh_user": "operator", "slots": [slot(1), slot(2)]})
        return argparse.Namespace(home=controller, bundle=source, deployment=deployment)

    def test_configure_freezes_public_tools_and_never_archives_keys(self):
        args = self.bundle()
        with patch.object(m.Path, "home", return_value=self.home), patch.object(m.shutil, "which", return_value="/bin/test"):
            m.configure(args)
        config = m.load(args.home / "config.json")
        m.validate_config(config, args.home)
        m.validate_state(m.load(args.home / "state.json"), config)
        for directory in ("bundle", "tools"):
            for path in (args.home / directory).iterdir():
                self.assertNotIn(b"TEST-ONLY-PRIVATE-MATERIAL", path.read_bytes())
        verified = (args.home / "bundle/install-head-verified.sh").read_text()
        self.assertLess(verified.index("get('pubkey')"), verified.index("systemctl enable"))
        with self.assertRaises(m.Refused):
            m.configure(args)
        (args.home / "tools/cloud-budget.py").write_text("modified")
        with self.assertRaises(m.Refused):
            m.validate_config(config, args.home)

    def test_restore_requires_exact_nested_pubkey_before_enable(self):
        args = self.bundle()
        with patch.object(m.Path, "home", return_value=self.home), patch.object(m.shutil, "which", return_value="/bin/test"):
            m.configure(args)
        verified = (args.home / "bundle/install-head-verified.sh").read_text()
        code = verified.split("<<'IDENTITY'\n", 1)[1].split("\nIDENTITY", 1)[0]
        public = self.home / "public.json"
        m.atomic(public, {"ok": True, "data": {"pubkey": "expected"}})
        for expected, returncode in (("expected", 0), ("different", 1)):
            result = subprocess.run([m.sys.executable, "-O", "-", expected, str(public)], input=code,
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, returncode)

    def test_persistent_run_uses_actual_supervisor_pid_and_no_parent_run(self):
        args = self.bundle()
        with patch.object(m.Path, "home", return_value=self.home), patch.object(m.shutil, "which", return_value="/bin/test"):
            m.configure(args)
        (args.home / "handoff-confirmed").touch()
        def backend(home, config, state):
            b = Lifecycle()
            b.done, b.state = True, state
            return b
        with patch.object(m, "Backend", side_effect=backend), patch.object(m.os, "getpid", return_value=4242), \
             patch.dict(m.os.environ, {"OA_RUN_ID": "the-app-run"}), \
             patch.object(m.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "independent-run\n", "")) as run:
            self.assertEqual(m.serve(args.home), 0)
        argv = run.call_args.args[0]
        self.assertEqual(argv[-2:], ["--root", "4242"])
        self.assertNotIn("OA_RUN_ID", run.call_args.kwargs["env"])

    def test_archive_cannot_hide_different_launch_settings(self):
        args = self.bundle()
        (args.bundle / "launch-1.json").write_text("different")
        with self.assertRaises(m.Refused):
            m.configure(args)

    def test_macos_archive_drops_only_recognized_metadata_companions(self):
        args = self.bundle()
        with tarfile.open(args.bundle / "service-files.tar.gz", "w:gz") as archive:
            for name in sorted(m.PUBLIC_MEMBERS):
                for entry, data in ((name, (args.bundle / name).read_bytes()), ("._" + name, b"MAC-METADATA-DO-NOT-COPY")):
                    info = tarfile.TarInfo(entry)
                    info.size = len(data)
                    archive.addfile(info, io.BytesIO(data))
        with patch.object(m.Path, "home", return_value=self.home), patch.object(m.shutil, "which", return_value="/bin/test"):
            m.configure(args)
        with tarfile.open(args.home / "bundle/service-files.tar.gz") as archive:
            self.assertEqual({v.name for v in archive.getmembers()}, m.PUBLIC_MEMBERS)
        self.assertNotIn(b"TEST-ONLY-SSH-KEY", (args.home / "config.json").read_bytes())

    def test_archive_rejects_secret_named_member(self):
        args = self.bundle()
        with tarfile.open(args.bundle / "service-files.tar.gz", "w:gz") as archive:
            for name in sorted(m.PUBLIC_MEMBERS | {"provider.key"}):
                data = b"not a permitted public bundle"
                info = tarfile.TarInfo(name)
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
        with self.assertRaises(m.Refused):
            m.configure(args)


if __name__ == "__main__":
    unittest.main()
