#!/usr/bin/env python3
"""Supervise the two externally prepared Maya services (#12520).

configure --deployment FILE --bundle DIR [--home DIR] snapshots public inputs.
install [--home DIR] installs a launchd job; use only after deployment handoff.
status, stop, and disable accept the same --home. No command uses cloud up.
"""
from __future__ import annotations

import argparse
import base64
import datetime as dt
import fcntl
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import plistlib
import re
import shlex
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
import uuid

REPO = Path(__file__).resolve().parents[2]
DEFAULT_HOME = Path.home() / ".openagents/maya-supervisor"
COMPLETION = [12502, 12503, 12504, 12505, 12506, 12507, 12508, 12509, 12514]
NAMES = ["oa-maya-pylon-1", "oa-maya-pylon-2"]
PROJECT, ZONE, BUDGET = "openagentsgemini", "asia-northeast1-b", "deepseek-v41"
BUNDLE = ["startup.sh", "bootstrap.sh", "service-files.tar.gz", "install-config.sh",
          "install-head.sh", "launch-1.json", "launch-2.json"]
LABEL = "com.openagents.maya-supervisor"
Q_HASH = "97a7bc631ddfeaff1efe0db4fa1b5d0d0a77d74ed0844f238c258335d012360d"
WORKER_HASH = "ad55c43f69901600cf70398ab9a8a0683fe53aebe533ae5dd70ce59d152c4274"
PUBLIC_MEMBERS = {"stage-run", "head-run", "launch-1.json", "launch-2.json", "maya-pylon.service"}
TOOL_NAMES = {"maya-supervisor.py", "cloud-budget.py", "cloud-prices.json", "agent-run.py"}


class Refused(Exception):
    pass


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def atomic(path, value):
    path = Path(path)
    fd, temporary = tempfile.mkstemp(prefix=".state-", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as stream:
            json.dump(value, stream, indent=2, sort_keys=True)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        Path(temporary).unlink(missing_ok=True)


def load(path):
    return json.loads(Path(path).read_text())


def private(path, directory=False):
    info = Path(path).lstat()
    if stat.S_ISLNK(info.st_mode) or info.st_uid != os.getuid():
        raise Refused("the identity vault must be owned by this operator without links")
    if stat.S_IMODE(info.st_mode) != (0o700 if directory else 0o600):
        raise Refused("use mode 0700 for the vault and 0600 for provider keys")
    if directory != stat.S_ISDIR(info.st_mode):
        raise Refused("invalid identity vault entry")
    if not directory and not stat.S_ISREG(info.st_mode):
        raise Refused("provider keys must be regular files")


def validate_config(config, home):
    if config.get("schema") != "openagents.maya.supervisor.v1" or config.get("completion") != COMPLETION:
        raise Refused("the supervisor must watch the approved browser follow-ups")
    if [s.get("vm") for s in config.get("slots", [])] != NAMES:
        raise Refused("the supervisor needs exactly the two fixed Maya slots")
    if config.get("maya_cap") != 1000 or config.get("global_cap") != 3000 or config.get("reserve") != 300:
        raise Refused("the approved Maya/global caps and reserve must remain unchanged")
    if config.get("storage_per_hour") != 0.5:
        raise Refused("the approved disk allowance is $0.50 per VM-hour")
    if not config.get("gcloud") or not config.get("gh"):
        raise Refused("gcloud and gh must be installed")
    if set(config["files"]) != set(BUNDLE) | {"install-head-verified.sh", "ssh-authorized-keys"}:
        raise Refused("the service snapshot is incomplete")
    if set(config["tools"]) != TOOL_NAMES:
        raise Refused("the controller snapshot is incomplete")
    for name, expected in config["tools"].items():
        if digest(home / "tools" / name) != expected:
            raise Refused("a frozen controller tool changed")
    for name, expected in config["files"].items():
        if Path(name).name != name or digest(home / "bundle" / name) != expected:
            raise Refused("a frozen service file changed")
    private(home / "vault", directory=True)
    private(Path(config["ssh_key"]))
    if not re.fullmatch(r"[a-z_][a-z0-9_-]*", config["ssh_user"]):
        raise Refused("invalid operator SSH username")
    for n, slot in enumerate(config["slots"], 1):
        private(home / "vault" / f"slot-{n}.provider.key")
        if not re.fullmatch(r"[0-9a-f]{64}", slot.get("producer", "")):
            raise Refused("missing public provider identity")
        launch = load(home / "bundle" / f"launch-{n}.json")["settings"]
        group = launch["group"]
        if (group["artifact"] != "sha256:" + Q_HASH or group["build"] != WORKER_HASH
                or group["profile"] != "consumer-q" or group["id"] != f"maya-consumer-q-{n}"
                or group["startup_timeout_ms"] != 3600000):
            raise Refused("the service bundle differs from the approved runtime")


def validate_state(state, config):
    if [s.get("vm") for s in state.get("slots", [])] != NAMES:
        raise Refused("saved service slots differ from the approved slots")
    if not state.get("owners") or not all(isinstance(v, str) and v for v in state["owners"]):
        raise Refused("saved run ownership is incomplete")
    for slot, original in zip(state["slots"], config["slots"]):
        if slot.get("producer") != original["producer"]:
            raise Refused("saved provider identity changed")
        if slot.get("allocation") and (not re.fullmatch(r"[0-9]+", str(slot.get("instance_id", "")))
                                      or not re.fullmatch(r"[0-9]+", str(slot.get("disk_id", "")))
                                      or not isinstance(slot.get("expires"), (float, int))):
            raise Refused("saved native service identity is incomplete")


def maya_commitment(guard, records, at):
    """Count Maya compute and its charges, independently of the global reserve."""
    records.refuse_if_corrupt()
    total = 0.0
    for record in records:
        if record.get("event") != "charge":
            continue
        why = record.get("why", "")
        if re.search(r"\bmaya\b|oa-maya-", why, re.IGNORECASE):
            total += float(record["amount"])
        elif not re.search(r"#(?:12369|12403)\b", why):
            raise Refused("a noncompute charge needs explicit Maya or qualification attribution")
    for machine in guard.machines(records):
        if machine["vm"].startswith("oa-maya-"):
            total += (guard.cost(machine, machine["stop_epoch"]) if machine.get("stop_epoch") is not None
                      else guard.committed(machine, at))
    return total


def safe_hours(config, guard, records, at, storage_booked=False, limit=24):
    records.refuse_if_corrupt()
    if guard.ledger_cap(records) != 3000 or guard.ledger_reserve(records) != 300:
        raise Refused("global budget settings changed; no reservation made")
    price, _ = guard.price_for(guard.load_prices(), "g4-standard-48", "spot")
    total = guard.totals(guard.machines(records), at, guard.ledger_reserve(records), guard.ledger_charges(records))
    remaining = min(1000 - maya_commitment(guard, records, at),
                    3000 - total["spent"] - total["running_committed"] - total["reserve"])
    # Round down to whole minutes; the guard rounds reservations up to minutes.
    rate = price if storage_booked else price + config["storage_per_hour"]
    minutes = min(math.floor(limit * 60), math.floor(remaining / rate * 60))
    if minutes < 60:
        raise Refused("Maya or global budget cannot fund another hour")
    return minutes / 60


def verify_instance(slot, info, owners):
    if not info or str(info.get("id")) != str(slot.get("instance_id")) or info.get("name") != slot["vm"]:
        raise Refused("native instance identity changed; no action taken")
    labels = info.get("labels", {})
    if (labels.get("run-id") not in owners or labels.get("budget") != BUDGET
            or labels.get("issue") not in ("12517", "12520")
            or labels.get("pylon-group") not in ({"maya-consumer-q", "maya-consumer-q-1"}
                                                if slot["vm"] == NAMES[0] else {"maya-consumer-q-2"})):
        raise Refused("service ownership changed; no action taken")
    disks = info.get("disks", [])
    if (len(disks) != 1 or not disks[0].get("boot") or not disks[0].get("autoDelete")
            or not disks[0].get("source", "").endswith("/disks/" + slot["vm"])):
        raise Refused("the service must have one automatically deleted boot disk")


class Backend:
    def __init__(self, home, config, state):
        self.home, self.config, self.state = home, config, state
        self.tools = home / "tools"
        spec = importlib.util.spec_from_file_location("maya_guard", self.tools / "cloud-budget.py")
        self.guard = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.guard)
        self.env = dict(os.environ)
        self.env.pop("OA_RUN_ID", None)
        self.env["CLOUDSDK_CONFIG"] = config["gcloud_config"]
        self.env["OA_GCLOUD"] = config["gcloud"]
        self.env["OA_PROJECT"] = PROJECT
        self.env["OA_CLOUD_BUDGET_DIR"] = config["ledger_dir"]
        self.env["OA_CLOUD_PRICES"] = str(self.tools / "cloud-prices.json")
        self.env["OA_AGENT_RUN"] = str(self.tools / "agent-run.py")

    def command(self, argv, timeout=180):
        result = subprocess.run(argv, env=self.env, capture_output=True, text=True, timeout=timeout)
        if result.returncode:
            # Never retain subprocess output from private-key transfer/installation.
            raise Refused(f"operator command failed ({Path(argv[0]).name}, exit {result.returncode})")
        return result.stdout

    def cloud(self, *args):
        return self.command([self.config["gcloud"], *args])

    def records(self):
        return self.guard.read_ledger(Path(self.config["ledger_dir"]) / (BUDGET + ".jsonl"))

    def guard_call(self, *args, timeout=300):
        args = list(args)
        index = args.index("--") if "--" in args else len(args)
        args[index:index] = ["--budget", BUDGET]
        return self.command([sys.executable, str(self.tools / "cloud-budget.py"), *args], timeout)

    def completion(self):
        fields = " ".join(f'i{n}:issue(number:{n}){{state stateReason}}' for n in COMPLETION)
        query = 'query { repository(owner:"OpenAgentsInc",name:"openagents") {' + fields + '} }'
        try:
            response = json.loads(self.command([self.config["gh"], "api", "graphql", "-f", "query=" + query]))
            rows = response["data"]["repository"]
            values = [rows["i" + str(n)] for n in COMPLETION]
            if any(not v or v.get("state") not in ("OPEN", "CLOSED")
                   or (v["state"] == "CLOSED" and v.get("stateReason") not in ("COMPLETED", "NOT_PLANNED"))
                   for v in values):
                return None
            return all(v["state"] == "CLOSED" and v["stateReason"] == "COMPLETED" for v in values)
        except (Refused, KeyError, TypeError, ValueError, OSError, subprocess.TimeoutExpired):
            return None

    def describe(self, slot):
        # Numeric IDs cannot accidentally select a replacement with the same name.
        ident = slot.get("instance_id") or slot["vm"]
        r = subprocess.run([self.config["gcloud"], "compute", "instances", "describe", str(ident),
                            "--project=" + PROJECT, "--zone=" + ZONE, "--format=json"],
                           env=self.env, capture_output=True, text=True, timeout=180)
        if r.returncode:
            if "was not found" in r.stderr or "notFound" in r.stderr:
                return None
            raise Refused("cloud observation unavailable; no action taken")
        return json.loads(r.stdout)

    def save(self):
        atomic(self.home / "state.json", self.state)

    def storage(self, marker, hours):
        amount = round(hours * self.config["storage_per_hour"], 6)
        why = f"Maya storage budget estimate {marker}: {hours:g}h at $0.50/h; actual storage within this allowance is not charged again"
        matches = [r for r in self.records() if r.get("event") == "charge" and r.get("why") == why]
        if len(matches) > 1:
            raise Refused("duplicate storage allowance needs operator review")
        if not matches:
            self.guard_call("charge", str(amount), "--why", why, "--estimate")
        matches = [r for r in self.records() if r.get("event") == "charge" and r.get("why") == why]
        if len(matches) != 1 or matches[0].get("amount") != amount or matches[0].get("estimate") is not True:
            raise Refused("storage allowance is not confirmed; no new machine reserved")

    def initial_storage(self):
        for original in self.config["slots"]:
            # Book the full initial day, even when the watcher adopts it later.
            self.storage("allocation=" + original["allocation"], 24.0)

    def cleanup(self, slot):
        info = self.describe(slot)
        if info:
            verify_instance(slot, info, self.state["owners"])
            disk = json.loads(self.cloud("compute", "disks", "describe", str(slot["disk_id"]),
                                         "--project=" + PROJECT, "--zone=" + ZONE, "--format=json(id,name)"))
            if str(disk.get("id")) != slot["disk_id"] or disk.get("name") != slot["vm"]:
                raise Refused("boot disk identity changed; no deletion attempted")
        self.guard_call("stop", slot["vm"], "--record", slot["allocation"])
        if self.describe(slot) is not None:
            raise Refused("old native instance still exists")
        # A reused disk name blocks renewal; no name-only disk deletion is attempted.
        r = subprocess.run([self.config["gcloud"], "compute", "disks", "describe", slot["vm"],
                            "--project=" + PROJECT, "--zone=" + ZONE, "--format=value(id)"],
                           env=self.env, capture_output=True, text=True, timeout=180)
        if r.returncode == 0 or not ("was not found" in r.stderr or "notFound" in r.stderr):
            raise Refused("boot disk absence is not confirmed")
        slot.update(allocation=None, instance_id=None, disk_id=None, installed=False, pending=None)
        self.save()

    def adopt(self, slot):
        if slot.get("run_id") == self.state["run_id"]:
            return
        self.command([sys.executable, str(self.tools / "agent-run.py"), "label-vm", str(slot["instance_id"]),
                      ZONE, "--project", PROJECT, "--id", self.state["run_id"]])
        slot["run_id"] = self.state["run_id"]
        self.save()

    def recover(self, slot):
        records = self.records()
        records.refuse_if_corrupt()
        pending = slot["pending"]
        matches = [m for m in self.guard.machines(records) if m["vm"] == slot["vm"]
                   and m.get("run_id") == pending["run_id"] and m["start_epoch"] >= pending["at"] - 1]
        if not matches:
            # The guard writes a start before invoking Compute. No start means
            # admission failed before creation. Keep the booked storage marker.
            slot["pending"] = None
            self.save()
            raise Refused("guard admission did not create a reservation; retry keeps its disk allowance")
        if len(matches) != 1:
            raise Refused("create outcome is unresolved; no retry reservation made")
        machine = matches[0]
        if machine.get("reason") == "create-failed":
            slot["pending"] = None
            self.save()
            raise Refused("capacity unavailable; the failed create is settled")
        found = self.describe({"vm": slot["vm"], "instance_id": machine.get("instance_id")})
        if found is None:
            raise Refused("reserved create is not yet visible; no retry reservation made")
        candidate = {**slot, "instance_id": str(found["id"])}
        verify_instance(candidate, found, [pending["run_id"]])
        # Labels and creation time must refer to this reservation, not an old same-name VM.
        created = dt.datetime.fromisoformat(found["creationTimestamp"].replace("Z", "+00:00")).timestamp()
        if abs(created - machine["start_epoch"]) > 600:
            raise Refused("create result differs from the reserved attempt")
        disk = json.loads(self.cloud("compute", "disks", "describe", slot["vm"], "--project=" + PROJECT,
                                     "--zone=" + ZONE, "--format=json(id)"))
        slot.update(allocation=machine["id"], instance_id=str(found["id"]), disk_id=str(disk["id"]),
                    expires=machine["expires_epoch"], run_id=pending["run_id"], pending=None, installed=False,
                    reservation=None, restarts=0)
        self.save()

    def create(self, slot):
        # No new reservation until every existing Maya VM is accounted for in one of our slots.
        seen = json.loads(self.cloud("compute", "instances", "list", "--project=" + PROJECT,
                                     "--filter=name~^oa-maya-", "--format=json(id,name)"))
        expected = {s["instance_id"] for s in self.state["slots"] if s.get("instance_id")}
        if len(seen) >= 2 or any(str(m["id"]) not in expected for m in seen):
            raise Refused("another Maya machine exists; no replacement reserved")
        self.guard_call("sync", "--project", PROJECT)
        if not slot.get("reservation"):
            hours = safe_hours(self.config, self.guard, self.records(), time.time())
            slot["reservation"] = {"storage": uuid.uuid4().hex, "hours": hours}
            self.save()
        reservation = slot["reservation"]
        self.storage("reservation=" + reservation["storage"], reservation["hours"])
        hours = safe_hours(self.config, self.guard, self.records(), time.time(),
                           storage_booked=True, limit=reservation["hours"])
        slot["pending"] = {"at": time.time(), "run_id": self.state["run_id"]}
        self.save()
        self.env["OA_RUN_ID"] = self.state["run_id"]
        args = ["create", slot["vm"], "--machine-type", "g4-standard-48", "--zone", ZONE,
                "--project", PROJECT, "--hours", str(hours), "--spot", "--on-expiry", "delete",
                "--cap", "3000", "--label", "issue=12520", "--label",
                "pylon-group=maya-consumer-q-" + slot["vm"][-1], "--json"]
        extra = ["--boot-disk-size=1200GB", "--boot-disk-type=hyperdisk-balanced",
                 "--boot-disk-provisioned-iops=10200", "--boot-disk-provisioned-throughput=1940",
                 "--image=common-cu129-ubuntu-2404-nvidia-580-v20261019",
                 "--image-project=deeplearning-platform-release", "--scopes=storage-ro",
                 "--metadata=enable-guest-attributes=TRUE",
                 "--metadata-from-file=startup-script=" + str(self.home / "bundle/startup.sh")
                 + ",ssh-keys=" + str(self.home / "bundle/ssh-authorized-keys")]
        try:
            self.guard_call(*args, "--", *extra, timeout=660)
        finally:
            # A timeout/lost reply must recover the durable reservation before any retry.
            self.recover(slot)

    def transport(self, slot):
        ident = str(slot["instance_id"])
        verify_instance(slot, self.describe(slot), self.state["owners"])
        # IAP resolves names, not numeric IDs. Obtain host keys through the
        # exact instance's authenticated Compute API, then pin that identity.
        known_hosts = self.home / ("known-hosts-" + ident)
        if not known_hosts.exists():
            attributes = json.loads(self.cloud("compute", "instances", "get-guest-attributes", ident,
                                              "--project=" + PROJECT, "--zone=" + ZONE,
                                              "--query-path=hostkeys/", "--format=json"))
            known_hosts.write_text(host_keys(attributes, ident))
            os.chmod(known_hosts, 0o600)
        private(known_hosts)
        if not all(line.startswith("compute." + ident + " ") for line in known_hosts.read_text().splitlines()):
            raise Refused("SSH host identity differs from the native service")
        verify_instance(slot, self.describe(slot), self.state["owners"])
        options = ["-oHostKeyAlias=compute." + ident, "-oStrictHostKeyChecking=yes", "-oIdentitiesOnly=yes",
                   "-oUserKnownHostsFile=" + str(known_hosts), "-oCheckHostIP=no", "-i" + self.config["ssh_key"]]
        destination = self.config["ssh_user"] + "@" + slot["vm"]
        # --plain is essential: gcloud's earlier default -o values otherwise
        # override extra flags because OpenSSH uses the first value it sees.
        ssh = [self.config["gcloud"], "compute", "ssh", destination, "--project=" + PROJECT,
               "--zone=" + ZONE, "--tunnel-through-iap", "--quiet", "--plain"]
        ssh += ["--ssh-flag=" + option for option in options]
        scp = [self.config["gcloud"], "compute", "scp", "--project=" + PROJECT,
               "--zone=" + ZONE, "--tunnel-through-iap", "--quiet", "--plain"]
        scp += ["--scp-flag=" + option for option in options]
        metadata_check = ("test \"$(curl --fail --silent --show-error -H 'Metadata-Flavor: Google' "
                          "http://metadata.google.internal/computeMetadata/v1/instance/id)\" = " + shlex.quote(ident) + " && ")
        return ssh, scp, metadata_check

    def install(self, slot, number):
        ssh, scp, metadata_check = self.transport(slot)
        bundle = self.home / "bundle"
        # Restore the same identity before the pinned head may announce anything.
        self.command(ssh + ["--command", metadata_check + "install -m600 /dev/null ~/maya-provider.key"])
        remote = self.config["ssh_user"] + "@" + slot["vm"]
        self.command(scp + [str(self.home / "vault" / f"slot-{number}.provider.key"), remote + ":maya-provider.key"])
        self.command(scp + [str(bundle / "service-files.tar.gz"), str(bundle / "install-config.sh"),
                            str(bundle / "install-head-verified.sh"), remote + ":"])
        command = (metadata_check + f"sudo bash ~/install-config.sh {number} \"$HOME/service-files.tar.gz\" && "
                   "sudo install -m600 -o maya-pylon -g maya-pylon ~/maya-provider.key /mnt/maya/provider/provider.key && "
                   "rm ~/maya-provider.key && sudo bash ~/install-head-verified.sh " + shlex.quote(slot["producer"]))
        self.command(ssh + ["--command", command], timeout=300)
        slot["installed"] = True
        self.save()

    def health(self, slot, now):
        ssh, _, metadata_check = self.transport(slot)
        read_public = ("import json;from pathlib import Path;"
                       "p=Path('/mnt/maya/provider');"
                       "print(json.dumps({'live':json.loads((p/'live.json').read_text()),"
                       "'settings':json.loads((p/'share.json').read_text()),"
                       "'identity':json.load(open('/mnt/maya/receipts/identity-public.json'))}))")
        observed = json.loads(self.command(ssh + ["--command", metadata_check + "sudo python3 -c " + shlex.quote(read_public)]))
        live = observed["live"]
        actual = observed["settings"]
        expected = load(self.home / "bundle" / ("launch-" + slot["vm"][-1] + ".json"))["settings"]
        for settings in (actual, expected):
            settings.pop("changed_at", None)
            settings["allow"] = sorted(settings.get("allow", []))
        if (actual != expected or observed["identity"].get("ok") is not True
                or observed["identity"].get("data", {}).get("pubkey") != slot["producer"]):
            raise Refused("service settings or public provider identity changed; no restart attempted")
        if abs(now - live["at"]) > 180:
            raise Refused("service status is stale; no restart attempted")
        group = live.get("group")
        if not group or group.get("build") != WORKER_HASH or group.get("pinned") is not True:
            raise Refused("service identity is not confirmed; no restart attempted")
        slot["health"] = {"at": live["at"], "ready": group.get("ready"), "busy": group.get("busy"),
                          "generation": group.get("generation"), "failed": group.get("failed")}
        if group.get("failed") is not None:
            if slot.get("restarts", 0) >= 3 or now < slot.get("restart_at", 0) + 3600:
                raise Refused("terminal service failure needs operator review after bounded retries")
            # Persist before the command: a lost SSH reply cannot repeat a restart.
            slot["restarts"] = slot.get("restarts", 0) + 1
            slot["restart_at"] = now
            self.save()
            self.command(ssh + ["--command", metadata_check + "sudo systemctl restart --no-block maya-pylon.service"])


def host_keys(attributes, ident):
    lines = []
    for item in attributes.get("queryValue", {}).get("items", []):
        kind, value = item.get("key"), item.get("value", "")
        if item.get("namespace") != "hostkeys" or kind not in ("ssh-rsa", "ssh-ed25519", "ecdsa-sha2-nistp256"):
            continue
        try:
            decoded = base64.b64decode(value, validate=True)
        except ValueError:
            raise Refused("invalid guest SSH host key") from None
        if not decoded or base64.b64encode(decoded).decode() != value:
            raise Refused("invalid guest SSH host key")
        lines.append(f"compute.{ident} {kind} {value}\n")
    if not lines:
        raise Refused("trusted guest SSH host keys are not available yet")
    return "".join(sorted(set(lines)))


def tick(backend, state, now, stopping=False):
    complete = backend.completion()
    if stopping or complete is True:
        errors = []
        for slot in state["slots"]:
            try:
                if slot.get("pending"):
                    backend.recover(slot)
                if slot.get("allocation"):
                    backend.cleanup(slot)
            except (Refused, OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
                errors.append(str(error))
        if errors:
            raise Refused("; ".join(errors))
        state["phase"] = "stopped" if stopping else "completed"
        backend.save()
        return True
    if complete is None:
        state["phase"] = "waiting-for-github"
        backend.save()
        return False
    backend.initial_storage()
    errors = []
    for number, slot in enumerate(state["slots"], 1):
        try:
            reconcile_slot(backend, state, slot, number, now, allow_create=not errors)
        except (Refused, OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
            errors.append(str(error))
    if errors:
        raise Refused("; ".join(errors))
    state["phase"] = "supervising"
    backend.save()
    return False


def reconcile_slot(backend, state, slot, number, now, allow_create):
    if slot.get("pending"):
        backend.recover(slot)
    if slot.get("allocation"):
        info = backend.describe(slot)
        if info:
            verify_instance(slot, info, state["owners"])
        if info is None or info["status"] in ("TERMINATED", "STOPPED") or now >= slot["expires"] - 7200:
            backend.cleanup(slot)
        elif info["status"] != "RUNNING":
            raise Refused("service transition is in progress; no replacement reserved")
        else:
            backend.adopt(slot)
    if not slot.get("allocation"):
        if not allow_create:
            raise Refused("another service has an unresolved observation; no replacement reserved")
        backend.create(slot)
    if not slot.get("installed"):
        backend.install(slot, number)
    else:
        backend.health(slot, now)


def configure(args):
    home = args.home
    if (home / "config.json").exists() or (home / "state.json").exists():
        raise Refused("this supervisor is already configured; do not replace its saved identities")
    home.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(home, 0o700)
    (home / "bundle").mkdir(mode=0o700, exist_ok=True)
    (home / "tools").mkdir(mode=0o700, exist_ok=True)
    deployment = load(args.deployment)
    # Public tar only; keys never enter metadata, archives, config, or logs.
    with tarfile.open(args.bundle / "service-files.tar.gz") as archive:
        members = archive.getmembers()
        if (len(members) != len(PUBLIC_MEMBERS) or {m.name for m in members} != PUBLIC_MEMBERS
                or any(not m.isfile() for m in members)):
            raise Refused("the public service archive contains unexpected files")
        for name in ("launch-1.json", "launch-2.json"):
            if archive.extractfile(name).read() != (args.bundle / name).read_bytes():
                raise Refused("archived and public service settings differ")
    files = {}
    for name in BUNDLE:
        data = (args.bundle / name).read_bytes()
        (home / "bundle" / name).write_bytes(data)
        files[name] = hashlib.sha256(data).hexdigest()
    head = (home / "bundle/install-head.sh").read_text()
    marker = "systemctl enable --now --no-block maya-pylon.service"
    if head.count(marker) != 1:
        raise Refused("the retained head installer changed")
    # Check public identity before enabling the service; leave inference settings unchanged.
    identity_check = """python3 - "$1" /mnt/maya/receipts/identity-public.json <<'IDENTITY'
import json,sys
v=json.load(open(sys.argv[2]))
if v.get('ok') is not True or v.get('data',{}).get('pubkey') != sys.argv[1]:
    raise SystemExit('restored public identity differs')
IDENTITY
"""
    verified = head.replace(marker, identity_check + marker)
    (home / "bundle/install-head-verified.sh").write_text(verified)
    files["install-head-verified.sh"] = digest(home / "bundle/install-head-verified.sh")
    config = {"schema": "openagents.maya.supervisor.v1", "completion": COMPLETION,
              "maya_cap": 1000, "global_cap": 3000, "reserve": 300, "storage_per_hour": 0.5, "files": files,
              "gcloud": shutil.which("gcloud"), "gh": shutil.which("gh"),
              "gcloud_config": str(Path.home() / "work/.secrets/gcloud-sa-config"),
              "ledger_dir": str(Path.home() / ".openagents/cloud-budget"), "slots": deployment["slots"], "tools": {},
              "ssh_user": deployment.get("ssh_user", Path.home().name),
              "ssh_key": deployment.get("ssh_key", str(Path.home() / ".ssh/google_compute_engine")),
              "path": os.environ.get("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")}
    public_key = Path(config["ssh_key"] + ".pub").read_text().split()
    if len(public_key) < 2 or public_key[0] not in ("ssh-rsa", "ssh-ed25519", "ecdsa-sha2-nistp256"):
        raise Refused("missing operator public SSH key")
    authorized = home / "bundle/ssh-authorized-keys"
    authorized.write_text(config["ssh_user"] + ":" + " ".join(public_key[:2]) + "\n")
    files[authorized.name] = digest(authorized)
    for source in [REPO / "scripts/dev/cloud-budget.py", REPO / "scripts/dev/cloud-prices.json",
                   REPO / "scripts/dev/agent-run.py", Path(__file__)]:
        destination = home / "tools" / ("maya-supervisor.py" if source == Path(__file__) else source.name)
        shutil.copyfile(source, destination)
        config["tools"][destination.name] = digest(destination)
    config["source_commit"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip()
    validate_config(config, home)
    state = {"phase": "configured", "owners": [deployment["run_id"]],
             "slots": deployment["slots"], "run_id": None}
    validate_state(state, config)
    # Copy only the two public host identities established during initial setup.
    existing_hosts = Path.home() / ".ssh/google_compute_known_hosts"
    known = existing_hosts.read_text().splitlines() if existing_hosts.exists() else []
    for slot in config["slots"]:
        lines = [line for line in known if line.startswith("compute." + slot["instance_id"] + " ")]
        if not lines:
            raise Refused("initial setup must establish trusted SSH host keys before handoff")
        path = home / ("known-hosts-" + slot["instance_id"])
        path.write_text("\n".join(lines) + "\n")
        os.chmod(path, 0o600)
    atomic(home / "config.json", config)
    atomic(home / "state.json", state)


def serve(home):
    with (home / "controller.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        config, state = load(home / "config.json"), load(home / "state.json")
        if state.get("phase") in ("completed", "stopped"):
            return 0
        validate_config(config, home)
        validate_state(state, config)
        if not (home / "handoff-confirmed").exists():
            raise Refused("confirm the initial deployment handoff before supervising")
        env = dict(os.environ)
        env.pop("OA_RUN_ID", None)
        run = subprocess.run([sys.executable, str(home / "tools/agent-run.py"), "new", "--kind", "maya-supervisor",
                              "--root", str(os.getpid())], env=env, capture_output=True, text=True, check=True)
        state["run_id"] = run.stdout.strip()
        state["owners"].append(state["run_id"])
        backend = Backend(home, config, state)
        backend.save()
        while True:
            try:
                validate_config(config, home)
                validate_state(state, config)
                if tick(backend, state, time.time(), (home / "stop").exists()):
                    return 0
                state.pop("error", None)
            except (Refused, backend.guard.Refused, OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
                state["error"] = str(error)
            state["checked_at"] = dt.datetime.now(dt.timezone.utc).isoformat()
            backend.save()
            print(json.dumps({"phase": state["phase"], "error": state.get("error"), "checked_at": state["checked_at"]}), flush=True)
            time.sleep(60)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["configure", "install", "serve", "status", "stop", "disable"])
    parser.add_argument("--home", type=Path, default=DEFAULT_HOME)
    parser.add_argument("--deployment", type=Path)
    parser.add_argument("--bundle", type=Path)
    parser.add_argument("--handoff-confirmed", action="store_true")
    args = parser.parse_args()
    home = args.home.resolve()
    if args.command == "configure":
        configure(args)
    elif args.command == "serve":
        return serve(home)
    elif args.command == "status":
        print(json.dumps(load(home / "state.json"), indent=2))
    elif args.command == "stop":
        (home / "stop").touch(mode=0o600)
        print("Stopping both Maya services through the spend guard.")
    elif args.command == "install":
        if not args.handoff_confirmed:
            raise Refused("install needs --handoff-confirmed after both services are ready")
        validate_config(load(home / "config.json"), home)
        (home / "handoff-confirmed").touch(mode=0o600)
        path = Path.home() / "Library/LaunchAgents" / (LABEL + ".plist")
        path.parent.mkdir(parents=True, exist_ok=True)
        value = {"Label": LABEL, "ProgramArguments": [sys.executable, str(home / "tools/maya-supervisor.py"), "serve", "--home", str(home)],
                 "RunAtLoad": True, "KeepAlive": {"SuccessfulExit": False}, "ThrottleInterval": 60,
                 "EnvironmentVariables": {"PATH": load(home / "config.json")["path"]},
                 "StandardOutPath": str(home / "supervisor.log"), "StandardErrorPath": str(home / "supervisor.error.log")}
        path.write_bytes(plistlib.dumps(value))
        subprocess.run(["launchctl", "bootstrap", f"gui/{os.getuid()}", str(path)], check=True)
    else:
        if load(home / "state.json").get("phase") not in ("completed", "stopped"):
            raise Refused("stop first and wait for verified deletion before disabling")
        subprocess.run(["launchctl", "bootout", f"gui/{os.getuid()}/{LABEL}"], check=True)
        (Path.home() / "Library/LaunchAgents" / (LABEL + ".plist")).unlink(missing_ok=True)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (Refused, OSError, ValueError) as error:
        print(f"maya-supervisor: {error}", file=sys.stderr)
        raise SystemExit(1)
