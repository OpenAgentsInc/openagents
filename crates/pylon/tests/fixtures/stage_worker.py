#!/usr/bin/env python3
"""Synthetic stage for Pylon's stage-group tests.

The stage connects to the front, announces itself, and answers health pings,
checks, steps, cancellations, and fences as line-delimited JSON. Faults are
files in --root named KIND-STAGE. A file holding a step number fires once at
that step and is then removed; "slow", "ignore-cancel", "always-die", and
"weights" stay until the test removes them. A file "end-STAGE" makes the
last stage end the answer at that step; "answer-STAGE", a JSON array of
strings, makes it answer with those pieces, one a step, and end at the last.
Every observation goes to events.jsonl in --root.

With --data-listen the stage uses the direct transport (W12): it listens for
the previous stage's link, announces that address, and after the front's
`link` message sends its stage state (--state-bytes of synthetic bytes, whose
first 32 bytes are the SHA-256 of the rest) straight to the next stage with
stage_link.py. Only the last stage acknowledges to the front. More faults:
"drop-link" closes the outbound link at a step, "dup-frame" sends a step's
frame twice, "partition" stops reading the inbound link at a step while
control stays up, "late-link" (milliseconds) has a forked copy reconnect to
the next stage with the old generation's key, "chunks" (on the first stage)
splits step zero into that many frames, and "link-slow" (milliseconds)
delays each inbound frame.
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
args.add_argument("--data-listen")
args.add_argument("--data-advertise")
args.add_argument("--state-bytes", type=int, default=44_300)
args.add_argument("--buffers", type=int, default=0)
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


direct = options.data_listen is not None
listener = None
advertise = None
if direct:
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import hashlib

    import stage_link

    listen_host, listen_port = options.data_listen.rsplit(":", 1)
    listener = stage_link.Listener(listen_host, int(listen_port), options.buffers)
    advertise = options.data_advertise or f"{listen_host}:{listener.port}"


def hello(reported):
    message = {"kind": "hello", "stage": name, "token": token, "generation": reported,
               "weights": text("weights") or "sha256:fixture",
               "runtime": "fixture-runtime/1", "protocol": 1}
    if direct:
        message["data"] = advertise
    return message


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
link_lock = threading.Lock()
link = {"config": None, "out": None, "in": None}
data_room = threading.Semaphore(1)
inbox = {}
cancel_sent = set()


def drop_links():
    with link_lock:
        if link["out"] is not None:
            link["out"].abort()
        if link["in"] is not None:
            link["in"].close()
        link.update(config=None, out=None, **{"in": None})


def link_failed(gen, peer, reason):
    record("link-failed", peer=peer, reason=reason)
    try:
        send({"kind": "link_failed", "generation": gen, "peer": peer, "reason": reason})
    except OSError:
        pass


def do_link(message):
    """Open this generation's link to the next stage."""
    gen = message["generation"]
    with link_lock:
        if gen != generation:
            record("link-stale", got=gen)
            return
        link["config"] = message
    key = bytes.fromhex(message["key"])
    nxt = message.get("next")
    if nxt is None:
        send({"kind": "linked", "generation": gen})
        return
    deadline = time.monotonic() + 5
    while True:
        try:
            out = stage_link.connect(nxt, key, message["group"], gen, message["stage"],
                                     message["stage"] + 1, message["queue"],
                                     message["send_timeout_ms"] / 1000, options.buffers)
            break
        except (OSError, stage_link.LinkError) as error:
            if time.monotonic() > deadline:
                return link_failed(gen, "next", f"cannot reach the next stage: {error}")
            time.sleep(0.05)
    with link_lock:
        if generation != gen:
            out.abort()
            return
        link["out"] = out
    record("linked", next=nxt)
    send({"kind": "linked", "generation": gen})
    late = take("late-link")
    if late is not None and os.fork() == 0:
        # A copy of this stage that outlives its generation and comes back
        # to the next stage with the old key.
        os.setsid()
        time.sleep(int(late) / 1000)
        try:
            stage_link.connect(nxt, key, message["group"], gen, message["stage"],
                               message["stage"] + 1)
            record("late-link", result="accepted")
        except (OSError, stage_link.LinkError) as error:
            record("late-link", result=str(error))
        os._exit(0)


def accept_links():
    """Accept the previous stage's link for the current generation."""
    while True:
        try:
            offer = listener.offer()
        except (OSError, stage_link.LinkError) as error:
            record("link-refused", reason=str(error))
            continue
        with link_lock:
            config = link["config"]
        if config is None or config["stage"] == 0:
            offer.refuse("generation")
            record("link-refused", reason="no link is expected", claimed=offer.hop)
            continue
        expected = {"group": config["group"], "generation": config["generation"],
                    "from": config["stage"] - 1, "to": config["stage"]}
        try:
            receiver = offer.accept(bytes.fromhex(config["key"]), expected)
        except stage_link.LinkError as error:
            record("link-refused", reason=str(error), claimed=offer.hop)
            continue
        with link_lock:
            if link["in"] is not None:
                link["in"].close()
            link["in"] = receiver
        threading.Thread(target=read_link, args=(receiver, config["generation"]),
                         daemon=True).start()


def read_link(receiver, gen):
    while True:
        try:
            frame = receiver.recv()
        except (OSError, stage_link.LinkError) as error:
            with link_lock:
                current = link["in"] is receiver and generation == gen
            if current:
                kind = getattr(error, "kind", "io")
                link_failed(gen, "prev", "the previous stage closed the link"
                            if kind == "closed" else str(error))
            return
        if at_step("partition", frame["step"]):
            record("partitioned", step=frame["step"])
            while True:
                time.sleep(1)
        if frame["kind"] == stage_link.CANCEL:
            record("cancel-frame", request=frame["request_id"])
            cancelled.add(frame["request_id"])
            work.put(("cancel-frame", frame))
            continue
        delay = text("link-slow")
        if delay:
            time.sleep(int(delay) / 1000)
        data_room.acquire()
        work.put(("data", frame))


def synthetic_state(size):
    body = os.urandom(max(size - 32, 0))
    return hashlib.sha256(body).digest() + body


def forward(request, attempt, step, meta, gen):
    """Send this step's state to the next stage, as many frames as asked."""
    with link_lock:
        out = link["out"]
    if out is None:
        link_failed(gen, "next", "no link to the next stage")
        return
    if at_step("drop-link", step):
        record("link-dropped", step=step)
        out.abort()
        return
    copies = 2 if at_step("dup-frame", step) else 1
    for _ in range(copies):
        frame = {"kind": stage_link.STATE, "generation": gen, "request_id": request,
                 "attempt": attempt, "step": step, "meta": json.dumps(meta).encode(),
                 "state": synthetic_state(options.state_bytes)}
        try:
            size = out.send(frame)
        except stage_link.LinkError as error:
            return link_failed(gen, "next", str(error))
        record("forwarded", request=request, step=step, chunk=meta.get("chunk", 0),
               bytes=size, queued=out.queued, peak=out.peak)
        send({"kind": "forwarded", "generation": gen, "request_id": request,
              "attempt": attempt, "step": step, "bytes": size})


def admit(frame, gen):
    """W9's inbox rules for the fake: one frame per step and chunk, in order."""
    meta = json.loads(frame["meta"] or b"{}")
    state = frame["state"]
    if hashlib.sha256(state[32:]).digest() != state[:32]:
        return None, "a frame's stage state is damaged"
    at = (frame["step"], meta.get("chunk", 0))
    key = (frame["request_id"], frame["attempt"])
    last = inbox.get(key)
    if last is not None and at <= last:
        return None, f"step {at[0]} chunk {at[1]} of request {key[0]} arrived twice"
    inbox[key] = at
    return meta, None


def stop_requested(request):
    return fencing.is_set() or (request in cancelled and not present("ignore-cancel"))


def worker():
    global generation
    while True:
        item = work.get()
        if item[0] == "fence":
            if direct:
                drop_links()
            generation = item[1]
            cancelled.clear()
            cancel_sent.clear()
            fencing.clear()
            record("fenced")
            send(hello(generation))
            continue
        if item[0] == "cancel":
            if direct and stage_index() == 0:
                # The first stage starts the cancellation down the chain.
                pass_cancel(item[1], item[2])
            if not present("ignore-cancel"):
                record("stopped", request=item[1])
                send({"kind": "stopped", "request_id": item[1], "attempt": item[2]})
            continue
        if item[0] == "cancel-frame":
            pass_cancel(item[1]["request_id"], item[1]["attempt"])
            continue
        if item[0] == "data":
            frame = item[1]
            try:
                handle_data(frame)
            finally:
                data_room.release()
            continue
        message = item[1]
        request, attempt, step = message["request_id"], message["attempt"], message["step"]
        if direct and message["stage"] + 1 < stage_count():
            # The first stage under the direct transport: pass the step on.
            if stop_requested(request):
                record("skipped", request=request, step=step)
                continue
            record("step", request=request, attempt=attempt, step=step)
            if "prompt" in message:
                record("prompt", prompt=message["prompt"])
            chunks = int(text("chunks") or 1) if step == 0 else 1
            for chunk in range(chunks):
                meta = {"sampling": message["sampling"], "chunk": chunk, "chunks": chunks}
                forward(request, attempt, step, meta, generation)
            continue
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
        finish(message, request, attempt, step)


def stage_index():
    with link_lock:
        config = link["config"]
    return config["stage"] if config else 0


def stage_count():
    with link_lock:
        config = link["config"]
    return config["stages"] if config else 1


def pass_cancel(request, attempt):
    """Pass a cancellation down the chain, once per request."""
    if request in cancel_sent:
        return
    cancel_sent.add(request)
    with link_lock:
        out = link["out"]
    if out is None:
        return
    try:
        out.send({"kind": stage_link.CANCEL, "generation": generation, "request_id": request,
                  "attempt": attempt, "step": 0, "meta": b"", "state": b""})
        record("cancel-passed", request=request)
    except stage_link.LinkError as error:
        record("cancel-not-passed", request=request, reason=str(error))


def handle_data(frame):
    """A frame from the previous stage: check it, run the step, pass it on."""
    gen = frame["generation"]
    request, attempt, step = frame["request_id"], frame["attempt"], frame["step"]
    if gen != generation or stop_requested(request):
        record("skipped", request=request, step=step)
        return
    meta, refusal = admit(frame, gen)
    if refusal:
        record("refused", reason=refusal)
        send({"kind": "refused", "reason": refusal})
        return
    record("data", request=request, attempt=attempt, step=step, chunk=meta.get("chunk", 0),
           bytes=len(frame["state"]))
    with link_lock:
        config = link["config"]
    index = config["stage"]
    if at_step("crash-before-ack", step):
        record("crash")
        os._exit(3)
    if at_step("freeze", step):
        record("frozen")
        while True:
            time.sleep(1)
    until = time.monotonic() + int(text("slow") or 0) / 1000
    while time.monotonic() < until and not stop_requested(request):
        time.sleep(0.005)
    if stop_requested(request):
        record("interrupted", request=request, step=step)
        return
    if index + 1 < config["stages"]:
        forward(request, attempt, step, meta, gen)
        return
    if meta.get("chunk", 0) + 1 < meta.get("chunks", 1):
        return
    finish({"stage": index, "sampling": meta["sampling"]}, request, attempt, step)


def finish(message, request, attempt, step):
    """Acknowledge a step to the front; the last stage adds its token."""
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
        pieces = json.loads(text("answer") or "null")
        if pieces is not None and step < len(pieces):
            # A scripted answer: each step's text, ending at the last.
            ack["text"] = pieces[step]
            if step == len(pieces) - 1:
                ack["end"] = True
        if at_step("end", step):
            ack["end"] = True
    if at_step("old-ack", step):
        send({**ack, "step": step - 1})
    if at_step("stale-ack", step):
        send({**ack, "generation": generation - 1})
    if at_step("drop-ack", step):
        record("dropped", request=request, step=step)
        return
    send(ack)
    if at_step("replay-ack", step):
        send(ack)
    if at_step("crash-after-ack", step):
        record("crash")
        os._exit(3)


threading.Thread(target=worker, daemon=True).start()
if direct:
    threading.Thread(target=accept_links, daemon=True).start()
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
    elif kind == "link":
        threading.Thread(target=do_link, args=(message,), daemon=True).start()
record("front-closed")
sys.exit(0)
