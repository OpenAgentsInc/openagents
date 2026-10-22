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
import json
import os
import signal
import subprocess
import sys
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
    out = []
    try:
        for line in (LEDGER_DIR / f"{BUDGET}.jsonl").read_text().splitlines():
            try:
                out.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    except OSError:
        pass
    return out


def spend(state: dict, now: float) -> float:
    """What the watcher's machines cost or may cost, plus transfers."""
    ids = set(state.get("ledger_ids", []))
    starts: dict[str, dict] = {}
    stops: dict[str, float] = {}
    for r in ledger_records():
        rid = r.get("id")
        if rid not in ids:
            continue
        if r.get("event") == "start":
            starts[rid] = r
        elif r.get("event") in ("stop", "correction") and isinstance(r.get("cost"), (int, float)):
            stops[rid] = float(r["cost"])
    total = float(state.get("transfer_usd", 0.0))
    for rid in ids:
        if rid in stops:
            total += stops[rid]
        elif rid in starts:
            s = starts[rid]
            total += float(s.get("price_per_hour", 0.0)) * float(s.get("hours_requested", 0.0))
    return round(total, 4)


def run_labels_env(state: dict) -> dict:
    env = dict(os.environ)
    env["OA_RUN_ID"] = state["run_id"]
    env["OPENAGENTS_PYLON_HOME"] = str(HOME)
    if "CLOUDSDK_CONFIG" not in env and SA_CONFIG.is_dir():
        env["CLOUDSDK_CONFIG"] = str(SA_CONFIG)
    return env


def launch(state: dict, zone: str, hours: float) -> tuple[str, str]:
    """One `pylon cloud up`; returns (outcome, detail): `ended` once a group
    ran and ended, `refused` when it never started."""
    region = zone.rsplit("-", 1)[0]
    words = [state["pylon"], "cloud", "up", PLACEMENT, "--zone", zone, "--hours", f"{hours:.2f}"]
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
    )
    state["child"] = child.pid
    save_state(state)
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
    state["child"] = None
    # The launch record names the machines and their ledger IDs.
    try:
        record = json.loads((HOME / "cloud.json").read_text())
        for m in record.get("machines", []):
            lid = m.get("ledger_id")
            if lid and lid not in state["ledger_ids"]:
                state["ledger_ids"].append(lid)
                if region not in REGIONAL:
                    state["transfer_usd"] = state.get("transfer_usd", 0.0) + TRANSFER_USD
    except (OSError, json.JSONDecodeError):
        pass
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
            region = zone.rsplit("-", 1)[0]
            h = hours if region in REGIONAL else min(hours, (allowance - TRANSFER_USD) / price)
            if h < MIN_HOURS:
                continue
            outcome, detail = launch(state, zone, round(h, 2))
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
