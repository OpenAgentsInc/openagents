#!/usr/bin/env python3
"""Run the phone's account-history controls against disposable local records."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

ROOT = Path(__file__).resolve().parents[2]

def chat(n):
    return f"{n:08x}-1234-4234-8234-123456789abc"

class Handler(BaseHTTPRequestHandler):
    credential = "oa_sk_" + "1" * 64
    def log_message(self, *_):
        pass

    def do_GET(self):
        path = urlparse(self.path)
        query = parse_qs(path.query)
        result = {}
        media = "application/json"
        if path.path == "/fixture-key":
            result = {"credential": self.credential}
        elif path.path == "/v1/account":
            result = {"account": {"id": "acct_fixture", "label": "History fixture"}, "workspaces": []}
        elif path.path == "/v1/history/projects":
            result = {"data": [{"id": "prj_0000000000000001", "name": "Shared project", "shared": True}], "next": None}
        elif path.path == "/v1/history":
            start = int(query["after"][0][:8], 16) + 1 if "after" in query else 0
            end = min(start + int(query.get("limit", [200])[0]), 205)
            rows = [{"id": chat(n), "title": f"Saved chat {n}", "revision": 1, "updated_unix": n+1,
                     "pinned_unix": 1 if n == 204 else None, "archived_unix": 1 if n == 5 else None,
                     "project": None, "source": ["web", "terminal", "phone"][n % 3], "messages": 405} for n in range(start, end)]
            result = {"data": rows, "next": chat(end-1) if end < 205 else None}
        elif path.path.endswith("/messages"):
            start = int(query["after"][0][4:]) + 1 if "after" in query else 0
            end = min(start + int(query.get("limit", [200])[0]), 405)
            rows = [{"id": f"msg_{n}", "role": "user" if n == 0 else "assistant", "text": "Saved question — full text" if n == 0 else f"Saved reply {n}",
                     "files": [{"id": "0123456789abcdef0123456789abcdef", "name": "fixture.txt", "kind": "text", "size": 21}] if n == 0 else []} for n in range(start, end)]
            result = {"thread": {"id": chat(204), "title": "Saved chat 204", "revision": 1, "updated_unix": 1, "pinned_unix": 1,
                                 "archived_unix": None, "project": None, "source": "web", "messages": 405},
                      "revision": 1, "data": rows, "next": f"msg_{end-1}" if end < 405 else None, "captures": []}
        elif "/files/" in path.path:
            result = b"fixture file contents"; media = "text/plain"
        else:
            self.send_error(404); return
        data = result if isinstance(result, bytes) else json.dumps(result).encode()
        self.send_response(200); self.send_header("Content-Type", media); self.send_header("Content-Length", str(len(data))); self.end_headers(); self.wfile.write(data)

    def do_DELETE(self):
        self.send_response(204); self.end_headers()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--scratch", type=Path, required=True)
    parser.add_argument("--simulator", default="8010197D-517D-43DA-9559-46742EFABC65")
    parser.add_argument("--public-history", action="store_true", help="Use only the dedicated-account fixtures from sdk-history.py")
    args = parser.parse_args()
    if args.public_history:
        Handler.credential = os.environ["OPENAGENTS_TOKEN"]
        assert Handler.credential.startswith("oa_sk_")
        chat_ids = json.loads(os.environ["SDK_FIXTURE_CHAT_IDS"])
    args.scratch.mkdir(parents=True, exist_ok=True)
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    env = dict(os.environ, ZERON_SKIP_CORE="1", ZERON_ACCOUNT_FIXTURE="1", CARGO_TARGET_DIR=str(args.target_dir),
               TEST_RUNNER_CODER_HISTORY_FIXTURE=f"http://127.0.0.1:{server.server_port}")
    if args.public_history:
        env["TEST_RUNNER_CODER_PUBLIC_HISTORY_CONFIG"] = json.dumps({"key_url": f"http://127.0.0.1:{server.server_port}/fixture-key", "chat": chat_ids[0]})
    test = "testPublicSavedAccountHistory" if args.public_history else "testSavedAccountHistory"
    try:
        with (args.scratch / "history-uitest.log").open("w") as log:
            subprocess.run(["xcodebuild", "test", "-project", "apps/ios/Zeron.xcodeproj", "-scheme", "Zeron", "-configuration", "Debug",
                            "-destination", "id=" + args.simulator, "-derivedDataPath", str(args.target_dir / "ios-derived"),
                            "CODE_SIGNING_ALLOWED=YES", "CODE_SIGN_IDENTITY=-", "SWIFT_ACTIVE_COMPILATION_CONDITIONS=DEBUG CODER_ACCOUNT_FIXTURE",
                            "-only-testing:ZeronUITests/OpenAgentsHistoryTests/" + test], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        print("Public phone history: complete 205-message read and sign-out passed." if args.public_history else
              "Phone account history: paging, full text, file, archives, projects, and sign-out passed.")
    finally:
        server.shutdown()

if __name__ == "__main__":
    main()
