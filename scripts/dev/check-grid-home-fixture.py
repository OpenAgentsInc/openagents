#!/usr/bin/env python3
"""Check /home against local chat replies, without writing to the upstream server."""

import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import secrets
import subprocess
import threading
import time
from urllib.parse import parse_qs, urlsplit
import urllib.request

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--upstream", default="http://localhost:4331")
parser.add_argument("--out", required=True, type=Path)
parser.add_argument("--check-script", type=Path, default=Path(__file__).with_name("check-grid-home.py"))
parser.add_argument("--browser")
parser.add_argument("--refuse-followup", action="store_true", help="Inject an auth error to check that the page never navigates automatically.")
parser.add_argument("--webgl", action="store_true", help="Use Verse's WebGL backend.")
parser.add_argument("--allow-context-loss", action="store_true", help="Input/API diagnosis on a software GPU only.")
args = parser.parse_args()
assert urlsplit(args.upstream).hostname in ("localhost", "127.0.0.1", "::1"), "Use a local preview server."
args.out.mkdir(parents=True, exist_ok=True)
csrf = secrets.token_urlsafe(24)
requests = []


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def reply(self, data, content_type="application/json", status=200, headers=()):
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        for name, value in headers:
            self.send_header(name, value)
        self.end_headers()
        try:
            self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def do_POST(self):
        # Every write ends here. No request body or account cookie goes upstream.
        if self.path not in ("/v1/threads", "/v1/threads/fixture-chat/messages"):
            self.send_error(404)
            return
        if self.headers.get("X-OpenAgents-Csrf") != csrf:
            self.send_error(403)
            return
        if self.headers.get("X-OpenAgents-Chat-Surface") != "home":
            self.send_error(400, "Home chat surface required")
            return
        length = int(self.headers.get("Content-Length", "0"))
        if not 0 < length <= 64 * 1024:
            self.send_error(400)
            return
        message = json.loads(self.rfile.read(length))
        requests.append(message)
        if args.refuse_followup and self.path.endswith("/messages"):
            self.reply(b'{"error":{"code":"sign_in_required","message":"Log in to keep chatting."}}', status=401)
        else:
            self.reply(b'{"id":"fixture-chat","answering":true}', status=201)

    def do_GET(self):
        path = urlsplit(self.path)
        if path.path in ("/home/me", "/lander8/session"):
            data = {"signed_in": False}
            if path.path == "/lander8/session":
                data["csrf"] = csrf
            self.reply(json.dumps(data).encode())
        elif path.path == "/pylons/live":
            now = int(time.time())
            pylon = dict(id="fixture-pylon", name="Test pylon", slug="fixture-pylon", npub="npub1test",
                         status="online", family="gpu", memory_gb=24, busy=0, total=2, since=now-60,
                         observed_at=now, expires_at=now+300, services=[], free=True, pools=[])
            data = json.dumps(dict(now=now, synced=True, pylons=[pylon]))
            self.reply(f"event: pylons\ndata: {data}\n\n".encode(), "text/event-stream")
        elif path.path == "/v1/threads/fixture-chat/events":
            request = parse_qs(path.query).get("request_id", [""])[0]
            if not any(r.get("request_id") == request for r in requests):
                self.send_error(404)
                return
            events = [
                ("text", dict(type="text", request_id=request, index=1, role="assistant",
                              text="Hello from the pylons. ", replace=False)),
                ("text", dict(type="text", request_id=request, index=1, role="assistant",
                              text="This is the fixture reply.", replace=False)),
                ("done", dict(type="done", request_id=request, outcome="complete", revision=1)),
            ]
            chunks = [f"event: {name}\ndata: {json.dumps(body)}\n\n".encode() for name, body in events]
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(sum(map(len, chunks))))
            self.end_headers()
            for chunk in chunks:
                self.wfile.write(chunk)
                self.wfile.flush()
                time.sleep(.15)
        elif path.path == "/home" or path.path.startswith(("/everglade/", "/lander8/build/", "/pylons/layer/", "/pylons/card/")):
            # Read only public page/assets; never forward cookies or authorization.
            request = urllib.request.Request(args.upstream.rstrip("/") + self.path,
                                             headers={"Accept-Encoding": "identity"})
            try:
                with urllib.request.urlopen(request, timeout=30) as response:
                    headers = [(name, value) for name, value in response.headers.items() if name.lower() in (
                        "content-security-policy", "cross-origin-opener-policy", "cross-origin-embedder-policy")]
                    self.reply(response.read(), response.headers.get("Content-Type", "application/octet-stream"), headers=headers)
            except Exception as error:
                self.send_error(502, str(error))
        else:
            self.send_error(404)


server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
url = f"http://127.0.0.1:{server.server_port}/home" + ("?gl" if args.webgl else "")
command = ["openagents", "browser", "run"]
if args.browser:
    command += ["--browser", args.browser]
command += ["--", "python3", str(args.check_script), "--url", url, "--out", str(args.out),
            "--movement", "--chat", "--timeout", "180"]
if args.refuse_followup:
    command += ["--auth-refusal"]
if args.allow_context_loss:
    command += ["--allow-context-loss"]
print(f"Fixture at {url}; page and assets from {args.upstream}; chat replies stay here.", flush=True)
try:
    result = subprocess.run(command)
    raise SystemExit(result.returncode)
finally:
    (args.out / "fixture-requests.json").write_text(json.dumps(requests, indent=2) + "\n")
    server.shutdown()
    server.server_close()
