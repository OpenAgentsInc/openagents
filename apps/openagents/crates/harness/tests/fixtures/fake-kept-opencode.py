#!/usr/bin/env python3
"""Fake `opencode serve` (the 1.x wire) for keeper tests
(crates/harness/tests/keeper_drivers.rs).

Serves HTTP Basic-authenticated routes and the /global/event bus on --port.
A prompt whose text carries `chunks=N` streams N slow text deltas into one
assistant message; any other prompt answers `heard <text>`. Each chunk is
also logged on stdout, which the keeper journals. The session's messages
(GET /session/{id}/message) hold what has streamed so far.
"""
import base64
import json
import os
import re
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from queue import Queue

port = int(sys.argv[sys.argv.index("--port") + 1])
password = os.environ["OPENCODE_SERVER_PASSWORD"]
auth = "Basic " + base64.b64encode(f"opencode:{password}".encode()).decode()
session = "ses_kept"
lock = threading.Lock()
subscribers = []
messages = []
busy = False
count = 0


def publish(payload):
    with lock:
        for queue in subscribers:
            queue.put(payload)


def status(kind):
    publish({"type": "session.status",
             "properties": {"sessionID": session, "status": {"type": kind}}})


def turn(text):
    global busy, count
    with lock:
        count += 1
        user = {"info": {"id": f"msg_u{count}", "sessionID": session, "role": "user"},
                "parts": [{"id": f"prt_u{count}", "messageID": f"msg_u{count}",
                           "sessionID": session, "type": "text", "text": text}]}
        info = {"id": f"msg_a{count}", "sessionID": session, "role": "assistant"}
        part = {"id": f"prt_a{count}", "messageID": info["id"], "sessionID": session,
                "type": "text", "text": ""}
        messages.extend([user, {"info": info, "parts": [part]}])
    status("busy")
    publish({"type": "message.updated", "properties": {"info": info}})
    found = re.search(r"chunks=(\d+)", text)
    pieces = [f"chunk {i}\n" for i in range(int(found.group(1)))] if found else [f"heard {text}\n"]
    for piece in pieces:
        with lock:
            part["text"] += piece
        publish({"type": "message.part.delta",
                 "properties": {"sessionID": session, "messageID": info["id"],
                                "partID": part["id"], "field": "text", "delta": piece}})
        print(f"streamed {piece.strip()}", flush=True)
        if found:
            time.sleep(0.03)
    with lock:
        snapshot = dict(part)
        busy = False
    publish({"type": "message.part.updated", "properties": {"part": snapshot}})
    status("idle")
    publish({"type": "session.idle", "properties": {"sessionID": session}})


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *args):
        pass

    def reply(self, code, body=None):
        data = b"" if body is None else json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def authorized(self):
        if self.headers.get("Authorization") == auth:
            return True
        self.reply(401, {"error": "unauthorized"})
        return False

    def do_GET(self):
        if not self.authorized():
            return
        path = self.path.split("?")[0]
        if path == "/global/health":
            self.reply(200, {"healthy": True, "version": "1.18.31"})
        elif path == "/command":
            self.reply(200, [])
        elif path == "/session/status":
            with lock:
                self.reply(200, {session: {"type": "busy"}} if busy else {})
        elif path == f"/session/{session}":
            self.reply(200, {"id": session})
        elif path == f"/session/{session}/message":
            with lock:
                self.reply(200, json.loads(json.dumps(messages)))
        elif path == "/global/event":
            queue = Queue()
            with lock:
                subscribers.append(queue)
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Cache-Control", "no-cache")
            self.end_headers()
            try:
                self.wfile.write(b'data: {"payload":{"type":"server.connected","properties":{}}}\n\n')
                self.wfile.flush()
                while True:
                    payload = queue.get()
                    self.wfile.write(f"data: {json.dumps({'payload': payload})}\n\n".encode())
                    self.wfile.flush()
            except OSError:
                with lock:
                    subscribers.remove(queue)
        else:
            self.reply(404, {"error": "not found"})

    def do_POST(self):
        global busy
        if not self.authorized():
            return
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)) or 0)
        path = self.path.split("?")[0]
        if path == "/session":
            self.reply(200, {"id": session})
        elif path == f"/session/{session}/prompt_async":
            text = json.loads(body)["parts"][0]["text"]
            with lock:
                if busy:
                    sys.stderr.write("prompt while busy\n")
                    os._exit(2)
                busy = True
            self.send_response(204)
            self.send_header("Content-Length", "0")
            self.end_headers()
            threading.Thread(target=turn, args=(text,), daemon=True).start()
        elif path == f"/session/{session}/abort":
            self.reply(200, True)
        else:
            self.reply(404, {"error": "not found"})


ThreadingHTTPServer.daemon_threads = True
ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
