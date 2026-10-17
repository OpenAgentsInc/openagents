#!/usr/bin/env python3
"""Local app-server fixture for terminal session control tests."""
import json
import os
import subprocess
import sys
import threading
import time

lock = threading.Lock()
thread_id = "0199aaaa-bbbb-7ccc-8ddd-standin00001"
state = {"steer": False, "turn": 0}


def emit(value):
    with lock:
        print(json.dumps(value), flush=True)


def notice(method, params):
    emit({"method": method, "params": params})


def turn(text, number):
    what = "resume" if "resume" in text else "briefing"
    turn_id = "t" + str(number)
    notice("turn/started", {"turn": {"id": turn_id}})
    if os.environ.get("STANDIN_OFFLINE"):
        while True:
            notice("error", {"error": {"message": "Reconnecting... waiting for network (Connection failed: error sending request)"}, "willRetry": True})
            time.sleep(0.05)
    time.sleep(float(os.environ.get("STANDIN_DELAY", "0")))
    if os.environ.get("STANDIN_HANG") and what == "briefing":
        subprocess.Popen(["/bin/sh", "-c", 'sleep 2; printf harmless > "$STANDIN_HANG"'], start_new_session=True)
        notice("item/completed", {"item": {"type": "agentMessage", "id": "hang", "text": "hanging"}})
        threading.Event().wait()
    item = {"id": "c" + str(number), "type": "commandExecution", "command": "false", "status": "inProgress"}
    notice("item/started", {"item": item})
    item.update(status="completed", exitCode=1)
    notice("item/completed", {"item": item})
    time.sleep(float(os.environ.get("STANDIN_DELAY", "0")))
    if state["steer"]:
        what = "steer"
        state["steer"] = False
    notice("item/completed", {"item": {"id": "m" + str(number), "type": "agentMessage", "text": "heard " + what}})
    notice("thread/tokenUsage/updated", {"tokenUsage": {"total": {"inputTokens": 10, "cachedInputTokens": 0, "outputTokens": 2}}})
    notice("turn/completed", {"turn": {"id": turn_id}})


for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    if "id" not in request:
        continue
    params = request.get("params", {})
    if method == "turn/steer" and os.environ.get("STANDIN_STEER_SILENT"):
        continue
    result = {}
    if method == "thread/resume":
        thread_id = params["threadId"]
        result = {"thread": {"id": thread_id}}
    elif method == "thread/start":
        result = {"thread": {"id": thread_id}}
    elif method == "turn/start":
        state["turn"] += 1
        result = {"turn": {"id": "t" + str(state["turn"])}}
    elif method == "turn/steer":
        state["steer"] = True
        result = {"turnId": "t" + str(state["turn"])}
    emit({"id": request["id"], "result": result})
    if method == "turn/start":
        text = " ".join(item.get("text", "") for item in params.get("input", []))
        threading.Thread(target=turn, args=(text, state["turn"]), daemon=True).start()
    elif method == "turn/interrupt":
        notice("turn/aborted", {"turn": {"id": "t" + str(state["turn"])}})
