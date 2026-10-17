#!/usr/bin/env python3
"""Check the phone's cloud controls against a disposable loopback service."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[2]


class Fixture(BaseHTTPRequestHandler):
    running = False
    calls = []

    def log_message(self, *_):
        pass

    def reply(self, body):
        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def host(self):
        self.reply({"computer": {"id": "cmp_fixture", "state": "turn" if self.running else "stopped"},
                    "engine": "running" if self.running else "stopped", "device_name": "Our cloud",
                    "started_unix": 1 if self.running else None, "until_unix": 4102444800 if self.running else None})

    def do_GET(self):
        path = urlparse(self.path).path
        if path == "/credential":
            self.reply({"credential": "oa_sk_" + "1" * 64})
        elif path == "/v1/account":
            self.reply({"account": {"id": "acct_fixture", "label": "Cloud controls fixture"}, "workspaces": []})
        elif path == "/v1/cloud-host":
            self.host()
        elif path in ("/v1/history", "/v1/history/projects"):
            self.reply({"data": [], "next": None})
        else:
            self.send_error(404)

    def do_POST(self):
        path = urlparse(self.path).path
        self.rfile.read(int(self.headers.get("Content-Length", 0)))
        if path in ("/v1/cloud-host", "/v1/cloud-host/stop"):
            Fixture.calls.append(path)
            Fixture.running = path == "/v1/cloud-host"
            self.host()
        elif path == "/v1/device/sign-out":
            self.reply({"signed_out": True})
        else:
            self.send_error(404)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--scratch", type=Path, required=True)
    parser.add_argument("--simulator", required=True, help="A disposable simulator ID")
    args = parser.parse_args()
    target = args.target_dir.expanduser().resolve()
    if target.is_relative_to(ROOT):
        parser.error("Use an external Cargo target directory.")
    args.scratch.mkdir(parents=True, exist_ok=True)
    server = ThreadingHTTPServer(("127.0.0.1", 0), Fixture)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    env = dict(os.environ, OPENAGENTS_SKIP_CORE="1", OPENAGENTS_ACCOUNT_FIXTURE="1",
               CARGO_TARGET_DIR=str(target), TEST_RUNNER_OUR_CLOUD_FIXTURE_URL=f"http://127.0.0.1:{server.server_port}")
    try:
        with (args.scratch / "cloud-controls-uitest.log").open("w") as log:
            subprocess.run(["xcodebuild", "test", "-project", "apps/ios/OpenAgents.xcodeproj", "-scheme", "OpenAgents",
                            "-configuration", "Debug", "-parallel-testing-enabled", "NO", "-destination", "id=" + args.simulator,
                            "-derivedDataPath", str(target / "ios-derived"), "CODE_SIGNING_ALLOWED=YES", "CODE_SIGN_IDENTITY=-",
                            "ARCHS=arm64", "SWIFT_ACTIVE_COMPILATION_CONDITIONS=DEBUG CODER_ACCOUNT_FIXTURE",
                            "-only-testing:OpenAgentsUITests/OurCloudLiveTests/testLocalCloudControls"],
                           cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        assert Fixture.calls == ["/v1/cloud-host", "/v1/cloud-host/stop"], Fixture.calls
        assert not Fixture.running
        print("PASS: phone cloud status, start, stop, and sign-out; one request per action.")
    finally:
        server.shutdown()


if __name__ == "__main__":
    main()
