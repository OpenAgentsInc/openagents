#!/usr/bin/env python3
"""Fake ACP agent for keeper tests (crates/harness/tests/keeper_drivers.rs).

initialize, then session/new, then any number of `session/prompt`s. A prompt
whose text carries `chunks=N ask=K` streams N slow chunks, asking a question
before chunk K; any other prompt answers `heard <text>`. Each turn ends with
the prompt's response. A request id used twice, or a second handshake, is a
protocol error: a driver that picks the run up must not repeat the earlier
engine's writes.
"""
import json
import re
import sys
import time

seen = set()
sid = "s-kept"


def emit(message):
    sys.stdout.write(json.dumps(message) + "\n")
    sys.stdout.flush()


def say(text):
    emit({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {
            "sessionId": sid,
            "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": {"type": "text", "text": text},
            },
        },
    })


def read():
    line = sys.stdin.readline()
    if not line:
        sys.exit(0)
    message = json.loads(line)
    if "id" in message and "method" in message:
        if message["id"] in seen:
            sys.stderr.write(f"request id {message['id']} was used before\n")
            sys.exit(3)
        seen.add(message["id"])
    return message


def expect(method):
    while True:
        message = read()
        if message.get("method") == method:
            return message
        if "id" in message and "method" in message:
            sys.stderr.write(f"expected {method}, got {message['method']}\n")
            sys.exit(2)


init = expect("initialize")
emit({"jsonrpc": "2.0", "id": init["id"], "result": {"protocolVersion": 1, "agentCapabilities": {}}})
new = expect("session/new")
emit({"jsonrpc": "2.0", "id": new["id"], "result": {"sessionId": sid}})

while True:
    message = read()
    method = message.get("method")
    if method in ("initialize", "session/new", "session/load"):
        sys.stderr.write(f"{method} again\n")
        sys.exit(2)
    if method != "session/prompt":
        if "id" in message and method is not None:
            emit({"jsonrpc": "2.0", "id": message["id"], "result": {}})
        continue
    text = message["params"]["prompt"][0]["text"]
    found = re.search(r"chunks=(\d+) ask=(-?\d+)", text)
    if found:
        chunks, ask = int(found.group(1)), int(found.group(2))
        for i in range(chunks):
            if i == ask:
                emit({
                    "jsonrpc": "2.0",
                    "id": 900,
                    "method": "session/request_permission",
                    "params": {
                        "sessionId": sid,
                        "toolCall": {"toolCallId": "q1", "title": "Which one?"},
                        "options": [
                            {"optionId": "a", "name": "A"},
                            {"optionId": "b", "name": "B"},
                        ],
                    },
                })
                while True:
                    answer = read()
                    if answer.get("id") == 900 and "method" not in answer:
                        break
                picked = answer.get("result", {}).get("outcome", {}).get("optionId", "none")
                say(f"answered {picked.upper()}\n")
            say(f"chunk {i}\n")
            time.sleep(0.03)
    else:
        say(f"heard {text}\n")
    emit({"jsonrpc": "2.0", "id": message["id"], "result": {"stopReason": "end_turn"}})
