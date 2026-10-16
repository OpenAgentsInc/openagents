#!/usr/bin/env python3
"""Run the phone's account-history controls against disposable local records."""
import argparse
import struct
import zlib
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

def fixture_png():
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xffffffff)
    pixels = b"".join(b"\0" + bytes([40, 160, 190, 255]) * 128 for _ in range(64))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 128, 64, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b"")

PNG = fixture_png()

class Handler(BaseHTTPRequestHandler):
    scratch = None
    credential = "oa_sk_" + "1" * 64
    live = False
    rich = False
    projects = False
    joined = False
    revision = 1
    offline = False
    def log_message(self, *_):
        pass

    def do_GET(self):
        path = urlparse(self.path)
        query = parse_qs(path.query)
        result = {}
        media = "application/json"
        if path.path == "/join-project":
            Handler.joined = True
            result = {"ok": True}
        elif path.path in ("/reply-ready", "/web-seen"):
            assert Handler.scratch is not None
            (Handler.scratch / ("reply-done" if path.path == "/reply-ready" else "web-seen")).write_text("ready")
        elif path.path == "/advance":
            Handler.revision += 1
            result = {"ok": True}
        elif path.path == "/offline":
            Handler.offline = True
        elif path.path == "/online":
            Handler.offline = False
        elif path.path == "/fixture-key":
            result = {"credential": self.credential}
        elif path.path == "/v1/account":
            result = {"account": {"id": "acct_fixture", "label": "History fixture"}, "workspaces": []}
        elif path.path == "/v1/history/projects":
            rows = [{"id": "prj_0000000000000001", "name": "Shared project", "shared": True}]
            if Handler.projects:
                rows.append({"id": "prj_0000000000000002", "name": "Empty project", "shared": True})
            if Handler.projects and Handler.joined:
                rows.append({"id": "prj_0000000000000003", "name": "New shared project", "shared": True})
            result = {"data": rows, "next": None}
        elif path.path == "/v1/history":
            start = int(query["after"][0][:8], 16) + 1 if "after" in query else 0
            count = 205
            if Handler.projects:
                count = 2 if query.get("project") == ["prj_0000000000000001"] else 0
            end = min(start + int(query.get("limit", [200])[0]), count)
            rows = [{"id": chat(n), "title": f"Saved chat {n}", "revision": Handler.revision, "updated_unix": n+1,
                     "pinned_unix": 1 if n == 204 else None, "archived_unix": 1 if n == 5 else None,
                     "project": None, "source": ["web", "terminal", "phone"][n % 3], "messages": 5 if Handler.rich else 2 * Handler.revision if Handler.live else 405} for n in range(start, end)]
            result = {"data": rows, "next": chat(end-1) if end < count else None}
        elif path.path.endswith("/messages"):
            if Handler.offline:
                self.send_error(503); return
            start = int(query["after"][0][4:]) + 1 if "after" in query else 0
            count = 5 if Handler.rich else 2 * Handler.revision if Handler.live else 405
            end = min(start + int(query.get("limit", [200])[0]), count)
            rows = []
            for n in range(start, end):
                row = {"id": f"msg_{n}", "role": "user" if n == 0 else "thinking" if n == 1 else "tool" if n == 2 else "assistant",
                       "text": "# Saved question\n\n**Markdown** and `code`.\n\n```rust\nlet answer = 42;\n```" if n == 0 else "Considering the earlier question." if n == 1 else "# Saved answer\n\n**Markdown** and `inline code`.\n\n```rust\nlet answer = 42;\n```" if n == 3 else f"Saved reply {n}", "files": []}
                if n == 0:
                    row["files"] = [{"id": "0123456789abcdef0123456789abcdef", "name": "fixture.txt", "kind": "text", "size": 21},
                                    {"id": "11111111111111111111111111111111", "name": "fixture.png", "kind": "png", "size": len(PNG)}]
                if n == 2:
                    row["tool"] = {"name": "read", "summary": "Read a file", "status": "done", "input": "fixture.txt", "output": "Fixture file contents"}
                rows.append(row)
            result = {"thread": {"id": path.path.split("/")[3], "title": "Saved chat", "revision": Handler.revision, "updated_unix": 1, "pinned_unix": 1,
                                 "archived_unix": None, "project": None, "source": "web", "messages": count},
                      "revision": Handler.revision, "data": rows, "next": f"msg_{end-1}" if end < count else None, "captures": []}
        elif "/files/" in path.path:
            if path.path.endswith("11111111111111111111111111111111"):
                result = PNG; media = "image/png"
            else:
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
    parser.add_argument("--public-chat", action="store_true", help="Continue the selected disposable cloud chat")
    parser.add_argument("--public-history", action="store_true", help="Use only the dedicated-account fixtures from sdk-history.py")
    parser.add_argument("--live", action="store_true")
    parser.add_argument("--rich", action="store_true")
    parser.add_argument("--projects", action="store_true")
    args = parser.parse_args()
    Handler.scratch = args.scratch
    Handler.live = args.live
    Handler.rich = args.rich
    Handler.projects = args.projects
    if args.public_history or args.public_chat:
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
    if args.public_chat:
        env["TEST_RUNNER_CODER_PUBLIC_CHAT_CONFIG"] = json.dumps({"key_url": f"http://127.0.0.1:{server.server_port}/fixture-key", "chat": chat_ids[0], "prompt": os.environ["SDK_CHAT_PROMPT"], "messages": os.environ["SDK_CHAT_MESSAGES"]})
    test = "testSharedProjectsWithoutComputer" if args.projects else "testPublicCloudChat" if args.public_chat else "testPublicSavedAccountHistory" if args.public_history else "testRichAccountHistory" if args.rich else "testLiveAccountHistory" if args.live else "testSavedAccountHistory"
    try:
        with (args.scratch / "history-uitest.log").open("w") as log:
            subprocess.run(["xcodebuild", "test", "-project", "apps/ios/Zeron.xcodeproj", "-scheme", "Zeron", "-configuration", "Debug",
                            "-destination", "id=" + args.simulator, "-derivedDataPath", str(args.target_dir / "ios-derived"),
                            "CODE_SIGNING_ALLOWED=YES", "CODE_SIGN_IDENTITY=-", "ARCHS=arm64", "SWIFT_ACTIVE_COMPILATION_CONDITIONS=DEBUG CODER_ACCOUNT_FIXTURE",
                            "-only-testing:ZeronUITests/OpenAgentsHistoryTests/" + test], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        print("Phone shared projects: Home rows, empty project, and shared chat without a computer passed." if args.projects else
              "Public phone cloud reply and sign-out passed." if args.public_chat else "Public phone history: complete 205-message read and sign-out passed." if args.public_history else
              "Phone account history: paging, full text, file, archives, projects, and sign-out passed.")
    finally:
        server.shutdown()

if __name__ == "__main__":
    main()
