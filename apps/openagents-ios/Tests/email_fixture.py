# Firm reason: 2 (independent HTTP oracle for the unchanged native auth/OTA services).
"""Serve a signed fixture package plus fake mail/auth; never logs credentials."""
import argparse
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("directory", type=Path)
p.add_argument("--port", type=int, default=13491)
a = p.parse_args()
manifest = json.loads((a.directory / "manifest.json").read_text())
objects = {f["sha256"]: a.directory / "Starter" / f["path"] for f in manifest["files"]}
threads = {}
counts = dict(sent=0, wrong=0, verified=0, authorized=0, logout=0, device_bound=0)

class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def answer(self, status, value):
        body = value if isinstance(value, bytes) else json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        if self.path == "/latest.cnu":
            return self.answer(200, (a.directory / "latest.cnu").read_bytes())
        if self.path.startswith("/objects/"):
            obj = objects.get(self.path.removeprefix("/objects/"))
            return self.answer(200, obj.read_bytes()) if obj else self.answer(404, {})
        if self.path.startswith("/api/chat/thread/v1?id="):
            if self.headers.get("Authorization") != "Bearer oa_at_fixture":
                return self.answer(401, {})
            record = threads.get(self.path.split("=", 1)[1])
            return self.answer(200, record) if record else self.answer(404, {})
        if self.path == "/api/chat/history/v1":
            if self.headers.get("Authorization") != "Bearer oa_at_fixture":
                return self.answer(401, {"error": "Log in first."})
            counts["authorized"] += 1
            return self.answer(200, {"schema": "openagents.chat.history.v1", "chats": [dict(id=k, title=v["messages"][0]["content"][:80], model="google/gemini-4-hydrogen", folder="", updated_ms=3, busy=False) for k,v in threads.items()] + [
                dict(id="42", title="My first saved chat", model="m", folder="openagents", updated_ms=2, busy=False),
                dict(id="41", title="Another saved chat", model="m", folder="openagents", updated_ms=1, busy=False)]})
        if self.path == "/report":
            return self.answer(200, counts)
        return self.answer(404, {})

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
        if self.path in ("/api/chat/thread/v1", "/api/chat/complete/v1"):
            if self.headers.get("Authorization") != "Bearer oa_at_fixture":
                return self.answer(401, {})
            if self.path.endswith("thread/v1"):
                key = body.get("id") or "m_01234567890123456789"
                current = threads.get(key, {}).get("revision", 0)
                if body["revision"] != current:
                    return self.answer(409, {})
                body.update(id=key, revision=current+1)
                threads[key] = body
                return self.answer(200, body)
            return self.answer(200, {"choices":[{"message":{"role":"assistant","content":"Hello from the account agent."},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":6}})
        if self.path == "/api/auth/email/start":
            if body != dict(email="ada@example.com", client="app"):
                return self.answer(400, {"error": "Unexpected email request"})
            counts["sent"] += 1
            return self.answer(200, {"pending": "fixture-challenge", "state": "sent"})
        if self.path == "/api/auth/email/verify":
            if body.get("pending") != "fixture-challenge":
                return self.answer(409, {"error": "Ask for a new code."})
            if body.get("code") != "415926":
                counts["wrong"] += 1
                return self.answer(401, {"error": "That code isn't right. 4 tries left."})
            if not self.headers.get("OA-Device-Key"):
                return self.answer(400, {"error": "Device key missing"})
            counts["verified"] += 1
            counts["device_bound"] += 1
            return self.answer(200, dict(access_token="oa_at_fixture", refresh_token="oa_rt_fixture", account="acct_fixture", expires_in=3600, device_bound=True))
        if self.path == "/api/auth/logout":
            counts["logout"] += 1
            return self.answer(200, {"state": "logged-out"})
        return self.answer(404, {})

ThreadingHTTPServer(("127.0.0.1", a.port), Handler).serve_forever()
