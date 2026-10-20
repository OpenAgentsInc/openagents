#!/usr/bin/env python3
"""Isolated model-server fixture; all controls and records live beside -m."""

import argparse
import fcntl
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import time

args = argparse.ArgumentParser()
args.add_argument("-m", type=Path, required=True)
args.add_argument("--port", type=int, required=True)
options, _ = args.parse_known_args()
root = options.m.parent


def record(event, **data):
    with (root / "events.jsonl").open("a") as output:
        output.write(json.dumps({"event": event, "pid": os.getpid(), **data}) + "\n")


record("spawn")
if (root / "die-on-start").exists():
    raise SystemExit(2)


class Model(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def send_json(self, status, data):
        data = json.dumps(data).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        self.send_json(503 if (root / "never-ready").exists() else 200,
                       {"data": [{"id": options.m.name}]})

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        with (root / "generation.lock").open("a") as guard:
            try:
                fcntl.flock(guard, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                record("OVERLAP")
                raise
            record("generation", request=request)
            if (root / "fork").exists():
                descendant = subprocess.Popen(["sleep", "600"])
                record("descendant", child=descendant.pid)
            if (root / "drop-response").exists():
                self.send_response(200)
                self.send_header("Content-Length", "100")
                self.end_headers()
                self.wfile.write(b"{")
                self.close_connection = True
                return
            if (root / "hang").exists():
                while True:
                    time.sleep(1)
            self.send_json(200, {"choices": [{"finish_reason": "length" if request["max_tokens"] == 1 or (root / "truncate").exists() else "stop",
                            "message": {"content": "\n\n2\n", "reasoning_content": "work"}}],
                            "usage": {"prompt_tokens": 4, "completion_tokens": 1}})
            record("finished")


ThreadingHTTPServer(("127.0.0.1", options.port), Model).serve_forever()
