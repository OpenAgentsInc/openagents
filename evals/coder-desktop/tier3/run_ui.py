#!/usr/bin/env python3
# Firm reason: 5 (Tier 3 runner: builds a scratch Coder.app, drives it through flows, compares window captures)
"""Tier 3: the real Coder.app, driven end to end, with window captures checked against goldens.

  python3 evals/coder-desktop/tier3/run_ui.py                    build, run every flow, compare
  python3 evals/coder-desktop/tier3/run_ui.py --filter settings  flows whose name contains TEXT
  python3 evals/coder-desktop/tier3/run_ui.py --no-build         reuse the last scratch build
  python3 evals/coder-desktop/tier3/run_ui.py --update-goldens   write the captures as the goldens

What it does:
1. Builds Coder.app with apps/coder-desktop/build.sh into results/tier3-build/ (never
   /Applications). It copies the bundle, gives the copy its own bundle id
   (com.openagents.coder.evals-t3, so its preferences never touch Coder's), and replaces the
   bundled mcworker and claudeworker with a two-line script that runs tier3/mock_router.py: every
   worker the app starts replays a Tier 1 mock script, so no model is called.
2. Compiles tier3/probe/probe.m into a dylib. Each flow starts the copy's binary with
   DYLD_INSERT_LIBRARIES set to it, a fresh temp HOME (CFFIXED_USER_HOME too, so Application
   Support/Coder lands there), updates off (CODER_UPDATE_URL empty) and BROWSER=/usr/bin/true.
   The probe keeps the app from taking focus, parks its window behind every other window with one
   point on screen, and drives it over a Unix socket with real NSEvents.
3. Captures the window with `screencapture -l` (needs Screen Recording for the terminal) once the
   frame stops changing, and compares it with tier3/goldens/NAME.png: a pixel differs when a
   channel moves more than --pixel-tolerance; a capture fails when more than --max-diff of its
   pixels differ. On a failure it writes the capture, the golden and a difference image (changed
   pixels red over a dimmed golden) to the run folder.
4. Quits the app (Command-Q through its menu; SIGTERM while a reply runs, as Coder would ask
   first), stops the run daemon (`coder-host serve`) that launch started in the temp HOME and any
   mock worker left, by their own PIDs (never by name), and deletes the copy's preference domain and
   the temp HOME. The copy is a test launch (CODER_TEST_LAUNCH, #13752): it stops its own daemon when
   it quits, and quits itself if no driver speaks for CODER_TEST_IDLE_S seconds, so a runner that dies
   cannot leave it running.
5. Checks memory: send_stream_new_chat samples `footprint -p PID` after launch and after the reply
   streamed, prints both, and fails when it grew more than --footprint-limit-mb (default 300).

Per run: results/tier3-<time>-<sha>/ with summary.json and, per flow, the captures, any
difference images, the worker records (rec/) and app.log. One line per flow on stdout:
  ok   flow NAME: DESCRIPTION (N ms, K captures)
  FAIL flow NAME: DESCRIPTION (N ms, K captures) -- why
"""

from __future__ import annotations

import argparse
import json
import os
import plistlib
import re
import shutil
import signal
import socket
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Callable

from PIL import Image, ImageChops

REPO = Path(__file__).resolve().parents[3]
HERE = REPO / "evals/coder-desktop/tier3"
GOLDENS = HERE / "goldens"
RESULTS = REPO / "evals/coder-desktop/results"
BUNDLE_ID = "com.openagents.desktop.evals-t3"


class FlowFailed(Exception):
    pass


# ---- the app --------------------------------------------------------------------------------

def build_app(build_dir: Path) -> Path:
    env = dict(os.environ, CODER_BUILD=str(build_dir))
    res = subprocess.run(["bash", str(REPO / "apps/openagents-desktop/build.sh")], cwd=REPO, env=env, capture_output=True, text=True)
    if res.returncode != 0:
        sys.stderr.write(res.stdout[-4000:] + res.stderr[-4000:])
        raise SystemExit(f"build.sh failed with exit {res.returncode}")
    return build_dir / "OpenAgents.app"


def stage_app(built: Path, run_dir: Path) -> tuple[Path, Path]:
    """The copy the flows run: own bundle id, mock workers, re-signed. Returns (binary, probe dylib)."""
    app = run_dir / "OpenAgents.app"
    shutil.copytree(built, app, symlinks=True)
    plist = app / "Contents/Info.plist"
    info = plistlib.loads(plist.read_bytes())
    info["CFBundleIdentifier"] = BUNDLE_ID
    plist.write_bytes(plistlib.dumps(info))
    # the real worker stays beside the stand-in: the follow-ups flow runs it on recorded answers
    shutil.copy2(app / "Contents/Resources/mcworker", app / "Contents/Resources/mcworker-real")
    for name in ("mcworker", "claudeworker"):
        w = app / "Contents/Resources" / name
        w.write_text(f'#!/bin/sh\nexec "{sys.executable}" -I "{HERE / "mock_router.py"}" "$@"\n', encoding="utf-8")
        w.chmod(0o755)
    subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True, capture_output=True)
    dylib = run_dir / "probe.dylib"
    subprocess.run(["clang", "-dynamiclib", "-fobjc-arc", "-mmacosx-version-min=14.0", "-framework", "AppKit",
                    "-o", str(dylib), str(HERE / "probe/probe.m")], check=True)
    return app / "Contents/MacOS/OpenAgents", dylib


def forget_prefs() -> None:
    subprocess.run(["defaults", "delete", BUNDLE_ID], capture_output=True)


def command_of(pid: int) -> str:
    return subprocess.run(["ps", "-o", "command=", "-p", str(pid)], capture_output=True, text=True).stdout


def stop_pids(pids: list[int], wait_s: float = 3.0) -> None:
    """SIGTERM these PIDs, then SIGKILL any still alive after wait_s."""
    for pid in pids:
        try:
            os.kill(pid, signal.SIGTERM)
        except OSError:
            pass
    deadline = time.time() + wait_s
    left = list(pids)
    while left and time.time() < deadline:
        time.sleep(0.1)
        left = [p for p in left if alive(p)]
    for pid in left:
        try:
            os.kill(pid, signal.SIGKILL)
        except OSError:
            pass


def alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except OSError:
        return False
    return True


def stop_coder_host(home: Path, marker: str) -> list[int]:
    """Stop the `coder-host serve` a launch under this temp HOME started (#13694 keeps it running past
    Coder's quit; #13752). Found by PID only: the pid in its daemon.json, and whoever holds its
    daemon.log open (lsof on that one path), each kept only when its command line is under `marker`
    (the staged Coder.app). Never by name. Returns the PIDs it stopped."""
    host = home / ".openagents/coder/host"
    pids: set[int] = set()
    try:
        pid = int(json.loads((host / "daemon.json").read_text()).get("pid") or 0)
        if pid:
            pids.add(pid)
    except (OSError, ValueError):
        pass
    log = host / "daemon.log"
    if log.exists():
        out = subprocess.run(["lsof", "-t", str(log)], capture_output=True, text=True).stdout
        pids.update(int(t) for t in out.split() if t.isdigit())
    mine = sorted(p for p in pids if p != os.getpid() and marker in command_of(p))
    stop_pids(mine)
    return mine


def footprint_mb(pid: int) -> float | None:
    """The process's physical footprint in MB, as `footprint -p` reports it."""
    out = subprocess.run(["footprint", "-p", str(pid)], capture_output=True, text=True).stdout
    m = re.search(r"Footprint:\s*([\d.]+)\s*(B|KB|MB|GB)", out)
    if not m:
        return None
    return float(m.group(1)) * {"B": 1 / 1048576, "KB": 1 / 1024, "MB": 1.0, "GB": 1024.0}[m.group(2)]


class App:
    """One launch of the scratch Coder.app under a temp HOME, with the probe's socket."""

    def __init__(self, binary: Path, dylib: Path, home: Path, case_dir: Path, workers: dict, args: list[str]) -> None:
        self.case_dir = case_dir
        self.sock_path = Path(os.environ.get("TMPDIR", "/tmp")) / f"ct3-{os.getpid()}.sock"
        rec = case_dir / "rec"
        rec.mkdir(parents=True, exist_ok=True)
        wfile = case_dir / "workers.json"
        wfile.write_text(json.dumps(workers), encoding="utf-8")
        env = {
            "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
            "HOME": str(home), "CFFIXED_USER_HOME": str(home), "TMPDIR": os.environ.get("TMPDIR", "/tmp"),
            # the daemon's rule is $CODER_HOME/.openagents/coder/host: CODER_HOME is the temp HOME itself
            "CODER_HOME": str(home), "CODER_RELEASE_HOME": str(home / ".cache/coder-release"),
            "CODER_UPDATE_URL": "", "BROWSER": "/usr/bin/true", "LANG": "en_US.UTF-8",
            "CODER_T3_SOCKET": str(self.sock_path), "CODER_T3_WORKERS": str(wfile), "CODER_T3_RECORD": str(rec),
            "CODER_T3_REAL_WORKER": str(binary.parent.parent / "Resources/mcworker-real"),
            "DYLD_INSERT_LIBRARIES": str(dylib),
            # a test launch (#13752): quits itself after this long with no driver command
            "CODER_TEST_LAUNCH": "tier3", "CODER_TEST_IDLE_S": "300",
        }
        self.home = home
        self.marker = str(binary.parent.parent.parent)   # the staged Coder.app
        if self.sock_path.exists():
            self.sock_path.unlink()
        self.log = open(case_dir / "app.log", "ab")
        self.proc = subprocess.Popen([str(binary), *args], env=env, stdout=self.log, stderr=self.log, stdin=subprocess.DEVNULL)
        self.conn: socket.socket | None = None
        self.reader = None
        try:
            self._await_probe()
        except BaseException:
            # The flow never gets this App, so its cleanup cannot quit it: a Coder hung at launch
            # (2026-11-19: 8 of them, 12-26 GB each) must be killed here, by the PID this runner started.
            self._kill_launch()
            raise

    def _kill_launch(self) -> None:
        if self.proc.poll() is None:
            os.kill(self.proc.pid, signal.SIGKILL)
            self.proc.wait(timeout=5)
        if self.conn is not None:
            self.conn.close()
            self.conn = None
        self.log.close()
        if self.sock_path.exists():
            self.sock_path.unlink()

    def _await_probe(self) -> None:
        deadline = time.time() + 20
        while time.time() < deadline:
            if self.proc.poll() is not None:
                raise FlowFailed(f"Coder exited at launch with {self.proc.returncode}")
            try:
                s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                s.connect(str(self.sock_path))
                s.settimeout(15)
                self.conn = s
                self.reader = s.makefile("r", encoding="utf-8")
                if self.call("ping").get("window"):
                    return
            except OSError:
                time.sleep(0.1)
                continue
            time.sleep(0.1)
        raise FlowFailed("the probe socket never answered")

    def call(self, line: str) -> dict:
        assert self.conn is not None and self.reader is not None
        self.conn.sendall((line + "\n").encode())
        try:
            reply = self.reader.readline()
        except OSError as e:   # socket.timeout: the main thread is stuck (a modal alert?)
            raise FlowFailed(f"no reply to {line!r} in 15 s: {e}")
        if not reply:
            raise FlowFailed(f"no reply to {line!r} (Coder gone?)")
        return json.loads(reply)

    def do(self, line: str) -> dict:
        r = self.call(line)
        if not r.get("ok"):
            raise FlowFailed(f"{line!r}: {r.get('error')}")
        return r

    def state(self) -> dict:
        return self.do("state")

    def wait(self, what: str, test: Callable[[dict], bool], timeout: float = 10.0) -> dict:
        deadline = time.time() + timeout
        st = self.state()
        while not test(st):
            if time.time() > deadline:
                raise FlowFailed(f"timed out waiting for {what}; state {json.dumps(st)}")
            time.sleep(0.1)
            st = self.state()
        return st

    def capture(self, out: Path, settle_s: float = 6.0) -> Path:
        """The window as WindowServer has it, once two captures 150 ms apart match (or settle_s passes)."""
        wid = self.do("window")["id"]
        last = None
        deadline = time.time() + settle_s
        tmp = out.with_suffix(".tmp.png")
        while True:
            time.sleep(0.15)
            res = subprocess.run(["screencapture", "-x", "-o", "-l", str(wid), str(tmp)], capture_output=True, text=True)
            if res.returncode != 0 or not tmp.exists():
                raise FlowFailed(f"screencapture failed: {res.stderr.strip()} (Screen Recording permission?)")
            data = tmp.read_bytes()
            if data == last or time.time() > deadline:
                break
            last = data
        tmp.replace(out)
        return out

    def quit(self) -> None:
        # Command-Q, as a person quits; while a reply runs Coder would ask first in a modal alert, so
        # then it gets SIGTERM (only the PID this runner started).
        try:
            if self.conn is not None and self.proc.poll() is None:
                if self.state()["running"]:
                    os.kill(self.proc.pid, signal.SIGTERM)
                else:
                    self.call("key cmd+q")
        except (OSError, FlowFailed, json.JSONDecodeError):
            if self.proc.poll() is None:
                os.kill(self.proc.pid, signal.SIGTERM)
        try:
            self.proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.kill(self.proc.pid, signal.SIGKILL)   # only the PID this runner started
            self.proc.wait(timeout=5)
        if self.conn is not None:
            self.conn.close()
            self.conn = None
        self.log.close()
        if self.sock_path.exists():
            self.sock_path.unlink()
        # Coder stops the daemon it started as it quits; one left (SIGKILL, or a daemon an earlier
        # launch on this HOME started) is stopped here, by its PID
        stop_coder_host(self.home, self.marker)


# ---- comparing ------------------------------------------------------------------------------

def compare(actual: Path, golden: Path, case_dir: Path, name: str, tol: int, max_diff: float, masks: list[list[float]]) -> str | None:
    """None when the capture matches its golden, else why (and difference images in case_dir)."""
    if not golden.exists():
        return f"no golden {golden.relative_to(REPO)} (run with --update-goldens)"
    a = Image.open(actual).convert("RGB")
    g = Image.open(golden).convert("RGB")
    if a.size != g.size:
        return f"{name}: size {a.size} vs golden {g.size} (a different backing scale or window size)"
    diff = ImageChops.difference(a, g)
    # a pixel differs when any channel moved more than tol
    r, gg, b = diff.split()
    lut = [255 if v > tol else 0 for v in range(256)]
    mask = ImageChops.lighter(ImageChops.lighter(r.point(lut), gg.point(lut)), b.point(lut))
    scale = a.size[0] / max(1.0, golden_points_width(golden, a.size[0]))
    for (x, y, w, h) in masks:
        mask.paste(0, (int(x * scale), int(y * scale), int((x + w) * scale), int((y + h) * scale)))
    changed = sum(mask.histogram()[1:])
    frac = changed / (a.size[0] * a.size[1])
    if frac <= max_diff:
        return None
    shutil.copy(golden, case_dir / f"{name}.golden.png")
    shown = Image.blend(g, Image.new("RGB", g.size, (0, 0, 0)), 0.6)
    shown.paste(Image.new("RGB", g.size, (255, 0, 0)), (0, 0), mask)
    shown.save(case_dir / f"{name}.diff.png")
    box = mask.getbbox()
    return f"{name}: {frac:.3%} of pixels differ (limit {max_diff:.3%}), in {box} px; see {name}.diff.png"


def golden_points_width(golden: Path, px_width: int) -> float:
    # captures are at the window's backing scale; masks are in points of the window (title bar included)
    meta = golden.with_suffix(".json")
    if meta.exists():
        return float(json.loads(meta.read_text())["points_w"])
    return px_width / 2.0


# ---- flows ----------------------------------------------------------------------------------

def reply_script(text: str, thought: str = "Checking project layout and symbols.") -> dict:
    half = len(text) // 2
    return {"turns": [{"steps": [
        {"emit": {"kind": "thought", "text": thought}}, {"sleep_ms": 250},
        {"emit": {"kind": "reply", "text": text[:half]}}, {"sleep_ms": 250},
        {"emit": {"kind": "reply", "text": text[half:]}},
        {"emit": {"kind": "attempt", "model": "gemini-4-neon", "in_tokens": 1200, "out_tokens": 300, "usd_micros": 15000}},
        {"emit": {"kind": "ended", "ending": "finished", "text": text}}]}]}


REPLY = "Here is a streamed answer about the layout."
STOP_SCRIPT = {"turns": [{"steps": [
    {"emit": {"kind": "thought", "text": "Starting a long operation."}},
    {"emit": {"kind": "reply", "text": "Working on step 1..."}},
    {"wait_for_stop": True, "then_emit": {"kind": "ended", "ending": "stopped", "text": "Stopped mid-turn."}}]}]}
DELEGATE_SCRIPT = {"turns": [{"steps": [
    {"emit": {"kind": "thought", "text": "Delegating two checks."}},
    {"emit": {"kind": "delegate", "tasks": [{"level": "hydrogen", "task": "Check module A"}, {"level": "hydrogen", "task": "Check module B"}], "ticket": 11}},
    {"expect": "results"},
    {"emit": {"kind": "reply", "text": "Both sub-agents finished their checks."}},
    {"emit": {"kind": "ended", "ending": "finished", "text": "Both sub-agents finished their checks."}}]}]}
AGENT_A = {"turns": [{"steps": [
    {"emit": {"kind": "thought", "text": "Listing module A."}},
    {"emit": {"kind": "command", "command": "ls module_a", "exit": 0, "timed_out": False, "output": "a.coax\nb.coax"}},
    {"emit": {"kind": "reply", "text": "Module A has two files and both look fine."}},
    {"emit": {"kind": "ended", "ending": "finished", "text": "Module A has two files and both look fine."}}]}]}   # ended carries the whole reply
AGENT_B = {"turns": [{"steps": [{"emit": {"kind": "ended", "ending": "finished", "text": "Module B ok"}}]}]}
WORKERS = {"main": [{"match": "long analysis", "script": STOP_SCRIPT}, {"match": "all modules", "script": DELEGATE_SCRIPT}],
           "default": reply_script(REPLY), "agents": {"0": AGENT_A, "1": AGENT_B}}


class Flow:
    def __init__(self, ctx: "Ctx", name: str) -> None:
        self.ctx, self.name = ctx, name
        self.case_dir = ctx.run_dir / "cases" / name
        self.case_dir.mkdir(parents=True, exist_ok=True)
        self.home = Path(os.environ.get("TMPDIR", "/tmp")) / "coder-t3-homes" / name
        if self.home.exists():
            shutil.rmtree(self.home)
        self.home.mkdir(parents=True)
        self.app: App | None = None
        self.workers: dict = WORKERS
        self.captures = 0
        self.problems: list[str] = []
        self.notes: list[str] = []

    def launch(self, *args: str, size: tuple[int, int] = (1200, 800)) -> App:
        forget_prefs()
        self.app = App(self.ctx.binary, self.ctx.dylib, self.home, self.case_dir, self.workers, list(args))
        self.app.do(f"size {size[0]} {size[1]}")
        time.sleep(0.4)
        return self.app

    def relaunch(self, *args: str, size: tuple[int, int] = (1200, 800)) -> App:
        """A second launch on the same HOME (the chats kept), preferences kept."""
        self.app = App(self.ctx.binary, self.ctx.dylib, self.home, self.case_dir, self.workers, list(args))
        self.app.do(f"size {size[0]} {size[1]}")
        time.sleep(0.4)
        return self.app

    def check(self, golden: str, masks: list[list[float]] | None = None) -> None:
        assert self.app is not None
        out = self.case_dir / f"{golden}.png"
        self.app.capture(out)
        self.captures += 1
        gpath = GOLDENS / f"{golden}.png"
        if self.ctx.update:
            GOLDENS.mkdir(parents=True, exist_ok=True)
            shutil.copy(out, gpath)
            w = self.app.do("window")
            gpath.with_suffix(".json").write_text(json.dumps({"points_w": w["w"], "points_h": w["h"], "scale": w["scale"]}) + "\n")
            return
        why = compare(out, gpath, self.case_dir, golden, self.ctx.tol, self.ctx.max_diff, masks or [])
        if why:
            self.problems.append(why)

    def stop_daemon(self) -> None:
        """Coder starts the run daemon (`coder-host serve`, #13694) in the temp HOME and it outlives the
        app on purpose. Stop it and any mock worker still running: only the PIDs this flow made (the
        daemon's own daemon.json and log, the workers' record file names), never by name."""
        stop_coder_host(self.home, str(self.ctx.run_dir))
        pids = []
        for rec in (self.case_dir / "rec").glob("*-*.jsonl"):
            pid = int(rec.stem.rsplit("-", 1)[1])
            if "mock_router.py" in command_of(pid):
                pids.append(pid)
        stop_pids(pids)

    def expect(self, ok: bool, why: str) -> None:
        if not ok:
            self.problems.append(why)

    def press(self, row: str) -> None:
        """A click in the middle of the page's row with this id, as drawn."""
        assert self.app is not None
        b = self.app.do("box " + row)
        self.app.do("click %d %d" % (b["x"] + b["w"] / 2, b["y"] + b["h"] / 2))

    def records(self, role: str) -> list[dict]:
        out = []
        for f in sorted((self.case_dir / "rec").glob(f"{role}-*.jsonl")):
            out += [json.loads(ln) for ln in f.read_text().splitlines() if ln.strip()]
        return out

    def chats(self) -> list[Path]:
        d = self.home / "Library/Application Support/Coder/chats"
        return [p for p in d.glob("*.json") if p.name != "index.json"] if d.exists() else []

    def send(self, text: str) -> None:
        assert self.app is not None
        self.app.do("type " + text)
        c = self.app.do("composer")
        self.expect(c["text"] == text, f"the composer holds {c['text']!r} after typing {text!r}")
        self.app.do("key return")


# Controls are pressed where the page draws them, by their row ids (Flow.press, the probe's `box`,
# coder_page_box): no fixed points, so a layout change moves the click with the control.
NEW_CHAT = "coder.new"   # the sidebar's New chat row (the first row with that id)


def side_edge_x(f: Flow) -> float:
    """The chats' sidebar's right edge: its rows are inset the same at both sides (app_side_action)."""
    assert f.app is not None
    b = f.app.do("box coder.search")
    return 2 * b["x"] + b["w"]


def press_chat(f: Flow) -> None:
    """A click on the one kept chat's sidebar row: its id is "chat." and the chat's id, its file's name (page/chats.coax)."""
    kept = f.chats()
    if len(kept) != 1:
        raise FlowFailed(f"{len(kept)} kept chats, want 1")
    f.press("chat." + kept[0].stem)


def flow_idle_layouts(f: Flow) -> str:
    """Idle at 1200x800 and the narrow 820x700 window"""
    app = f.launch()
    f.check("idle_1200x800")
    app.do("size 820 700")
    time.sleep(0.4)
    f.check("narrow_820x700")
    return "idle layouts"


def flow_send_stream(f: Flow) -> str:
    """New chat, type, Return, the reply streams in and is kept; New chat empties the page"""
    app = f.launch()
    at_launch = footprint_mb(app.proc.pid)
    f.send("Explain the layout")
    app.wait("the reply to start", lambda s: s["running"] == 1 or s["cost_usd_micros"] > 0, 5)
    st = app.wait("the reply to end", lambda s: s["running"] == 0 and s["cost_usd_micros"] > 0, 10)
    # memory (#13752): a leftover test Coder once grew 3.5 MB/s to 26 GB; the footprint after the
    # streamed reply may be at most --footprint-limit-mb over the one at launch
    after = footprint_mb(app.proc.pid)
    limit = f.ctx.footprint_limit_mb
    if at_launch is None or after is None:
        f.problems.append(f"footprint -p {app.proc.pid} gave no number (launch {at_launch}, after {after})")
    else:
        f.notes.append(f"footprint {at_launch:.0f} MB at launch, {after:.0f} MB after the stream (limit +{limit:.0f} MB)")
        f.expect(after - at_launch <= limit, f"footprint grew {after - at_launch:.0f} MB ({at_launch:.0f} -> {after:.0f} MB), limit {limit:.0f} MB")
    f.expect(st["title"] == "Explain the layout", f"chat title {st['title']!r}")
    f.expect(st["cost_usd_micros"] == 15000, f"cost {st['cost_usd_micros']}")
    tasks = [r for r in f.records("main") if r["kind"] == "task"]
    f.expect(len(tasks) == 1 and tasks[0]["msg"]["task"] == "Explain the layout", f"worker tasks {tasks}")
    f.check("replied_1200x800")
    f.expect(len(f.chats()) == 1, f"{len(f.chats())} chats kept, want 1")
    f.press(NEW_CHAT)
    st = app.wait("a new chat", lambda s: s["title"] == "", 3)
    c = app.do("composer")
    f.expect(c["text"] == "" and c["focused"], f"composer after New chat: {c}")
    f.check("new_chat_1200x800")
    return "send and stream, new chat"


def flow_stop(f: Flow) -> str:
    """A running reply shows Stop; a click on it stops the worker"""
    app = f.launch()
    f.send("Run a long analysis")
    app.wait("the reply to run", lambda s: s["running"] == 1, 5)
    time.sleep(0.5)
    # the sidebar row's spinner, the thought's spinner and the working line's spinner and clock move
    # (window points: the page fills the window under its transparent title bar since #13741)
    f.check("streaming_1200x800", masks=[[230, 170, 34, 32], [400, 118, 28, 36], [400, 220, 130, 34]])
    f.press("coder.composer.send")
    app.wait("the reply to stop", lambda s: s["running"] == 0, 5)
    stops = [r for r in f.records("main") if r["kind"] == "stop"]
    f.expect(len(stops) == 1, f"{len(stops)} stop lines reached the worker, want 1")
    f.check("stopped_1200x800", masks=[WORKED_FOR])
    return "stop"


def flow_subagent_pane(f: Flow) -> str:
    """Two sub-agents run; the first one's transcript opens in the right pane (--agent 1)"""
    app = f.launch("--agent", "1", size=(1440, 900))
    f.send("Check all modules")
    app.wait("the delegated reply to end", lambda s: s["running"] == 0 and s["title"] != "", 10)
    time.sleep(0.5)
    f.expect(len(f.records("agent-0")) >= 1 and len(f.records("agent-1")) >= 1, "both sub-agent workers started")
    results = [r for r in f.records("main") if r["kind"] == "results"]
    f.expect(len(results) == 1, f"{len(results)} results lines reached the main worker, want 1")
    f.check("subagent_pane_1440x900")
    return "sub-agent pane"


def flow_sidebar_resize(f: Flow) -> str:
    """Dragging the sidebar's edge widens it, and the width is kept"""
    app = f.launch()
    before = app.state()["side_width"]
    edge = side_edge_x(f)
    app.do(f"drag {edge:.0f} 400 {edge + 90:.0f} 400 15")
    st = app.wait("the width to be kept", lambda s: s["side_width"] > 0, 3)
    # kept in the page's units, points since #13737: the edge dragged 90 further
    want = edge + 90
    f.expect(before == 0 and abs(st["side_width"] - want) <= 15, f"side width {before} -> {st['side_width']}, want about {want:.0f} (edge at {edge:.0f})")
    f.check("sidebar_resized_1200x800")
    return "sidebar resize"


SETTINGS = ["general", "models", "foreman", "account", "about"]


def flow_settings(f: Flow) -> str:
    """Each settings page at launch (--settings PAGE); Command-comma opens and Command-[ closes"""
    for i, page in enumerate(SETTINGS):
        app = f.launch("--settings", page)
        st = app.wait("settings", lambda s: s["settings_page"] == i, 3)
        masks = [[300, 120, 900, 700]] if page == "about" else None   # the version and update state change every build
        f.check(f"settings_{page}_1200x800", masks=masks)
        if page == "general":
            app.do("key cmd+[")
            app.wait("settings to close", lambda s: s["settings_page"] == -1, 3)
            app.do("key cmd+,")
            app.wait("settings to open", lambda s: s["settings_page"] >= 0, 3)
        app.quit()
        f.app = None
    return "settings pages"


def flow_quit_reopen(f: Flow) -> str:
    """A finished chat survives Command-Q; on reopen it is listed and a click loads it"""
    app = f.launch()
    f.send("Explain the layout")
    app.wait("the reply to end", lambda s: s["running"] == 0 and s["cost_usd_micros"] > 0, 10)
    app.quit()
    f.expect(app.proc.returncode == 0, f"Coder exited with {app.proc.returncode} on Command-Q")
    f.expect(len(f.chats()) == 1, f"{len(f.chats())} chats kept after quit, want 1")
    app = f.relaunch()
    st = app.state()
    f.expect(st["title"] == "", f"reopened on {st['title']!r}, want a new chat")
    f.check("reopen_1200x800")
    press_chat(f)
    st = app.wait("the kept chat to open", lambda s: s["title"] == "Explain the layout", 3)
    f.check("reopened_chat_1200x800")
    return "quit and reopen"


# A first turn's "Worked for Ns" head, at the conversation's left under the ask: its seconds depend
# on the machine.
WORKED_FOR = [400, 122, 90, 28]


def act(reply: str = "", commands: list[str] | None = None) -> dict:
    return {"action": {"rationale": "r", "commands": commands or [], "view": [], "finished": not commands, "reply": reply, "ask": "none"}}


# The real worker (#13718) on these answers, in order, shared by every worker of the flow: turn 1
# runs a command while the person steers and queues, then takes the steer and finishes; the first
# follow-up runs a command and Esc stops it; after a relaunch Resume runs the second. Turn 2's command
# marks that it has started in the flow's record folder (the worker's environment has it).
FU_STARTED = "turn2.started"
FOLLOWUPS = {"real": {"answers": [act(commands=["sleep 4"]), act("Fixed the parser, and the empty string is covered too."),
                                  act(commands=['touch "$CODER_T3_RECORD/%s"; sleep 3' % FU_STARTED]), act("Added the changelog entry.")]}}
FU_ASK, FU_STEER = "Fix the parser", "Also cover the empty string"
FU_QUEUED = ["Then update the docs", "And add a changelog"]
# Turn 1's "Worked for Ns" (its command sleeps 4 s) and turn 2's activity row, which says "Ran 1
# command", or "1 failed" when the command was stopped as it started (a slow machine): masked.
FU_MASK = [WORKED_FOR, [380, 370, 440, 36]]


def flow_followups(f: Flow) -> str:
    """Steer a running reply and queue two follow-ups; Esc stops the first and pauses the queue; after a relaunch the queue is back and Resume runs it"""
    f.workers = FOLLOWUPS
    app = f.launch()
    f.send(FU_ASK)
    app.wait("turn 1 to run", lambda s: s["running"] == 1, 5)
    deadline = time.time() + 5
    while app.do("followups")["turn"] != 1:
        if time.time() > deadline:
            raise FlowFailed("the worker never said turn 1 started")
        time.sleep(0.1)
    # Return steers while a reply runs; Command-Return queues. The host takes each draft on its next
    # frame: the next one is typed once the composer is empty again.
    def taken(what: str) -> None:
        deadline = time.time() + 5
        while app.do("composer")["text"] != "":
            if time.time() > deadline:
                raise FlowFailed(f"the composer kept the draft after {what}")
            time.sleep(0.05)
    f.send(FU_STEER)
    taken("Return")
    for text in FU_QUEUED:
        app.do("type " + text)
        app.do("key cmd+return")
        taken("Command-Return")
    time.sleep(0.3)
    # the worker answers these while turn 1's command runs (#13750)
    fu = app.do("followups")
    f.expect(len(fu["steers"]) == 1, f"steers while turn 1 runs: {fu}")
    # the steer is taken in (applied), turn 1 ends, and the queue's head starts on its own as turn 2
    fu = None
    deadline = time.time() + 20
    while True:
        fu = app.do("followups")
        if fu["turn"] == 2 and app.state()["running"] == 1:
            break
        if time.time() > deadline:
            raise FlowFailed(f"the queue's head never started; {fu}")
        time.sleep(0.1)
    f.expect(fu["steers"] == [3], f"the steer's status after turn 1: {fu['steers']}, want [3] (applied)")
    f.expect(fu["queue"] == FU_QUEUED[1:], f"queue as turn 2 runs: {fu['queue']}")
    # Esc stops the reply while turn 2's command runs: the worker kills it at once (#13750), and the
    # queue pauses with what is left. Esc waits for the command to start (an Esc before the model's
    # recorded answer would stop the turn with no command, and that answer would go to turn 3).
    started = f.case_dir / "rec" / FU_STARTED
    deadline = time.time() + 15
    while not started.exists():
        if time.time() > deadline:
            raise FlowFailed("turn 2's command never started")
        time.sleep(0.05)
    app.do("key escape")
    app.wait("turn 2 to stop", lambda s: s["running"] == 0, 15)
    deadline = time.time() + 5
    while not (fu := app.do("followups"))["paused"]:
        if time.time() > deadline:
            break
        time.sleep(0.1)
    f.expect(fu["paused"] == 1 and fu["reason"] == "interrupted" and fu["queue"] == FU_QUEUED[1:], f"after Esc: {fu}")
    main = f.records("main")
    kinds = [r["kind"] for r in main]
    f.expect(kinds.count("steer") == 1 and kinds.count("interrupt") == 1, f"lines to the worker: {kinds}")
    adds = [r["msg"].get("text") for r in main if r["kind"] == "queue" and r["msg"].get("queue") == "add"]
    f.expect(adds == FU_QUEUED, f"queue adds {adds}")
    f.check("followups_paused_1200x800", masks=FU_MASK)
    app.quit()
    f.app = None
    # the kept chat opens again with no worker on it: its queue is back from the file, paused
    app = f.relaunch()
    press_chat(f)
    app.wait("the kept chat to open", lambda s: s["title"] == FU_ASK, 5)
    time.sleep(0.5)
    fu = app.do("followups")
    f.expect(fu["queue"] == FU_QUEUED[1:] and fu["paused"] == 1 and fu["turn"] == 0, f"restored: {fu}")
    f.check("followups_restored_1200x800", masks=FU_MASK)
    # Resume starts a worker on the chat, which runs the queued message as turn 3
    f.press("coder.fu.resume")
    st = app.wait("the queued message to run and end", lambda s: s["running"] == 0 and app.do("followups")["turn"] >= 1 and not app.do("followups")["queue"], 20)
    fu = app.do("followups")
    f.expect(fu["queue"] == [] and fu["paused"] == 0, f"after Resume: {fu}")
    f.expect(st["title"] == FU_ASK, f"title {st['title']!r}")
    time.sleep(0.5)
    f.check("followups_resumed_1200x800", masks=FU_MASK)
    # the kept chat has the steer in turn 1, and the queued turns
    kept = [json.loads(p.read_text()) for p in f.chats()]
    turns = kept[0]["turns"] if len(kept) == 1 else []
    f.expect([t["ask"] for t in turns] == [FU_ASK, *FU_QUEUED], f"kept turns {[t['ask'] for t in turns]}")
    f.expect(bool(turns) and turns[0].get("steers") == [FU_STEER], f"kept steers {turns[0].get('steers') if turns else None}")
    return "follow-ups"


FILE_SRC = 'main :: () {\n  print("hello from the pane");\n}\n'


def file_state(f: Flow, what: str, test: Callable[[dict], bool], timeout: float = 5.0) -> dict:
    """The file in the pane (the probe's `file`), once test holds."""
    assert f.app is not None
    deadline = time.time() + timeout
    st = f.app.do("file")
    while not test(st):
        if time.time() > deadline:
            raise FlowFailed(f"timed out waiting for {what}; file {json.dumps(st)}")
        time.sleep(0.1)
        st = f.app.do("file")
    return st


def flow_file_pane(f: Flow) -> str:
    """A .coax file opened with Coder shows in the right pane; typing makes it Edited, Command-S saves it, Close closes it"""
    path = f.home / "work" / "hello.coax"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(FILE_SRC, encoding="utf-8")
    app = f.launch()
    app.do("open " + str(path))   # application(_:open:), as the Finder hands a file over (#13759)
    st = file_state(f, "the file to open", lambda s: s["mode"] == 1)
    f.expect(st["name"] == "hello.coax" and st["text"] == FILE_SRC and st["dirty"] == 0, f"opened file {st}")
    f.check("file_pane_1200x800")
    # a click in its text gives it the keyboard; typing goes in at the end
    f.press("coder.file.body")
    app.do("key cmd+down")
    app.do("type // edited")
    st = file_state(f, "the edit", lambda s: s["dirty"] == 1)
    f.expect(st["text"] == FILE_SRC + "// edited", f"edited text {st['text']!r}")
    f.expect(app.do("composer")["text"] == "", "the typing stayed out of the composer")
    f.check("file_pane_edited_1200x800")
    app.do("key cmd+s")
    st = file_state(f, "the save", lambda s: s["dirty"] == 0)
    f.expect(path.read_text(encoding="utf-8") == FILE_SRC + "// edited", f"on disk after Command-S: {path.read_text(encoding='utf-8')!r}")
    f.press("coder.file.close")
    file_state(f, "the pane to close", lambda s: s["mode"] == 0)
    return "file pane"


def flow_worktree_first_message(f: Flow) -> str:
    """The first message of a new chat in a repository (--folder REPO) runs in the chat's own worktree, made before the turn starts"""
    repo = f.home / "work" / "proj"
    repo.mkdir(parents=True)
    git = ["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@example.invalid"]
    subprocess.run(["git", "init", "-q", "-b", "main", str(repo)], check=True)
    (repo / "a.txt").write_text("one\n", encoding="utf-8")
    subprocess.run(git + ["add", "a.txt"], check=True)
    subprocess.run(git + ["commit", "-qm", "start"], check=True)
    app = f.launch("--folder", str(repo))
    f.send("Explain the layout")
    app.wait("the reply to end", lambda s: s["running"] == 0 and s["cost_usd_micros"] > 0, 20)
    tasks = [r for r in f.records("main") if r["kind"] == "task"]
    f.expect(len(tasks) == 1, f"{len(tasks)} task lines reached the worker, want 1")
    seen = [json.loads(p.read_text()) for p in sorted((f.case_dir / "rec").glob("seen-main-*.json"))]
    if len(tasks) != 1 or len(seen) != 1:
        raise FlowFailed(f"tasks {tasks}, seen {seen}")
    cwd = Path(tasks[0]["msg"]["cwd"])
    root = f.home / ".openagents" / "worktrees"
    f.expect(cwd.parent.parent == root and cwd.parent.name.startswith("proj-"), f"the turn ran in {cwd}, not a worktree under {root}")
    # when the task line reached the worker, its folder was a worktree already: a .git file, not a folder
    f.expect(seen[0]["cwd"] == str(cwd) and seen[0]["git"] == "file", f"the worker saw {seen[0]} as the turn started")
    branch = subprocess.run(["git", "-C", str(cwd), "rev-parse", "--abbrev-ref", "HEAD"], capture_output=True, text=True).stdout.strip()
    f.expect(branch == "coder/" + cwd.name, f"the worktree is on {branch!r}, want coder/{cwd.name}")
    main = subprocess.run(["git", "-C", str(repo), "rev-parse", "--abbrev-ref", "HEAD"], capture_output=True, text=True).stdout.strip()
    f.expect(main == "main", f"the main checkout moved to {main!r}")
    # the worker builds Rust into its repository's own Cargo folder, named as its worktrees folder is
    want = f.home / ".cache" / "coder" / "cargo-target" / cwd.parent.name
    f.expect(seen[0]["cargo_target_dir"] == str(want), f"CARGO_TARGET_DIR {seen[0]['cargo_target_dir']!r}, want {want}")
    kept = f.chats()
    f.expect(len(kept) == 1 and kept[0].stem == cwd.name, f"kept chats {[p.stem for p in kept]}, want the worktree's chat {cwd.name}")
    f.notes.append(f"worktree {cwd.relative_to(f.home)}, Cargo {want.relative_to(f.home)}")
    return "worktree before the first message"


EXPERIMENTAL = ("-experimentalFeatures", "YES")


def ple_tool(f: Flow, *args: str) -> tuple[int, str]:
    """A tool from the staged app's Resources (ple-run, foreman-ctl), under the flow's temp HOME"""
    res = f.ctx.binary.parent.parent / "Resources"
    env = dict(os.environ, HOME=str(f.home), CFFIXED_USER_HOME=str(f.home))
    r = subprocess.run([str(res / args[0]), *args[1:]], env=env, capture_output=True, text=True, timeout=60)
    return r.returncode, r.stdout + r.stderr


def ple_feeders(f: Flow) -> str:
    code, text = ple_tool(f, "ple-run", "feeders")
    assert code == 0, text
    return text


def native_views(app: Any, cls: str) -> list[dict]:
    return [v for v in app.do("views")["views"] if v["class"].split(".")[-1] == cls]


def flow_live_card(f: Flow) -> str:
    """A command's live 3D card in a chat: the viewport over the card, scrolling with it, a palette above it"""
    shows = "\n".join(ln for ln in ple_feeders(f).splitlines() if ln.startswith("OA-SHOW"))
    f.notes.append("card lines: " + " | ".join(shows.splitlines()[-4:]))
    script = {"turns": [{"steps": [
        {"emit": {"kind": "command", "command": "ple-run feeders", "exit": 0, "timed_out": False, "output": "Routed 4 feeders\n" + shows}},
        {"emit": {"kind": "reply", "text": "The run is in the card above."}},
        {"emit": {"kind": "ended", "ending": "finished", "text": "The run is in the card above."}}]}]}
    f.workers = {"main": [{"match": "live", "script": script}], "default": reply_script("ok")}
    # a stand-in Gemini key in the temp HOME, so the app starts the (mock) worker; no model is called
    kd = f.home / "Library/Application Support/Coder"; kd.mkdir(parents=True, exist_ok=True)
    (kd / "gemini-key").write_text("t3-stand-in-not-a-key\n"); os.chmod(kd / "gemini-key", 0o600)
    # the message goes with Experimental off (the mock worker's model); the chat is then reopened with it on
    app = f.launch(size=(1200, 900))
    f.send("show me the run live")
    app.wait("the reply to end", lambda s: s["running"] == 0 and s["title"] != "", 15)
    time.sleep(1.0)
    app.capture(f.case_dir / "card_experimental_off.png")
    off = [p for p in native_views(app, "PleScenePanelView") if p["shown"]]
    f.expect(not off, f"no viewport while Experimental is off: {off}")
    app.quit()
    app = f.relaunch(*EXPERIMENTAL, size=(1200, 900))
    time.sleep(1.0)
    press_chat(f)
    time.sleep(2.0)
    card = app.do("box t0.show1")
    panels = native_views(app, "PleScenePanelView"); boxes = native_views(app, "PassThroughView")
    f.notes.append(f"card row {card}; live box {boxes}; panels {panels}")
    shown = [p for p in panels if p["shown"]]
    f.expect(len(boxes) == 1 and boxes[0]["shown"], f"the live card's clip is shown: {boxes}")
    f.expect(len(shown) == 1, f"one viewport shown: {panels}")
    if shown:
        p = shown[0]
        f.expect(abs(p["x"] - card["x"]) <= 1 and abs(p["y"] - card["y"]) <= 1 and abs(p["w"] - card["w"]) <= 1 and abs(p["h"] - card["h"]) <= 1,
                 f"viewport {p} over the card {card}")
        hit = app.do("hit %d %d" % (card["x"] + card["w"] / 2, card["y"] + card["h"] / 2))["class"]
        f.expect(hit.split(".")[-1] in ("PleSceneMetalView", "PleScenePanelView"), f"a press on the card goes to the viewport: {hit}")
    app.capture(f.case_dir / "live_card.png")
    # the window made smaller: the chat keeps its foot in view, so the card moves; the viewport follows
    # (the probe's wheel events don't reach this page, so the move is a resize, not a scroll)
    app.do("size 1000 700")
    time.sleep(1.0)
    card2 = app.do("box t0.show1")
    p2 = [p for p in native_views(app, "PleScenePanelView") if p["shown"]]
    f.expect(abs(card2["y"] - card["y"]) > 20 or abs(card2["x"] - card["x"]) > 20, f"the card moved: {card} -> {card2}")
    f.notes.append(f"after the scroll: card {card2}, viewport {p2}")
    if p2:
        f.expect(abs(p2[0]["y"] - card2["y"]) <= 1 and abs(p2[0]["x"] - card2["x"]) <= 1, f"viewport {p2[0]} follows the card {card2}")
        f.expect(any(b["shown"] and b["y"] <= p2[0]["y"] for b in native_views(app, "PassThroughView")), "the clip still holds it")
    app.capture(f.case_dir / "live_card_small.png")
    # the palette over the card: cut out of the viewport, and a press there is the page's
    app.do("key cmd+k")
    time.sleep(1.2)
    p3 = [p for p in native_views(app, "PleScenePanelView") if p["shown"]]
    f.notes.append(f"palette open: viewport {p3}")
    f.expect(bool(p3) and p3[0]["masked"], f"the viewport is cut while the palette is up: {p3}")
    if p3:
        hit = app.do("hit %d %d" % (p3[0]["x"] + p3[0]["w"] / 2, p3[0]["y"] + 40))["class"]
        f.notes.append(f"a press on the card under the palette goes to {hit}")
        f.expect(hit.split(".")[-1] == "UiView", f"a press in the cut goes to the page: {hit}")
    app.capture(f.case_dir / "live_card_palette.png")
    app.do("key escape")
    time.sleep(1.2)
    p4 = [p for p in native_views(app, "PleScenePanelView") if p["shown"]]
    f.expect(bool(p4) and not p4[0]["masked"], f"no cut once it closed: {p4}")
    # past 4096 device pixels the frame is flat: the Before card's picture must still show (#13817)
    app.do("size 2150 800")
    time.sleep(2.0)
    w = app.do("window"); f.notes.append(f"wide window {w['w']}x{w['h']} points at scale {w['scale']}")
    app.capture(f.case_dir / "flat_wide.png")
    return "live card"


def flow_gym_route(f: Flow) -> str:
    """The Gym route in the app: ple-run gym and foreman-ctl gym steer the Workbench"""
    ple_feeders(f)
    app = f.launch(*EXPERIMENTAL, size=(1200, 900))
    time.sleep(1.0)
    code, text = ple_tool(f, "ple-run", "gym", "latest", "--camera", "plan", "--only", "racks,pdu", "--step", "last", "--wait", "15")
    f.notes.append(f"ple-run gym -> {code}: {text.strip()}")
    f.expect(code == 0 and text.startswith("The Gym shows "), f"ple-run gym answered: {text}")
    time.sleep(1.0)
    gyms = native_views(app, "GymWorkbenchView")
    f.notes.append(f"gym views {gyms}")
    f.expect(any(g["shown"] for g in gyms), f"the Gym is on screen: {gyms}")
    app.capture(f.case_dir / "gym_plan.png")
    code, text = ple_tool(f, "foreman-ctl", "gym")
    f.notes.append(f"foreman-ctl gym -> {code}: {text.strip()[:600]}")
    f.expect(code == 0 and '"ok":true' in text.replace(" ", ""), f"foreman-ctl gym: {text}")
    code, text = ple_tool(f, "foreman-ctl", "gym", "--camera", "iso", "--hide", "racks")
    f.notes.append(f"foreman-ctl gym --camera iso --hide racks -> {code}: {text.strip()[:600]}")
    f.expect(code == 0 and '"ok":true' in text.replace(" ", ""), f"foreman-ctl gym post: {text}")
    code, text = ple_tool(f, "ple-run", "gym", "no-such-run", "--wait", "10")
    f.notes.append(f"ple-run gym no-such-run -> {code}: {text.strip()}")
    f.expect(code == 1 and "refused" in text, f"an unknown run is refused: {text}")
    time.sleep(0.8)
    app.capture(f.case_dir / "gym_iso.png")
    return "gym"


FLOWS: list[tuple[str, Callable[[Flow], str]]] = [
    ("idle_layouts", flow_idle_layouts),
    ("send_stream_new_chat", flow_send_stream),
    ("stop", flow_stop),
    ("subagent_pane", flow_subagent_pane),
    ("sidebar_resize", flow_sidebar_resize),
    ("settings", flow_settings),
    ("quit_reopen", flow_quit_reopen),
    ("followups", flow_followups),
    ("file_pane", flow_file_pane),
    ("worktree_first_message", flow_worktree_first_message),
    ("live_card", flow_live_card),
    ("gym_route", flow_gym_route),
]


class Ctx:
    def __init__(self, run_dir: Path, binary: Path, dylib: Path, update: bool, tol: int, max_diff: float, footprint_limit_mb: float = 300.0) -> None:
        self.run_dir, self.binary, self.dylib, self.update, self.tol, self.max_diff = run_dir, binary, dylib, update, tol, max_diff
        self.footprint_limit_mb = footprint_limit_mb


def run_flow(ctx: Ctx, name: str, fn: Callable[[Flow], str]) -> dict[str, Any]:
    f = Flow(ctx, name)
    t0 = time.time()
    error = None
    try:
        fn(f)
    except FlowFailed as e:
        error = str(e)
    except Exception as e:  # noqa: BLE001 - a flow's crash is its failure, the run goes on
        error = f"{type(e).__name__}: {e}"
    finally:
        if f.app is not None:
            f.app.quit()
        f.stop_daemon()
        forget_prefs()
        shutil.rmtree(f.home, ignore_errors=True)
    problems = f.problems + ([error] if error else [])
    return {"flow": name, "description": (fn.__doc__ or "").strip(), "passed": not problems, "problems": problems,
            "wall_ms": int((time.time() - t0) * 1000), "captures": f.captures, "notes": f.notes}


def main() -> int:
    ap = argparse.ArgumentParser(description="Tier 3: drive the real Coder.app and compare window captures.")
    ap.add_argument("--filter", default="", help="run flows whose name contains one of these (comma-separated)")
    ap.add_argument("--no-build", action="store_true", help="reuse results/tier3-build/Coder.app")
    ap.add_argument("--app", default="", help="use this built Coder.app (it is copied, never changed)")
    ap.add_argument("--update-goldens", action="store_true", help="write the captures to tier3/goldens/")
    ap.add_argument("--pixel-tolerance", type=int, default=24, help="a channel may move this much (0-255)")
    ap.add_argument("--max-diff", type=float, default=0.002, help="fraction of pixels that may differ")
    ap.add_argument("--footprint-limit-mb", type=float, default=300.0, help="how much Coder's footprint may grow over the streaming flow")
    args = ap.parse_args()
    # a SIGTERM to this runner still runs each flow's cleanup (quit, stop the daemon), as Control-C does
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    if sys.platform != "darwin":
        print("Tier 3 runs on macOS only")
        return 2

    sha = subprocess.run(["git", "-C", str(REPO), "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip() or "nogit"
    run_dir = RESULTS / f"tier3-{time.strftime('%Y%m%dT%H%M%S')}-{sha}"
    run_dir.mkdir(parents=True, exist_ok=True)
    if args.app:
        built = Path(args.app)
    elif args.no_build and (RESULTS / "tier3-build/OpenAgents.app").exists():
        built = RESULTS / "tier3-build/OpenAgents.app"
    else:
        built = build_app(RESULTS / "tier3-build")
    binary, dylib = stage_app(built, run_dir)
    ctx = Ctx(run_dir, binary, dylib, args.update_goldens, args.pixel_tolerance, args.max_diff, args.footprint_limit_mb)

    wanted = [w for w in args.filter.split(",") if w]
    results = []
    for name, fn in FLOWS:
        if wanted and not any(w in name for w in wanted):
            continue
        r = run_flow(ctx, name, fn)
        results.append(r)
        tag = "ok  " if r["passed"] else "FAIL"
        tail = "" if r["passed"] else " -- " + " | ".join(r["problems"])
        print(f"{tag} flow {name}: {r['description']} ({r['wall_ms']} ms, {r['captures']} captures){tail}", flush=True)
        for note in r["notes"]:
            print(f"     {note}", flush=True)
    shutil.rmtree(run_dir / "OpenAgents.app", ignore_errors=True)
    passed = sum(1 for r in results if r["passed"])
    summary = {"run": run_dir.name, "update_goldens": args.update_goldens, "passed": passed, "failed": len(results) - passed, "flows": results}
    (run_dir / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(f"{len(results)} checks, {passed} passed, {len(results) - passed} failed")
    print(f"artifacts: {run_dir}")
    return 0 if passed == len(results) else 1


if __name__ == "__main__":
    sys.exit(main())
