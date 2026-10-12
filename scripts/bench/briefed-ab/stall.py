"""A stall detector for the bench loops (#11433).

On 2026-10-12 `ab.py batch` spun for about 20 hours past its last result row
(docs/audits/2026-10-12-claude-usage-burn): its dollar cap never tripped
because nothing was being spent. `StallWatch` aborts a run when no new
result row (result.json, error.txt or judge.json under the tag directory)
lands for a bounded interval.

    watch = StallWatch(tag_dir, minutes=120)
    watch.start()        # a daemon thread; on a stall it stops the run

The interval comes from `--stall-min`, a plan's "stall_min", or
AB_STALL_MIN (default 120); 0 turns the watch off. On a stall it writes
stalled.txt in the tag directory, terminates this process's children (the
trials' `claude -p`, ssh and cargo trees) and exits with status 3.
"""

from __future__ import annotations

import os
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import Callable

ROW_FILES = ("result.json", "error.txt", "judge.json")
DEFAULT_MIN = 120.0
STALL_EXIT = 3


def stall_minutes(cli: float | None, plan: dict | None) -> float:
    if cli is not None:
        return cli
    if plan and plan.get("stall_min") is not None:
        return float(plan["stall_min"])
    return float(os.environ.get("AB_STALL_MIN", DEFAULT_MIN))


def last_row(tag_dir: Path) -> float:
    """The newest result row's mtime under tag_dir (0 when none)."""
    newest = 0.0
    try:
        for d in tag_dir.iterdir():
            for name in ROW_FILES:
                try:
                    newest = max(newest, (d / name).stat().st_mtime)
                except OSError:
                    pass
    except OSError:
        pass
    return newest


def kill_children(pid: int | None = None) -> None:
    """TERM every descendant of pid (default: this process), then KILL."""
    pid = pid or os.getpid()
    out = subprocess.run(["ps", "-A", "-o", "pid=,ppid="], capture_output=True, text=True).stdout
    kids: dict[int, list[int]] = {}
    for line in out.split("\n"):
        f = line.split()
        if len(f) == 2 and f[0].isdigit() and f[1].isdigit():
            kids.setdefault(int(f[1]), []).append(int(f[0]))
    tree, stack = [], [pid]
    while stack:
        for k in kids.get(stack.pop(), []):
            tree.append(k)
            stack.append(k)
    for sig in (signal.SIGTERM, signal.SIGKILL):
        for k in tree:
            try:
                os.kill(k, sig)
            except (ProcessLookupError, PermissionError):
                pass
        if sig == signal.SIGTERM and tree:
            time.sleep(3)


class StallWatch:
    def __init__(self, tag_dir: Path, minutes: float, clock: Callable[[], float] = time.time,
                 on_stall: Callable[[str], None] | None = None):
        self.tag_dir = Path(tag_dir)
        self.limit = minutes * 60
        self.clock = clock
        self.started = clock()
        self.on_stall = on_stall or self.abort
        self._stop = threading.Event()

    def progress(self) -> float:
        """When the run last made progress: its start or its newest row."""
        return max(self.started, last_row(self.tag_dir))

    def idle(self) -> float:
        return self.clock() - self.progress()

    def stalled(self) -> bool:
        return self.limit > 0 and self.idle() >= self.limit

    def check(self) -> bool:
        if self.stalled():
            mins = self.idle() / 60
            self.on_stall(f"no result row in {mins:.0f} minutes (limit {self.limit / 60:.0f}); aborting")
            return True
        return False

    def abort(self, why: str) -> None:
        print(f"stall: {why}", flush=True)
        try:
            (self.tag_dir / "stalled.txt").write_text(f"{time.strftime('%Y-%m-%dT%H:%M:%S')} {why}\n")
        except OSError:
            pass
        kill_children()
        sys.stdout.flush()
        os._exit(STALL_EXIT)

    def start(self, every: float | None = None) -> "StallWatch":
        if self.limit <= 0:
            return self
        every = every or max(5.0, min(60.0, self.limit / 10))

        def loop() -> None:
            while not self._stop.wait(every):
                if self.check():
                    return

        threading.Thread(target=loop, name="stall-watch", daemon=True).start()
        return self

    def stop(self) -> None:
        self._stop.set()
