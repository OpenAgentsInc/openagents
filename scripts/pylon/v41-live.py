#!/usr/bin/env python3
"""v41-live: keep one DeepSeek-V4.1 group served, and relaunch it when a spot
machine is reclaimed (#12403).

`pylon cloud up` launches a group through the spend guard and serves it until
`down`, its machines' expiry, the budget's cap, or the loss of a machine. A spot
reclaim ends the group for good. This watcher runs `pylon cloud up` again, with
the same pinned bundles and flags, until its own end time or its own spend
limit:

    v41-live.py start --pylon BIN --until 2026-10-23T12:00Z --cap 300 \\
        [--zones asia-northeast1-b,...] -- [more `pylon cloud up` options]
    v41-live.py status
    v41-live.py stop

`start` detaches. The watcher is the root of an agent run of its own
(`scripts/dev/agent-run.py new --kind v41-live`), so ending the session that
started it reaps neither the watcher nor the machines, which carry that run's
labels. The run lives while the watcher lives; once the watcher exits, the
nightly sweep may stop any machine left over.

Each launch:

- asks for the hours left before `--until`, and no more than the spend limit
  leaves at the machine's spot price (the guard's price table), less a
  transfer allowance for a zone without a regional copy of the weights;
- tries the zones in order; a zone out of capacity moves on to the next one;
- reads the weights from the zone's regional copy when there is one.

The watcher's spend is the sum, over the machines it launched, of the guard
ledger's recorded cost for each machine that ended and price x hours booked for
each one still running, plus the transfer allowances. Every machine still goes
through the guard, and the budget's own cap still applies.

`stop` asks the watcher to end: it stops `pylon cloud up` with SIGTERM, which
drains the group and deletes its machines through the guard.

State lives in ~/.openagents/v41-live/ (OA_V41_LIVE_DIR): `state.json`, `log`,
and the pylon home the launches use.
"""

from __future__ import annotations

import argparse
import datetime as dt
import importlib.util
import json
import math
import os
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
DIR = Path(os.environ.get("OA_V41_LIVE_DIR", str(Path.home() / ".openagents" / "v41-live")))
STATE = DIR / "state.json"
LOG = DIR / "log"
STOP = DIR / "stop"
HOME = DIR / "pylon-home"
LEDGER_DIR = Path(os.environ.get("OA_CLOUD_BUDGET_DIR", str(Path.home() / ".openagents" / "cloud-budget")))
PRICES = REPO / "scripts" / "dev" / "cloud-prices.json"
AGENT_RUN = REPO / "scripts" / "dev" / "agent-run.py"
GUARD = REPO / "scripts" / "dev" / "cloud-budget.py"
SA_CONFIG = Path.home() / "work" / ".secrets" / "gcloud-sa-config"

PLACEMENT = "one-quad"
MACHINE_TYPE = "g4-standard-192"
BUDGET = "deepseek-v41"
# Regional copies of the weights; any other zone reads the us-central1 original.
REGIONAL = {
    "asia-northeast1": "gs://openagents-psionic-models-ane1/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1/",
}
# About 467 GB copied across regions at about $0.08/GB.
TRANSFER_USD = 40.0
DEFAULT_ZONES = [
    "asia-northeast1-b",
    "asia-northeast1-a",
    "asia-northeast1-c",
    "us-central1-b",
    "us-east1-b",
    "europe-west1-b",
    "southamerica-east1-b",
]
# The shortest launch worth starting, h: weights and stages take about 20 min.
MIN_HOURS = 1.0
# How long to wait after a group ends before the next launch, s.
RELAUNCH_WAIT = 60
# How long to wait when every zone refused, s.
ALL_REFUSED_WAIT = 300


def log(line: str) -> None:
    stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    with open(LOG, "a") as fh:
        fh.write(f"{stamp} {line}\n")


def load_state() -> dict:
    try:
        return json.loads(STATE.read_text())
    except (OSError, json.JSONDecodeError):
        return {}


def save_state(state: dict) -> None:
    tmp = STATE.with_suffix(".tmp")
    tmp.write_text(json.dumps(state, indent=1) + "\n")
    tmp.replace(STATE)


def parse_time(text: str) -> float:
    text = text.strip().replace("Z", "+00:00")
    when = dt.datetime.fromisoformat(text)
    if when.tzinfo is None:
        when = when.replace(tzinfo=dt.timezone.utc)
    return when.timestamp()


def iso(epoch: float) -> str:
    return dt.datetime.fromtimestamp(epoch, dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def spot_price() -> float:
    return float(json.loads(PRICES.read_text())["machines"][MACHINE_TYPE]["spot"])


def ledger_records() -> list[dict]:
    # Use the guard's corruption handling and settlement rules. Losing a
    # reservation must stop admission, not make the watcher spend less.
    records = budget_module().read_ledger(LEDGER_DIR / f"{BUDGET}.jsonl")
    records.refuse_if_corrupt()
    return records


_budget_module = None


def budget_module():
    global _budget_module
    if _budget_module is None:
        spec = importlib.util.spec_from_file_location("v41_live_budget", GUARD)
        module = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = module
        spec.loader.exec_module(module)
        _budget_module = module
    return _budget_module


def owned_machines(state: dict) -> list[dict]:
    records = ledger_records()
    known = set(state.get("ledger_ids", []))
    # A start is durable before create. It remains attributable even when
    # gcloud loses its reply and pylon never writes cloud.json.
    ids = known | {r["id"] for r in records if r.get("event") == "start"
                   and state.get("run_id") and r.get("run_id") == state["run_id"]}
    machines = [m for m in budget_module().machines(records) if m["id"] in ids]
    for m in machines:
        if m["id"] not in known and m.get("reason") != "create-failed":
            if m["zone"].rsplit("-", 1)[0] not in REGIONAL:
                state["transfer_usd"] = state.get("transfer_usd", 0.0) + TRANSFER_USD
    state["ledger_ids"] = sorted(ids)
    return machines


def spend(state: dict, now: float) -> float:
    """What the watcher's machines cost or may cost, plus transfers."""
    machines = owned_machines(state)
    total = float(state.get("transfer_usd", 0.0))
    guard = budget_module()
    for m in machines:
        total += (guard.cost(m, m["stop_epoch"]) if m.get("stop_epoch") is not None
                  else guard.committed(m, now))
    return round(total, 4)


def run_guard(state: dict, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(GUARD), *args, "--budget", BUDGET],
                          env=run_labels_env(state), capture_output=True, text=True, timeout=180)


def cleanup_owned(state: dict) -> bool:
    """End this watcher's stranded allocations before another generation."""
    if not state.get("run_id") and not state.get("ledger_ids"):
        return True
    live = [m for m in owned_machines(state) if m.get("stop_epoch") is None]
    if not live:
        return True
    try:
        result = run_guard(state, "sync")
        if result.returncode:
            log(f"cleanup waits for guard sync: {result.stderr.strip()[-500:]}")
            return False
        for m in owned_machines(state):
            if m.get("stop_epoch") is None:
                result = run_guard(state, "stop", m["vm"], "--record", m["id"])
                if result.returncode:
                    log(f"cleanup could not settle {m['vm']} ({m['id']}): {result.stderr.strip()[-500:]}")
        remaining = [m for m in owned_machines(state) if m.get("stop_epoch") is None]
        state["cleanup_pending"] = [m["id"] for m in remaining]
        save_state(state)
        return not remaining
    except (OSError, subprocess.TimeoutExpired) as error:
        log(f"cleanup waits: {error}")
        return False


def run_labels_env(state: dict) -> dict:
    env = dict(os.environ)
    env["OA_RUN_ID"] = state["run_id"]
    env["OPENAGENTS_PYLON_HOME"] = str(HOME)
    if "CLOUDSDK_CONFIG" not in env and SA_CONFIG.is_dir():
        env["CLOUDSDK_CONFIG"] = str(SA_CONFIG)
    return env


def signal_launch(child, signum):
    try:
        os.killpg(child.pid, signum)
    except ProcessLookupError:
        pass


def launch(state: dict, zone: str, hours: float) -> tuple[str, str]:
    """One `pylon cloud up`; returns (outcome, detail): `ended` once a group
    ran and ended, `refused` when it never started."""
    region = zone.rsplit("-", 1)[0]
    words = [state["pylon"], "cloud", "up", PLACEMENT, "--zone", zone, "--hours", f"{hours:.9f}"]
    if region in REGIONAL:
        words += ["--weights", REGIONAL[region]]
    words += state["extra"]
    log(f"launch in {zone} for {hours:.2f} h: {' '.join(words[1:])}")
    child = subprocess.Popen(
        words,
        cwd=state["repo"],
        env=run_labels_env(state),
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        start_new_session=True,
    )
    state["child"] = child.pid
    save_state(state)
    # A silent or blocked create must not extend the absolute end time.
    remaining = max(0.0, state["until"] - time.time())
    timer = threading.Timer(remaining, signal_launch, args=(child, signal.SIGTERM))
    kill_timer = threading.Timer(remaining + 30, signal_launch, args=(child, signal.SIGKILL))
    timer.daemon = True
    kill_timer.daemon = True
    timer.start()
    kill_timer.start()
    ready = False
    last = ""
    assert child.stdout is not None
    for line in child.stdout:
        line = line.rstrip()
        if not line:
            continue
        last = line
        if "is ready at generation" in line:
            ready = True
            state["ready_at"] = time.time()
            state["zone"] = zone
            save_state(state)
            log(line)
        elif line.startswith("group ") or "was not created" in line or "error" in line.lower():
            log(line)
        with open(DIR / "up.log", "a") as fh:
            fh.write(line + "\n")
    code = child.wait()
    timer.cancel()
    kill_timer.cancel()
    state["child"] = None
    owned_machines(state)
    save_state(state)
    log(f"pylon cloud up exited {code}: {last}")
    return ("ended" if ready else "refused", last)


def serve(args: argparse.Namespace) -> int:
    state = load_state()
    price = spot_price()
    zones = state["zones"]
    while True:
        now = time.time()
        if STOP.exists():
            log("stopped on request")
            break
        left_h = (state["until"] - now) / 3600
        spent = spend(state, now)
        allowance = state["cap"] - spent
        hours = min(left_h, allowance / price)
        state["spent"] = spent
        save_state(state)
        if hours < MIN_HOURS:
            log(f"done: {left_h:.2f} h left before the end time, ${allowance:.2f} left of ${state['cap']:.2f}")
            break
        started = False
        for zone in zones:
            if STOP.exists():
                break
            if not cleanup_owned(state):
                log("no new launch until this run's earlier allocations are stopped and settled")
                break
            # A failed launch may have spent money or hours. Reconcile the
            # allowance for every attempt, not just once per zone sweep.
            now = time.time()
            allowance = state["cap"] - spend(state, now)
            left_h = (state["until"] - now) / 3600
            region = zone.rsplit("-", 1)[0]
            transfer = 0.0 if region in REGIONAL else TRANSFER_USD
            # The guard rounds bookings up to whole minutes. Book whole
            # minutes down here, and preserve them in the CLI argument.
            h = math.floor(min(left_h, (allowance - transfer) / price) * 60) / 60
            if h < MIN_HOURS:
                continue
            outcome, detail = launch(state, zone, h)
            if outcome == "ended":
                started = True
                state["launches"] = state.get("launches", 0) + 1
                save_state(state)
                break
            log(f"{zone} refused: {detail}")
        if STOP.exists():
            log("stopped on request")
            break
        time.sleep(RELAUNCH_WAIT if started else ALL_REFUSED_WAIT)
    cleanup_owned(state)
    state["ended_at"] = time.time()
    state["spent"] = spend(state, time.time())
    save_state(state)
    log(f"watcher ends; spent ${state['spent']:.2f}")
    return 0


def daemonize() -> None:
    if os.fork() > 0:
        os._exit(0)
    os.setsid()
    if os.fork() > 0:
        os._exit(0)
    devnull = os.open(os.devnull, os.O_RDWR)
    for fd in (0, 1, 2):
        os.dup2(devnull, fd)


def alive(pid: int | None) -> bool:
    if not pid:
        return False
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def cmd_start(args: argparse.Namespace) -> int:
    DIR.mkdir(parents=True, exist_ok=True)
    HOME.mkdir(parents=True, exist_ok=True)
    held = load_state()
    if alive(held.get("pid")) and not held.get("ended_at"):
        print(f"v41-live: already running as pid {held['pid']}; `stop` first", file=sys.stderr)
        return 1
    until = parse_time(args.until)
    if until <= time.time() + MIN_HOURS * 3600:
        print("v41-live: --until must be at least an hour away", file=sys.stderr)
        return 1
    pylon = Path(args.pylon).resolve()
    if not os.access(pylon, os.X_OK):
        print(f"v41-live: {pylon} is not executable", file=sys.stderr)
        return 1
    extra = list(args.extra)
    if extra and extra[0] == "--":
        extra = extra[1:]
    for word in ("--zone", "--hours", "--weights"):
        if word in extra:
            print(f"v41-live: the watcher sets {word} itself", file=sys.stderr)
            return 1
    STOP.unlink(missing_ok=True)
    state = {
        "schema": "openagents.v41-live.v1",
        "pylon": str(pylon),
        "repo": str(REPO),
        "until": until,
        "until_iso": iso(until),
        "cap": args.cap,
        "zones": args.zones.split(",") if args.zones else DEFAULT_ZONES,
        "extra": extra,
        "ledger_ids": [],
        "transfer_usd": 0.0,
        "started_at": time.time(),
    }
    save_state(state)
    daemonize()
    state["pid"] = os.getpid()
    # A run of its own, with no parent: a parent run's reap would end it.
    env = {k: v for k, v in os.environ.items() if k != "OA_RUN_ID"}
    r = subprocess.run(
        [sys.executable, str(AGENT_RUN), "new", "--kind", "v41-live", "--root", str(os.getpid())],
        capture_output=True,
        text=True,
        env=env,
    )
    state["run_id"] = r.stdout.strip().splitlines()[-1] if r.stdout.strip() else ""
    save_state(state)
    if not state["run_id"]:
        log(f"no agent run: {r.stderr.strip()}")
        return 1
    log(f"watcher pid {state['pid']}, run {state['run_id']}, until {state['until_iso']}, cap ${state['cap']:.2f}")

    def on_term(_sig, _frame):
        STOP.touch()
        child = load_state().get("child")
        if alive(child):
            os.kill(child, signal.SIGTERM)

    signal.signal(signal.SIGTERM, on_term)
    return serve(args)


def cmd_stop(_args: argparse.Namespace) -> int:
    state = load_state()
    STOP.touch()
    child = state.get("child")
    if alive(child):
        os.kill(child, signal.SIGTERM)
        print(f"v41-live: stopping the group (pid {child}); its machines are deleted through the guard")
    if alive(state.get("pid")):
        print(f"v41-live: the watcher (pid {state['pid']}) ends once the group is down")
    else:
        print("v41-live: the watcher is not running")
    return 0


def cmd_status(_args: argparse.Namespace) -> int:
    state = load_state()
    if not state:
        print("v41-live: never started")
        return 1
    state["running"] = alive(state.get("pid")) and not state.get("ended_at")
    state["spent_now"] = spend(state, time.time())
    try:
        state["group"] = json.loads((HOME / "cloud.json").read_text())
    except (OSError, json.JSONDecodeError):
        pass
    print(json.dumps(state, indent=1))
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("start")
    s.add_argument("--pylon", required=True, help="the pylon binary that runs `cloud up`")
    s.add_argument("--until", required=True, help="when to stop relaunching, UTC ISO time")
    s.add_argument("--cap", type=float, required=True, help="the watcher's own spend limit, dollars")
    s.add_argument("--zones", help="zones to try, in order, comma-separated")
    s.add_argument("extra", nargs=argparse.REMAINDER, help="-- then more `pylon cloud up` options")
    sub.add_parser("stop")
    sub.add_parser("status")
    args = p.parse_args()
    return {"start": cmd_start, "stop": cmd_stop, "status": cmd_status}[args.cmd](args)


if __name__ == "__main__":
    sys.exit(main())
