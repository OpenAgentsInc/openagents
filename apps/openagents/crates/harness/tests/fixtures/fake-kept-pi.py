#!/usr/bin/env python3
"""Fake Pi RPC agent for keeper tests (crates/harness/tests/keeper_drivers.rs).

Speaks Pi's JSONL RPC. A prompt whose text carries `chunks=N ask=K` streams N
slow chunks, opening a select dialog before chunk K; any other prompt answers
`heard <text>`. A request id used twice is a protocol error: a driver that
picks the run up must not repeat the earlier engine's writes.
"""
import json
import re
import sys
import threading
import time
from pathlib import Path

session = "pi-kept-session"
session_file = Path.cwd() / "pi-kept-session.jsonl"
session_file.write_text(json.dumps({"type": "session", "id": session}) + "\n")
lock = threading.Lock()
streaming = threading.Event()
answers = {}
answered = threading.Condition(lock)
seen = set()


def emit(frame):
    with lock:
        sys.stdout.write(json.dumps(frame) + "\n")
        sys.stdout.flush()


def respond(request, data=None):
    emit({"type": "response", "id": request["id"], "command": request["type"],
          "success": True, "data": data or {}})


def delta(text):
    emit({"type": "message_update",
          "assistantMessageEvent": {"type": "text_delta", "contentIndex": 0, "delta": text}})


def run(text):
    emit({"type": "agent_start"})
    emit({"type": "message_start", "message": {"role": "user"}})
    emit({"type": "message_start", "message": {"role": "assistant"}})
    full = ""
    found = re.search(r"chunks=(\d+) ask=(-?\d+)", text)
    if found:
        for i in range(int(found.group(1))):
            if i == int(found.group(2)):
                emit({"type": "extension_ui_request", "id": "q1", "method": "select",
                      "title": "Which one?", "options": ["A", "B"]})
                with answered:
                    answered.wait_for(lambda: "q1" in answers)
                line = f"answered {answers['q1']}\n"
                delta(line)
                full += line
            line = f"chunk {i}\n"
            delta(line)
            full += line
            time.sleep(0.03)
    else:
        full = f"heard {text}\n"
        delta(full)
    emit({"type": "message_end", "message": {"role": "assistant",
          "content": [{"type": "text", "text": full}], "stopReason": "stop"}})
    emit({"type": "agent_end"})
    streaming.clear()
    emit({"type": "agent_settled"})


for line in sys.stdin:
    request = json.loads(line)
    kind = request["type"]
    if kind == "extension_ui_response":
        with answered:
            answers[request["id"]] = request.get("value", "none")
            answered.notify_all()
        continue
    if request["id"] in seen:
        sys.stderr.write(f"request id {request['id']} was used before\n")
        sys.exit(3)
    seen.add(request["id"])
    if kind == "get_state":
        respond(request, {"sessionId": session, "sessionFile": str(session_file),
                          "isStreaming": streaming.is_set(), "isCompacting": False,
                          "messageCount": 2, "model": {"id": "fake"}})
    elif kind == "get_commands":
        respond(request, {"commands": []})
    elif kind == "prompt":
        if streaming.is_set():
            sys.stderr.write("prompt while streaming\n")
            sys.exit(2)
        streaming.set()
        respond(request)
        threading.Thread(target=run, args=(request["message"],), daemon=True).start()
    else:
        respond(request)
