#!/usr/bin/env python3
"""Queue the out-of-sample study's confirmation runs (#9683).

Run this on the execution host. It watches a round's outcomes and, for every
held-out task whose screen run passed and that is neither burned
(burned.txt) nor ungradeable (notes.txt), queues the confirmation runs
exactly once: 2 more knowledge-on and 3 knowledge-off runs, interleaved
(off, on, off, on, off). They use the round's binary, the round screen
script's flags and knowledge folder, the task's time cap from the round's
queue, and the extra arguments on the round driver's command line. At most
3 confirmation runs go at a time. Each run writes
`<round>/<task>.<arm>.<epoch-ms>.log` and appends an outcome line to
`<round>/outcomes.txt` in the screen script's format, plus ` log=<name>`.

It reads only what study.py collects: outcome lines and whitelisted
summary.json fields. It never reads transcripts or logs beyond the result
line the screen script itself greps.

    confirm.py --round r2 --dry-run      # print what it would queue, change nothing
    confirm.py --round r2                # watch and run until the round is done
    confirm.py --round r2 --once         # queue what is eligible now, run it, exit
"""

from __future__ import annotations

import argparse
import fcntl
import json
import os
import re
import shlex
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import study  # noqa: E402

PLAN = ("off", "on", "off", "on", "off")
RESULT_LINE = re.compile(r" · reward|needs several|refus|Interrupted")


def expand(p: str) -> Path:
    return Path(os.path.expanduser(os.path.expandvars(p)))


def find_driver(round_name: str, ps_text: str | None = None) -> dict | None:
    """The round's screen driver from the process table: script, queue, extras."""
    if ps_text is None:
        ps_text = subprocess.run(["ps", "-eo", "args"], capture_output=True, text=True).stdout
    for line in ps_text.splitlines():
        try:
            argv = shlex.split(line)
        except ValueError:
            continue
        for i, a in enumerate(argv):
            if re.search(r"(^|/)screen\d*\.sh$", a) and len(argv) > i + 2 and argv[i + 2] == round_name:
                rest = argv[i + 3:]
                par = rest[0] if rest else None
                return {"script": a, "queue": argv[i + 1], "round": round_name, "concurrency": par,
                        "extra": rest[1:], "command": line.strip()}
    return None


def script_settings(script_text: str) -> dict:
    """The fixed flags and knowledge folder in the round's screen script."""
    def grab(pat, default):
        m = re.search(pat, script_text)
        return m.group(1) if m else default
    return {
        "max_steps": grab(r"--max-steps\s+(\d+)", None),
        "max_usd": grab(r"--max-usd\s+([\d.]+)", None),
        "knowledge": grab(r"export OPENAGENTS_KNOWLEDGE=(\S+)", None),
        "bin": grab(r"\bbin=(\S+)", None),
    }


class Confirmer:
    def __init__(self, a):
        self.a = a
        self.study_dir = expand(a.study_dir)
        self.rdir = self.study_dir / a.round
        self.state_path = self.rdir / "confirm-state.json"
        self.lock = threading.Lock()
        self.state = self._load_state()
        self.reported = set()

    # ------------------------------------------------------------ state
    def _load_state(self) -> dict:
        try:
            return json.loads(self.state_path.read_text())
        except (OSError, ValueError):
            return {"round": self.a.round, "driver": None, "tasks": {}}

    def _save_state(self) -> None:
        if self.a.dry_run:
            return
        tmp = self.state_path.with_suffix(".tmp")
        tmp.write_text(json.dumps(self.state, indent=1) + "\n")
        tmp.replace(self.state_path)

    # ------------------------------------------------------------ config
    def config(self, collected: dict, runs: list) -> dict:
        """Binary, flags, env, extras, and time caps, all from the round."""
        a = self.a
        driver = find_driver(a.round)
        source = "running driver"
        if driver is None and self.state.get("driver"):
            driver, source = self.state["driver"], "saved from the driver in confirm-state.json"
        if a.driver_args is not None:
            driver = dict(driver or {}, extra=shlex.split(a.driver_args))
            source = "--driver-args"
        if driver is None and a.script and "EXTRA" not in expand(a.script).read_text():
            driver = {"extra": [], "script": a.script, "queue": None}
            source = "--script, which passes no extra args"
        if driver is None:
            # Fall back to what the round's records say it ran with.
            prov = {(r["summary"] or {}).get("provider") for r in runs if r.get("summary")} - {None}
            model = {(r["summary"] or {}).get("model") for r in runs if r.get("summary")} - {None}
            if len(prov) == 1 and len(model) == 1:
                driver = {"extra": ["--provider", prov.pop(), "--model", model.pop()], "script": None,
                          "queue": None}
                source = "round records' provider and model"
            else:
                raise SystemExit("can't find the round driver's command line; pass --driver-args")
        script = expand(a.script or driver.get("script") or self._default_script())
        if not script.is_absolute():
            script = self.study_dir / script
        settings = script_settings(script.read_text())
        if not settings["max_steps"] or not settings["max_usd"]:
            raise SystemExit(f"can't read --max-steps/--max-usd from {script}")
        binary = a.bin or (settings["bin"] or "").replace("${round}", a.round).replace("$round", a.round)
        if not binary:
            raise SystemExit(f"can't read the binary from {script}; pass --bin")
        binary = expand(binary)
        queue_path = expand(driver.get("queue") or f"queue-{a.round}.txt")
        if not queue_path.is_absolute():
            queue_path = self.study_dir / queue_path
        minutes = study.queue_minutes(queue_path.read_text() if queue_path.exists() else collected.get("queue"))
        # Cross-check the extras against the round record.
        meta = collected.get("meta") or ""
        m = re.search(r"\bprovider (\S+)", meta)
        extra = list(driver.get("extra") or [])
        if m and not ("--provider" in extra and extra[extra.index("--provider") + 1] == m.group(1)):
            raise SystemExit(f"driver extras {extra} disagree with {a.round}.meta provider {m.group(1)}")
        knowledge = settings["knowledge"]
        return {
            "binary": str(binary), "script": str(script), "driver_source": source,
            "driver_command": driver.get("command"), "extra": extra,
            "max_steps": settings["max_steps"], "max_usd": settings["max_usd"],
            "knowledge": str(expand(knowledge)) if knowledge else None, "minutes": minutes,
        }

    def _default_script(self) -> str:
        for name in ("screen2.sh", "screen.sh"):
            if (self.study_dir / name).exists():
                return str(self.study_dir / name)
        raise SystemExit("no screen script found")

    # ------------------------------------------------------------ plan
    def eligible(self, collected: dict, runs: list) -> tuple[list, list]:
        held, _ = study.pools()
        burned = study.listed(collected.get("burned"), "BURNED")
        ungradeable = study.listed(collected.get("notes"), "UNGRADEABLE")
        todo, skipped = [], []
        for task in held:
            if not study.screen_passed(runs, task):
                continue
            if task in self.state["tasks"]:
                continue
            if task in burned:
                skipped.append((task, "burned"))
                continue
            if task in ungradeable:
                skipped.append((task, "ungradeable"))
                continue
            mine = [r for r in runs if r["task"] == task]
            if any(r["arm"] == "off" for r in mine) or sum(r["arm"] == "on" for r in mine) >= 3:
                skipped.append((task, "confirmation-shaped runs already exist in the round directory"))
                continue
            todo.append(task)
        return todo, skipped

    def command(self, cfg: dict, task: str, arm: str) -> list:
        mins = cfg["minutes"].get(task)
        if mins is None:
            raise SystemExit(f"{task} has no time cap in the round's queue")
        kb = "off" if arm == "off" else "candidates"
        return [cfg["binary"], task, "--kb", kb, "--max-steps", cfg["max_steps"], "--max-minutes",
                str(mins), "--max-usd", cfg["max_usd"], *cfg["extra"]]

    # ------------------------------------------------------------ run
    def run_one(self, cfg: dict, task: str, arm: str, index: int) -> None:
        with self.lock:
            ms = int(time.time() * 1000)
            time.sleep(0.002)  # distinct epoch-ms names
        log = self.rdir / f"{task}.{arm}.{ms}.log"
        env = dict(os.environ)
        if cfg["knowledge"]:
            env["OPENAGENTS_KNOWLEDGE"] = cfg["knowledge"]
        with self.lock:
            self.state["tasks"][task]["runs"][index].update(log=log.name, started=ms, status="running")
            self._save_state()
        with log.open("wb") as out:
            try:
                rc = subprocess.run(self.command(cfg, task, arm), stdout=out, stderr=subprocess.STDOUT,
                                    env=env).returncode
            except OSError as e:
                out.write(f"confirm.py: couldn't start the run: {e}\n".encode())
                rc = 127
        # The screen script's result line: its last line matching the same
        # pattern, cut to 300 characters.
        found = ""
        with log.open("rb") as f:
            for raw in f:
                line = raw.decode("utf-8", "replace").rstrip("\n")
                if RESULT_LINE.search(line):
                    found = line
        stamp = datetime.now().astimezone().isoformat(timespec="seconds")
        entry = f"{stamp} {task} {arm} rc={rc} {found[:300]} log={log.name}\n"
        with self.lock:
            with (self.rdir / "outcomes.txt").open("a") as f:
                f.write(entry)
            self.state["tasks"][task]["runs"][index].update(status="done", rc=rc)
            self._save_state()

    def loop(self) -> int:
        a = self.a
        pool = None if a.dry_run else ThreadPoolExecutor(max_workers=a.max_parallel)
        futures = []
        while True:
            # Look at the driver before the outcomes, so a screen that ends as
            # the driver exits is still seen on this pass.
            driver_alive = find_driver(a.round) is not None
            collected = study.collect(str(self.study_dir), a.round, a.runs_dir)
            runs = study.assemble(collected)
            todo, skipped = self.eligible(collected, runs)
            cfg = self.config(collected, runs) if (todo or a.dry_run) else None
            if a.dry_run:
                self.print_plan(cfg, todo, skipped, runs)
                return 0
            if cfg and not self.state.get("driver") and cfg["driver_source"] == "running driver":
                self.state["driver"] = find_driver(a.round)
            for task in todo:
                with self.lock:
                    self.state["tasks"][task] = {
                        "queued_at": datetime.now().astimezone().isoformat(timespec="seconds"),
                        "config": {k: cfg[k] for k in ("binary", "extra", "max_steps", "max_usd", "knowledge")},
                        "minutes": cfg["minutes"].get(task),
                        "runs": [{"arm": arm, "status": "queued"} for arm in PLAN]}
                    self._save_state()
                print(f"queued {task}: " + ", ".join(PLAN), flush=True)
                for i, arm in enumerate(PLAN):
                    futures.append(pool.submit(self.run_one, cfg, task, arm, i))
            for task, why in skipped:
                if task not in self.reported:
                    print(f"skip {task}: {why}", flush=True)
                    self.reported.add(task)
            if a.once:
                for f in futures:
                    f.result()
                pool.shutdown(wait=True)
                return 0
            for f in [f for f in futures if f.done()]:
                f.result()
                futures.remove(f)
            if not driver_alive and not futures and not todo:
                pool.shutdown(wait=True)
                print("screen driver gone and no confirmation runs left; done", flush=True)
                return 0
            time.sleep(a.poll)

    def print_plan(self, cfg, todo, skipped, runs) -> None:
        a = self.a
        passed = [t for t in study.pools()[0] if study.screen_passed(runs, t)]
        print(f"round {a.round}: dry run, nothing started or written")
        if cfg:
            print(f"  binary        {cfg['binary']}")
            print(f"  flags from    {cfg['script']}: --max-steps {cfg['max_steps']} --max-usd {cfg['max_usd']}")
            print(f"  knowledge     OPENAGENTS_KNOWLEDGE={cfg['knowledge']}")
            print(f"  extra args    {' '.join(cfg['extra']) or '(none)'}  [{cfg['driver_source']}]")
            if cfg["driver_command"]:
                print(f"  driver        {cfg['driver_command']}")
        graded = sum(1 for r in runs if r["kind"] in study.RESULT_KINDS)
        print(f"  outcomes      {graded} graded runs; held-out screens passed: {', '.join(passed) or 'none'}")
        already = sorted(self.state["tasks"])
        if already:
            print(f"  already queued (never again): {', '.join(already)}")
        for task, why in skipped:
            print(f"  skip {task}: {why}")
        if not todo:
            print("  would queue: nothing")
        for task in todo:
            print(f"  would queue {task} ({cfg['minutes'].get(task)} min cap), at most {a.max_parallel} at a time:")
            for arm in PLAN:
                print("    " + " ".join(shlex.quote(x) for x in self.command(cfg, task, arm)))


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--round", required=True)
    ap.add_argument("--study-dir", default="~/study-oos")
    ap.add_argument("--runs-dir", default="~/.openagents/microcoder/runs")
    ap.add_argument("--bin", help="override the round binary (default ~/.local/bin/microcoder-study-<round>)")
    ap.add_argument("--script", help="the round's screen script, when its driver isn't running "
                                     "(default: the driver's, else screen2.sh)")
    ap.add_argument("--driver-args", help="extra microcoder args when the round driver isn't running, "
                                          "e.g. '--provider openrouter --model openai/gpt-6-luna'")
    ap.add_argument("--max-parallel", type=int, default=3)
    ap.add_argument("--poll", type=float, default=60.0, help="seconds between looks at the outcomes")
    ap.add_argument("--once", action="store_true", help="queue what is eligible now, run it, then exit")
    ap.add_argument("--dry-run", action="store_true", help="print what would be queued; start and write nothing")
    a = ap.parse_args()
    if a.max_parallel < 1 or a.max_parallel > 3:
        ap.error("--max-parallel must be 1-3 (the study allows at most 3 confirmation runs at a time)")
    c = Confirmer(a)
    if not c.rdir.is_dir():
        raise SystemExit(f"no round directory {c.rdir}")
    if a.dry_run:
        sys.exit(c.loop())
    lock = (c.rdir / "confirm.lock").open("w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        raise SystemExit("another confirm.py is running for this round")
    sys.exit(c.loop())


if __name__ == "__main__":
    main()
