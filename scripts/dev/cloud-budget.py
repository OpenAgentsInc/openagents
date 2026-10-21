#!/usr/bin/env python3
"""cloud-budget: the spend guard for Google Compute machines rented for one
piece of work, such as the DeepSeek-V4.1 work (#12370, cap $3,000).

Every machine the work rents goes through `create`, which:

  1. looks up the hourly price in scripts/dev/cloud-prices.json and refuses a
     machine type (or spot/on-demand choice) that has no price;
  2. refuses the machine if spent + running + hours x price would pass the cap,
     where a running machine counts up to its expiry, because it may run that
     long;
  3. gives the machine an expiry: an `expires-at` label, plus
     --max-run-duration and --instance-termination-action (DELETE by default,
     STOP with --on-expiry stop for machines whose disks must survive);
  4. labels it with the run (`run-id`, `run-host` from scripts/dev/agent-run.py)
     and the budget, and records it in the run, so ending the session reaps it;
  5. writes a `start` record to the ledger before gcloud runs, and a `stop`
     record if gcloud fails.

Admission also counts the budget's reserve for what compute prices miss
(persistent disks, buckets, checkpoint copies, network egress), set with
`reserve`.

The ledger is one append-only JSON-lines file per budget:
~/.openagents/cloud-budget/<budget>.jsonl (OA_CLOUD_BUDGET_DIR). Each machine
has a `start` record and, once it ends, a `stop` record with its cost. A torn
last line (a crash mid-write) is skipped on read; the next append starts a
fresh line and writes a `torn` record naming the torn line, so it still reads.
Any other unreadable line makes `create` and `reserve` refuse (#12376).

One host and one ledger path allocate for a budget (#12376). The owner is in
the budget's first record, or in an `owner` record that the first `create`
after #12376 appends to an older ledger; ~/.openagents/cloud-budget-authority.json
names the ledger path each budget uses on this host. `create` from another
host or path refuses unless --take-over, which is recorded.

Manual `stop`, `sync`, and adoption settle a machine the same way (#12409), from
Google's records for that instance ID: `describe`, the zone's operations, and the
audit log once operations age out. A flex-start request bills from when its
machine started (its insert finished, or `lastStartTimestamp`), and costs nothing
only when its insert ended with an error and nothing shows it started. When the
records can't yet say, the machine stays counted: `stop` writes a `pending`
record after its delete, and `sync` settles it later. A finished record that
holds a wrong cost gets a `correction` record, which supersedes its stop without
rewriting the ledger.

A reused name is another machine (#12411). A described machine whose instance ID
differs from the record's, or from its creation operation's when the record has
none, lends no timestamps to the record. `stop` never deletes such a replacement:
it settles the record from Google's records, or leaves it counted, and exits 1.

Before admitting, `create` runs `sync`, which also records machines that carry
the budget's label but have no running record (a lost create reply), so they
count.

    cloud-budget.py create NAME --machine-type T --zone Z --hours H [--spot]
        [--on-expiry delete|stop] [--label K=V ...] [--take-over] [--dry-run]
        [--json] [-- EXTRA GCLOUD FLAGS ...]
    cloud-budget.py stop NAME [--keep-machine]   delete the machine, record cost
    cloud-budget.py sync                         record machines that ended or appeared
    cloud-budget.py correct ID --why TEXT (--never-started | --from-google | --end T [--run-start T])
                                                 append a correction to a finished record
    cloud-budget.py reserve DOLLARS --why TEXT   set the reserve for storage and network
    cloud-budget.py report [--offline] [--json]  spent, running, reserve, remaining
    cloud-budget.py prices                       the price table

Common options: --budget NAME (default deepseek-v41), --cap DOLLARS (default
3000, or the cap the budget's ledger first recorded), --project P (default
openagentsgemini).

Environment: OA_CLOUD_BUDGET_DIR (ledger directory), OA_CLOUD_BUDGET_HOST (this
host's name in owner records; default the macOS LocalHostName or the short host
name), OA_CLOUD_PRICES (price table), OA_GCLOUD (gcloud binary), OA_AGENT_RUN (agent-run.py path),
OA_CLOUD_BUDGET_NOW (epoch seconds, for tests), CLOUDSDK_CONFIG (when unset,
~/work/.secrets/gcloud-sa-config is used if present, as agent-run.py does).
"""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import math
import os
import re
import shlex
import socket
import subprocess
import sys
import time
import uuid
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
LEDGERS = Path(os.environ.get("OA_CLOUD_BUDGET_DIR", str(Path.home() / ".openagents" / "cloud-budget")))
PRICES = Path(os.environ.get("OA_CLOUD_PRICES", str(HERE / "cloud-prices.json")))
AGENT_RUN = Path(os.environ.get("OA_AGENT_RUN", str(HERE / "agent-run.py")))
SA_CONFIG = Path.home() / "work" / ".secrets" / "gcloud-sa-config"
AUTHORITY = Path.home() / ".openagents" / "cloud-budget-authority.json"
DEFAULT_BUDGET = "deepseek-v41"
DEFAULT_CAP = 3000.0
DEFAULT_PROJECT = "openagentsgemini"
# Production machines this guard never creates, stops, or counts.
PROTECTED = re.compile(r"^(clef-decisions-h100|clef-decisions-a100|oa-pylon-gpu-1|oa-att-.*)$")
# Flags the guard sets itself; passing them as extra flags is refused.
OWNED_FLAGS = ("--labels", "--max-run-duration", "--termination-time", "--instance-termination-action",
               "--provisioning-model", "--preemptible", "--machine-type", "--zone", "--project",
               "--request-valid-for-duration", "--reservation-affinity")
MIN_BILLED_SECONDS = 60  # Compute Engine bills per second with a one-minute minimum.
LABEL_VALUE = re.compile(r"^[a-z0-9_-]{0,63}$")
LABEL_KEY = re.compile(r"^[a-z][a-z0-9_-]{0,62}$")
# A machine whose create time is this far from its start record is another machine.
SAME_MACHINE_SECONDS = 1800
# A labeled machine with no expiry label counts this long from when sync finds it.
ADOPT_HOURS = 24
LIVE_STATUSES = ("PROVISIONING", "STAGING", "RUNNING", "REPAIRING", "STOPPING", "SUSPENDING")


class Refused(Exception):
    pass


# ------------------------------------------------------------------ helpers

def now() -> float:
    raw = os.environ.get("OA_CLOUD_BUDGET_NOW")
    return float(raw) if raw else time.time()


def iso(epoch: float) -> str:
    return datetime.fromtimestamp(epoch, timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def label_time(epoch: float) -> str:
    return datetime.fromtimestamp(epoch, timezone.utc).strftime("%Y-%m-%dt%H-%M-%Sz")


def parse_time(text: str) -> float | None:
    try:
        return datetime.fromisoformat(text.replace("Z", "+00:00")).timestamp()
    except ValueError:
        return None


def money(x: float) -> str:
    return f"${x:,.4f}" if abs(x) < 1 else f"${x:,.2f}"


def duration(seconds: float) -> str:
    seconds = max(0, int(seconds))
    h, rem = divmod(seconds, 3600)
    return f"{h}h{rem // 60:02d}m"


def gcloud(*args: str, timeout: int = 600) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    if "CLOUDSDK_CONFIG" not in env and SA_CONFIG.is_dir():
        env["CLOUDSDK_CONFIG"] = str(SA_CONFIG)
    argv = [os.environ.get("OA_GCLOUD", "gcloud"), *args]
    try:
        return subprocess.run(argv, capture_output=True, text=True, env=env, timeout=timeout)
    except (OSError, subprocess.TimeoutExpired) as error:
        return subprocess.CompletedProcess(argv, 1, "", str(error))


def load_prices() -> dict:
    return json.loads(PRICES.read_text())


KIND_NAMES = {"spot": "spot", "standard": "on-demand", "flex": "flex-start"}


def price_for(prices: dict, machine_type: str, spot: bool | str) -> tuple[float, str]:
    """`spot` is True for spot, False for on demand, or a kind name: spot, standard, flex."""
    kind = spot if isinstance(spot, str) else ("spot" if spot else "standard")
    entry = prices.get("machines", {}).get(machine_type)
    if not entry or not isinstance(entry.get(kind), (int, float)) or entry[kind] <= 0:
        raise Refused(f"no {KIND_NAMES[kind]} price for {machine_type} in {PRICES}; "
                      "add it with its source and date before renting")
    source = entry.get("source", "")
    return float(entry[kind]), prices.get("sources", {}).get(source, source)


# ------------------------------------------------------------------- ledger

def ledger_path(budget: str) -> Path:
    if not LABEL_VALUE.match(budget) or not budget:
        raise Refused(f"budget name {budget!r} must be lowercase letters, digits, - or _")
    return LEDGERS / f"{budget}.jsonl"


@contextmanager
def locked(budget: str):
    path = ledger_path(budget)
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path.with_suffix(".lock"), "a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        try:
            yield path
        finally:
            fcntl.flock(lock, fcntl.LOCK_UN)


class Ledger(list):
    """The records of a ledger, plus the line numbers of any unreadable interior records."""

    def __init__(self, records=(), corrupt=(), path=None):
        super().__init__(records)
        self.corrupt: list[int] = list(corrupt)
        self.path = path

    def refuse_if_corrupt(self) -> None:
        if self.corrupt:
            where = ", ".join(str(n) for n in self.corrupt)
            raise Refused(f"{self.path} has unreadable records on line {where}; a lost record may hide "
                          "a reservation. Repair the line from Google's records "
                          "(`gcloud compute operations list`), then try again")


def _well_formed(r) -> bool:
    if not isinstance(r, dict):
        return False
    num = (int, float)
    if r.get("event") == "start":
        return (isinstance(r.get("id"), str) and isinstance(r.get("vm"), str)
                and all(isinstance(r.get(k), num) for k in ("price_per_hour", "start_epoch", "expires_epoch")))
    if r.get("event") in ("stop", "correction"):
        return isinstance(r.get("id"), str) and isinstance(r.get("stop_epoch"), num)
    if r.get("event") == "pending":
        return isinstance(r.get("id"), str) and isinstance(r.get("deleted_epoch"), num)
    if r.get("event") == "budget":
        return isinstance(r.get("cap"), num)
    if r.get("event") == "reserve":
        return isinstance(r.get("amount"), num) and r["amount"] >= 0
    return isinstance(r.get("event"), str)


def read_ledger(path: Path, quiet: bool = False) -> Ledger:
    """Read every record. A torn final line is skipped. An unreadable line before it counts as
    corruption, unless a later `torn` record names it with its digest (the append that followed a
    crash wrote that record)."""
    try:
        data = path.read_bytes()
    except FileNotFoundError:
        return Ledger(path=path)
    records, bad = [], []
    lines = data.split(b"\n")
    for i, raw in enumerate(lines):
        if not raw.strip():
            continue
        try:
            r = json.loads(raw)
        except (json.JSONDecodeError, UnicodeDecodeError):
            r = None
        if _well_formed(r):
            records.append(r)
        elif i == len(lines) - 1:
            if not quiet:
                print(f"cloud-budget: skipped a torn last line in {path}", file=sys.stderr)
        else:
            bad.append((i + 1, hashlib.sha256(raw).hexdigest()))
    healed = {(r.get("line"), r.get("sha256")) for r in records if r.get("event") == "torn"}
    corrupt = [n for n, digest in bad if (n, digest) not in healed]
    if corrupt and not quiet:
        print(f"cloud-budget: {path} has unreadable records on line "
              f"{', '.join(map(str, corrupt))}; create refuses until they are repaired", file=sys.stderr)
    return Ledger(records, corrupt, path)


def append(path: Path, record: dict) -> None:
    line = (json.dumps(record, sort_keys=True) + "\n").encode()
    fd = os.open(path, os.O_RDWR | os.O_CREAT | os.O_APPEND, 0o600)
    try:
        size = os.fstat(fd).st_size
        if size:
            os.lseek(fd, size - 1, os.SEEK_SET)
            if os.read(fd, 1) != b"\n":
                # A crash tore the last record. Start a new line, and name the torn line so a
                # reader can tell it from corruption. Its record never finished, so the gcloud
                # call it came before never ran.
                os.lseek(fd, 0, os.SEEK_SET)
                data = b""
                while len(data) < size:
                    chunk = os.read(fd, size - len(data))
                    if not chunk:
                        break
                    data += chunk
                torn = data.split(b"\n")[-1]
                note = {"event": "torn", "line": data.count(b"\n") + 1,
                        "sha256": hashlib.sha256(torn).hexdigest(), "bytes": len(torn), "at": iso(now())}
                line = b"\n" + (json.dumps(note, sort_keys=True) + "\n").encode() + line
        os.write(fd, line)
        os.fsync(fd)
    finally:
        os.close(fd)


def machines(records: list[dict]) -> list[dict]:
    """Fold start and stop records into one entry per machine, in start order."""
    by_id: dict[str, dict] = {}
    cap = None
    for r in records:
        if r.get("event") == "budget" and cap is None:
            cap = r.get("cap")
        elif r.get("event") == "start" and r.get("id"):
            by_id[r["id"]] = dict(r)
        elif r.get("event") in ("stop", "correction") and r.get("id") in by_id:
            # A later stop or correction for the same machine supersedes the earlier settlement.
            m = by_id[r["id"]]
            m["stop_epoch"], m["stop"], m["reason"] = r["stop_epoch"], r.get("stop"), r.get("reason")
            if r.get("run_start_epoch") is not None:
                m["run_start_epoch"] = r["run_start_epoch"]
            elif r.get("event") == "correction":
                m.pop("run_start_epoch", None)
            if r.get("instance_id"):
                m["instance_id"] = r["instance_id"]
        elif r.get("event") == "pending" and r.get("id") in by_id:
            # `stop` deleted it but couldn't yet tell whether it started; sync settles it.
            m = by_id[r["id"]]
            m["deleted_epoch"] = r.get("deleted_epoch")
            if r.get("instance_id"):
                m["instance_id"] = r["instance_id"]
    return list(by_id.values())


def ledger_cap(records: list[dict]) -> float | None:
    for r in records:
        if r.get("event") == "budget":
            return float(r["cap"])
    return None


def cost(m: dict, at: float) -> float:
    """What a machine has cost by `at`: per second, one-minute minimum.

    A finished machine costs until its recorded end, which may be after its expiry when Google
    deleted it late. A running machine costs until `at`; once past its expiry it keeps counting
    until `sync` settles it from Google's records.
    """
    if m.get("reason") in ("create-failed", "never-started"):
        return 0.0
    end = m.get("stop_epoch")
    if end is None:
        end = at
    # A flex-start machine bills from when it started running, not from its queued request.
    start = max(m["start_epoch"], m.get("run_start_epoch") or m["start_epoch"])
    seconds = max(end - start, MIN_BILLED_SECONDS)
    return round(m["price_per_hour"] * seconds / 3600, 6)


def committed(m: dict, at: float | None = None) -> float:
    """The most a running machine can cost: until its expiry, or until now if that's later."""
    end = max(m["expires_epoch"], at if at is not None else 0)
    return round(m["price_per_hour"] * max(end - m["start_epoch"], MIN_BILLED_SECONDS) / 3600, 6)


def ledger_reserve(records: list[dict]) -> float:
    amount = 0.0
    for r in records:
        if r.get("event") == "reserve":
            amount = float(r["amount"])
    return amount


def ledger_charges(records: list[dict]) -> float:
    """Spend that isn't a machine, such as network transfer or bucket copies (`charge`)."""
    return round(sum(float(r["amount"]) for r in records if r.get("event") == "charge"), 6)


def totals(ms: list[dict], at: float, reserve: float = 0.0, charges: float = 0.0) -> dict:
    done = [m for m in ms if m.get("stop_epoch") is not None]
    live = [m for m in ms if m.get("stop_epoch") is None]
    return {
        "spent": round(sum(cost(m, at) for m in done) + charges, 6),
        "charges": round(charges, 6),
        "running": round(sum(cost(m, at) for m in live), 6),
        "running_committed": round(sum(committed(m, at) for m in live), 6),
        "reserve": round(reserve, 6),
        "finished": len(done),
        "live": live,
    }


# ---------------------------------------------------------------- admission

def admit(ms: list[dict], cap: float, price: float, hours: float, at: float, reserve: float = 0.0,
          charges: float = 0.0) -> dict:
    t = totals(ms, at, reserve, charges)
    request = round(price * hours, 6)
    after = t["spent"] + t["running_committed"] + t["reserve"] + request
    if after > cap + 1e-9:
        raise Refused(f"over the cap: spent {money(t['spent'])} + running up to "
                      f"{money(t['running_committed'])} + reserve {money(t['reserve'])} + this machine "
                      f"{money(request)} = {money(after)} > cap {money(cap)}")
    return {"request": request, "after": after, **{k: t[k] for k in ("spent", "running_committed", "reserve")}}


# ---------------------------------------------------------------- authority

def host_id() -> str:
    name = os.environ.get("OA_CLOUD_BUDGET_HOST", "")
    if not name and sys.platform == "darwin":
        try:
            name = subprocess.run(["scutil", "--get", "LocalHostName"], capture_output=True, text=True,
                                  timeout=5).stdout.strip()
        except (OSError, subprocess.TimeoutExpired):
            name = ""
    return (name or socket.gethostname().split(".")[0]).lower()


def ledger_owner(records: list[dict]) -> dict | None:
    """Who allocates for this budget: the first budget record's host and path, or the latest
    `owner` record (written by the first create after #12376, or by --take-over)."""
    owner = None
    for r in records:
        if r.get("event") == "budget" and owner is None and r.get("host"):
            owner = {"host": r["host"], "ledger": r.get("ledger")}
        elif r.get("event") == "owner" and r.get("host"):
            owner = {"host": r["host"], "ledger": r.get("ledger")}
    return owner


@contextmanager
def authority_book():
    AUTHORITY.parent.mkdir(parents=True, exist_ok=True)
    with open(AUTHORITY.with_suffix(".lock"), "a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        try:
            try:
                book = json.loads(AUTHORITY.read_text())
            except FileNotFoundError:
                book = {}
            except json.JSONDecodeError as error:
                raise Refused(f"{AUTHORITY} is unreadable ({error}); repair it before allocating")
            before = json.dumps(book, sort_keys=True)
            yield book
            if json.dumps(book, sort_keys=True) != before:
                tmp = AUTHORITY.with_suffix(".tmp")
                tmp.write_text(json.dumps(book, indent=1, sort_keys=True) + "\n")
                os.replace(tmp, AUTHORITY)
        finally:
            fcntl.flock(lock, fcntl.LOCK_UN)


def claim_authority(budget: str, path: Path, records: list[dict], take_over: bool, at: float) -> list[dict]:
    """Refuse unless this host and ledger path allocate for the budget. Returns the records to
    append (an owner record when the ledger has none, or when taking over)."""
    me, where = host_id(), str(path.resolve())
    owner = ledger_owner(records)
    pending = []
    with authority_book() as book:
        held = book.get(budget, {}).get("ledger")
        problems = []
        if owner and owner["host"] != me:
            problems.append(f"host {owner['host']} allocates for budget {budget}, not {me}")
        if owner and owner.get("ledger") and owner["ledger"] != where:
            problems.append(f"budget {budget} belongs to the ledger at {owner['ledger']}, not {where}")
        if held and held != where:
            problems.append(f"on this host, budget {budget} uses the ledger at {held}, not {where}")
        if problems and not take_over:
            raise Refused("; ".join(problems) + ". Allocate from that host and ledger, or pass "
                          "--take-over once the other one has stopped allocating")
        if take_over and problems:
            pending.append({"event": "owner", "budget": budget, "host": me, "ledger": where, "at": iso(at),
                            "take_over": True, "from": owner, "from_ledger": held, "why": "; ".join(problems)})
        elif owner is None and records:
            pending.append({"event": "owner", "budget": budget, "host": me, "ledger": where, "at": iso(at)})
        book[budget] = {"ledger": where, "host": me}
    return pending


def run_labels() -> tuple[str, str]:
    r = subprocess.run([sys.executable, str(AGENT_RUN), "labels"], capture_output=True, text=True)
    if r.returncode != 0:
        raise Refused("no run: set OA_RUN_ID to a run from scripts/dev/agent-run.py, so ending the "
                      "session reaps the machine (" + (r.stderr.strip() or r.stdout.strip())[:200] + ")")
    labels = dict(kv.split("=", 1) for kv in r.stdout.strip().split(",") if "=" in kv)
    return labels["run-id"], labels["run-host"]


def check_extra(extra: list[str]) -> None:
    for arg in extra:
        flag = arg.split("=", 1)[0]
        if flag in OWNED_FLAGS:
            raise Refused(f"{flag} is set by the guard; use --label K=V for extra labels")


def build_command(name: str, a: argparse.Namespace, labels: dict[str, str], seconds: int) -> list[str]:
    cmd = ["gcloud", "compute", "instances", "create", name, f"--project={a.project}", f"--zone={a.zone}",
           f"--machine-type={a.machine_type}",
           "--labels=" + ",".join(f"{k}={v}" for k, v in labels.items()),
           f"--max-run-duration={seconds}s",
           f"--instance-termination-action={a.on_expiry.upper()}"]
    if a.spot:
        cmd.append("--provisioning-model=SPOT")
    if getattr(a, "flex_start", False):
        # Dynamic Workload Scheduler: the request waits up to the queue time for capacity,
        # then the machine runs at most its run duration.
        cmd += ["--provisioning-model=FLEX_START", "--reservation-affinity=none",
                f"--request-valid-for-duration={queue_seconds(a)}s"]
    return cmd + list(a.extra)


def queue_seconds(a: argparse.Namespace) -> int:
    return max(60, math.ceil(round(a.queue_hours * 3600) / 60) * 60) if getattr(a, "flex_start", False) else 0


def cmd_create(a: argparse.Namespace) -> int:
    name = a.name
    if not re.match(r"^[a-z]([-a-z0-9]{0,61}[a-z0-9])?$", name):
        raise Refused(f"{name!r} is not a valid machine name")
    if PROTECTED.match(name):
        raise Refused(f"{name} is a production machine; this guard never touches it")
    if not (a.hours > 0):
        raise Refused("--hours must be more than zero")
    check_extra(a.extra)
    flex = getattr(a, "flex_start", False)
    if flex and a.spot:
        raise Refused("--spot and --flex-start are separate provisioning models; choose one")
    if flex and a.on_expiry != "delete":
        raise Refused("a flex-start machine is deleted at the end of its run; --on-expiry stop is refused")
    if flex and not (a.queue_hours > 0):
        raise Refused("--queue-hours must be more than zero")
    kind = "flex" if flex else ("spot" if a.spot else "standard")
    prices = load_prices()
    price, source = price_for(prices, a.machine_type, kind)
    seconds = max(60, math.ceil(round(a.hours * 3600) / 60) * 60)
    # A queued flex-start request may start as late as the end of its queue time, so it counts
    # from now until its queue time plus its run; sync settles it at the actual deletion.
    booked = seconds + queue_seconds(a)
    hours = booked / 3600
    try:
        rid, rhost = run_labels()
    except Refused:
        if not a.dry_run:
            raise
        rid, rhost = "no-run", "dry-run"
    extra_labels = {}
    for kv in a.label:
        k, _, v = kv.partition("=")
        if not LABEL_KEY.match(k) or not LABEL_VALUE.match(v):
            raise Refused(f"label {kv!r}: keys and values are lowercase letters, digits, - or _")
        extra_labels[k] = v

    if not a.dry_run:
        with locked(a.budget) as path:
            read_ledger(path, quiet=True).refuse_if_corrupt()
        sync(a.budget, quiet=True, project=a.project)

    with locked(a.budget) as path:
        records = read_ledger(path)
        records.refuse_if_corrupt()
        cap = ledger_cap(records)
        if a.cap is not None and cap is not None and abs(a.cap - cap) > 1e-9:
            raise Refused(f"budget {a.budget} was opened with cap {money(cap)}; it can't change here")
        cap = cap if cap is not None else (a.cap if a.cap is not None else DEFAULT_CAP)
        ms = machines(records)
        if any(m["vm"] == name and m.get("stop_epoch") is None for m in ms):
            raise Refused(f"{name} is already running in budget {a.budget}")
        at = now()
        reserve = ledger_reserve(records)
        verdict = admit(ms, cap, price, hours, at, reserve, ledger_charges(records))
        expires = at + booked
        labels = {"run-id": rid, "run-host": rhost, "budget": a.budget,
                  "expires-at": label_time(expires), **extra_labels}
        command = build_command(name, a, labels, seconds)
        record = {
            "event": "start", "id": uuid.uuid4().hex, "budget": a.budget, "run_id": rid,
            "vm": name, "zone": a.zone, "project": a.project, "machine_type": a.machine_type,
            "provisioning": {"flex": "flex-start", "standard": "standard"}.get(kind, kind),
            "price_per_hour": price,
            "price_source": source, "hours_requested": hours, "on_expiry": a.on_expiry,
            "start_epoch": at, "start": iso(at), "expires_epoch": expires, "expires_at": iso(expires),
        }
        summary = {"vm": name, "zone": a.zone, "project": a.project, "budget": a.budget, "cap": cap,
                   "reserve": reserve,
                   "price_per_hour": price, "hours": hours, "expires_at": iso(expires), "run_id": rid,
                   "cost_if_run_to_expiry": verdict["request"], "total_after": round(verdict["after"], 6),
                   "command": command, "dry_run": a.dry_run}
        if a.dry_run:
            if a.json:
                print(json.dumps(summary, indent=1))
            else:
                print(shlex.join(command))
                print(f"admitted: {money(verdict['request'])} for {hours:g} h; budget total would be "
                      f"{money(verdict['after'])} of {money(cap)} (dry run, nothing created)", file=sys.stderr)
            return 0
        for note in claim_authority(a.budget, path, records, a.take_over, at):
            append(path, note)
        if ledger_cap(records) is None:
            append(path, {"event": "budget", "budget": a.budget, "cap": cap, "at": iso(at),
                          "host": host_id(), "ledger": str(path.resolve())})
        append(path, record)

    r = gcloud(*command[1:])
    if r.returncode != 0:
        print(r.stderr.strip()[-2000:], file=sys.stderr)
        exists = gcloud("compute", "instances", "describe", name, f"--project={a.project}",
                        f"--zone={a.zone}", "--format=value(status)")
        if exists.returncode == 0 or "was not found" not in exists.stderr:
            print(f"cloud-budget: gcloud reported an error, but {name} may exist; it stays counted "
                  f"until `cloud-budget.py sync` or `stop {name}` records it", file=sys.stderr)
            return 1
        with locked(a.budget) as path:
            append(path, {"event": "stop", "id": record["id"], "vm": name, "stop_epoch": record["start_epoch"],
                          "stop": record["start"], "reason": "create-failed"})
        print(f"cloud-budget: gcloud did not create {name}; recorded no cost", file=sys.stderr)
        return 1
    if rid != "no-run":
        subprocess.run([sys.executable, str(AGENT_RUN), "add-vm", rid, name, a.zone, a.project],
                       capture_output=True, text=True)
    if a.json:
        print(json.dumps(summary, indent=1))
    else:
        print(f"vm {name} zone {a.zone} expires {iso(expires)} price {money(price)}/h "
              f"up to {money(verdict['request'])}; budget {money(verdict['after'])} of {money(cap)}")
    return 0


# --------------------------------------------------------------- reconcile

ENDING_OPS = ("delete", "stop", "preempted", "suspend")
STARTING_OPS = ("start", "resume")
# Records this long before a start record belong to an earlier machine of the same name.
EARLY_SLACK_SECONDS = 300


def _json(text: str):
    try:
        return json.loads(text or "[]")
    except json.JSONDecodeError:
        return None


def _kind(method: str) -> str | None:
    method = method.lower()
    if "insert" in method:
        return "insert"
    if any(k in method for k in ENDING_OPS):
        return "end"
    if any(k in method.rsplit(".", 1)[-1] for k in STARTING_OPS):
        return "start"
    return None


def describe(m: dict) -> dict:
    """The machine as Google sees it now: {"status": ..., "id": ..., "last_start": ..., ...}.
    The status is GONE when the name is not found or names another machine (a reused name), and
    None when gcloud could not answer. Another machine is one whose instance ID differs from the
    record's, or, when the record has no ID, from its creation operation's, or one created outside
    the record's start window; its timestamps are never returned, only its ID as `other_id`."""
    r = gcloud("compute", "instances", "describe", m["vm"], f"--project={m['project']}",
               f"--zone={m['zone']}",
               "--format=json(id,status,creationTimestamp,lastStartTimestamp,lastStopTimestamp)")
    if r.returncode != 0:
        gone = "was not found" in r.stderr or "notFound" in r.stderr
        return {"status": "GONE" if gone else None}
    info = _json(r.stdout or "{}")
    if not isinstance(info, dict):
        return {"status": None}
    ident = str(info.get("id") or "") or None
    created = parse_time(info.get("creationTimestamp", ""))
    if not same_machine(m, created):
        return {"status": "GONE", "other_id": ident}
    if ident:
        ours = m.get("instance_id") or creation_id(m, lifecycle_events(m, audit=False))
        if ours and ours != ident:
            return {"status": "GONE", "other_id": ident}
    return {"status": info.get("status"), "id": ident, "created": created,
            "last_start": parse_time(info.get("lastStartTimestamp") or ""),
            "last_stop": parse_time(info.get("lastStopTimestamp") or "")}


def lifecycle_events(m: dict, audit: bool = True) -> list[dict]:
    """Google's lifecycle records for the machine's name: its zone's operations and, because
    operations age out, the audit log. Each event is {"kind": insert|start|end, "at", "done",
    "ok", "instance_id", "begun"}; an insert's `at` is when it finished, which for a flex-start
    request is when the machine started."""
    events: list[dict] = []
    r = gcloud("compute", "operations", "list", f"--project={m['project']}", f"--zones={m['zone']}",
               f"--filter=targetLink~/instances/{m['vm']}$",
               "--format=json(operationType,status,insertTime,endTime,targetId,error)")
    ops = _json(r.stdout) if r.returncode == 0 else None
    for o in ops if isinstance(ops, list) else []:
        kind = _kind(str(o.get("operationType", "")))
        if not kind:
            continue
        begun = parse_time(o.get("insertTime", "") or "")
        ended = parse_time(o.get("endTime", "") or "")
        failed = bool(o.get("error"))
        done = o.get("status") == "DONE" or ended is not None or failed
        events.append({"kind": kind, "at": ended or (begun if failed else None), "done": done,
                       "ok": not failed, "instance_id": str(o.get("targetId") or "") or None,
                       "begun": begun, "source": "operation"})
    if audit:
        query = (f'resource.type="gce_instance" AND protoPayload.resourceName:"instances/{m["vm"]}" '
                 f'AND timestamp>="{iso(m["start_epoch"] - EARLY_SLACK_SECONDS)}"')
        r = gcloud("logging", "read", query, f"--project={m['project']}", "--freshness=400d",
                   "--format=json(timestamp,severity,protoPayload.methodName,protoPayload.resourceName,"
                   "protoPayload.status,operation,resource.labels.instance_id)")
        entries = _json(r.stdout) if r.returncode == 0 else None
        for e in entries if isinstance(entries, list) else []:
            payload = e.get("protoPayload") or {}
            resource = str(payload.get("resourceName", ""))
            if resource and not resource.endswith(f"/instances/{m['vm']}"):
                continue  # a longer name that shares this prefix
            kind = _kind(str(payload.get("methodName", "")))
            at = parse_time(e.get("timestamp", "") or "")
            if not kind or at is None:
                continue
            op = e.get("operation") or {}
            failed = bool((payload.get("status") or {}).get("code")) or e.get("severity") == "ERROR"
            if op.get("first") and not op.get("last"):
                events.append({"kind": kind, "at": None, "done": False, "ok": True, "begun": at,
                               "instance_id": ((e.get("resource") or {}).get("labels") or {}).get("instance_id"),
                               "source": "audit"})
                continue
            events.append({"kind": kind, "at": at, "done": True, "ok": not failed, "begun": None,
                           "instance_id": ((e.get("resource") or {}).get("labels") or {}).get("instance_id"),
                           "source": "audit"})
    return events


def creation_id(m: dict, events: list[dict]) -> str | None:
    """The instance ID of the insert that began nearest record `m`'s start, if one is on record."""
    inserts = [e for e in events if e["kind"] == "insert" and e.get("instance_id")
               and (e.get("begun") or e.get("at")) is not None
               and m["start_epoch"] - EARLY_SLACK_SECONDS <= (e.get("begun") or e["at"])
               <= m["start_epoch"] + SAME_MACHINE_SECONDS]
    if not inserts:
        return None
    return min(inserts, key=lambda e: abs((e.get("begun") or e["at"]) - m["start_epoch"]))["instance_id"]


def instance_of(m: dict, events: list[dict], seen: dict) -> str | None:
    """The instance ID behind record `m`: the record's own, else its creation operation's, else
    the one Google describes for it."""
    return m.get("instance_id") or creation_id(m, events) or seen.get("id")


def validated(m: dict, seen: dict, ident: str | None) -> dict:
    """The describe snapshot when it is record `m`'s machine, else a bare GONE: a snapshot of
    another instance ID, or of a machine created outside the start window, lends no timestamps."""
    if seen.get("other_id") or (ident and seen.get("id") and seen["id"] != ident) \
            or not same_machine(m, seen.get("created")):
        return {"status": "GONE"}
    return seen


def settle(m: dict, seen: dict, events: list[dict], deleted_at: float | None = None) -> dict | None:
    """The one lifecycle settlement that manual stop, sync, and adoption share.

    Given the ledger record, the describe snapshot, and Google's lifecycle events, return the stop
    record's fields ({"stop_epoch", "reason", "run_start_epoch", "instance_id", "evidence"}), or
    None while the evidence can't say what the machine cost; it then stays counted.

    A flex-start request bills from when its machine started (its insert finished without an
    error, or `lastStartTimestamp`), and costs nothing when its insert finished with an error and
    no start is on record. Other machines bill from their start record, as before. `deleted_at` is
    when this guard deleted the machine: it bounds the end."""
    ident = instance_of(m, events, seen)
    seen = validated(m, seen, ident)
    if ident:
        events = [e for e in events if e.get("instance_id") in (None, "", ident)]
    floor = m["start_epoch"] - EARLY_SLACK_SECONDS
    events = [e for e in events if (e.get("at") or e.get("begun") or floor) >= floor]
    flex = m.get("provisioning") == "flex-start"
    inserts = [e for e in events if e["kind"] == "insert" and e["done"]]
    started = sorted(e["at"] for e in events if e["at"] is not None and e["ok"] and e["done"]
                     and e["kind"] in ("insert", "start"))
    run_start = m.get("run_start_epoch") or seen.get("last_start")
    if flex and run_start is None and started:
        run_start = started[0]
    failed = [e for e in inserts if not e["ok"]]
    begin = run_start if (flex and run_start) else m["start_epoch"]
    ends = sorted(e["at"] for e in events if e["kind"] == "end" and e["ok"] and e["done"]
                  and e["at"] is not None and e["at"] >= begin)
    if seen.get("status") in ("TERMINATED", "STOPPED") and (seen.get("last_stop") or 0) >= begin:
        ends.append(seen["last_stop"])
        ends.sort()
    out = {"instance_id": ident}
    if flex and run_start is None and failed and not any(e["ok"] for e in inserts):
        # The request left the queue without a machine: capacity never came, or it was cancelled.
        at = min(e["at"] for e in failed if e["at"] is not None) if any(e["at"] for e in failed) else None
        at = at or deleted_at or now()
        return {**out, "stop_epoch": at, "reason": "never-started", "run_start_epoch": None,
                "evidence": "insert ended with an error and no start is on record"}
    if flex and run_start is None:
        return None  # it may have started before it ended; wait for the insert's outcome
    end = ends[0] if ends else None
    if deleted_at is not None:
        end = min(end, deleted_at) if end is not None else deleted_at
    if end is None:
        return None
    return {**out, "stop_epoch": end, "reason": "gone", "run_start_epoch": run_start if flex else None,
            "evidence": "lifecycle records"}


def label_epoch(text: str) -> float | None:
    m = re.match(r"^(\d{4})-(\d\d)-(\d\d)t(\d\d)-(\d\d)-(\d\d)z$", text or "")
    if not m:
        return None
    return datetime(*map(int, m.groups()), tzinfo=timezone.utc).timestamp()


def same_machine(m: dict, created: float | None) -> bool:
    """Whether a machine Google created at `created` is the one record `m` started: a reused name
    is a different machine."""
    return created is None or abs(created - m["start_epoch"]) <= SAME_MACHINE_SECONDS


def adopt(budget: str, project: str, quiet: bool) -> int:
    """Record machines labeled with this budget that have no running record, such as a create
    whose reply was lost, so they count. Returns how many it recorded."""
    r = gcloud("compute", "instances", "list", f"--project={project}", f"--filter=labels.budget={budget}",
               "--format=json(id,name,zone,machineType,status,creationTimestamp,lastStartTimestamp,labels,"
               "scheduling)")
    found = _json(r.stdout) if r.returncode == 0 else None
    if not isinstance(found, list):
        print(f"cloud-budget: couldn't list machines labeled budget={budget}; only the ledger's "
              "machines count", file=sys.stderr)
        return 0
    prices = load_prices()
    added = 0
    with locked(budget) as path:
        live = [m for m in machines(read_ledger(path, quiet=True)) if m.get("stop_epoch") is None]
        for inst in found:
            name, zone = inst.get("name", ""), str(inst.get("zone", "")).rsplit("/", 1)[-1]
            created = parse_time(inst.get("creationTimestamp", ""))
            ident = str(inst.get("id") or "") or None
            if inst.get("status") not in LIVE_STATUSES + ("PENDING",) or PROTECTED.match(name) or created is None:
                continue
            if any(m["vm"] == name and m["zone"] == zone and same_machine(m, created)
                   and (not ident or not m.get("instance_id") or m["instance_id"] == ident) for m in live):
                continue
            mtype = str(inst.get("machineType", "")).rsplit("/", 1)[-1]
            model = (inst.get("scheduling") or {}).get("provisioningModel")
            spot = model == "SPOT"
            labels = inst.get("labels") or {}
            try:
                price, source = price_for(prices, mtype, "flex" if model == "FLEX_START" else spot)
            except Refused:
                price = max(float(e.get("standard") or 0) for e in prices["machines"].values())
                source = f"no price for {mtype}; counted at the highest listed price"
            at = now()
            expires = label_epoch(labels.get("expires-at", "")) or at + ADOPT_HOURS * 3600
            record = {"event": "start", "id": uuid.uuid4().hex, "budget": budget, "adopted": True,
                      "run_id": labels.get("run-id", ""), "vm": name, "zone": zone, "project": project,
                      "machine_type": mtype,
                      "provisioning": "flex-start" if model == "FLEX_START" else ("spot" if spot else "standard"),
                      "price_per_hour": price, "price_source": source,
                      "hours_requested": round((expires - created) / 3600, 6), "on_expiry": "unknown",
                      "start_epoch": created, "start": iso(created), "expires_epoch": expires,
                      "expires_at": iso(expires)}
            if ident:
                record["instance_id"] = ident
            started = parse_time(inst.get("lastStartTimestamp") or "")
            if model == "FLEX_START" and started:
                record["run_start_epoch"] = started  # it bills from its start, not its queued request
            append(path, record)
            live.append(record)
            added += 1
            print(f"cloud-budget: {name} carries budget={budget} but had no running record; it now "
                  f"counts from {iso(created)} to {iso(expires)} at {money(price)}/h", file=sys.stderr)
    return added


def resolve(m: dict, seen: dict, deleted_at: float | None = None) -> dict | None:
    """Settle from the zone's operations, and read the slower audit log only when they can't."""
    done = settle(m, seen, lifecycle_events(m, audit=False), deleted_at)
    return done if done is not None else settle(m, seen, lifecycle_events(m, audit=True), deleted_at)


def stop_record(m: dict, done: dict) -> dict:
    at = done["stop_epoch"]
    c = cost({**m, "stop_epoch": at, "reason": done["reason"], "run_start_epoch": done.get("run_start_epoch")}, at)
    record = {"event": "stop", "id": m["id"], "vm": m["vm"], "stop_epoch": at, "stop": iso(at),
              "reason": done["reason"], "cost": c}
    for key in ("run_start_epoch", "instance_id"):
        if done.get(key) is not None:
            record[key] = done[key]
    return record


def sync(budget: str, quiet: bool = False, project: str = DEFAULT_PROJECT) -> int:
    with locked(budget) as path:
        records = read_ledger(path, quiet=quiet)
    projects = {project} | {m["project"] for m in machines(records)}
    for p in sorted(projects):
        adopt(budget, p, quiet)
    with locked(budget) as path:
        live = [m for m in machines(read_ledger(path, quiet=True)) if m.get("stop_epoch") is None]
    ended = 0
    for m in live:
        seen = describe(m)
        status = seen.get("status")
        if status not in ("GONE", "TERMINATED", "STOPPED") and m.get("deleted_epoch") is None:
            continue
        if status is None or status in LIVE_STATUSES or status == "PENDING":
            continue  # deleted by `stop`, but Google still has it
        done = resolve(m, seen, m.get("deleted_epoch"))
        label = "gone" if status == "GONE" else "stopped"
        if done is not None and done["reason"] == "gone":
            done["reason"] = label
        if done is None:
            # Google's records don't say when it started or ended. Never release money on a guess:
            # it counts in full until its expiry, and is settled at the expiry (or this guard's
            # delete, if earlier) from its start record after that.
            if now() < m["expires_epoch"]:
                if not quiet:
                    print(f"{m['vm']}: {label}, but Google's records don't say when it ran; it counts "
                          f"until its expiry {m['expires_at']}")
                continue
            end = min(m["expires_epoch"], m.get("deleted_epoch") or m["expires_epoch"])
            done = {"stop_epoch": end, "reason": label + "-at-expiry", "run_start_epoch": None}
        with locked(budget) as path:
            still = [x for x in machines(read_ledger(path)) if x["id"] == m["id"] and x.get("stop_epoch") is None]
            if still:
                record = stop_record(m, done)
                append(path, record)
                ended += 1
                if not quiet:
                    print(f"{m['vm']}: {record['reason']} at {record['stop']}, cost {money(record['cost'])}")
    return ended


def cmd_stop(a: argparse.Namespace) -> int:
    if PROTECTED.match(a.name):
        raise Refused(f"{a.name} is a production machine; this guard never touches it")
    with locked(a.budget) as path:
        live = [m for m in machines(read_ledger(path)) if m["vm"] == a.name and m.get("stop_epoch") is None]
    if not live:
        raise Refused(f"{a.name} is not running in budget {a.budget}")
    m = live[-1]
    flex = m.get("provisioning") == "flex-start"
    # A queued flex-start request costs nothing until Google starts the machine. The snapshot
    # before the delete only names the machine: it may start between it and the delete.
    seen = describe(m)
    if seen.get("other_id"):
        # The ledger's machine is gone and another machine now has its name: never delete that one.
        done = resolve(m, {"status": "GONE"})
        print(f"cloud-budget: {m['vm']} is now another machine (instance {seen['other_id']}); the "
              "ledger's machine is gone, so stop didn't delete it", file=sys.stderr)
        if done is None:
            print(f"cloud-budget: Google's records don't yet say when the ledger's {m['vm']} ran; it "
                  "counts until `cloud-budget.py sync` settles it", file=sys.stderr)
            return 1
        with locked(a.budget) as path:
            if any(x["id"] == m["id"] and x.get("stop_epoch") is None for x in machines(read_ledger(path))):
                record = stop_record(m, done)
                append(path, record)
                print(f"{m['vm']}: {record['reason']} at {record['stop']}, cost {money(record['cost'])}")
        return 1
    if seen.get("status") == "GONE" or not flex:
        seen = {}
    if not a.keep_machine:
        r = gcloud("compute", "instances", "delete", m["vm"], f"--project={m['project']}",
                   f"--zone={m['zone']}", "--quiet")
        if r.returncode != 0 and "was not found" not in r.stderr:
            print(r.stderr.strip()[-2000:], file=sys.stderr)
            return 1
    at = min(now(), m["expires_epoch"])
    if a.keep_machine:
        # The machine stays, so nothing ends it: count it until now, from its start if Google says
        # it started, else from its start record.
        done = {"stop_epoch": at, "reason": "kept", "instance_id": seen.get("id"),
                "run_start_epoch": seen.get("last_start") if flex else None}
    elif flex:
        done = resolve(m, seen, deleted_at=at)
        if done is not None and done["reason"] != "never-started":
            done["reason"] = "deleted"
    else:
        done = {"stop_epoch": at, "reason": "deleted", "run_start_epoch": None}
    if done is None:
        with locked(a.budget) as path:
            append(path, {"event": "pending", "id": m["id"], "vm": m["vm"], "deleted_epoch": at,
                          "deleted": iso(at), "reason": "start-unknown", "instance_id": seen.get("id")})
        print(f"{m['vm']}: deleted at {iso(at)}; Google's records don't yet say whether it started, so "
              f"it counts until `cloud-budget.py sync` settles it")
        return 0
    with locked(a.budget) as path:
        record = stop_record(m, done)
        append(path, record)
    print(f"{m['vm']}: stopped at {record['stop']}, cost {money(record['cost'])}")
    return 0


def find_closed(records: list[dict], key: str) -> dict:
    ms = [m for m in machines(records) if m.get("stop_epoch") is not None]
    hits = [m for m in ms if m["id"] == key] or [m for m in ms if len(key) >= 6 and m["id"].startswith(key)]
    if not hits:
        hits = [m for m in ms if m["vm"] == key and m.get("reason") != "create-failed"]
    if not hits:
        raise Refused(f"no finished machine in the ledger has the ID or name {key}")
    if len(hits) > 1:
        raise Refused(f"{key} matches {len(hits)} finished records; name one by ID: "
                      + ", ".join(f"{m['id'][:12]} ({m['vm']}, {m.get('start')})" for m in hits))
    return hits[0]


def cmd_correct(a: argparse.Namespace) -> int:
    """Append a correction to a finished machine's settlement. History is never rewritten: the
    correction is a new record, and the ledger reads it in place of the earlier stop."""
    if not a.why.strip():
        raise Refused("say why the record is wrong with --why")
    chosen = [bool(a.never_started), bool(a.from_google), a.end is not None]
    if sum(chosen) != 1:
        raise Refused("choose one of --never-started, --from-google, or --end TIME [--run-start TIME]")
    with locked(a.budget) as path:
        records = read_ledger(path)
    m = find_closed(records, a.record)
    if a.never_started:
        done = {"stop_epoch": m["stop_epoch"], "reason": "never-started", "run_start_epoch": None}
    elif a.from_google:
        # Settle from Google's records alone, not from the settlement being corrected.
        fresh = {k: v for k, v in m.items() if k not in ("run_start_epoch", "stop_epoch", "stop", "reason")}
        done = resolve(fresh, describe(fresh))
        if done is None:
            raise Refused(f"Google's records don't say when {m['vm']} ran; correct it with --end and "
                          "--run-start from the billing console")
        done["reason"] = "corrected" if done["reason"] in ("gone", "stopped") else done["reason"]
    else:
        end = parse_time(a.end)
        run_start = parse_time(a.run_start) if a.run_start else None
        if end is None or (a.run_start and run_start is None):
            raise Refused("--end and --run-start take ISO times, such as 2026-10-21T10:00:00Z")
        done = {"stop_epoch": end, "reason": "corrected", "run_start_epoch": run_start}
    before = cost(m, m["stop_epoch"])
    record = stop_record(m, done)
    if (abs(record["cost"] - before) < 1e-6 and record["reason"] == m.get("reason")
            and record.get("run_start_epoch") == m.get("run_start_epoch")):
        print(f"{m['vm']} ({m['id'][:12]}): already settled at {money(before)}; nothing to correct")
        return 0
    record.update({"event": "correction", "previous_cost": before, "previous_reason": m.get("reason"),
                   "why": a.why.strip(), "at": iso(now()), "host": host_id()})
    if a.dry_run:
        print(json.dumps(record, indent=1, sort_keys=True))
        return 0
    with locked(a.budget) as path:
        append(path, record)
    print(f"{m['vm']} ({m['id'][:12]}): corrected from {money(before)} to {money(record['cost'])}")
    return 0


def cmd_report(a: argparse.Namespace) -> int:
    if not a.offline:
        sync(a.budget, quiet=a.json)
    with locked(a.budget) as path:
        records = read_ledger(path, quiet=True)
    cap = ledger_cap(records) or (a.cap if a.cap is not None else DEFAULT_CAP)
    at = now()
    ms = machines(records)
    t = totals(ms, at, ledger_reserve(records), ledger_charges(records))
    remaining = cap - t["spent"] - t["running_committed"] - t["reserve"]
    owner = ledger_owner(records)
    rows = [{"vm": m["vm"], "zone": m["zone"], "machine_type": m["machine_type"],
             "provisioning": m["provisioning"], "price_per_hour": m["price_per_hour"],
             "age_seconds": int(at - m["start_epoch"]), "expires_at": m["expires_at"],
             "cost_so_far": cost(m, at), "cost_to_expiry": committed(m, at)} for m in t["live"]]
    if a.json:
        print(json.dumps({"budget": a.budget, "cap": cap, "spent": t["spent"], "charges": t["charges"],
                          "running": t["running"],
                          "running_committed": t["running_committed"], "reserve": t["reserve"],
                          "remaining": round(remaining, 6), "finished": t["finished"],
                          "owner": owner, "corrupt_lines": records.corrupt, "running_machines": rows}, indent=1))
        return 0
    print(f"budget {a.budget}  cap {money(cap)}")
    if records.corrupt:
        print(f"CORRUPT    line {', '.join(map(str, records.corrupt))} of {records.path} can't be read; "
              "the totals below leave it out, and create refuses until it is repaired")
    print(f"spent      {money(t['spent'])}  ({t['finished']} finished"
          + (f", {money(t['charges'])} of it charges other than machines" if t["charges"] else "") + ")")
    print(f"running    {money(t['running'])} so far, up to {money(t['running_committed'])} by expiry "
          f"({len(rows)} running)")
    print(f"reserve    {money(t['reserve'])}" + ("" if t["reserve"] else
                                                  "  (none set; storage and network aren't counted)"))
    print(f"remaining  {money(remaining)}")
    if owner:
        print(f"allocates  from {owner['host']}")
    for row in rows:
        left = parse_time(row["expires_at"]) - at
        print(f"  {row['vm']}  {row['zone']}  {row['machine_type']} {row['provisioning']}  "
              f"{money(row['price_per_hour'])}/h  age {duration(row['age_seconds'])}  "
              f"expires {row['expires_at']} (in {duration(left)})  {money(row['cost_so_far'])} so far")
    return 0


def cmd_reserve(a: argparse.Namespace) -> int:
    if not (a.amount >= 0):
        raise Refused("the reserve can't be negative")
    if not a.why.strip():
        raise Refused("say what the reserve covers with --why")
    with locked(a.budget) as path:
        records = read_ledger(path)
        records.refuse_if_corrupt()
        at = now()
        cap = ledger_cap(records)
        if a.cap is not None and cap is not None and abs(a.cap - cap) > 1e-9:
            raise Refused(f"budget {a.budget} was opened with cap {money(cap)}; it can't change here")
        cap = cap if cap is not None else (a.cap if a.cap is not None else DEFAULT_CAP)
        for note in claim_authority(a.budget, path, records, a.take_over, at):
            append(path, note)
        if ledger_cap(records) is None:
            append(path, {"event": "budget", "budget": a.budget, "cap": cap, "at": iso(at),
                          "host": host_id(), "ledger": str(path.resolve())})
        append(path, {"event": "reserve", "budget": a.budget, "amount": round(a.amount, 6),
                      "why": a.why.strip(), "at": iso(at), "host": host_id()})
        t = totals(machines(records), at, a.amount, ledger_charges(records))
    left = cap - t["spent"] - t["running_committed"] - t["reserve"]
    print(f"reserve {money(a.amount)} for {a.why.strip()}; remaining {money(left)} of {money(cap)}")
    if left < 0:
        print("cloud-budget: the reserve is more than the budget has left; create refuses until "
              "machines finish", file=sys.stderr)
    return 0


def cmd_charge(a: argparse.Namespace) -> int:
    """Record spend that isn't a machine, such as network transfer, as spent."""
    if not (a.amount > 0):
        raise Refused("a charge must be more than zero")
    if not a.why.strip():
        raise Refused("say what the charge is for with --why")
    with locked(a.budget) as path:
        records = read_ledger(path)
        records.refuse_if_corrupt()
        at = now()
        cap = ledger_cap(records)
        if a.cap is not None and cap is not None and abs(a.cap - cap) > 1e-9:
            raise Refused(f"budget {a.budget} was opened with cap {money(cap)}; it can't change here")
        cap = cap if cap is not None else (a.cap if a.cap is not None else DEFAULT_CAP)
        for note in claim_authority(a.budget, path, records, a.take_over, at):
            append(path, note)
        if ledger_cap(records) is None:
            append(path, {"event": "budget", "budget": a.budget, "cap": cap, "at": iso(at),
                          "host": host_id(), "ledger": str(path.resolve())})
        entry = {"event": "charge", "budget": a.budget, "amount": round(a.amount, 6),
                 "why": a.why.strip(), "at": iso(at), "host": host_id()}
        if a.estimate:
            entry["estimate"] = True
        append(path, entry)
        records = read_ledger(path, quiet=True)
        t = totals(machines(records), at, ledger_reserve(records), ledger_charges(records))
    left = cap - t["spent"] - t["running_committed"] - t["reserve"]
    print(f"charged {money(a.amount)} for {a.why.strip()}; spent {money(t['spent'])}, "
          f"remaining {money(left)} of {money(cap)}")
    return 0


def cmd_prices(_: argparse.Namespace) -> int:
    prices = load_prices()
    for name, e in sorted(prices["machines"].items()):
        spot = e.get("spot")
        print(f"{name:<18} on demand {money(e['standard']) + '/h':>12}  spot "
              f"{(money(spot) + '/h') if spot else 'no price':>12}  ({prices['sources'].get(e.get('source'), '')})")
    return 0


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    extra: list[str] = []
    if "--" in argv:
        i = argv.index("--")
        argv, extra = argv[:i], argv[i + 1:]
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--budget", default=DEFAULT_BUDGET)
    common.add_argument("--cap", type=float)
    ap = argparse.ArgumentParser(description="spend guard for rented Google Compute machines (#12370)")
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("create", parents=[common])
    c.add_argument("name")
    c.add_argument("--machine-type", required=True)
    c.add_argument("--zone", required=True)
    c.add_argument("--hours", type=float, required=True)
    c.add_argument("--spot", action="store_true")
    c.add_argument("--flex-start", action="store_true",
                   help="queue for capacity through the Dynamic Workload Scheduler (FLEX_START)")
    c.add_argument("--queue-hours", type=float, default=2.0,
                   help="flex-start: how long the request may wait for capacity (default 2)")
    c.add_argument("--on-expiry", choices=["delete", "stop"], default="delete")
    c.add_argument("--label", action="append", default=[])
    c.add_argument("--project", default=os.environ.get("OA_PROJECT", DEFAULT_PROJECT))
    c.add_argument("--take-over", action="store_true",
                   help="allocate from this host and ledger although another one did; recorded")
    c.add_argument("--dry-run", action="store_true")
    c.add_argument("--json", action="store_true")
    s = sub.add_parser("stop", parents=[common])
    s.add_argument("name")
    s.add_argument("--keep-machine", action="store_true", help="record the stop without deleting the machine")
    k = sub.add_parser("correct", parents=[common],
                       help="append a correction to a finished machine's cost; history is never rewritten")
    k.add_argument("record", help="the start record's ID (or a unique prefix of 6+), or the machine name")
    k.add_argument("--why", required=True)
    k.add_argument("--never-started", action="store_true", help="it never ran: no cost")
    k.add_argument("--from-google", action="store_true", help="settle again from Google's lifecycle records")
    k.add_argument("--run-start", help="flex-start: when the machine started running (ISO time)")
    k.add_argument("--end", help="when the machine stopped (ISO time)")
    k.add_argument("--dry-run", action="store_true")
    y = sub.add_parser("sync", parents=[common])
    y.add_argument("--project", default=os.environ.get("OA_PROJECT", DEFAULT_PROJECT))
    v = sub.add_parser("reserve", parents=[common],
                       help="set the reserve for disks, buckets, checkpoint copies, and network egress")
    v.add_argument("amount", type=float)
    v.add_argument("--why", required=True)
    v.add_argument("--take-over", action="store_true")
    g = sub.add_parser("charge", parents=[common],
                       help="record spend that isn't a machine, such as network transfer or a bucket copy")
    g.add_argument("amount", type=float)
    g.add_argument("--why", required=True)
    g.add_argument("--estimate", action="store_true",
                   help="the amount is an estimate, to correct from the bill")
    g.add_argument("--take-over", action="store_true")
    r = sub.add_parser("report", parents=[common])
    r.add_argument("--offline", action="store_true")
    r.add_argument("--json", action="store_true")
    sub.add_parser("prices", parents=[common])
    a = ap.parse_args(argv)
    a.extra = extra
    if extra and a.cmd != "create":
        ap.error("extra gcloud flags after -- only go with create")
    try:
        if a.cmd == "create":
            return cmd_create(a)
        if a.cmd == "stop":
            return cmd_stop(a)
        if a.cmd == "sync":
            sync(a.budget, project=a.project)
            return 0
        if a.cmd == "reserve":
            return cmd_reserve(a)
        if a.cmd == "correct":
            return cmd_correct(a)
        if a.cmd == "charge":
            return cmd_charge(a)
        if a.cmd == "report":
            return cmd_report(a)
        return cmd_prices(a)
    except Refused as why:
        print(f"cloud-budget: refused: {why}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
