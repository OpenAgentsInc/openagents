#!/usr/bin/env python3
"""Runs the pre-registered microluna-v18 Luna-sized family on coderos-4080.

Protocol: bench/terminal-bench/experiments/2026-09-25-microluna-v18-family/protocol.md.
3 attempts x 6 tasks, round by round in rank order, at most 2 trials at
once, live-database-cutover alone. A trial with no reward is rerun once.
Stops when spend passes $3.00 or more than 3 trials can't be graded.
Waits while / has under 20 GiB free before each launch.
"""
import glob
import json
import os
import shutil
import subprocess
import sys
import time
from datetime import datetime

HOME = os.path.expanduser("~")
BASE = f"{HOME}/.cache/openagents/v18-family"
STATE = f"{BASE}/state.json"
WORKTREE = f"{HOME}/.cache/openagents/worktrees/microluna-v18-family/bench/terminal-bench"
ARTIFACT = f"{HOME}/.cache/openagents/artifacts/coder-one-3a25a0ff1f"
SHA = "cdbf781be1c00814ba61bfddb6d69a581f6e3c345215b97afa4bba42b656bcd7"
DIGEST = "05aac15cefa419cfc3dc2db9225ace213bda351c252a9590d5d50de6b717c445"
UV = "/nix/store/ipjv9qq222qldhqmvg4g1bdz3frppg65-uv-0.11.21/bin/uv"
JOBS = f"{HOME}/.openagents/terminal-bench/jobs"
LOGS = f"{HOME}/.openagents/terminal-bench"
RANK = [
    "payments-pipeline-fix",
    "mp-checkpoint-consolidation",
    "cumulative-layout-shift",
    "live-database-cutover",
    "telecom-entity-resolution",
    "photonic-waveguide-routing",
]
ALONE = {"live-database-cutover"}
MAX_CONCURRENT = 2
CEILING = 3.00
FLOOR_GIB = 20
MAX_UNGRADABLE = 3


def log(msg):
    line = f"{datetime.now().isoformat(timespec='seconds')} {msg}"
    print(line, flush=True)


def free_gib():
    return shutil.disk_usage("/").free / 2**30


def save(state):
    tmp = STATE + ".tmp"
    with open(tmp, "w") as f:
        json.dump(state, f, indent=1)
    os.replace(tmp, STATE)


def read_trial(job):
    files = glob.glob(f"{JOBS}/{job}/tbench/attempts/*.json")
    row = {"job": job, "attempt_record": bool(files)}
    if not files:
        return row
    d = json.load(open(files[0]))
    out = d.get("outcome") or {}
    row["trial"] = (d.get("attempt") or {}).get("trial")
    row["reward"] = out.get("reward")
    exc = out.get("exception")
    row["exception"] = exc if exc is None else (exc.get("type") if isinstance(exc, dict) else str(exc))
    t = d.get("timing") or {}
    row["started_at"] = t.get("started_at")
    row["finished_at"] = t.get("finished_at")
    row["total_ms"] = t.get("total_ms")
    row["agent_ms"] = t.get("agent_execution_ms")
    row["cost_usd"] = (d.get("cost") or {}).get("amount_usd") or 0.0
    doctor = glob.glob(f"{JOBS}/{job}/*/agent/episode-doctor.txt")
    text = open(doctor[0]).read() if doctor else ""
    row["digest_ok"] = DIGEST in text if doctor else None
    return row


def launch(task, label):
    job = f"tb4--coder-one-microluna-v18--{task}--family-{label}-{datetime.now():%Y%m%dT%H%M%S}"
    env = dict(os.environ)
    env["TYPESAFE_API_KEY"] = json.load(open(f"{HOME}/.openagents/jev.json"))["api_key"]
    env["OPENAGENTS_API_KEY"] = open(f"{HOME}/.openagents/bearer").read().strip()
    env["CODEX_FORCE_AUTH_JSON"] = "1"
    for k in ("ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "OPENAI_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN"):
        env.pop(k, None)
    out = open(f"{LOGS}/{job}.log", "w")
    proc = subprocess.Popen(
        ["systemd-run", "--user", "--scope", "--quiet", "--collect", "--slice=agents.slice", "--",
         UV, "run", "-q", "tbench", "run", "--profile", "tb4", "--agent", "coder-one-microluna-v18",
         "--task", task, "--job-name", job,
         "--agent-kwarg", f"artifact_path={ARTIFACT}", "--agent-kwarg", f"artifact_sha256={SHA}"],
        cwd=WORKTREE, env=env, stdout=out, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
    )
    return job, proc


def main():
    os.makedirs(BASE, exist_ok=True)
    import hashlib
    if hashlib.sha256(open(ARTIFACT, "rb").read()).hexdigest() != SHA:
        log("artifact digest changed; stopping")
        sys.exit(2)
    queue = [(r, t, f"a{r}") for r in (1, 2, 3) for t in RANK]
    state = {"started": datetime.now().isoformat(timespec="seconds"), "trials": [], "stopped": None}
    running = {}  # job -> (task, round, label, proc, launched)
    spend = 0.0
    ungradable = 0
    stop = None
    while queue or running:
        # Reap.
        for job in list(running):
            task, rnd, label, proc, launched = running[job]
            if proc.poll() is None:
                continue
            del running[job]
            row = read_trial(job)
            row.update(task=task, round=rnd, label=label, launched_at=launched,
                       rerun=label.endswith("r"), exit=proc.returncode)
            row["gradable"] = row.get("reward") is not None
            spend += row.get("cost_usd") or 0.0
            state["trials"].append(row)
            log(f"finished {job}: reward={row.get('reward')} exc={row.get('exception')} "
                f"cost={row.get('cost_usd')} digest_ok={row.get('digest_ok')} spend={spend:.4f}")
            if not row["gradable"]:
                if not row["rerun"]:
                    queue.insert(0, (rnd, task, label + "r"))
                    log(f"infrastructure failure; rerun queued: {task} {label}r")
                else:
                    ungradable += 1
                    log(f"ungradable after rerun: {task} {label} ({ungradable})")
            state["spend_usd"] = spend
            state["ungradable"] = ungradable
            save(state)
        if stop is None and spend > CEILING:
            stop = f"spend ${spend:.4f} passed the ${CEILING:.2f} ceiling"
        if stop is None and ungradable > MAX_UNGRADABLE:
            stop = f"{ungradable} trials ungradable after rerun: run invalid"
        if stop is not None and queue:
            log(f"stopping: {stop}; {len(queue)} trials not launched")
            state["stopped"] = stop
            state["not_launched"] = [f"{t} {l}" for _, t, l in queue]
            queue = []
            save(state)
        if not queue:
            time.sleep(30)
            continue
        rnd, task, label = queue[0]
        busy_alone = any(running[j][0] in ALONE for j in running)
        if busy_alone:
            time.sleep(30)
            continue
        if task in ALONE and running:
            time.sleep(30)
            continue
        if len(running) >= MAX_CONCURRENT:
            time.sleep(30)
            continue
        if free_gib() < FLOOR_GIB:
            log(f"disk guard: {free_gib():.1f} GiB free on /; waiting")
            time.sleep(120)
            continue
        queue.pop(0)
        job, proc = launch(task, label)
        running[job] = (task, rnd, label, proc, datetime.now().isoformat(timespec="seconds"))
        log(f"launched {job} ({free_gib():.0f} GiB free)")
        time.sleep(40)
    state["finished"] = datetime.now().isoformat(timespec="seconds")
    save(state)
    log(f"done: spend ${spend:.4f}, ungradable {ungradable}, stopped={stop}")


if __name__ == "__main__":
    main()
