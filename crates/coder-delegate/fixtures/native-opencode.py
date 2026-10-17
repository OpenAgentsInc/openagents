#!/usr/bin/env python3
"""Local OpenCode HTTP fixture. It never calls a model or reads a login."""
import http.server
import json
import os
import queue
import subprocess
import sys
import threading
import time
import urllib.parse

SESSION = "ses_0standin00000000000000001"
subscribers = []
status = "idle"
counter = 0


def publish(kind, properties):
    frame = json.dumps({"type": kind, "properties": properties})
    for subscriber in list(subscribers):
        subscriber.put(frame)


def turn(text):
    global status, counter
    counter += 1
    number = counter
    what = "steer" if "steer" in text else "resume" if "resume" in text else "briefing"
    status = "busy"
    publish("session.status", {"sessionID": SESSION, "status": {"type": "busy"}})
    time.sleep(float(os.environ.get("STANDIN_DELAY", "0")))
    if os.environ.get("STANDIN_REFUSAL"):
        code = int(os.environ["STANDIN_REFUSAL"])
        publish("session.error", {"sessionID": SESSION, "error": {"name": "APIError", "data": {"statusCode": code, "message": "Fixture provider refusal", "responseHeaders": {"Retry-After": "60"}}}})
        status = "idle"
        publish("session.idle", {"sessionID": SESSION})
        return
    if os.environ.get("STANDIN_HANG") and what == "briefing":
        subprocess.Popen(["/bin/sh", "-c", 'sleep 2; printf harmless > "$STANDIN_HANG"'], start_new_session=True)
        publish("message.updated", {"info": {"id": "m0", "sessionID": SESSION, "role": "assistant"}})
        publish("message.part.updated", {"part": {"id": "hang", "messageID": "m0", "sessionID": SESSION, "type": "text", "text": "hanging"}})
        threading.Event().wait()
    publish("message.updated", {"info": {"id": "m" + str(number), "sessionID": SESSION, "role": "assistant", "tokens": {"input": 20, "output": 5, "cache": {"read": 4}}, "cost": 0.003}})
    publish("message.part.updated", {"part": {"id": "tool" + str(number), "messageID": "m" + str(number), "sessionID": SESSION, "type": "tool", "tool": "bash", "callID": "c" + str(number), "state": {"status": "running", "input": {"command": "false"}}}})
    publish("message.part.updated", {"part": {"id": "tool" + str(number), "messageID": "m" + str(number), "sessionID": SESSION, "type": "tool", "tool": "bash", "callID": "c" + str(number), "state": {"status": "completed", "input": {"command": "false"}, "metadata": {"exit": 1}, "output": ""}}})
    publish("message.part.updated", {"part": {"id": "text" + str(number), "messageID": "m" + str(number), "sessionID": SESSION, "type": "text", "text": "heard " + what}})
    time.sleep(float(os.environ.get("STANDIN_DELAY", "0")))
    status = "idle"
    publish("session.idle", {"sessionID": SESSION})


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def reply(self, body):
        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        path = urllib.parse.urlparse(self.path).path
        if path == "/global/event":
            subscriber = queue.Queue()
            subscribers.append(subscriber)
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            self.wfile.write(b'data: {"type":"server.connected","properties":{}}\n\n')
            self.wfile.flush()
            try:
                while True:
                    self.wfile.write(("data: " + subscriber.get() + "\n\n").encode())
                    self.wfile.flush()
            except (BrokenPipeError, ConnectionResetError):
                subscribers.remove(subscriber)
        elif path == "/global/health":
            self.reply({"healthy": True, "version": "1.18.26"})
        elif path == "/provider":
            self.reply({"all": []})
        elif path == "/command":
            self.reply([])
        elif path == "/session/status":
            self.reply({SESSION: {"type": status}} if status != "idle" else {})
        elif path.endswith("/message"):
            self.reply([])
        else:
            self.reply({"id": SESSION})

    def do_POST(self):
        global status
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))) or "{}")
        path = urllib.parse.urlparse(self.path).path
        self.reply({"id": SESSION} if path == "/session" else {})
        if path.endswith("/prompt_async"):
            text = " ".join(part.get("text", "") for part in body.get("parts", []))
            threading.Thread(target=turn, args=(text,), daemon=True).start()
        elif path.endswith("/abort"):
            status = "idle"
            publish("session.idle", {"sessionID": SESSION})


if "--version" in sys.argv:
    print("1.18.26")
else:
    port = int(sys.argv[sys.argv.index("--port") + 1])
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler)
    server.daemon_threads = True
    server.serve_forever()
