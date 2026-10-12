#!/usr/bin/env python3
"""agent-run: a run id for everything an agent run starts, and the reaper
that stops it all (#11433, docs/audits/2026-10-12-claude-usage-burn).

A run is a directory, ~/.openagents/runs/<id>/ (OA_RUNS_DIR):

    run.json   id, kind, host, created, root pid and its start time, the
               parent run, the Claude Code session ids, ended
    shells     one line per Bash tool shell: "pid epoch". A tool shell leads
               its own process group, so everything it starts (even with
               nohup or `&`, after the shell exits) keeps that group.
    pids       one line per recorded process: "pid pgid start-epoch note"
    vms        one line per cloud machine: "name zone project"

Claude Code drives it through hooks (.claude/settings.json):

    hook session-start   creates the run for this claude process (one run
                         per process, so /clear and resume stay in it) and
                         writes CLAUDE_ENV_FILE: every Bash tool shell then
                         exports OA_RUN_ID and appends its pid to `shells`
    hook bash-post       after a run_in_background Bash call, records the
                         children of the run's live tool shells that left
                         their shell's process group (setsid)
    hook session-end     reaps the run (not on /clear): its process groups,
                         recorded pids, child runs, and its VMs (detached)

By hand (scripts/dev/stop-agent-burn.sh wraps reap and sweep):

    agent-run.py new [--kind K] [--root PID]      create a run, print its id
    agent-run.py ensure [--kind K] [--pid PID]    join $OA_RUN_ID or create one
    agent-run.py add-pid ID PID [NOTE]            record a process
    agent-run.py add-vm ID NAME ZONE [PROJECT]    record a cloud machine
    agent-run.py labels [ID]                      run-id=..,run-host=.. for --labels
    agent-run.py label-vm NAME ZONE [--id ID]     label an existing VM and record it
    agent-run.py list                             runs and whether they are live
    agent-run.py reap ID [--dry-run] [--with-root] [--no-vms]
    agent-run.py sweep [--hours N] [--kill]       the nightly orphan sweep: launchd-
                         parented claude, coder-new and bench processes and
                         orphaned Claude Code tool shells, with no terminal,
                         older than N hours; leftovers of gone runs; and this
                         host's run-labelled VMs whose run is gone

Never signalled: this process and its ancestors, pid 1, any process with a
controlling terminal (an interactive session), and the run's root unless
--with-root. VMs are listed in the manifest or found by label run-id=<id>.

Environment: OA_RUNS_DIR, OA_GCLOUD (gcloud binary), OA_PROJECT (default
openagentsgemini), OA_RUN_REAP=off (SessionEnd does nothing),
OA_RUN_REAP_VMS=off (SessionEnd leaves VMs alone), OA_PS_CMD (a ps
replacement printing `pid ppid pgid tty lstart command`, for tests),
OA_SWEEP_PREFIX (sweep only commands starting with it, for tests).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import secrets
import shlex
import signal
import socket
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

RUNS = Path(os.environ.get("OA_RUNS_DIR", str(Path.home() / ".openagents" / "runs")))
PROJECT = os.environ.get("OA_PROJECT", "openagentsgemini")
SA_CONFIG = Path.home() / "work" / ".secrets" / "gcloud-sa-config"
SAFE = re.compile(r"[^a-z0-9_-]")
NO_TTY = ("??", "?", "-", "")


# ------------------------------------------------------------- processes

@dataclass
class Proc:
    pid: int
    ppid: int
    pgid: int
    tty: str
    start: float
    cmd: str


def parse_lstart(text: str) -> float:
    try:
        return time.mktime(time.strptime(" ".join(text.split()), "%a %b %d %H:%M:%S %Y"))
    except ValueError:
        return 0.0


def ps() -> dict[int, Proc]:
    cmd = os.environ.get("OA_PS_CMD")
    argv = shlex.split(cmd) if cmd else ["ps", "-A", "-ww", "-o", "pid=,ppid=,pgid=,tty=,lstart=,command="]
    out = subprocess.run(argv, capture_output=True, text=True, env=dict(os.environ, LC_ALL="C")).stdout
    procs: dict[int, Proc] = {}
    for line in out.splitlines():
        f = line.split(None, 9)
        if len(f) < 9 or not f[0].isdigit():
            continue
        p = Proc(int(f[0]), int(f[1]), int(f[2]), f[3], parse_lstart(" ".join(f[4:9])), f[9] if len(f) > 9 else "")
        procs[p.pid] = p
    return procs


def ancestors(procs: dict[int, Proc], pid: int) -> set[int]:
    seen: set[int] = set()
    while pid > 1 and pid not in seen:
        seen.add(pid)
        p = procs.get(pid)
        if not p:
            break
        pid = p.ppid
    return seen


def descendants(procs: dict[int, Proc], roots: set[int]) -> set[int]:
    kids: dict[int, list[int]] = {}
    for p in procs.values():
        kids.setdefault(p.ppid, []).append(p.pid)
    out: set[int] = set()
    stack = list(roots)
    while stack:
        for k in kids.get(stack.pop(), []):
            if k not in out:
                out.add(k)
                stack.append(k)
    return out


def alive(pid: int, start: float, procs: dict[int, Proc]) -> bool:
    p = procs.get(pid)
    return bool(p) and (not start or abs(p.start - start) <= 2)


def signal_all(pids: set[int], grace: float = 3.0) -> None:
    for pid in pids:
        try:
            os.kill(pid, signal.SIGTERM)
        except (ProcessLookupError, PermissionError):
            pass
    deadline = time.time() + grace
    left = set(pids)
    while left and time.time() < deadline:
        time.sleep(0.1)
        for pid in list(left):
            try:
                os.kill(pid, 0)
                # A zombie answers kill 0; ask ps whether it is really there.
            except (ProcessLookupError, PermissionError):
                left.discard(pid)
    for pid in left:
        try:
            os.kill(pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass


# ------------------------------------------------------------------- runs

def host() -> str:
    return SAFE.sub("-", socket.gethostname().split(".")[0].lower())[:63] or "host"


def safe_id(raw: str) -> str:
    return SAFE.sub("-", raw.lower())[:63]


def run_dir(rid: str) -> Path:
    return RUNS / safe_id(rid)


def load(rid: str) -> dict | None:
    try:
        return json.loads((run_dir(rid) / "run.json").read_text())
    except (OSError, json.JSONDecodeError):
        return None


def save(meta: dict) -> None:
    d = run_dir(meta["id"])
    d.mkdir(parents=True, exist_ok=True)
    tmp = d / "run.json.tmp"
    tmp.write_text(json.dumps(meta, indent=1) + "\n")
    tmp.replace(d / "run.json")


def create(kind: str, root: int, rid: str | None = None, parent: str | None = None,
           procs: dict[int, Proc] | None = None) -> dict:
    procs = procs if procs is not None else ps()
    rid = safe_id(rid or f"{kind}-{time.strftime('%Y%m%d-%H%M%S')}-{secrets.token_hex(2)}")
    meta = load(rid)
    if meta:
        return meta
    rp = procs.get(root)
    meta = {"id": rid, "kind": kind, "host": host(), "created": int(time.time()),
            "root_pid": root, "root_start": rp.start if rp else 0, "parent": parent,
            "sessions": [], "ended": None}
    save(meta)
    for name in ("shells", "pids", "vms"):
        (run_dir(rid) / name).touch()
    return meta


def add_pid(rid: str, pid: int, note: str = "", procs: dict[int, Proc] | None = None) -> None:
    procs = procs if procs is not None else ps()
    p = procs.get(pid)
    if not p:
        return
    with open(run_dir(rid) / "pids", "a") as fh:
        fh.write(f"{p.pid} {p.pgid} {int(p.start)} {note.replace(chr(10), ' ')[:80]}\n")


def add_vm(rid: str, name: str, zone: str, project: str = PROJECT) -> None:
    path = run_dir(rid) / "vms"
    line = f"{name} {zone} {project}\n"
    if path.exists() and line in path.read_text():
        return
    with open(path, "a") as fh:
        fh.write(line)


def lines(rid: str, name: str) -> list[list[str]]:
    try:
        return [l.split() for l in (run_dir(rid) / name).read_text().splitlines() if l.strip()]
    except OSError:
        return []


def is_live(meta: dict, procs: dict[int, Proc]) -> bool:
    if meta.get("ended"):
        return False
    return alive(meta.get("root_pid") or 0, meta.get("root_start") or 0, procs)


def all_runs() -> list[dict]:
    out = []
    if RUNS.is_dir():
        for d in sorted(RUNS.iterdir()):
            m = load(d.name)
            if m:
                out.append(m)
    return out


def targets(rid: str, procs: dict[int, Proc], with_root: bool, protected: set[int],
            seen: set[str] | None = None) -> set[int]:
    """Every live process of the run (and of its child runs)."""
    seen = seen if seen is not None else set()
    if rid in seen:
        return set()
    seen.add(rid)
    meta = load(rid) or {}
    root = meta.get("root_pid") or 0
    root_live = alive(root, meta.get("root_start") or 0, procs)
    found: set[int] = set()
    groups: set[int] = set()
    for f in lines(rid, "shells"):
        if len(f) < 2 or not f[0].isdigit():
            continue
        pgid, since = int(f[0]), float(f[1])
        leader = procs.get(pgid)
        # A reused pid leads a different group: its start time gives it away.
        if leader and abs(leader.start - since) > 2:
            continue
        groups.add(pgid)
    for f in lines(rid, "pids"):
        if len(f) < 3 or not f[0].isdigit():
            continue
        pid, pgid, start = int(f[0]), int(f[1]), float(f[2])
        if alive(pid, start, procs):
            found.add(pid)
            if pid == pgid:
                groups.add(pgid)
    for p in procs.values():
        if p.pgid in groups:
            found.add(p.pid)
    found |= descendants(procs, found)
    if root_live and with_root:
        # Cancelling the whole run: the root and everything under it.
        found |= descendants(procs, {root}) | {root}
    for child in all_runs():
        if child.get("parent") == rid:
            found |= targets(child["id"], procs, True, protected, seen)
    if not with_root and root:
        protected = protected | {root}
    return {pid for pid in found if pid not in protected and pid > 1
            and procs.get(pid) and procs[pid].tty in NO_TTY}


def gcloud(*args: str, timeout: int = 120) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    if "CLOUDSDK_CONFIG" not in env and SA_CONFIG.is_dir():
        env["CLOUDSDK_CONFIG"] = str(SA_CONFIG)
    argv = [os.environ.get("OA_GCLOUD", "gcloud"), *args]
    try:
        return subprocess.run(argv, capture_output=True, text=True, env=env, timeout=timeout)
    except (OSError, subprocess.TimeoutExpired) as error:
        return subprocess.CompletedProcess(argv, 1, "", str(error))


def labelled_vms(filter_extra: str = "") -> list[list[str]]:
    flt = "status=RUNNING AND labels.run-id:*" + (f" AND {filter_extra}" if filter_extra else "")
    r = gcloud("compute", "instances", "list", f"--project={PROJECT}", f"--filter={flt}",
               "--format=value(name,zone.basename(),labels.run-id,labels.run-host)")
    return [l.split() for l in r.stdout.splitlines() if l.strip()]


def stop_vms(vms: list[tuple[str, str, str]], dry: bool) -> None:
    for name, zone, project in vms:
        print(f"vm  {name}  ({zone}, {project})")
        if not dry:
            gcloud("compute", "instances", "stop", name, f"--zone={zone}", f"--project={project}",
                   "--async", "--quiet")


def run_vms(rid: str, query: bool) -> list[tuple[str, str, str]]:
    vms = {(f[0], f[1], f[2] if len(f) > 2 else PROJECT) for f in lines(rid, "vms") if len(f) >= 2}
    if query:
        for f in labelled_vms(f"labels.run-id={safe_id(rid)}"):
            if len(f) >= 2:
                vms.add((f[0], f[1], PROJECT))
    return sorted(vms)


def reap(rid: str, dry: bool, with_root: bool, vms: bool, query: bool = True) -> int:
    meta = load(rid)
    if not meta:
        print(f"no run {rid} under {RUNS}", file=sys.stderr)
        return 1
    procs = ps()
    protected = ancestors(procs, os.getpid()) | {os.getpid(), 1}
    pids = targets(meta["id"], procs, with_root, protected)
    for pid in sorted(pids):
        p = procs[pid]
        print(f"pid {pid:>7}  pgid {p.pgid:>7}  {p.cmd[:110]}")
    if not dry and pids:
        signal_all(pids)
    if vms:
        stop_vms(run_vms(meta["id"], query), dry)
    if not pids:
        print(f"run {meta['id']}: no live processes")
    if not dry:
        meta["ended"] = meta.get("ended") or int(time.time())
        save(meta)
    return 0


# ------------------------------------------------------------------ sweep

def sweep_class(cmd: str) -> str | None:
    """Which agent class an orphaned command belongs to, judged by what it
    runs (its executable), not by words elsewhere on its command line."""
    exe = os.path.basename(cmd.split(" ", 1)[0])
    if "/.claude/shell-snapshots/" in cmd:
        return "tool-shell"  # a Claude Code Bash shell whose session died
    if exe == "claude" or re.search(r"claude.*daemon.*run|bg-pty-host|bg-spare", cmd):
        return "claude"
    if exe.startswith(("coder-new", "coder_new-")):
        return "coder-new"
    if exe.lower().startswith("python") and re.search(r"(^|[ /])ab\.py( |$)|bench\S*\.py (batch|run)|/scripts/bench/", cmd):
        return "bench"
    return None


def sweep(hours: float, kill: bool) -> int:
    procs = ps()
    now = time.time()
    protected = ancestors(procs, os.getpid()) | {os.getpid(), 1}
    prefix = os.environ.get("OA_SWEEP_PREFIX")
    chosen: dict[int, str] = {}
    for p in procs.values():
        if p.ppid != 1 or p.tty not in NO_TTY or p.pid in protected:
            continue
        if prefix and not p.cmd.startswith(prefix):
            continue
        if now - p.start < hours * 3600:
            continue
        name = sweep_class(p.cmd)
        if name:
            chosen[p.pid] = name
    # Leftovers of runs whose root is gone and that ended over N hours ago.
    gone: list[dict] = []
    for meta in all_runs():
        if meta.get("host") != host() or is_live(meta, procs):
            continue
        gone.append(meta)
        last = meta.get("ended") or meta.get("created") or 0
        if now - last < hours * 3600:
            continue
        for pid in targets(meta["id"], procs, False, protected):
            if prefix and not procs[pid].cmd.startswith(prefix):
                continue
            chosen.setdefault(pid, f"run {meta['id']}")
    for pid in sorted(chosen):
        p = procs[pid]
        age = (now - p.start) / 3600
        print(f"pid {pid:>7}  {chosen[pid]:<10}  {age:6.1f}h  {p.cmd[:100]}")
    if kill and chosen:
        signal_all(set(chosen))
    # VMs labelled for a run of this host that is no longer live.
    gone_ids = {m["id"] for m in gone}
    known = {m["id"] for m in all_runs()}
    vms = []
    for f in labelled_vms(f"labels.run-host={host()}"):
        if len(f) >= 3 and (f[2] in gone_ids or f[2] not in known):
            vms.append((f[0], f[1], PROJECT))
    stop_vms(vms, not kill)
    if not chosen and not vms:
        print("sweep: nothing to stop")
    elif not kill:
        print("report only; --kill stops these")
    return 0


# ------------------------------------------------------------------ hooks

def claude_pid(procs: dict[int, Proc]) -> int:
    env = os.environ.get("CLAUDE_PID", "")
    if env.isdigit() and int(env) in procs:
        return int(env)
    pid = os.getppid()
    for _ in range(8):
        p = procs.get(pid)
        if not p:
            break
        if re.search(r"(^|/)claude( |$)", p.cmd):
            return pid
        pid = p.ppid
    return os.getppid()


def session_run_id(procs: dict[int, Proc], root: int) -> str:
    start = procs[root].start if root in procs else time.time()
    return f"cc-{time.strftime('%Y%m%d%H%M%S', time.localtime(start))}-{root}"


def hook(event: str) -> int:
    try:
        payload = json.loads(sys.stdin.read() or "{}")
    except json.JSONDecodeError:
        payload = {}
    if event == "bash-post" and not (payload.get("tool_input") or {}).get("run_in_background"):
        return 0
    procs = ps()
    root = claude_pid(procs)
    rid = session_run_id(procs, root)
    if event == "session-start":
        meta = create("claude", root, rid, os.environ.get("OA_RUN_ID") or None, procs)
        sid = payload.get("session_id")
        if sid and sid not in meta["sessions"]:
            meta["sessions"].append(sid)
            meta["ended"] = None
            save(meta)
        env_file = os.environ.get("CLAUDE_ENV_FILE")
        if env_file:
            shells = run_dir(rid) / "shells"
            with open(env_file, "a") as fh:
                fh.write(f"export OA_RUN_ID={rid}\n"
                         f"echo \"$$ $(date +%s)\" >> {shlex.quote(str(shells))} 2>/dev/null || true\n")
        return 0
    if not load(rid):
        return 0
    if event == "bash-post":
        # Children of the run's live tool shells, caught while their shell
        # still holds them (a setsid child leaves the shell's group).
        shells = {int(f[0]) for f in lines(rid, "shells") if f and f[0].isdigit()}
        mine = ancestors(procs, os.getpid())
        known = {int(f[0]) for f in lines(rid, "pids") if f and f[0].isdigit()}
        for pid in descendants(procs, {s for s in shells if s in procs}):
            if pid not in mine and pid not in known and procs[pid].pgid not in shells:
                add_pid(rid, pid, "background", procs)
        return 0
    if event == "session-end":
        if payload.get("reason") == "clear" or os.environ.get("OA_RUN_REAP") == "off":
            return 0
        reap(rid, dry=False, with_root=False, vms=False)
        if os.environ.get("OA_RUN_REAP_VMS") != "off" and lines(rid, "vms") + lines_children(rid):
            # Stopping machines is slow; it must not hold up the exit.
            subprocess.Popen([sys.executable, __file__, "reap", rid, "--vms-only"],
                             stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                             stderr=subprocess.DEVNULL, start_new_session=True)
        return 0
    return 2


def lines_children(rid: str) -> list[list[str]]:
    out = []
    for m in all_runs():
        if m.get("parent") == rid:
            out += lines(m["id"], "vms")
    return out


# ------------------------------------------------------------------- main

def env_run(explicit: str | None) -> str:
    rid = explicit or os.environ.get("OA_RUN_ID")
    if not rid or not load(rid):
        raise SystemExit("no run: pass --id or set OA_RUN_ID to an existing run")
    return safe_id(rid)


def main() -> int:
    ap = argparse.ArgumentParser(description="run ids and the reaper (#11433)")
    sub = ap.add_subparsers(dest="cmd", required=True)
    n = sub.add_parser("new"); n.add_argument("--kind", default="run"); n.add_argument("--root", type=int)
    n.add_argument("--id"); n.add_argument("--parent")
    e = sub.add_parser("ensure"); e.add_argument("--kind", default="run"); e.add_argument("--pid", type=int)
    a = sub.add_parser("add-pid"); a.add_argument("id"); a.add_argument("pid", type=int); a.add_argument("note", nargs="?", default="")
    v = sub.add_parser("add-vm"); v.add_argument("id"); v.add_argument("name"); v.add_argument("zone")
    v.add_argument("project", nargs="?", default=PROJECT)
    lb = sub.add_parser("labels"); lb.add_argument("id", nargs="?")
    lv = sub.add_parser("label-vm"); lv.add_argument("name"); lv.add_argument("zone"); lv.add_argument("--id")
    lv.add_argument("--project", default=PROJECT)
    sub.add_parser("list")
    r = sub.add_parser("reap"); r.add_argument("id"); r.add_argument("--dry-run", action="store_true")
    r.add_argument("--with-root", action="store_true"); r.add_argument("--no-vms", action="store_true")
    r.add_argument("--vms-only", action="store_true")
    s = sub.add_parser("sweep"); s.add_argument("--hours", type=float, default=float(os.environ.get("OA_SWEEP_HOURS", 12)))
    s.add_argument("--kill", action="store_true")
    h = sub.add_parser("hook"); h.add_argument("event", choices=["session-start", "bash-post", "session-end"])
    args = ap.parse_args()

    if args.cmd == "hook":
        try:
            return hook(args.event)
        except Exception as error:  # noqa: BLE001 - a hook must never break the session
            print(f"agent-run hook {args.event}: {error}", file=sys.stderr)
            return 0
    if args.cmd == "new":
        meta = create(args.kind, args.root or os.getppid(), args.id, args.parent or os.environ.get("OA_RUN_ID"))
        print(meta["id"])
    elif args.cmd == "ensure":
        pid = args.pid or os.getppid()
        rid = os.environ.get("OA_RUN_ID")
        if rid and load(rid):
            add_pid(safe_id(rid), pid, args.kind)
            print(safe_id(rid))
        else:
            meta = create(args.kind, pid)
            add_pid(meta["id"], pid, args.kind)
            print(meta["id"])
    elif args.cmd == "add-pid":
        add_pid(env_run(args.id), args.pid, args.note)
    elif args.cmd == "add-vm":
        add_vm(env_run(args.id), args.name, args.zone, args.project)
    elif args.cmd == "labels":
        print(f"run-id={env_run(args.id)},run-host={host()}")
    elif args.cmd == "label-vm":
        rid = env_run(args.id)
        r = gcloud("compute", "instances", "add-labels", args.name, f"--zone={args.zone}",
                   f"--project={args.project}", f"--labels=run-id={rid},run-host={host()}")
        add_vm(rid, args.name, args.zone, args.project)
        if r.returncode != 0:
            print(f"label-vm: recorded, but labelling failed: {r.stderr.strip()[:300]}", file=sys.stderr)
            return 1
    elif args.cmd == "list":
        procs = ps()
        for m in all_runs():
            state = "live" if is_live(m, procs) else ("ended" if m.get("ended") else "gone")
            print(f"{m['id']:<40} {m.get('kind', ''):<8} {state:<6} root {m.get('root_pid')}  "
                  f"vms {len(lines(m['id'], 'vms'))}  {time.strftime('%Y-%m-%d %H:%M', time.localtime(m.get('created', 0)))}")
    elif args.cmd == "reap":
        if args.vms_only:
            if not load(args.id):
                return 1
            stop_vms(run_vms(safe_id(args.id), True), args.dry_run)
            for m in all_runs():
                if m.get("parent") == safe_id(args.id):
                    stop_vms(run_vms(m["id"], True), args.dry_run)
            return 0
        rc = reap(args.id, args.dry_run, args.with_root, not args.no_vms)
        if not args.no_vms:
            for m in all_runs():
                if m.get("parent") == safe_id(args.id):
                    stop_vms(run_vms(m["id"], True), args.dry_run)
        return rc
    elif args.cmd == "sweep":
        return sweep(args.hours, args.kill)
    return 0


if __name__ == "__main__":
    sys.exit(main())
