#!/usr/bin/env python3
# Firm reason: 2 (an independent oracle: checks the Coax worker's and page's follow-up lines from outside Coax, as the other Tier 1 drivers do)
"""Follow-ups on the real worker (GitHub #13715, #13716): steer, queue and interrupt.

Builds coder/tui/mcworker.coax (as apps/coder-desktop/build.sh does), then runs it in a temp
HOME with GW_REPLAY: each model call is answered by a generated recorded response, so nothing
calls a model and nothing touches real Coder data. Every case drives the worker's own lines
(coax/lib/agent/engine/mcworker-wire.coax) and checks what it says back:

  steer_applied        a steer sent mid-turn is accepted, then applied as a step; the turn goes on
  steer_rejections     no_active_turn, expected_turn_mismatch, active_turn_not_steerable
  queue_fifo           queued messages start in order when the turn ends; ops; idempotent ids
  error_pauses         a failed turn pauses the queue instead of starting the next; resume starts it
  interrupt_resubmit   interrupt with resubmit sends the pending steers as a new turn
  interrupt_pauses     interrupt without resubmit pauses the queue; pending steers head it
  promote_now          promote with now stops the turn and starts that message next
  restart_restores     a killed worker's queue comes back from its file in a new worker, paused,
                       with nothing lost and nothing run twice
  approvals_separate   a command's approval and a queued follow-up do not mix: queue lines and a
                       steer during an ask leave it waiting; the answer goes on; then the queue
  during_command       a steer, a queue op and an interrupt while a 30 s command runs are answered
                       at once (GitHub #13750): the interrupt kills the command's process group and
                       ends the turn in under a second, and the same for an approved command
  page_host            the same through the desktop's real host.c and page (host_driver): the
                       page's steer statuses, the queue it shows, and a turn the worker starts

Usage: python3 evals/coder-desktop/followups/run_followups.py [--worker BIN] [--filter NAME]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]


def build_worker() -> Path:
    coax = REPO / "coax/bin/coax"
    digest = hashlib.sha256(coax.read_bytes()).hexdigest()
    out = REPO / f"coax/web/build/tools/mcworker-{digest}"
    with tempfile.TemporaryDirectory(prefix="coder-fu-build-") as th:
        env = dict(os.environ, COAX=str(coax), BROWSER="/usr/bin/true", HOME=th)
        res = subprocess.run([str(REPO / "coax/web/tool.sh"), str(REPO / "coder/tui/mcworker.coax")], cwd=str(REPO), env=env,
                             stdin=subprocess.DEVNULL, capture_output=True, text=True, check=False, timeout=900)
    if not out.exists():
        sys.stderr.write(res.stdout[-4000:] + res.stderr[-4000:])
        raise SystemExit("building mcworker failed")
    return out


def act(reply: str = "", commands: list[str] | None = None, finished: bool = True) -> dict:
    return {"rationale": "r", "commands": commands or [], "view": [], "finished": finished, "reply": reply, "ask": "none"}


def http(action: dict, think: str = "") -> str:
    """A recorded streamed answer; think, when given, comes first as the step's reasoning."""
    body = json.dumps({"choices": [{"delta": {"content": json.dumps(action), "role": "assistant"}, "index": 0}]})
    end = json.dumps({"choices": [{"delta": {"content": ""}, "finish_reason": "stop", "index": 0}]})
    first = "data: " + json.dumps({"choices": [{"delta": {"reasoning": think, "role": "assistant"}, "index": 0}]}) + "\n\n" if think else ""
    return "HTTP/2 200 \ncontent-type: text/event-stream\n\n" + first + "data: " + body + "\n\ndata: " + end + "\n\ndata: [DONE]\n\n"


class Box:
    """A temp HOME, a work folder and a replay folder of model answers, in that order."""

    def __init__(self, answers: list[dict]):
        self.root = Path(tempfile.mkdtemp(prefix="coder-fu-"))
        self.home = self.root / "home"; self.work = self.root / "work"; self.replay = self.root / "replay"
        for d in (self.home, self.work, self.replay):
            d.mkdir()
        for i, a in enumerate(answers):
            (self.replay / f"{i + 1}.http").write_text(http(a))

    def task(self, text: str, **more) -> dict:
        t = {"task": text, "cwd": str(self.work), "sandbox": "danger-full-access", "session": "fu"}
        t.update(more)
        return t

    def queue_file(self, key: str) -> dict:
        return json.loads((self.home / f".openagents/coder/sessions/{key}.queue.json").read_text())

    def close(self) -> None:
        shutil.rmtree(self.root, ignore_errors=True)


class Worker:
    def __init__(self, bin_: Path, box: Box):
        env = dict(os.environ, HOME=str(box.home), GW_REPLAY=str(box.replay), BROWSER="/usr/bin/true")
        env.pop("CODER_CLAUDE_KEY", None)
        self.p = subprocess.Popen([str(bin_)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                  env=env, cwd=str(box.work), text=True, bufsize=1)
        self.lines: list[dict] = []
        self.pending: list[dict] = []   # read but not yet looked at
        self.wire: list[tuple[str, str]] = []   # every line each way, as sent: ("owner"|"worker", line); capture_wire.py keeps them
        self.held = b""
        self.wait(lambda l: l.get("kind") == "ready", 20)

    def send(self, obj: dict) -> None:
        assert self.p.stdin
        line = json.dumps(obj, separators=(",", ":"), ensure_ascii=False)
        self.wire.append(("owner", line))
        self.p.stdin.write(line + "\n"); self.p.stdin.flush()

    def wait(self, pred, timeout: float = 20.0) -> dict:
        """The first line from here on for which pred holds (lines read on the way are kept). It reads
        the pipe's bytes itself: a buffered reader would hold lines that select() then never reports."""
        assert self.p.stdout
        fd = self.p.stdout.fileno()
        end = time.time() + timeout
        while True:
            while self.pending:
                l = self.pending.pop(0)
                self.lines.append(l)
                if pred(l):
                    return l
            if time.time() >= end:
                break
            r, _, _ = select.select([fd], [], [], 0.1)
            if not r:
                continue
            chunk = os.read(fd, 65536)
            if not chunk:
                break
            self.held += chunk
            while b"\n" in self.held:
                raw, self.held = self.held.split(b"\n", 1)
                self.wire.append(("worker", raw.decode("utf-8", "replace")))
                try:
                    self.pending.append(json.loads(raw))
                except ValueError:
                    pass
        raise AssertionError("timed out waiting; last lines:\n" + "\n".join(json.dumps(x)[:240] for x in self.lines[-12:]))

    def kinds(self, kind: str) -> list[dict]:
        return [l for l in self.lines if l.get("kind") == kind]

    def kill(self) -> None:
        self.p.send_signal(signal.SIGKILL); self.p.wait(5)

    def close(self) -> None:
        if self.p.poll() is None:
            assert self.p.stdin
            self.p.stdin.close()
            try:
                self.p.wait(5)
            except subprocess.TimeoutExpired:
                self.kill()


def kind(k: str, **fields):
    return lambda l: l.get("kind") == k and all(l.get(a) == b for a, b in fields.items())


def ended():
    return lambda l: l.get("kind") == "ended"


# ---------------------------------------------------------------- cases

def steer_applied(w_bin: Path) -> None:
    # the steer arrives while the step's command runs, so the next model call already has it (#13750)
    box = Box([act(commands=["sleep 1"], finished=False), act("took the steer")])
    w = Worker(w_bin, box)
    try:
        w.send(box.task("do a"))
        t = w.wait(kind("turn"))
        assert t["source"] == "message" and t["turn"] == 1, t
        w.wait(kind("attempt"))
        w.send({"steer": "also b", "id": "st1", "turn": 1})
        w.wait(kind("steer", id="st1", status="accepted"))
        a = w.wait(kind("steer", id="st1", status="applied"))
        assert a["step"] > 0, a
        e = w.wait(ended())
        assert e["ending"] == "finished" and e["text"] == "took the steer", e
        # the same id again after it was taken in changes nothing: there is no turn now
        w.send({"steer": "also b", "id": "st1"})
        r = w.wait(kind("steer", id="st1"))
        assert r["status"] == "rejected" and r["reason"] == "no_active_turn", r
    finally:
        w.close(); box.close()


def steer_rejections(w_bin: Path) -> None:
    box = Box([act(commands=["sleep 1"], finished=False), act("a done"), act(commands=["sleep 1"], finished=False), act("b done")])
    w = Worker(w_bin, box)
    try:
        w.send({"steer": "x", "id": "n1"})
        assert w.wait(kind("steer", id="n1"))["reason"] == "no_active_turn"
        w.send(box.task("do a"))
        w.wait(kind("turn"))
        w.send({"steer": "x", "id": "m1", "turn": 7})
        r = w.wait(kind("steer", id="m1"))
        assert r["status"] == "rejected" and r["reason"] == "expected_turn_mismatch", r
        w.send({"steer": "", "id": "e1", "turn": 1})
        assert w.wait(kind("steer", id="e1"))["reason"] == "invalid"
        w.wait(ended())
        w.send(box.task("do b", steerable=False))
        w.wait(kind("turn", turn=2))
        w.send({"steer": "x", "id": "s1", "turn": 2})
        r = w.wait(kind("steer", id="s1"))
        assert r["status"] == "rejected" and r["reason"] == "active_turn_not_steerable", r
        w.wait(ended())
    finally:
        w.close(); box.close()


def queue_fifo(w_bin: Path) -> None:
    box = Box([act(commands=["sleep 1"], finished=False), act("a done"), act("b done"), act("c done")])
    w = Worker(w_bin, box)
    try:
        w.send({"queue": "open", "key": "chat-fifo"})
        w.wait(kind("queue"))
        w.send(box.task("do a"))
        w.wait(kind("turn"))
        for qid, text in (("q1", "do b"), ("q2", "do x"), ("q3", "do c")):
            w.send({"queue": "add", "id": qid, "text": text, "message": box.task(text)})
        w.send({"queue": "add", "id": "q1", "text": "do b", "message": box.task("do b")})   # the same id again: once
        w.send({"queue": "delete", "id": "q2"})
        w.send({"queue": "delete", "id": "nope"})
        assert w.wait(kind("queue_refused", op="delete"))["reason"] == "unknown_id"
        w.send({"queue": "update", "id": "q3", "text": "do c", "message": box.task("do c")})
        snap = w.wait(lambda l: l.get("kind") == "queue" and [i["id"] for i in l["items"]] == ["q1", "q3"])
        assert not snap["paused"], snap
        assert box.queue_file("chat-fifo")["items"][0]["id"] == "q1"   # kept as it changes
        turns = []
        while len(turns) < 2:
            turns.append(w.wait(lambda l: l.get("kind") == "turn" and l.get("source") == "queue"))
            w.wait(ended())
        assert [t["id"] for t in turns] == ["q1", "q3"] and turns[0]["text"] == "do b", turns
        assert [e["text"] for e in w.kinds("ended")] == ["a done", "b done", "c done"], w.kinds("ended")
        # a dispatched id is not queued again (a host that writes its add twice)
        w.send({"queue": "add", "id": "q1", "text": "do b", "message": box.task("do b")})
        assert w.wait(kind("queue_refused", op="add"))["reason"] == "dispatched"
        f = box.queue_file("chat-fifo")
        assert f["items"] == [] and f["active"] == "" and "q1" in f["done"] and "q3" in f["done"], f
    finally:
        w.close(); box.close()


def error_pauses(w_bin: Path) -> None:
    # the first turn's second model call is refused (400, not retried): the turn fails
    box = Box([act(commands=["sleep 1"], finished=False)])
    (box.replay / "2.http").write_text("HTTP/2 400 \ncontent-type: application/json\n\n{\"error\":{\"code\":400,\"message\":\"bad request\"}}\n")
    w = Worker(w_bin, box)
    try:
        w.send({"queue": "open", "key": "chat-err"})
        w.send(box.task("do a"))
        w.wait(kind("turn"))
        w.send({"queue": "add", "id": "q1", "text": "do b", "message": box.task("do b")})
        e = w.wait(ended(), 120)
        assert e["ending"] != "finished", e
        snap = w.wait(lambda l: l.get("kind") == "queue" and l.get("paused"))
        assert snap["reason"] == "error" and [i["id"] for i in snap["items"]] == ["q1"], snap
        time.sleep(0.5)
        assert len(w.kinds("turn")) == 1, w.kinds("turn")       # the next one did not start
        assert box.queue_file("chat-err")["paused"]
        # a recorded answer now, and resume: the head starts
        nxt = int((box.replay / ".next").read_text().strip())
        for k in range(nxt, nxt + 3):
            (box.replay / f"{k}.http").write_text(http(act("b done")))
        w.send({"queue": "resume"})
        t = w.wait(lambda l: l.get("kind") == "turn" and l.get("source") == "queue")
        assert t["id"] == "q1", t
        assert w.wait(ended(), 60)["text"] == "b done"
    finally:
        w.close(); box.close()


def interrupt_resubmit(w_bin: Path) -> None:
    box = Box([act(commands=["sleep 3"], finished=False), act("answered the steer")])
    w = Worker(w_bin, box)
    try:
        w.send(box.task("do a"))
        w.wait(kind("turn"))
        w.wait(kind("attempt"))
        w.send({"steer": "actually do b", "id": "st1", "turn": 1})
        w.send({"interrupt": True, "resubmit": True})
        e = w.wait(ended())
        assert e["ending"] == "stopped", e
        w.wait(kind("steer", id="st1", status="resubmitted"))
        t = w.wait(kind("turn", source="resubmit"))
        assert t["text"] == "actually do b" and t["turn"] == 2, t
        assert w.wait(ended(), 60)["text"] == "answered the steer"
    finally:
        w.close(); box.close()


def interrupt_pauses(w_bin: Path) -> None:
    box = Box([act(commands=["sleep 3"], finished=False)])
    w = Worker(w_bin, box)
    try:
        w.send({"queue": "open", "key": "chat-int"})
        w.send(box.task("do a"))
        w.wait(kind("turn"))
        w.send({"queue": "add", "id": "q1", "text": "do c", "message": box.task("do c")})
        w.wait(kind("attempt"))
        w.send({"steer": "and b", "id": "st1", "turn": 1})
        w.wait(kind("steer", id="st1", status="accepted"))
        w.send({"interrupt": True})
        assert w.wait(ended())["ending"] == "stopped"
        w.wait(kind("steer", id="st1", status="queued"))
        snap = w.wait(lambda l: l.get("kind") == "queue" and l.get("paused"))
        assert snap["reason"] == "interrupted" and [i["id"] for i in snap["items"]] == ["st1", "q1"], snap
        time.sleep(0.5)
        assert len(w.kinds("turn")) == 1
    finally:
        w.close(); box.close()


def promote_now(w_bin: Path) -> None:
    box = Box([act(commands=["sleep 3"], finished=False), act("c first")])
    w = Worker(w_bin, box)
    try:
        w.send(box.task("do a"))
        w.wait(kind("turn"))
        w.send({"queue": "add", "id": "q1", "text": "do b", "message": box.task("do b")})
        w.send({"queue": "add", "id": "q2", "text": "do c", "message": box.task("do c")})
        w.wait(kind("attempt"))
        w.send({"queue": "promote", "id": "q2", "now": True})
        assert w.wait(ended())["ending"] == "stopped"
        t = w.wait(kind("turn", source="queue"))
        assert t["id"] == "q2", t
        assert w.wait(ended(), 60)["text"] == "c first"
        snap = w.wait(lambda l: l.get("kind") == "queue" and l.get("active") == "")
        # an interrupt by promote does not pause the queue: q1 starts after
        assert [i["id"] for i in snap["items"]] in (["q1"], []), snap
    finally:
        w.close(); box.close()


def restart_restores(w_bin: Path) -> None:
    box = Box([act(commands=["sleep 5"], finished=False)])
    w = Worker(w_bin, box)
    try:
        w.send({"queue": "open", "key": "chat-rs"})
        w.send(box.task("do a"))
        w.wait(kind("turn"))
        w.send({"queue": "add", "id": "q1", "text": "do b", "message": box.task("do b")})
        w.send({"queue": "add", "id": "q2", "text": "do c", "message": box.task("do c")})
        w.wait(lambda l: l.get("kind") == "queue" and len(l["items"]) == 2)
        w.kill()
    finally:
        w.close()
    # a worker that went away mid-turn: the next one reads the file, paused, and starts nothing
    w2 = Worker(w_bin, box)
    try:
        w2.send({"queue": "open", "key": "chat-rs"})
        snap = w2.wait(kind("queue"))
        assert snap["paused"] and snap["reason"] == "restored" and [i["id"] for i in snap["items"]] == ["q1", "q2"], snap
        time.sleep(0.5)
        assert len(w2.kinds("turn")) == 0, w2.kinds("turn")
        # one whose head was running when it went away: that head is not run twice
        w2.send({"queue": "open", "key": "chat-other"})     # saves chat-rs as it is, then lets go of it
        w2.wait(lambda l: l.get("kind") == "queue" and not l["items"])
        f = box.queue_file("chat-rs"); f["active"] = "q0"; f["done"] = ["q0"]; f["paused"] = False
        (box.home / ".openagents/coder/sessions/chat-rs.queue.json").write_text(json.dumps(f))
        w2.send({"queue": "open", "key": "chat-rs"})
        snap = w2.wait(lambda l: l.get("kind") == "queue" and l.get("paused") and len(l["items"]) == 2)
        assert snap["reason"] == "restored", snap
        w2.send({"queue": "add", "id": "q0", "text": "again", "message": box.task("again")})
        assert w2.wait(kind("queue_refused", op="add"))["reason"] == "dispatched"
        # resume: the head starts, once
        nxt = int((box.replay / ".next").read_text().strip()) if (box.replay / ".next").exists() else 1
        for k in range(nxt, nxt + 3):
            (box.replay / f"{k}.http").write_text(http(act("b done")))
        w2.send({"queue": "resume"})
        t = w2.wait(kind("turn", source="queue"))
        assert t["id"] == "q1", t
        assert w2.wait(ended(), 60)["text"] == "b done"
        assert box.queue_file("chat-rs")["done"].count("q1") == 1
    finally:
        w2.close(); box.close()


def approvals_separate(w_bin: Path) -> None:
    box = Box([act(commands=["echo approved-ran"], finished=False), act("ran it"), act("queued done")])
    w = Worker(w_bin, box)
    try:
        w.send({"queue": "open", "key": "chat-ask"})
        w.send(box.task("run it", gated=True))
        ask = w.wait(kind("ask"))
        # queue lines and a steer while the command waits for its answer: none of them answers it
        w.send({"queue": "add", "id": "q1", "text": "do b", "message": box.task("do b")})
        w.send({"queue": "list"})
        w.send({"steer": "and then say so", "id": "st1", "turn": 1})
        w.wait(kind("steer", id="st1", status="accepted"))
        time.sleep(0.5)
        assert not w.kinds("command") and not w.kinds("ended") and len(w.kinds("turn")) == 1, w.lines[-6:]
        w.send({"answer": True, "ticket": ask["ticket"]})
        c = w.wait(kind("command"))
        assert "approved-ran" in c["output"], c
        w.wait(kind("steer", id="st1", status="applied"))
        assert w.wait(ended())["text"] == "ran it"
        t = w.wait(kind("turn", source="queue"))
        assert t["id"] == "q1", t
        assert w.wait(ended())["text"] == "queued done"
        # an answer with no ask waiting starts nothing and is not queued
        w.send({"answer": True, "ticket": ask["ticket"]})
        w.send({"queue": "list"})
        snap = w.wait(kind("queue"))
        assert snap["items"] == [] and len(w.kinds("turn")) == 2, snap
    finally:
        w.close(); box.close()


def _group_gone(pid: int, timeout: float = 2.0) -> bool:
    end = time.time() + timeout
    while time.time() < end:
        try:
            os.killpg(pid, 0)
        except ProcessLookupError:
            return True
        except PermissionError:
            pass
        time.sleep(0.05)
    return False


def _wait_file(path: Path, timeout: float = 10.0) -> str:
    end = time.time() + timeout
    while time.time() < end:
        if path.exists() and path.read_text().strip():
            return path.read_text().strip()
        time.sleep(0.02)
    raise AssertionError(f"{path.name} never appeared: the command did not start")


def during_command(w_bin: Path) -> None:
    long_cmd = "echo $$ > cmd.pid; echo begun; sleep 30"
    box = Box([act(commands=[long_cmd], finished=False), act("never")])
    w = Worker(w_bin, box)
    try:
        w.send({"queue": "open", "key": "chat-live"})
        w.wait(kind("queue"))
        w.send(box.task("do a"))
        w.wait(kind("turn"))
        pid = int(_wait_file(box.work / "cmd.pid"))       # the command runs now, for 30 s
        t0 = time.time()
        w.send({"steer": "and b", "id": "st1", "turn": 1})
        w.wait(kind("steer", id="st1", status="accepted"), 5)
        took = time.time() - t0
        assert took < 1.0, f"steer accepted after {took:.2f} s"
        t0 = time.time()
        w.send({"queue": "add", "id": "q1", "text": "do c", "message": box.task("do c")})
        w.wait(lambda l: l.get("kind") == "queue" and [i["id"] for i in l["items"]] == ["q1"], 5)
        took = time.time() - t0
        assert took < 1.0, f"queue op answered after {took:.2f} s"
        assert not w.kinds("command") and not w.kinds("ended"), w.lines[-6:]
        t0 = time.time()
        w.send({"interrupt": True})
        e = w.wait(ended(), 5)
        took = time.time() - t0
        assert e["ending"] == "stopped" and took < 1.0, (e, f"{took:.2f} s")
        c = w.kinds("command")
        assert len(c) == 1 and c[0]["command"] == long_cmd and c[0]["exit"] == -1 and "begun" in c[0]["output"] and "[stopped]" in c[0]["output"], c
        assert _group_gone(pid), "the command's process group outlived the interrupt"
        # the existing semantics: the steer it never took heads the queue, which pauses (interrupted)
        w.wait(kind("steer", id="st1", status="queued"))
        snap = w.wait(lambda l: l.get("kind") == "queue" and l.get("paused"))
        assert snap["reason"] == "interrupted" and [i["id"] for i in snap["items"]] == ["st1", "q1"], snap
        time.sleep(0.3)
        assert len(w.kinds("turn")) == 1, w.lines[-6:]
    finally:
        w.close(); box.close()
    # an approved command in a gated run: an interrupt with resubmit kills it, and the steer goes on as a turn
    box = Box([act(commands=[long_cmd], finished=False), act("answered the steer")])
    w = Worker(w_bin, box)
    try:
        w.send(box.task("run it", gated=True))
        ask = w.wait(kind("ask"))
        w.send({"answer": True, "ticket": ask["ticket"]})
        pid = int(_wait_file(box.work / "cmd.pid"))
        t0 = time.time()
        w.send({"steer": "do b instead", "id": "st2", "turn": 1})
        w.wait(kind("steer", id="st2", status="accepted"), 5)
        assert time.time() - t0 < 1.0
        t0 = time.time()
        w.send({"interrupt": True, "resubmit": True})
        e = w.wait(ended(), 5)
        took = time.time() - t0
        assert e["ending"] == "stopped" and took < 1.0, (e, f"{took:.2f} s")
        assert _group_gone(pid), "the approved command's process group outlived the interrupt"
        w.wait(kind("steer", id="st2", status="resubmitted"))
        t = w.wait(kind("turn", source="resubmit"))
        assert t["text"] == "do b instead" and t["turn"] == 2, t
        assert w.wait(ended(), 60)["text"] == "answered the steer"
    finally:
        w.close(); box.close()


HOST_DRIVER = REPO / "evals/coder-desktop/host_driver/build/host_driver"


class Host:
    """host_driver (the real apps/coder-desktop/host.c and page) on the real worker, in a Box."""

    def __init__(self, w_bin: Path, box: Box):
        env = dict(os.environ, HOME=str(box.home), GW_REPLAY=str(box.replay), BROWSER="/usr/bin/true")
        self.p = subprocess.Popen([str(HOST_DRIVER), "--worker", str(w_bin)], cwd=str(box.root), env=env,
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)

    def cmd(self, line: str) -> dict:
        assert self.p.stdin and self.p.stdout
        self.p.stdin.write(line + "\n"); self.p.stdin.flush()
        out = self.p.stdout.readline()
        return json.loads(out) if out else {}

    def until(self, pred, timeout: float = 30.0) -> dict:
        end = time.time() + timeout
        st: dict = {}
        while time.time() < end:
            st = self.cmd("PUMP 0 200 0")
            if pred(st):
                return st
        raise AssertionError("page never got there: " + json.dumps(st)[:600])

    def close(self) -> None:
        try:
            self.cmd("QUIT")
            self.p.wait(10)
        except Exception:  # noqa: BLE001
            self.p.kill()


def page_host(w_bin: Path) -> None:
    if not HOST_DRIVER.exists():
        r = subprocess.run(["bash", str(REPO / "evals/coder-desktop/host_driver/build.sh")], cwd=str(REPO), capture_output=True, text=True, timeout=900)
        if r.returncode != 0:
            raise AssertionError("host_driver build failed: " + (r.stdout + r.stderr)[-2000:])
    box = Box([act(commands=["sleep 1"], finished=False), act("first done"), act("took the steer"), act("queued done")])
    h = Host(w_bin, box)
    try:
        assert h.cmd(f"OPEN 0 1 {REPO / 'components/zeron'}").get("ok")
        assert h.cmd(f"SET_CWD 0 {box.work}").get("ok") and h.cmd("SET_SESSION 0 fu-page").get("ok")
        assert h.cmd("SPAWN_EXEC 0").get("ok")
        assert h.cmd("QUEUE 0 open - 0 0 chat-page").get("ok")
        assert h.cmd("SET_TEXT 0 do a").get("ok") and h.cmd("SUBMIT 0").get("ok")
        h.until(lambda st: st.get("worker_turn") == 1 and st.get("running") == 1)
        assert h.cmd("STEER 0 st1 also b").get("ok")
        assert h.cmd("QUEUE_ADD 0 q1 then c").get("ok")
        st = h.until(lambda st: st.get("queue_ids") == ["q1"])
        assert st["queue_waiting"] == 1, st
        # the steer is applied in turn 1; the worker then starts q1 itself, as turn 2 of the page
        st = h.until(lambda st: st.get("worker_turn") == 2 and st.get("running") == 0 and st.get("queue_count") == 0, 60)
        assert st["steer_status"] == [3], st          # CP_STEER_APPLIED
        assert st["actions"].count(1) == 1, st        # one message sent by the page; the queued turn needed none
        # a steer with no turn running is refused by the page itself
        assert not h.cmd("STEER 0 st2 too late").get("ok")
        assert box.queue_file("chat-page")["done"] == ["q1"]
    finally:
        h.close(); box.close()


CASES = [steer_applied, steer_rejections, queue_fifo, error_pauses, interrupt_resubmit, interrupt_pauses, promote_now, restart_restores, approvals_separate, during_command, page_host]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--worker")
    ap.add_argument("--filter", default="")
    a = ap.parse_args()
    w_bin = Path(a.worker).resolve() if a.worker else build_worker()
    passed = failed = 0
    for case in CASES:
        if a.filter and a.filter not in case.__name__:
            continue
        t0 = time.time()
        try:
            case(w_bin)
            passed += 1
            print(f"ok   {case.__name__} ({int((time.time() - t0) * 1000)} ms)")
        except Exception as e:  # noqa: BLE001
            failed += 1
            print(f"FAIL {case.__name__}: {e}")
    print(f"{passed + failed} checks, {passed} passed, {failed} failed")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
