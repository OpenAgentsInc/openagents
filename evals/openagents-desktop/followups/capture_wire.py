#!/usr/bin/env python3
# Firm reason: 2 (an independent oracle: records what the Coax worker and the desktop's page say to each other from outside Coax, as run_followups.py does)
"""Record Microcoder's worker lines for the kinds the profile lacked (GitHub #13718).

Runs the real worker (coder/tui/mcworker.coax) in a temp HOME on generated GW_REPLAY answers,
as run_followups.py does, and keeps every line each way byte for byte. The runs it writes go in
proto/coder/microcoder-worker-lines-v1.json under "runs" (replacing runs it wrote before, by
name), where coax/lib/agent/engine/mcworker-profile.coax reads each line through the codec and
writes it back.

  followups-steer-queue   a thought, turns, steers (accepted, applied, rejected), queue lines,
                          queue_refused, a queued turn
  followups-delegate      the recall line, a run with sub-agents: delegate and the results
  followups-delegate-stop a steer while the run waits on its sub-agents, results with one still
                          running, then a step that stops one sub-agent: stop_agent (GitHub #13766)
  followups-interrupt     interrupt with resubmit: the steer resubmitted as a turn of its own
  followups-page          the desktop's host.c and page (host_driver) on the same worker: the
                          queue, steer and interrupt lines as the page writes them

An owner's line this script wrote itself (the first three runs) is marked read_only: the profile
reads it as the worker does and does not write it back. The page's lines are written back.

Usage: python3 evals/openagents-desktop/followups/capture_wire.py [--worker BIN] [--write] [--only NAME]
"""
from __future__ import annotations

import argparse
import json
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import run_followups as rf  # noqa: E402

REPO = rf.REPO
PROFILE = REPO / "proto/coder/microcoder-worker-lines-v1.json"
# the profile's name for a line: an owner's by its first member, a worker's by its kind
OWNER_KINDS = ["task", "answer", "stop", "forget", "embed", "results", "recall", "steer", "interrupt", "queue"]
WORKER_KINDS = {"steer": "steered", "queue": "queued"}


def label(end: str, line: str) -> str:
    obj = json.loads(line)
    if end == "worker":
        return WORKER_KINDS.get(obj["kind"], obj["kind"])
    for k in ("stop", "forget", "embed", "recall", "results", "answer", "steer", "interrupt", "queue"):
        if k in obj:
            return k
    return "task"


def keep(wire: list[tuple[str, str]], read_only: bool) -> list[dict]:
    out = []
    for end, line in wire:
        # the run's report after it ended is aware.coax's line (mc_aw_report), not this codec's: owners skip it
        if end == "worker" and json.loads(line).get("kind") == "aware":
            continue
        item = {"from": end, "kind": label(end, line), "line": line}
        if read_only and end == "owner":
            item["read_only"] = True
        out.append(item)
    return out


def steer_queue(w_bin: Path) -> list[dict]:
    box = rf.Box([])
    (box.replay / "1.http").write_text(rf.http(rf.act(commands=["sleep 1"], finished=False), think="Look at a first."))
    (box.replay / "2.http").write_text(rf.http(rf.act("took the steer")))
    (box.replay / "3.http").write_text(rf.http(rf.act("b done")))
    w = rf.Worker(w_bin, box)
    try:
        w.send({"queue": "open", "key": "chat-wire"})
        w.wait(rf.kind("queue"))
        w.send(box.task("do a"))
        w.wait(rf.kind("turn"))
        w.wait(rf.kind("thought"))
        w.send({"steer": "also b", "id": "st1", "turn": 1})
        w.wait(rf.kind("steer", id="st1", status="accepted"))
        w.send({"steer": "x", "id": "st2", "turn": 7})
        w.wait(rf.kind("steer", id="st2", status="rejected"))
        w.send({"queue": "add", "id": "q1", "text": "do b", "message": box.task("do b")})
        w.send({"queue": "delete", "id": "nope"})
        w.wait(rf.ended())
        assert [l for l in w.lines if l.get("kind") == "queue_refused" and l.get("op") == "delete"], w.lines
        assert [l for l in w.lines if l.get("kind") == "steer" and l.get("id") == "st1" and l.get("status") == "applied"], w.lines
        w.wait(rf.kind("turn", source="queue"))
        w.wait(rf.ended())
        w.wait(lambda l: l.get("kind") == "queue" and l.get("active") == "" and not l["items"])
        return w.wire
    finally:
        w.close(); box.close()


def delegate(w_bin: Path) -> list[dict]:
    box = rf.Box([])
    split = {"rationale": "split", "delegate": [{"level": "hydrogen", "task": "count the files"}], "edits": [], "commands": [], "view": [],
             "finished": False, "reply": "", "ask": "none"}
    (box.replay / "1.http").write_text(rf.http(split))
    (box.replay / "2.http").write_text(rf.http(rf.act("three files")))
    w = rf.Worker(w_bin, box)
    try:
        w.send({"recall": [{"task": "what is here", "reply": "a few files"}]})
        w.send(box.task("count them", agents=True))
        d = w.wait(rf.kind("delegate"))
        w.send({"results": [{"exit": 0, "output": "3 files"}], "ticket": d["ticket"]})
        e = w.wait(rf.ended())
        assert e["text"] == "three files", e
        return w.wire
    finally:
        w.close(); box.close()


def delegate_stop(w_bin: Path) -> list[dict]:
    split = {"rationale": "split", "delegate": [{"level": "hydrogen", "task": "count the files"}, {"level": "carbon", "task": "find the bug"}],
             "edits": [], "commands": [], "view": [], "finished": False, "reply": "", "ask": "none"}
    stop = {"rationale": "enough", "delegate": [{"level": "carbon", "task": "", "resume": 2, "stop": True}],
            "edits": [], "commands": [], "view": [], "finished": False, "reply": "", "ask": "none"}
    box = rf.Box([split, stop, rf.act("counted; the bug search is stopped")])
    w = rf.Worker(w_bin, box)
    try:
        w.send(box.task("count and find", agents=True))
        d = w.wait(rf.kind("delegate"))
        w.send({"steer": "and stop the bug search", "id": "st1", "turn": 1})
        w.wait(rf.kind("steer", id="st1", status="accepted"))
        w.send({"results": [{"exit": 0, "output": "3 files"}, {"exit": 1, "output": "still running: sub-agent number 2"}], "ticket": d["ticket"]})
        w.wait(rf.kind("stop_agent"))
        e = w.wait(rf.ended())
        assert e["text"] == "counted; the bug search is stopped", e
        return w.wire
    finally:
        w.close(); box.close()


def interrupt(w_bin: Path) -> list[dict]:
    box = rf.Box([rf.act(commands=["sleep 3"], finished=False), rf.act("answered the steer")])
    w = rf.Worker(w_bin, box)
    try:
        w.send(box.task("do a"))
        w.wait(rf.kind("turn"))
        w.wait(rf.kind("attempt"))
        w.send({"steer": "actually do b", "id": "st1", "turn": 1})
        w.wait(rf.kind("steer", id="st1", status="accepted"))
        w.send({"interrupt": True, "resubmit": True})
        w.wait(rf.ended())
        w.wait(rf.kind("turn", source="resubmit"))
        w.wait(rf.ended(), 60)
        return w.wire
    finally:
        w.close(); box.close()


def page(w_bin: Path) -> list[tuple[str, str]]:
    if not rf.HOST_DRIVER.exists():
        raise SystemExit("build evals/openagents-desktop/host_driver first (bash evals/openagents-desktop/host_driver/build.sh)")
    box = rf.Box([rf.act(commands=["sleep 2"], finished=False), rf.act("first done"), rf.act("took the steer"), rf.act(commands=["sleep 5"], finished=False)])
    log = box.root / "wire.jsonl"
    h = rf.Host(w_bin, box)
    try:
        assert h.cmd(f"WIRE_LOG {log}").get("ok")
        assert h.cmd(f"OPEN 0 1 {REPO / 'components'}").get("ok")
        assert h.cmd(f"SET_CWD 0 {box.work}").get("ok") and h.cmd("SET_SESSION 0 fu-wire").get("ok")
        assert h.cmd("SPAWN_EXEC 0").get("ok")
        assert h.cmd("QUEUE 0 open - 0 0 chat-page").get("ok")
        assert h.cmd("SET_TEXT 0 do a").get("ok") and h.cmd("SUBMIT 0").get("ok")
        h.until(lambda st: st.get("worker_turn") == 1 and st.get("running") == 1)
        assert h.cmd("STEER 0 st1 also b").get("ok")
        # paused first, so turn 1's end starts nothing: the ops that follow are not racing it
        assert h.cmd("QUEUE 0 pause - 0 0 -").get("ok")
        assert h.cmd("QUEUE_ADD 0 q1 then c").get("ok")
        assert h.cmd("QUEUE_ADD 0 q2 and d").get("ok")
        h.until(lambda st: st.get("queue_ids") == ["q1", "q2"])
        assert h.cmd("QUEUE 0 move q2 0 0 -").get("ok")
        h.until(lambda st: st.get("queue_ids") == ["q2", "q1"])
        assert h.cmd("QUEUE 0 delete q1 0 0 -").get("ok")
        h.until(lambda st: st.get("worker_turn") == 1 and st.get("running") == 0, 60)
        assert h.cmd("QUEUE 0 promote q2 0 1 -").get("ok")
        h.until(lambda st: st.get("worker_turn") == 2 and st.get("running") == 1, 60)
        assert h.cmd("INTERRUPT 0 0").get("ok")
        h.until(lambda st: st.get("running") == 0, 60)
        assert h.cmd("QUEUE 0 list - 0 0 -").get("ok")
        h.cmd("PUMP 0 300 0")
    finally:
        h.close()
    wire = []
    for raw in log.read_text().splitlines():
        r = json.loads(raw)
        if r.get("agent", -1) >= 0:
            continue
        if r["dir"] == "to_worker":
            wire.append(("owner", r["line"]))
        elif r["dir"] == "from_worker":
            wire.append(("worker", r["line"]))
    box.close()
    return wire


RUNS = [
    ("followups-steer-queue", steer_queue, True,
     "the worker on a temp HOME with GW_REPLAY answers: a step's thought; turn 1 from a message; a steer accepted then applied, one for another turn rejected; a follow-up queued and an unknown id refused; turn 2 from the queue"),
    ("followups-delegate", delegate, True,
     "the recall line, then a task with agents: the worker delegates one task and takes the owner's results"),
    ("followups-delegate-stop", delegate_stop, True,
     "a task with agents: a steer while the run waits on its two sub-agents is accepted; the owner answers at once with one still running; the next step takes the steer and stops sub-agent 2 (stop_agent), waiting on nothing"),
    ("followups-interrupt", interrupt, True,
     "a steer accepted, then interrupt with resubmit: the turn stops and the steer starts turn 2"),
    ("followups-page", page, False,
     "the desktop's host.c and page (evals/openagents-desktop/host_driver) on the worker: queue open, a message, a steer, pause, two follow-ups, move, delete, promote now, interrupt, list"),
]
FROM_DRIVER = ("capture_wire.py (evals/openagents-desktop/followups/) on the worker built from this tree; nothing called a model. "
               "Each end's lines in the order they were sent, interleaved as the driver saw them. The owner's lines here were written by the driver: read_only.")
FROM_PAGE = ("capture_wire.py through host_driver's WIRE_LOG: the owner's lines are what the page (apps/coder-desktop/page/followups.coax, persist.coax) "
             "and host.c wrote, the worker's what it sent; nothing called a model.")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--worker")
    ap.add_argument("--write", action="store_true", help="put the runs in the profile")
    ap.add_argument("--only", default="", help="capture only the runs whose name contains this")
    a = ap.parse_args()
    w_bin = Path(a.worker).resolve() if a.worker else rf.build_worker()
    runs = []
    for name, fn, read_only, what in RUNS:
        if a.only and a.only not in name:
            continue
        lines = keep(fn(w_bin), read_only)
        runs.append({"name": name, "what": what, "from": FROM_DRIVER if read_only else FROM_PAGE, "lines": lines})
        print(f"ok   {name}: {len(lines)} lines, kinds {sorted({l['kind'] for l in lines})}")
    if a.write:
        d = json.loads(PROFILE.read_text())
        names = {r["name"] for r in runs}
        d["runs"] = [r for r in d["runs"] if r["name"] not in names] + runs
        PROFILE.write_text(json.dumps(d, indent=1, ensure_ascii=False) + "\n")
        print(f"wrote {len(runs)} runs to {PROFILE.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
