#!/usr/bin/env python3
"""Side-by-side demo (#10211): the same task in OpenAgents Terminal and in
Claude Code, with the measured difference.

Three real repository fixes from the standing study's task set
(efficiency-v1: `bottle-etag`, `mi-seekable`, `mi-one`), each run twice from
the same commit in two scratch checkouts at the same time:

  left   OpenAgents Terminal: `openagents chat send`, routed to Claude Code
         as one lean session (the shipped Claude route, #10246)
  right  Claude Code: `claude -p` on its own defaults

Both runs go through study.py's own arms and independent checks, so every
number printed at the end comes from the run records (cost at list price
from the engine's reported usage plus Jev; wall time; the check's pass or
fail), exactly as the standing study records them. The only difference
from a study run: raw Claude Code streams its events
(`--output-format stream-json --verbose`) so the pane can show its work;
the cost, usage, and turns come from the same final result event.

Usage (on a host with Claude Code signed in; see the write-up,
docs/cost/2026-10-02-terminal-vs-claude-code-demo.md):

  demo.py start [RUN_ID] [TASKS]   tmux session `oa-demo`: two panes side by
                                   side, the driver and the summary below;
                                   attaches when run from a terminal
  demo.py drive RUN_ID [TASKS]     the same without tmux: both runs at once,
                                   progress as lines, the summary at the end
  demo.py summary RUN_ID [TASKS]   print the summary again from the records

TASKS is a comma list (default bottle-etag,mi-seekable,mi-one). RUN_ID
defaults to demo-<UTC time>; a finished run is never repeated, so rerunning
with the same RUN_ID only reprints. Environment as for study.py
(EFFICIENCY_BIN must hold `openagents` and `microcoder`).
"""
import glob
import json
import os
import shlex
import shutil
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import study  # noqa: E402

TASKS = ["bottle-etag", "mi-seekable", "mi-one"]
LEFT, RIGHT = "routed-lean", "raw-claude"
TITLE = {
    LEFT: "OpenAgents Terminal  (routed: lean Claude Code session)",
    RIGHT: "Claude Code  (raw: claude -p, its own defaults)",
}
SHORT = {LEFT: "OpenAgents", RIGHT: "Claude Code"}
TRIAL = 1
SESSION = "oa-demo"
# Its own tmux server: a server already running for this user keeps the
# environment (HOME, PATH) of whoever started it.
TMUX = ["tmux", "-L", SESSION]


def run_dir(run_id, task, arm):
    return os.path.join(study.runs_dir(run_id), task, arm, str(TRIAL))


def live_path(run_id, task, arm):
    # Beside the run's folder: study.one() clears an unfinished folder.
    return run_dir(run_id, task, arm) + ".live"


def done_path(run_id, task, arm):
    return run_dir(run_id, task, arm) + ".done"


def result(run_id, task, arm):
    try:
        return json.load(open(os.path.join(run_dir(run_id, task, arm), "result.json")))
    except (OSError, ValueError):
        return None


def base_commit(task):
    tmpl = os.path.join(study.TMPL, task)
    r = subprocess.run(["git", "rev-parse", "--short=10", "HEAD"], cwd=tmpl, capture_output=True, text=True)
    return r.stdout.strip() or "?"


# ---------------------------------------------------------------- running

def live_timed(live, raw):
    """study.timed, writing the command's output to `live` as it comes."""
    def timed(cmd, cwd, env=None):
        cmd = list(cmd)
        if raw and "--output-format" in cmd:
            i = cmd.index("--output-format")
            cmd[i + 1:i + 2] = ["stream-json", "--verbose"]
        start = time.time()
        with open(live, "a") as f:
            offset = f.tell()
            p = subprocess.Popen(cmd, cwd=cwd, env=env, stdout=f, stderr=subprocess.PIPE, text=True,
                                 stdin=subprocess.DEVNULL)
            try:
                _, err = p.communicate(timeout=study.TIMEOUT)
                code = p.returncode
            except subprocess.TimeoutExpired:
                p.kill()
                p.communicate()
                err, code = "timeout", -1
        with open(live) as f:
            f.seek(offset)
            out = f.read()
        if raw:
            # study.run_raw_claude reads one JSON result: the stream's last event.
            res = [ln for ln in out.splitlines() if ln.startswith("{") and '"type":"result"' in ln.replace(" ", "")]
            out = res[-1] if res else ""
        return out, err, code, time.time() - start
    return timed


def run_one(run_id, task, arm):
    """One arm on one task through study.one(), its output streamed to the live file."""
    if result(run_id, task, arm):
        return result(run_id, task, arm)
    os.makedirs(os.path.dirname(run_dir(run_id, task, arm)), exist_ok=True)
    live = live_path(run_id, task, arm)
    open(live, "w").close()
    study.timed = live_timed(live, arm == RIGHT)
    try:
        return study.one(run_id, arm, task, TRIAL)
    finally:
        open(done_path(run_id, task, arm), "w").close()


# ---------------------------------------------------------------- showing

def short(s, n):
    s = " ".join(str(s).split())
    return s if len(s) <= n else s[:n - 1] + "…"


def tool_line(name, inp):
    inp = inp or {}
    for k in ("command", "file_path", "pattern", "path", "url", "description"):
        if inp.get(k):
            v = str(inp[k])
            # Claude Code prefixes most commands with `cd <checkout>;`.
            if k == "command" and v.startswith("cd ") and ";" in v.split("\n")[0]:
                v = v.split(";", 1)[1].strip()
            return f"{name}  {v}"
    return name


def activity(live, raw):
    """What the run has done so far, one line per event worth showing."""
    lines = []
    try:
        text = open(live).read()
    except OSError:
        return lines
    for ln in text.splitlines():
        try:
            e = json.loads(ln)
        except ValueError:
            continue
        if raw:
            t = e.get("type")
            if t == "system" and e.get("subtype") == "init":
                lines.append(f"session  {e.get('model', '')}")
            elif t == "assistant":
                for b in (e.get("message") or {}).get("content") or []:
                    if b.get("type") == "tool_use":
                        lines.append("> " + tool_line(b.get("name"), b.get("input")))
                    elif b.get("type") == "text" and b.get("text", "").strip():
                        lines.append(b["text"])
            elif t == "result":
                lines.append(f"done  ({e.get('subtype')}, {e.get('num_turns')} turns)")
        else:
            ev = e.get("event")
            if ev == "route":
                lines.append("routed  " + (e.get("runner_text") or e.get("route") or ""))
            elif ev == "coder_started":
                lines.append(f"started  {e.get('provider')} {e.get('model')}: {e.get('reason', '')}")
            elif ev == "step":
                src, kind, txt = e.get("source"), e.get("kind"), e.get("text") or ""
                if src == "user":
                    continue
                if kind == "tool_call":
                    # The session's step record names the call, not its command.
                    lines.append("> " + ("ran a command" if txt.startswith("Ran toolu_") else txt))
                elif kind == "observation":
                    first = next((x for x in txt.splitlines() if x.strip()), "")
                    if first:
                        lines.append("| " + first.strip())
                elif txt.strip():
                    lines.append(txt)
            elif ev == "status" and e.get("text"):
                lines.append(e["text"])
            elif ev == "result" and e.get("task"):
                fc = e.get("files_changed")
                files = fc if isinstance(fc, list) else []
                n = len(files) if isinstance(fc, list) else fc
                ins = e.get("insertions") if e.get("insertions") is not None else sum(f.get("added") or 0 for f in files)
                dels = e.get("deletions") if e.get("deletions") is not None else sum(f.get("removed") or 0 for f in files)
                lines.append(f"done  {n} files changed, +{ins} -{dels}")
    return lines


def final_lines(r):
    if not r:
        return ["no result recorded (see err.txt in the run folder)"]
    out = [
        ("PASS" if r.get("passed") else "FAIL") + "  (independent check: the fix commit's own tests)",
        f"cost   ${(r.get('cost_usd') or 0):.3f}   list price, from the engine's reported usage"
        + (f" + Jev ${r.get('jev_usd', 0):.3f}" if r.get("mode") == "routed" else ""),
        f"time   {r.get('wall_s', 0):.0f} s   wall clock, command start to settled",
        f"tokens {r.get('input_tokens', 0):,} in ({r.get('cache_read', 0):,} cached), {r.get('output_tokens', 0):,} out",
    ]
    return out


def pane(run_id, task, arm):
    """One side of the split: run the arm and draw its progress."""
    live = live_path(run_id, task, arm)
    box = {}
    th = threading.Thread(target=lambda: box.update(r=run_one(run_id, task, arm)), daemon=True)
    start = time.time()
    th.start()
    while True:
        alive = th.is_alive()
        cols, rows = shutil.get_terminal_size((100, 40))
        head = [
            "\033[1m" + short(TITLE[arm], cols) + "\033[0m",
            short(f"task {task}   base commit {base_commit(task)}   run {run_id}", cols),
            "-" * min(cols, 100),
        ]
        acts = [short(a, cols) for a in activity(live, arm == RIGHT)]
        if alive:
            foot = ["", f"\033[1mworking  {time.time() - start:.0f} s\033[0m"]
        else:
            foot = ["", *("\033[1m" + short(x, cols) + "\033[0m" for x in final_lines(box.get("r") or result(run_id, task, arm)))]
        room = max(rows - len(head) - len(foot) - 1, 1)
        sys.stdout.write("\033[H\033[2J" + "\n".join(head + acts[-room:] + foot))
        sys.stdout.flush()
        if not alive:
            break
        time.sleep(1)
    sys.stdout.write("\n")


# ---------------------------------------------------------------- summary

def standing_medians(tasks):
    """The standing study's medians for the same tasks, for context."""
    rows = []
    for f in sorted(glob.glob(os.path.join(HERE, "results", "*.jsonl"))):
        rows = [json.loads(l) for l in open(f) if l.strip()]
    if not rows:
        return None
    def med(xs):
        xs = sorted(xs)
        return xs[len(xs) // 2] if xs else None
    out = {}
    for t in tasks:
        for arm in (LEFT, RIGHT):
            rs = [r for r in rows if r["task"] == t and r["arm"] == arm]
            out[(t, arm)] = (med([r["cost_usd"] for r in rs]), med([r["wall_s"] for r in rs]),
                             sum(1 for r in rs if r.get("passed")), len(rs), rs[0]["study"] if rs else None)
    return out


def summary(run_id, tasks):
    w = 14
    lines = [
        "",
        f"Measured difference, run {run_id} (records: {study.runs_dir(run_id)})",
        "",
        f"{'task':<{w}} {'OpenAgents Terminal':<28} {'Claude Code':<28} {'cost':>7} {'time':>7}",
        f"{'':<{w}} {'check   cost      time':<28} {'check   cost      time':<28} {'ratio':>7} {'ratio':>7}",
    ]
    tot = {LEFT: [0, 0.0, 0.0, 0], RIGHT: [0, 0.0, 0.0, 0]}
    for t in tasks:
        cells = []
        rs = {a: result(run_id, t, a) for a in (LEFT, RIGHT)}
        for a in (LEFT, RIGHT):
            r = rs[a]
            if not r:
                cells.append(f"{'(no record)':<28}")
                continue
            tot[a][0] += 1 if r.get("passed") else 0
            tot[a][1] += r.get("cost_usd") or 0
            tot[a][2] += r.get("wall_s") or 0
            tot[a][3] += 1
            cells.append(f"{'pass' if r.get('passed') else 'FAIL':<7} ${(r.get('cost_usd') or 0):<8.3f} {r.get('wall_s', 0):>5.0f} s   ")
        if rs[LEFT] and rs[RIGHT] and rs[RIGHT].get("cost_usd") and rs[RIGHT].get("wall_s"):
            cr = f"{rs[LEFT]['cost_usd'] / rs[RIGHT]['cost_usd']:.2f}x"
            tr = f"{rs[LEFT]['wall_s'] / rs[RIGHT]['wall_s']:.2f}x"
        else:
            cr = tr = "-"
        lines.append(f"{t:<{w}} {cells[0]} {cells[1]} {cr:>7} {tr:>7}")
    L, R = tot[LEFT], tot[RIGHT]
    if L[3] and R[3] and R[1] and R[2]:
        lines.append(f"{'total':<{w}} {f'{L[0]}/{L[3]}':<7} ${L[1]:<8.3f} {L[2]:>5.0f} s    "
                     f"{f'{R[0]}/{R[3]}':<7} ${R[1]:<8.3f} {R[2]:>5.0f} s    {L[1] / R[1]:>6.2f}x {L[2] / R[2]:>6.2f}x")
        saved = 1 - L[1] / R[1]
        tline = (f"took {L[2] / R[2]:.2f}x as long" if L[2] > R[2] else f"took {L[2] / R[2]:.2f}x the time")
        lines += ["",
                  f"OpenAgents Terminal cost {saved:.0%} less than Claude Code ({L[0]}/{L[3]} against {R[0]}/{R[3]} passing) "
                  f"and {tline}.",
                  "Cost is list price from each engine's reported usage (Claude Code subscription usage is not billed "
                  "per run); the routed cost includes Jev's routing and briefing calls."]
    ctx = standing_medians(tasks)
    if ctx:
        lines += ["", f"For context, the standing study's medians on the same tasks (3 trials each, study "
                      f"{next((v[4] for v in ctx.values() if v[4]), '?')}):"]
        for t in tasks:
            (lc, lw, lp, ln), (rc, rw, rp, rn) = ctx[(t, LEFT)][:4], ctx[(t, RIGHT)][:4]
            if lc and rc:
                lines.append(f"  {t:<{w}} OpenAgents ${lc:.3f} {lw:.0f} s ({lp}/{ln})   Claude Code ${rc:.3f} {rw:.0f} s "
                             f"({rp}/{rn})   cost {lc / rc:.2f}x   time {lw / rw:.2f}x")
    return "\n".join(lines)


def efficiency_report(run_id):
    """`openagents efficiency --study` over this run's rows: the same report as the terminal's."""
    study.collect(run_id)
    rows = os.path.join(study.BASE, f"{run_id}.jsonl")
    r = subprocess.run([study.OA, "efficiency", "--study", rows], capture_output=True, text=True)
    text = r.stdout.strip()
    # Only this run's section and the findings: the report also covers the
    # committed studies and this computer's own runs.
    keep, on = [], False
    for ln in text.splitlines():
        if ln.startswith("Study ") and rows in ln:
            on = True
        elif ln.startswith(("Shadow baselines", "Routed runs on")):
            on = False
        if on:
            keep.append(ln)
    return "\n".join(keep) if keep else text


# ---------------------------------------------------------------- driving

def drive(run_id, tasks, panes=None):
    study.prepare()
    print(f"OpenAgents Terminal against Claude Code, run {run_id}, tasks {', '.join(tasks)}", flush=True)
    print(f"binaries: {study.versions()}", flush=True)
    me = os.path.abspath(__file__)
    for t in tasks:
        print(f"\n== {t}: both from base commit {base_commit(t)}, two scratch checkouts", flush=True)
        if panes:
            for arm, p in zip((LEFT, RIGHT), panes):
                cmd = f"{shlex.quote(sys.executable)} {shlex.quote(me)} pane {run_id} {t} {arm}; exec sleep infinity"
                subprocess.run([*TMUX, "respawn-pane", "-k", "-t", p, cmd], check=True)
            start = time.time()
            while not all(os.path.exists(done_path(run_id, t, a)) or result(run_id, t, a) for a in (LEFT, RIGHT)):
                time.sleep(2)
        else:
            start = time.time()
            ths = [threading.Thread(target=run_one, args=(run_id, t, a)) for a in (LEFT, RIGHT)]
            for th in ths:
                th.start()
            seen = {a: 0 for a in (LEFT, RIGHT)}
            while any(th.is_alive() for th in ths):
                for a in (LEFT, RIGHT):
                    acts = activity(live_path(run_id, t, a), a == RIGHT)
                    for x in acts[seen[a]:]:
                        print(f"  [{SHORT[a]:<11} {time.time() - start:4.0f}s] {short(x, 110)}", flush=True)
                    seen[a] = len(acts)
                time.sleep(1)
        for a in (LEFT, RIGHT):
            r = result(run_id, t, a)
            print(f"  {SHORT[a]:<11} {'pass' if r.get('passed') else 'FAIL'}  ${(r.get('cost_usd') or 0):.3f}  "
                  f"{r.get('wall_s', 0):.0f} s" if r else f"  {SHORT[a]:<11} no result", flush=True)
    s = summary(run_id, tasks)
    print(s, flush=True)
    rep = efficiency_report(run_id)
    print("\n`openagents efficiency --study` on this run's rows:\n" + rep, flush=True)
    open(os.path.join(study.BASE, f"{run_id}-summary.txt"), "w").write(s + "\n\n" + rep + "\n")
    print(f"\nSummary saved: {os.path.join(study.BASE, run_id + '-summary.txt')}", flush=True)


def start(run_id, tasks):
    if not shutil.which("tmux"):
        raise SystemExit("tmux is not installed: use `demo.py drive` instead")
    subprocess.run([*TMUX, "kill-session", "-t", SESSION], capture_output=True)
    me = os.path.abspath(__file__)
    cols, rows = shutil.get_terminal_size((240, 60))
    first = subprocess.run([*TMUX, "new-session", "-d", "-s", SESSION, "-x", str(max(cols, 160)), "-y", str(max(rows, 45)),
                            "-P", "-F", "#{pane_id}", "sleep infinity"], capture_output=True, text=True, check=True).stdout.strip()
    right = subprocess.run([*TMUX, "split-window", "-h", "-t", first, "-P", "-F", "#{pane_id}", "sleep infinity"],
                           capture_output=True, text=True, check=True).stdout.strip()
    env = " ".join(f"{k}={shlex.quote(v)}" for k, v in os.environ.items() if k.startswith("EFFICIENCY_"))
    log = os.path.join(study.BASE, f"{run_id}-drive.txt")
    os.makedirs(study.BASE, exist_ok=True)
    drv = (f"{env} {shlex.quote(sys.executable)} {shlex.quote(me)} drive {run_id} {','.join(tasks)} "
           f"--panes {first},{right} 2>&1 | tee {shlex.quote(log)}; exec $SHELL")
    subprocess.run([*TMUX, "split-window", "-v", "-f", "-l", "35%", "-t", first, drv], check=True)
    subprocess.run([*TMUX, "set-option", "-t", SESSION, "status", "off"], check=True)
    if sys.stdin.isatty() and not os.environ.get("TMUX"):
        os.execvp("tmux", [*TMUX, "attach", "-t", SESSION])
    print(f"tmux session `{SESSION}` started: `tmux -L {SESSION} attach -t {SESSION}` (driver log: {log})")


def main():
    a = sys.argv[1:]
    cmd = a[0] if a else "help"
    panes = None
    if "--panes" in a:
        i = a.index("--panes")
        panes = a[i + 1].split(",")
        a = a[:i] + a[i + 2:]
    run_id = a[1] if len(a) > 1 else "demo-" + time.strftime("%Y%m%d-%H%M%S", time.gmtime())
    tasks = a[2].split(",") if len(a) > 2 else TASKS
    for t in tasks:
        if t not in study.TASKS:
            raise SystemExit(f"unknown task {t}: one of {', '.join(study.TASKS)}")
    if cmd == "start":
        start(run_id, tasks)
    elif cmd == "drive":
        drive(run_id, tasks, panes)
    elif cmd == "pane":
        pane(a[1], a[2], a[3])
    elif cmd == "summary":
        print(summary(run_id, tasks))
    else:
        print(__doc__)


if __name__ == "__main__":
    main()
