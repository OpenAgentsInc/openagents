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

The ledger is one append-only JSON-lines file per budget:
~/.openagents/cloud-budget/<budget>.jsonl (OA_CLOUD_BUDGET_DIR). Each machine
has a `start` record and, once it ends, a `stop` record with its cost. A torn
last line (a crash mid-write) is skipped on read, and the next append starts a
fresh line.

    cloud-budget.py create NAME --machine-type T --zone Z --hours H [--spot]
        [--on-expiry delete|stop] [--label K=V ...] [--dry-run] [--json]
        [-- EXTRA GCLOUD FLAGS ...]
    cloud-budget.py stop NAME [--keep-machine]   delete the machine, record cost
    cloud-budget.py sync                         record machines that ended
    cloud-budget.py report [--offline] [--json]  spent, running, remaining
    cloud-budget.py prices                       the price table

Common options: --budget NAME (default deepseek-v41), --cap DOLLARS (default
3000, or the cap the budget's ledger first recorded), --project P (default
openagentsgemini).

Environment: OA_CLOUD_BUDGET_DIR (ledger directory), OA_CLOUD_PRICES (price
table), OA_GCLOUD (gcloud binary), OA_AGENT_RUN (agent-run.py path),
OA_CLOUD_BUDGET_NOW (epoch seconds, for tests), CLOUDSDK_CONFIG (when unset,
~/work/.secrets/gcloud-sa-config is used if present, as agent-run.py does).
"""

from __future__ import annotations

import argparse
import fcntl
import json
import math
import os
import re
import shlex
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
DEFAULT_BUDGET = "deepseek-v41"
DEFAULT_CAP = 3000.0
DEFAULT_PROJECT = "openagentsgemini"
# Production machines this guard never creates, stops, or counts.
PROTECTED = re.compile(r"^(clef-decisions-h100|clef-decisions-a100|oa-pylon-gpu-1|oa-att-.*)$")
# Flags the guard sets itself; passing them as extra flags is refused.
OWNED_FLAGS = ("--labels", "--max-run-duration", "--termination-time", "--instance-termination-action",
               "--provisioning-model", "--preemptible", "--machine-type", "--zone", "--project")
MIN_BILLED_SECONDS = 60  # Compute Engine bills per second with a one-minute minimum.
LABEL_VALUE = re.compile(r"^[a-z0-9_-]{0,63}$")
LABEL_KEY = re.compile(r"^[a-z][a-z0-9_-]{0,62}$")


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


def price_for(prices: dict, machine_type: str, spot: bool) -> tuple[float, str]:
    entry = prices.get("machines", {}).get(machine_type)
    kind = "spot" if spot else "standard"
    if not entry or not isinstance(entry.get(kind), (int, float)) or entry[kind] <= 0:
        raise Refused(f"no {'spot' if spot else 'on-demand'} price for {machine_type} in {PRICES}; "
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


def read_ledger(path: Path) -> list[dict]:
    try:
        data = path.read_bytes()
    except FileNotFoundError:
        return []
    records = []
    lines = data.split(b"\n")
    for i, raw in enumerate(lines):
        if not raw.strip():
            continue
        try:
            records.append(json.loads(raw))
        except (json.JSONDecodeError, UnicodeDecodeError):
            where = "torn last line" if i == len(lines) - 1 else f"unreadable line {i + 1}"
            print(f"cloud-budget: skipped a {where} in {path}", file=sys.stderr)
    return records


def append(path: Path, record: dict) -> None:
    line = (json.dumps(record, sort_keys=True) + "\n").encode()
    fd = os.open(path, os.O_RDWR | os.O_CREAT | os.O_APPEND, 0o600)
    try:
        size = os.fstat(fd).st_size
        if size:
            os.lseek(fd, size - 1, os.SEEK_SET)
            if os.read(fd, 1) != b"\n":
                line = b"\n" + line  # a crash tore the last record; start a new line
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
        elif r.get("event") == "stop" and r.get("id") in by_id:
            m = by_id[r["id"]]
            m["stop_epoch"], m["stop"], m["reason"] = r["stop_epoch"], r.get("stop"), r.get("reason")
    return list(by_id.values())


def ledger_cap(records: list[dict]) -> float | None:
    for r in records:
        if r.get("event") == "budget":
            return float(r["cap"])
    return None


def cost(m: dict, at: float) -> float:
    """What a machine has cost by `at`: per second, one-minute minimum, never past its expiry."""
    if m.get("reason") == "create-failed":
        return 0.0
    end = m.get("stop_epoch")
    if end is None:
        end = min(at, m["expires_epoch"])
    seconds = max(end - m["start_epoch"], MIN_BILLED_SECONDS)
    return round(m["price_per_hour"] * seconds / 3600, 6)


def committed(m: dict) -> float:
    """The most a running machine can cost: until its expiry."""
    return round(m["price_per_hour"] * max(m["expires_epoch"] - m["start_epoch"], MIN_BILLED_SECONDS) / 3600, 6)


def totals(ms: list[dict], at: float) -> dict:
    done = [m for m in ms if m.get("stop_epoch") is not None]
    live = [m for m in ms if m.get("stop_epoch") is None]
    return {
        "spent": round(sum(cost(m, at) for m in done), 6),
        "running": round(sum(cost(m, at) for m in live), 6),
        "running_committed": round(sum(committed(m) for m in live), 6),
        "finished": len(done),
        "live": live,
    }


# ---------------------------------------------------------------- admission

def admit(ms: list[dict], cap: float, price: float, hours: float, at: float) -> dict:
    t = totals(ms, at)
    request = round(price * hours, 6)
    after = t["spent"] + t["running_committed"] + request
    if after > cap + 1e-9:
        raise Refused(f"over the cap: spent {money(t['spent'])} + running up to "
                      f"{money(t['running_committed'])} + this machine {money(request)} "
                      f"= {money(after)} > cap {money(cap)}")
    return {"request": request, "after": after, **{k: t[k] for k in ("spent", "running_committed")}}


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
    return cmd + list(a.extra)


def cmd_create(a: argparse.Namespace) -> int:
    name = a.name
    if not re.match(r"^[a-z]([-a-z0-9]{0,61}[a-z0-9])?$", name):
        raise Refused(f"{name!r} is not a valid machine name")
    if PROTECTED.match(name):
        raise Refused(f"{name} is a production machine; this guard never touches it")
    if not (a.hours > 0):
        raise Refused("--hours must be more than zero")
    check_extra(a.extra)
    prices = load_prices()
    price, source = price_for(prices, a.machine_type, a.spot)
    seconds = max(60, math.ceil(round(a.hours * 3600) / 60) * 60)
    hours = seconds / 3600
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

    with locked(a.budget) as path:
        records = read_ledger(path)
        cap = ledger_cap(records)
        if a.cap is not None and cap is not None and abs(a.cap - cap) > 1e-9:
            raise Refused(f"budget {a.budget} was opened with cap {money(cap)}; it can't change here")
        cap = cap if cap is not None else (a.cap if a.cap is not None else DEFAULT_CAP)
        ms = machines(records)
        if any(m["vm"] == name and m.get("stop_epoch") is None for m in ms):
            raise Refused(f"{name} is already running in budget {a.budget}")
        at = now()
        verdict = admit(ms, cap, price, hours, at)
        expires = at + seconds
        labels = {"run-id": rid, "run-host": rhost, "budget": a.budget,
                  "expires-at": label_time(expires), **extra_labels}
        command = build_command(name, a, labels, seconds)
        record = {
            "event": "start", "id": uuid.uuid4().hex, "budget": a.budget, "run_id": rid,
            "vm": name, "zone": a.zone, "project": a.project, "machine_type": a.machine_type,
            "provisioning": "spot" if a.spot else "standard", "price_per_hour": price,
            "price_source": source, "hours_requested": hours, "on_expiry": a.on_expiry,
            "start_epoch": at, "start": iso(at), "expires_epoch": expires, "expires_at": iso(expires),
        }
        summary = {"vm": name, "zone": a.zone, "project": a.project, "budget": a.budget, "cap": cap,
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
        if ledger_cap(records) is None:
            append(path, {"event": "budget", "budget": a.budget, "cap": cap, "at": iso(at)})
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

def end_time(m: dict) -> float | None:
    """When a machine that no longer runs stopped, from its zone's operations.

    Expiry shows as `compute.instances.deferredDelete` (or a stop) run by `system`.
    """
    r = gcloud("compute", "operations", "list", f"--project={m['project']}", f"--zones={m['zone']}",
               f"--filter=targetLink~/instances/{m['vm']}$", "--format=json(operationType,endTime)")
    if r.returncode != 0:
        return None
    try:
        ops = json.loads(r.stdout or "[]")
    except json.JSONDecodeError:
        return None
    ends = [parse_time(o.get("endTime", "")) for o in ops
            if any(k in o.get("operationType", "").lower() for k in ("delete", "stop", "preempted"))]
    ends = [e for e in ends if e and e >= m["start_epoch"]]
    return min(ends) if ends else None


def sync(budget: str, quiet: bool = False) -> int:
    with locked(budget) as path:
        live = [m for m in machines(read_ledger(path)) if m.get("stop_epoch") is None]
    ended = 0
    for m in live:
        r = gcloud("compute", "instances", "describe", m["vm"], f"--project={m['project']}",
                   f"--zone={m['zone']}", "--format=json(status,lastStopTimestamp,creationTimestamp)")
        status = None
        if r.returncode == 0:
            info = json.loads(r.stdout or "{}")
            created = parse_time(info.get("creationTimestamp", "")) or 0
            if created and created > m["expires_epoch"]:
                status = "GONE"  # a newer machine reuses the name; ours is gone
            else:
                status = info.get("status")
        elif "was not found" in r.stderr or "notFound" in r.stderr:
            status = "GONE"
        if status in ("GONE", "TERMINATED", "STOPPED"):
            at = end_time(m) or min(now(), m["expires_epoch"])
            reason = "gone" if status == "GONE" else "stopped"
            with locked(budget) as path:
                still = [x for x in machines(read_ledger(path)) if x["id"] == m["id"] and x.get("stop_epoch") is None]
                if still:
                    append(path, {"event": "stop", "id": m["id"], "vm": m["vm"], "stop_epoch": at,
                                  "stop": iso(at), "reason": reason, "cost": cost({**m, "stop_epoch": at}, at)})
                    ended += 1
                    if not quiet:
                        print(f"{m['vm']}: {reason} at {iso(at)}, cost {money(cost({**m, 'stop_epoch': at}, at))}")
    return ended


def cmd_stop(a: argparse.Namespace) -> int:
    if PROTECTED.match(a.name):
        raise Refused(f"{a.name} is a production machine; this guard never touches it")
    with locked(a.budget) as path:
        live = [m for m in machines(read_ledger(path)) if m["vm"] == a.name and m.get("stop_epoch") is None]
    if not live:
        raise Refused(f"{a.name} is not running in budget {a.budget}")
    m = live[-1]
    if not a.keep_machine:
        r = gcloud("compute", "instances", "delete", m["vm"], f"--project={m['project']}",
                   f"--zone={m['zone']}", "--quiet")
        if r.returncode != 0 and "was not found" not in r.stderr:
            print(r.stderr.strip()[-2000:], file=sys.stderr)
            return 1
    at = min(now(), m["expires_epoch"])
    with locked(a.budget) as path:
        c = cost({**m, "stop_epoch": at}, at)
        append(path, {"event": "stop", "id": m["id"], "vm": m["vm"], "stop_epoch": at, "stop": iso(at),
                      "reason": "kept" if a.keep_machine else "deleted", "cost": c})
    print(f"{m['vm']}: stopped at {iso(at)}, cost {money(c)}")
    return 0


def cmd_report(a: argparse.Namespace) -> int:
    if not a.offline:
        sync(a.budget, quiet=a.json)
    with locked(a.budget) as path:
        records = read_ledger(path)
    cap = ledger_cap(records) or (a.cap if a.cap is not None else DEFAULT_CAP)
    at = now()
    ms = machines(records)
    t = totals(ms, at)
    remaining = cap - t["spent"] - t["running_committed"]
    rows = [{"vm": m["vm"], "zone": m["zone"], "machine_type": m["machine_type"],
             "provisioning": m["provisioning"], "price_per_hour": m["price_per_hour"],
             "age_seconds": int(at - m["start_epoch"]), "expires_at": m["expires_at"],
             "cost_so_far": cost(m, at), "cost_to_expiry": committed(m)} for m in t["live"]]
    if a.json:
        print(json.dumps({"budget": a.budget, "cap": cap, "spent": t["spent"], "running": t["running"],
                          "running_committed": t["running_committed"], "remaining": round(remaining, 6),
                          "finished": t["finished"], "running_machines": rows}, indent=1))
        return 0
    print(f"budget {a.budget}  cap {money(cap)}")
    print(f"spent      {money(t['spent'])}  ({t['finished']} finished)")
    print(f"running    {money(t['running'])} so far, up to {money(t['running_committed'])} by expiry "
          f"({len(rows)} running)")
    print(f"remaining  {money(remaining)}")
    for row in rows:
        left = parse_time(row["expires_at"]) - at
        print(f"  {row['vm']}  {row['zone']}  {row['machine_type']} {row['provisioning']}  "
              f"{money(row['price_per_hour'])}/h  age {duration(row['age_seconds'])}  "
              f"expires {row['expires_at']} (in {duration(left)})  {money(row['cost_so_far'])} so far")
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
    c.add_argument("--on-expiry", choices=["delete", "stop"], default="delete")
    c.add_argument("--label", action="append", default=[])
    c.add_argument("--project", default=os.environ.get("OA_PROJECT", DEFAULT_PROJECT))
    c.add_argument("--dry-run", action="store_true")
    c.add_argument("--json", action="store_true")
    s = sub.add_parser("stop", parents=[common])
    s.add_argument("name")
    s.add_argument("--keep-machine", action="store_true", help="record the stop without deleting the machine")
    sub.add_parser("sync", parents=[common])
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
            sync(a.budget)
            return 0
        if a.cmd == "report":
            return cmd_report(a)
        return cmd_prices(a)
    except Refused as why:
        print(f"cloud-budget: refused: {why}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
