#!/usr/bin/env python3
"""A work host (#11258): takes "work on this issue" runs from openagents.com
and runs the engine (`work_issue.py`) on this machine.

    worker.py [--once]

The website keeps the runs; this host asks for the next one
(`POST /v1/work-hosts/NAME/claim`), runs it, and reports its progress
lines and its result (`POST /v1/work-hosts/NAME/runs/ID`). The website never
reaches the host.

**Whose Claude.** Each run arrives with the requester's own Claude sign-in
(the variables Claude Code reads). It runs in a home of its own
(`RUN/home`), with no `~/.claude`, and with this host's own Claude
variables removed, so the requester's sign-in is the only one Claude Code
can see. The run's process environment is the only place it lives; it is
never written to disk or printed.

Environment:

| Variable | Default | What |
| --- | --- | --- |
| `OA_WORK_SITES` | `https://openagents.com` | Comma-separated sites to take runs from |
| `OA_WORK_HOST_TOKEN` | Secret Manager `openagents-work-host-token` | The host token the sites check |
| `OA_WORK_HOST_NAME` | the hostname | This host's name on the sites |
| `OA_WORK_SLOTS` | `2` | Runs at once (builds take turns under one lock) |
| `OA_WORK_POLL` | `10` | Seconds between asks when idle |

GitHub (`GH_TOKEN`), the finder's embeddings key, and gcloud come from the
host's own session (`scripts/cloud/dev-env-session.sh`).
"""

from __future__ import annotations

import argparse
import json
import os
import signal
import socket
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
HOME = Path.home()
WORK = HOME / ".openagents" / "work"
# Claude variables a run never inherits from this host.
HOST_CLAUDE = ("CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN",
               "CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX", "CLAUDE_CODE_USE_FOUNDRY",
               "AWS_BEARER_TOKEN_BEDROCK", "ANTHROPIC_FOUNDRY_API_KEY", "CLAUDECODE",
               "CLAUDE_CODE_ENTRYPOINT", "TYPESAFE_API_KEY")
REPORT_EVERY = 5.0
HEARTBEAT = 30.0


def log(text: str) -> None:
    print(f"{time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())} {text}", flush=True)


def host_token() -> str:
    token = os.environ.get("OA_WORK_HOST_TOKEN", "").strip()
    if token:
        return token
    proc = subprocess.run(["gcloud", "secrets", "versions", "access", "latest",
                           "--secret", "openagents-work-host-token", "--project",
                           os.environ.get("OA_PROJECT", "openagentsgemini")],
                          capture_output=True, text=True)
    if proc.returncode != 0 or not proc.stdout.strip():
        sys.exit("worker: no work host token (OA_WORK_HOST_TOKEN or the secret)")
    return proc.stdout.strip()


class Site:
    def __init__(self, base: str, name: str, token: str):
        self.base, self.name, self.token = base.rstrip("/"), name, token

    def post(self, path: str, body: dict) -> dict | None:
        request = urllib.request.Request(
            f"{self.base}{path}", data=json.dumps(body).encode(), method="POST",
            headers={"Authorization": f"Bearer {self.token}", "Content-Type": "application/json",
                     "User-Agent": "openagents-work-host/1"})
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return json.loads(response.read() or b"{}")
        except urllib.error.HTTPError as error:
            log(f"{self.base}{path}: {error.code}")
        except (urllib.error.URLError, TimeoutError, json.JSONDecodeError, OSError) as error:
            log(f"{self.base}{path}: {str(error)[:120]}")
        return None

    def claim(self) -> dict | None:
        answer = self.post(f"/v1/work-hosts/{self.name}/claim", {})
        return (answer or {}).get("run")

    def report(self, run_id: str, body: dict) -> dict | None:
        return self.post(f"/v1/work-hosts/{self.name}/runs/{run_id}", body)


def run_home(run_dir: Path) -> Path:
    """A home of the run's own: the host's caches, gcloud and git identity,
    and no Claude sign-in."""
    home = run_dir / "home"
    home.mkdir(parents=True, exist_ok=True)
    for name in (".cache", ".cargo", ".rustup"):
        target = HOME / name
        link = home / name
        if target.exists() and not link.exists():
            link.symlink_to(target)
    (home / ".config").mkdir(exist_ok=True)
    for name in ("gcloud", "gh"):
        target, link = HOME / ".config" / name, home / ".config" / name
        if target.exists() and not link.exists():
            link.symlink_to(target)
    gitconfig = HOME / ".gitconfig"
    if gitconfig.exists():
        (home / ".gitconfig").write_text(gitconfig.read_text())
    (home / ".openagents").mkdir(exist_ok=True)
    return home


def work(site: Site, job: dict, stop: threading.Event) -> None:
    run_id = job["id"]
    run_dir = WORK / "host-runs" / run_id
    run_dir.mkdir(parents=True, exist_ok=True)
    home = run_home(run_dir)
    env = {k: v for k, v in os.environ.items() if k not in HOST_CLAUDE and k != "OA_WORK_HOST_TOKEN"}
    credential = {str(k): str(v) for k, v in (job.get("env") or {}).items()}
    env.update(credential)
    if "ANTHROPIC_API_KEY" in credential:
        env["OA_WORK_KEEP_API_KEY"] = "1"
    env.update({
        "HOME": str(home), "CARGO_HOME": str(HOME / ".cargo"), "RUSTUP_HOME": str(HOME / ".rustup"),
        "CLOUDSDK_CONFIG": str(HOME / ".config" / "gcloud"),
        "PATH": f"{HOME / '.cargo' / 'bin'}:{HOME / '.local' / 'bin'}:{env.get('PATH', '')}",
        "OA_BRIEFED_AGENT": os.environ.get("OA_BRIEFED_AGENT", str(WORK / "bin" / "briefed-agent")),
        "OA_CLI": os.environ.get("OA_CLI", str(WORK / "bin" / "openagents")),
        "AB_WORK": str(WORK / "briefing"),
        "CARGO_TARGET_DIR": os.environ.get("OA_WORK_TARGET", str(WORK / "target")),
    })
    argv = [sys.executable, str(HERE / "work_issue.py"), "--repo", job["repo"], "--issue", str(job["issue"]),
            "--land", job.get("land", "pr"), "--engine", job.get("engine", "briefed"),
            "--run-id", run_id, "--out", str(run_dir / "out")]
    if job["repo"].lower() == "openagentsinc/openagents":
        argv += ["--checkout", str(ROOT)]
    log(f"run {run_id}: {job['repo']}#{job['issue']} ({job.get('engine')}, land {job.get('land')})")
    proc = subprocess.Popen(argv, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=open(run_dir / "stderr.txt", "w"),
                            stdin=subprocess.DEVNULL, text=True, start_new_session=True)
    del credential, env
    pending: list[dict] = []
    title: str | None = None
    result: dict | None = None
    lock = threading.Lock()
    done = threading.Event()

    def flush(final: dict | None = None) -> None:
        nonlocal title
        with lock:
            lines, pending[:] = list(pending), []
        body: dict = {"lines": [{"secs": ln.get("secs", 0), "phase": ln.get("phase", ""), "text": ln.get("text", "")}
                                for ln in lines]}
        if title:
            body["title"], title = title, None
        if final is not None:
            body.update(final)
        answer = site.report(run_id, body)
        if answer and answer.get("cancel") and proc.poll() is None:
            log(f"run {run_id}: cancelled")
            try:
                os.killpg(proc.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass

    def reporter() -> None:
        last = time.time()
        while not done.wait(REPORT_EVERY):
            with lock:
                busy = bool(pending)
            if busy or time.time() - last >= HEARTBEAT:
                flush()
                last = time.time()

    thread = threading.Thread(target=reporter, daemon=True)
    thread.start()
    for raw in proc.stdout:
        try:
            row = json.loads(raw)
        except json.JSONDecodeError:
            continue
        if row.get("type") == "result":
            result = row
        elif row.get("type") == "progress":
            with lock:
                pending.append(row)
                if row.get("title"):
                    title = row["title"]
        if stop.is_set() and proc.poll() is None:
            os.killpg(proc.pid, signal.SIGTERM)
    proc.wait()
    done.set()
    thread.join(timeout=10)
    if result is not None:
        flush({"result": result})
        log(f"run {run_id}: {'passed' if result.get('ok') else 'stopped'} "
            f"({result.get('engine')}, {result.get('secs')} s, ${result.get('cost_usd')})")
    else:
        tail = (run_dir / "stderr.txt").read_text()[-400:].strip()
        flush({"failed": f"The engine stopped without a result. {tail}"[:480]})
        log(f"run {run_id}: no result (exit {proc.returncode})")


def refresh_checkout() -> None:
    """Keep the host's checkout (the scripts and the base for worktrees)
    at origin/main when nothing runs."""
    status = subprocess.run(["git", "-C", str(ROOT), "status", "--porcelain", "--untracked-files=no"],
                            capture_output=True, text=True).stdout
    if status.strip():
        return
    subprocess.run(["git", "-C", str(ROOT), "pull", "-q", "--ff-only", "origin", "main"],
                   capture_output=True, text=True, timeout=600)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--once", action="store_true", help="take at most one run, then stop when it ends")
    args = ap.parse_args()
    token = host_token()
    name = os.environ.get("OA_WORK_HOST_NAME") or socket.gethostname().split(".")[0]
    sites = [Site(base.strip(), name, token)
             for base in os.environ.get("OA_WORK_SITES", "https://openagents.com").split(",") if base.strip()]
    slots = max(1, int(os.environ.get("OA_WORK_SLOTS", "2")))
    poll = max(2.0, float(os.environ.get("OA_WORK_POLL", "10")))
    stop = threading.Event()
    signal.signal(signal.SIGTERM, lambda *_: stop.set())
    running: list[threading.Thread] = []
    taken = 0
    log(f"work host {name}: {', '.join(s.base for s in sites)}, {slots} slots")
    last_refresh = 0.0
    while not stop.is_set():
        running = [t for t in running if t.is_alive()]
        if not running and time.time() - last_refresh > 600:
            refresh_checkout()
            last_refresh = time.time()
        got = False
        for site in sites:
            if len(running) >= slots or (args.once and taken):
                break
            job = site.claim()
            if job:
                got, taken = True, taken + 1
                thread = threading.Thread(target=work, args=(site, job, stop), daemon=False)
                thread.start()
                running.append(thread)
                # Busy marker for the environment's idle stop.
                (HOME / ".openagents").mkdir(exist_ok=True)
        marker = HOME / ".openagents" / "work-busy"
        if running:
            marker.touch()
        else:
            marker.unlink(missing_ok=True)
        if args.once and taken and not running:
            break
        if not got:
            stop.wait(poll)
    for thread in running:
        thread.join()
    return 0


if __name__ == "__main__":
    sys.exit(main())
