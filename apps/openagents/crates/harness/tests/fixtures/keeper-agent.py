#!/usr/bin/env python3
"""A fake agent for keeper tests: it speaks the keeper line protocol
(`openagents_harness::keeper::lines`), streams numbered chunks, optionally
asks one question, and ends with Done. It never reaches a network."""
import json
import os
import queue
import signal
import sys
import threading
import time

chunks = int(os.environ.get("KEEPER_FAKE_CHUNKS", "20"))
interval = float(os.environ.get("KEEPER_FAKE_INTERVAL_MS", "20")) / 1000
ask_at = int(os.environ.get("KEEPER_FAKE_ASK_AT", "-1"))
lock = threading.Lock()


def emit(message):
    with lock:
        sys.stdout.write(json.dumps(message) + "\n")
        sys.stdout.flush()


def done(status):
    emit({"type": "done", "status": status, "result": None, "error": None, "sessionId": None})


def on_interrupt(*_):
    done("interrupted")
    os._exit(0)


signal.signal(signal.SIGINT, on_interrupt)
inbox = queue.Queue()


def read():
    for line in sys.stdin:
        inbox.put(json.loads(line))


threading.Thread(target=read, daemon=True).start()
inbox.get()  # the first prompt
for i in range(chunks):
    if i == ask_at:
        emit({"ask": {"id": "q1", "questions": [
            {"id": "pick", "header": "Question", "question": "Which one?", "options": ["A", "B"]}
        ]}})
        while True:
            message = inbox.get()
            if "answer" in message:
                break
        labels = ",".join(a["labels"][0] for a in message["answer"]["answers"])
        emit({"type": "textDelta", "text": f"answered {labels}\n"})
    emit({"type": "textDelta", "text": f"chunk {i}\n"})
    time.sleep(interval)
done("completed")
