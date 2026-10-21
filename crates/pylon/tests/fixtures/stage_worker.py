#!/usr/bin/env python3
"""Synthetic stage for Pylon's stage-group tests.

The stage connects to the front, announces itself, and answers health pings,
checks, steps, cancellations, and fences as line-delimited JSON. Faults are
files in --root named KIND-STAGE. A file holding a step number fires once at
that step and is then removed; "slow", "ignore-cancel", "always-die", and
"weights" stay until the test removes them. A file "end-STAGE" makes the
last stage end the answer at that step. Every observation goes to
events.jsonl in --root.
"""

import argparse
import json
import os
import queue
import socket
import sys
import threading
import time
from pathlib import Path

args = argparse.ArgumentParser()
args.add_argument("--root", type=Path, required=True)
args.add_argument("--stage", required=True)
args.add_argument("--generation", type=int, required=True)
args.add_argument("--front", required=True)
args.add_argument("--token", required=True)
options, _ = args.parse_known_args()
root, name, token = options.root, options.stage, options.token
generation = options.generation
lock = threading.Lock()


def record(event, **data):
    line = json.dumps({"event": event, "stage": name, "pid": os.getpid(),
                       "generation": generation, **data})
    with lock, (root / "events.jsonl").open("a") as output:
        output.write(line + "\n")


def path(kind):
    return root / f"{kind}-{name}"


def present(kind):
    return path(kind).exists()


def text(kind):
    try:
        return path(kind).read_text().strip()
    except FileNotFoundError:
        return None


def take(kind):
    """Read a one-shot fault and remove it."""
    value = text(kind)
    if value is not None:
        path(kind).unlink(missing_ok=True)
    return value


def at_step(kind, step):
    value = text(kind)
    if value is not None and int(value or 0) == step:
        path(kind).unlink(missing_ok=True)
        return True
    return False


def hello(reported):
    return {"kind": "hello", "stage": name, "token": token, "generation": reported,
            "weights": text("weights") or "sha256:fixture",
            "runtime": "fixture-runtime/1", "protocol": 1}


host, port = options.front.rsplit(":", 1)
record("spawn")
if take("die-on-start") is not None or present("always-die"):
    raise SystemExit(2)

late = take("late")
if late is not None and os.fork() == 0:
    # A stage instance that escapes its process group and speaks after the
    # group has replaced it, as a remote stage might.
    os.setsid()
    time.sleep(int(late) / 1000)
    try:
        old = socket.create_connection((host, int(port)))
        lines = [hello(generation)]
        for request in range(1, 5):
            for step in range(4):
                lines.append({"kind": "ack", "stage": int(name[1:]), "generation": generation,
                              "request_id": str(request), "attempt": 1, "step": step,
                              "token": 9999})
        old.sendall("".join(json.dumps(line) + "\n" for line in lines).encode())
        record("late-sent", late_pid=os.getpid())
        time.sleep(0.2)
    except OSError as error:
        record("late-failed", error=str(error))
    os._exit(0)

delay = text("start-delay")
if delay:
    time.sleep(int(delay) / 1000)
connection = socket.create_connection((host, int(port)))
output_lock = threading.Lock()


def send(message):
    with output_lock:
        connection.sendall((json.dumps(message) + "\n").encode())


send(hello(generation - 1 if take("stale-generation") is not None else generation))
work = queue.Queue()
cancelled = set()
fencing = threading.Event()


def stop_requested(request):
    return fencing.is_set() or (request in cancelled and not present("ignore-cancel"))


def worker():
    global generation
    while True:
        item = work.get()
        if item[0] == "fence":
            generation = item[1]
            cancelled.clear()
            fencing.clear()
            record("fenced")
            send(hello(generation))
            continue
        if item[0] == "cancel":
            if not present("ignore-cancel"):
                record("stopped", request=item[1])
                send({"kind": "stopped", "request_id": item[1], "attempt": item[2]})
            continue
        message = item[1]
        request, attempt, step = message["request_id"], message["attempt"], message["step"]
        if stop_requested(request):
            record("skipped", request=request, step=step)
            continue
        record("step", request=request, attempt=attempt, step=step)
        if at_step("crash-before-ack", step):
            record("crash")
            os._exit(3)
        if at_step("drop-step", step):
            record("dropped", request=request, step=step)
            continue
        if at_step("freeze", step):
            record("frozen")
            while True:
                time.sleep(1)
        if at_step("disconnect", step):
            record("disconnected")
            connection.shutdown(socket.SHUT_RDWR)
            while True:
                time.sleep(1)
        until = time.monotonic() + int(text("slow") or 0) / 1000
        while time.monotonic() < until and not stop_requested(request):
            time.sleep(0.005)
        if stop_requested(request):
            record("interrupted", request=request, step=step)
            continue
        ack = {"kind": "ack", "stage": message["stage"], "generation": generation,
               "request_id": request, "attempt": attempt, "step": step}
        if "prompt" in message:
            # The first stage gets the conversation with step zero.
            record("prompt", prompt=message["prompt"])
        if "sampling" in message:
            # The last stage samples with the front's settings and returns
            # the token id, seed * 1000 + step, and its text.
            ack["token"] = message["sampling"]["seed"] * 1000 + step
            ack["text"] = f"<{ack['token']}>"
            if at_step("end", step):
                ack["end"] = True
        if at_step("old-ack", step):
            send({**ack, "step": step - 1})
        if at_step("stale-ack", step):
            send({**ack, "generation": generation - 1})
        if at_step("drop-ack", step):
            record("dropped", request=request, step=step)
            continue
        send(ack)
        if at_step("replay-ack", step):
            send(ack)
        if at_step("crash-after-ack", step):
            record("crash")
            os._exit(3)


threading.Thread(target=worker, daemon=True).start()
reader = connection.makefile("r")
for line in reader:
    message = json.loads(line)
    kind = message["kind"]
    if kind == "ping":
        send({"kind": "pong", "nonce": message["nonce"]})
    elif kind == "check":
        wrong = take("fail-check") is not None
        send({"kind": "checked", "nonce": message["nonce"],
              "output": message["input"] * 2 + (2 if wrong else 1)})
    elif kind == "step":
        work.put(("step", message))
    elif kind == "cancel":
        cancelled.add(message["request_id"])
        work.put(("cancel", message["request_id"], message["attempt"]))
    elif kind == "fence":
        fencing.set()
        work.put(("fence", message["generation"]))
record("front-closed")
sys.exit(0)
