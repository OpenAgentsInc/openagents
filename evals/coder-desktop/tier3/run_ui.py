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
   first), stops the run daemon Coder started in the temp HOME and any mock worker left, by their
   own PIDs, and deletes the copy's preference domain and the temp HOME.

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
BUNDLE_ID = "com.openagents.coder.evals-t3"


class FlowFailed(Exception):
    pass


# ---- the app --------------------------------------------------------------------------------

def build_app(build_dir: Path) -> Path:
    env = dict(os.environ, CODER_BUILD=str(build_dir))
    res = subprocess.run(["bash", str(REPO / "apps/coder-desktop/build.sh")], cwd=REPO, env=env, capture_output=True, text=True)
    if res.returncode != 0:
        sys.stderr.write(res.stdout[-4000:] + res.stderr[-4000:])
        raise SystemExit(f"build.sh failed with exit {res.returncode}")
    return build_dir / "Coder.app"


def stage_app(built: Path, run_dir: Path) -> tuple[Path, Path]:
    """The copy the flows run: own bundle id, mock workers, re-signed. Returns (binary, probe dylib)."""
    app = run_dir / "Coder.app"
    shutil.copytree(built, app, symlinks=True)
    plist = app / "Contents/Info.plist"
    info = plistlib.loads(plist.read_bytes())
    info["CFBundleIdentifier"] = BUNDLE_ID
    plist.write_bytes(plistlib.dumps(info))
    for name in ("mcworker", "claudeworker"):
        w = app / "Contents/Resources" / name
        w.write_text(f'#!/bin/sh\nexec "{sys.executable}" -I "{HERE / "mock_router.py"}" "$@"\n', encoding="utf-8")
        w.chmod(0o755)
    subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True, capture_output=True)
    dylib = run_dir / "probe.dylib"
    subprocess.run(["clang", "-dynamiclib", "-fobjc-arc", "-mmacosx-version-min=14.0", "-framework", "AppKit",
                    "-o", str(dylib), str(HERE / "probe/probe.m")], check=True)
    return app / "Contents/MacOS/Coder", dylib


def forget_prefs() -> None:
    subprocess.run(["defaults", "delete", BUNDLE_ID], capture_output=True)


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
            "CODER_HOME": str(home / ".openagents/coder/host"), "CODER_RELEASE_HOME": str(home / ".cache/coder-release"),
            "CODER_UPDATE_URL": "", "BROWSER": "/usr/bin/true", "LANG": "en_US.UTF-8",
            "CODER_T3_SOCKET": str(self.sock_path), "CODER_T3_WORKERS": str(wfile), "CODER_T3_RECORD": str(rec),
            "DYLD_INSERT_LIBRARIES": str(dylib),
        }
        if self.sock_path.exists():
            self.sock_path.unlink()
        self.log = open(case_dir / "app.log", "ab")
        self.proc = subprocess.Popen([str(binary), *args], env=env, stdout=self.log, stderr=self.log, stdin=subprocess.DEVNULL)
        self.conn: socket.socket | None = None
        self.reader = None
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
    {"emit": {"kind": "ended", "ending": "finished", "text": "Module A ok"}}]}]}
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
        self.captures = 0
        self.problems: list[str] = []

    def launch(self, *args: str, size: tuple[int, int] = (1200, 800)) -> App:
        forget_prefs()
        self.app = App(self.ctx.binary, self.ctx.dylib, self.home, self.case_dir, WORKERS, list(args))
        self.app.do(f"size {size[0]} {size[1]}")
        time.sleep(0.4)
        return self.app

    def relaunch(self, *args: str, size: tuple[int, int] = (1200, 800)) -> App:
        """A second launch on the same HOME (the chats kept), preferences kept."""
        self.app = App(self.ctx.binary, self.ctx.dylib, self.home, self.case_dir, WORKERS, list(args))
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
        daemon's own daemon.json, the workers' record file names)."""
        pids = []
        disc = self.home / ".openagents/coder/host/daemon.json"
        try:
            pid = int(json.loads(disc.read_text()).get("pid") or 0)
            if pid and str(self.ctx.run_dir) in subprocess.run(["ps", "-o", "command=", "-p", str(pid)], capture_output=True, text=True).stdout:
                pids.append(pid)
        except (OSError, ValueError):
            pass
        for rec in (self.case_dir / "rec").glob("*-*.jsonl"):
            pid = int(rec.stem.rsplit("-", 1)[1])
            if "mock_router.py" in subprocess.run(["ps", "-o", "command=", "-p", str(pid)], capture_output=True, text=True).stdout:
                pids.append(pid)
        for pid in pids:
            try:
                os.kill(pid, signal.SIGTERM)
            except OSError:
                pass

    def expect(self, ok: bool, why: str) -> None:
        if not ok:
            self.problems.append(why)

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


# Points in a 1200 by 800 window's content (top-left), from the goldens: the sidebar's New chat
# row, its right edge, the first Recents row and the composer's send / stop button.
NEW_CHAT = (70, 26)
SIDE_EDGE_X = 281
FIRST_CHAT = (100, 126)


def send_button(w: int, h: int) -> tuple[int, int]:
    return (w - 78, h - 36)


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
    f.send("Explain the layout")
    app.wait("the reply to start", lambda s: s["running"] == 1 or s["cost_usd_micros"] > 0, 5)
    st = app.wait("the reply to end", lambda s: s["running"] == 0 and s["cost_usd_micros"] > 0, 10)
    f.expect(st["title"] == "Explain the layout", f"chat title {st['title']!r}")
    f.expect(st["cost_usd_micros"] == 15000, f"cost {st['cost_usd_micros']}")
    tasks = [r for r in f.records("main") if r["kind"] == "task"]
    f.expect(len(tasks) == 1 and tasks[0]["msg"]["task"] == "Explain the layout", f"worker tasks {tasks}")
    f.check("replied_1200x800")
    f.expect(len(f.chats()) == 1, f"{len(f.chats())} chats kept, want 1")
    app.do("click %d %d" % NEW_CHAT)
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
    # the sidebar row's spinner and the transcript's typing dots move
    f.check("streaming_1200x800", masks=[[232, 138, 44, 40], [322, 228, 56, 28]])
    app.do("click %d %d" % send_button(1200, 800))
    app.wait("the reply to stop", lambda s: s["running"] == 0, 5)
    stops = [r for r in f.records("main") if r["kind"] == "stop"]
    f.expect(len(stops) == 1, f"{len(stops)} stop lines reached the worker, want 1")
    f.check("stopped_1200x800")
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
    app.do(f"drag {SIDE_EDGE_X} 400 {SIDE_EDGE_X + 90} 400 15")
    st = app.wait("the width to be kept", lambda s: s["side_width"] > 0, 3)
    # kept in the page's units: points / 1.2 at the usual text size, so 371 points is about 309
    f.expect(before == 0 and 295 <= st["side_width"] <= 325, f"side width {before} -> {st['side_width']}, want about 309")
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
    app.do("click %d %d" % FIRST_CHAT)
    st = app.wait("the kept chat to open", lambda s: s["title"] == "Explain the layout", 3)
    f.check("reopened_chat_1200x800")
    return "quit and reopen"


FLOWS: list[tuple[str, Callable[[Flow], str]]] = [
    ("idle_layouts", flow_idle_layouts),
    ("send_stream_new_chat", flow_send_stream),
    ("stop", flow_stop),
    ("subagent_pane", flow_subagent_pane),
    ("sidebar_resize", flow_sidebar_resize),
    ("settings", flow_settings),
    ("quit_reopen", flow_quit_reopen),
]


class Ctx:
    def __init__(self, run_dir: Path, binary: Path, dylib: Path, update: bool, tol: int, max_diff: float) -> None:
        self.run_dir, self.binary, self.dylib, self.update, self.tol, self.max_diff = run_dir, binary, dylib, update, tol, max_diff


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
            "wall_ms": int((time.time() - t0) * 1000), "captures": f.captures}


def main() -> int:
    ap = argparse.ArgumentParser(description="Tier 3: drive the real Coder.app and compare window captures.")
    ap.add_argument("--filter", default="", help="run flows whose name contains one of these (comma-separated)")
    ap.add_argument("--no-build", action="store_true", help="reuse results/tier3-build/Coder.app")
    ap.add_argument("--app", default="", help="use this built Coder.app (it is copied, never changed)")
    ap.add_argument("--update-goldens", action="store_true", help="write the captures to tier3/goldens/")
    ap.add_argument("--pixel-tolerance", type=int, default=24, help="a channel may move this much (0-255)")
    ap.add_argument("--max-diff", type=float, default=0.002, help="fraction of pixels that may differ")
    args = ap.parse_args()
    if sys.platform != "darwin":
        print("Tier 3 runs on macOS only")
        return 2

    sha = subprocess.run(["git", "-C", str(REPO), "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip() or "nogit"
    run_dir = RESULTS / f"tier3-{time.strftime('%Y%m%dT%H%M%S')}-{sha}"
    run_dir.mkdir(parents=True, exist_ok=True)
    if args.app:
        built = Path(args.app)
    elif args.no_build and (RESULTS / "tier3-build/Coder.app").exists():
        built = RESULTS / "tier3-build/Coder.app"
    else:
        built = build_app(RESULTS / "tier3-build")
    binary, dylib = stage_app(built, run_dir)
    ctx = Ctx(run_dir, binary, dylib, args.update_goldens, args.pixel_tolerance, args.max_diff)

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
    shutil.rmtree(run_dir / "Coder.app", ignore_errors=True)
    passed = sum(1 for r in results if r["passed"])
    summary = {"run": run_dir.name, "update_goldens": args.update_goldens, "passed": passed, "failed": len(results) - passed, "flows": results}
    (run_dir / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(f"{len(results)} checks, {passed} passed, {len(results) - passed} failed")
    print(f"artifacts: {run_dir}")
    return 0 if passed == len(results) else 1


if __name__ == "__main__":
    sys.exit(main())
