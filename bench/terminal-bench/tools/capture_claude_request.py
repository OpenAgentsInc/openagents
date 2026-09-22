"""Record the requests Claude Code would send, without sending any.

Point Claude Code at this server with ANTHROPIC_BASE_URL. It writes each
request's path and body to <out>/req-NN.json and answers every call with an
error, so the CLI stops after its first attempt. It never records headers,
and it forwards nothing, so run the CLI with a dummy credential:

    python3 tools/capture_claude_request.py OUT &
    env -i HOME="$(mktemp -d)" PATH="$PATH" \
      ANTHROPIC_BASE_URL=http://127.0.0.1:8765 \
      CLAUDE_CODE_OAUTH_TOKEN=sk-ant-oat01-dummy \
      claude -p --model claude-opus-5-5 --permission-mode bypassPermissions \
      --tools Bash,Read,Edit,Write,Glob,Grep --effort low < briefing.md
"""

import http.server
import json
import os
import sys

OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)


class Capture(http.server.BaseHTTPRequestHandler):
    count = 0

    def do_POST(self) -> None:
        body = self.rfile.read(int(self.headers.get("content-length", 0)))
        Capture.count += 1
        with open(os.path.join(OUT, f"req-{Capture.count:02d}.json"), "wb") as out:
            out.write(self.path.encode() + b"\n" + body)
        self.send_response(400)
        self.send_header("content-type", "application/json")
        self.end_headers()
        error = {"type": "invalid_request_error", "message": "capture only"}
        self.wfile.write(json.dumps({"type": "error", "error": error}).encode())

    do_GET = do_POST

    def log_message(self, *_: object) -> None:
        pass


http.server.HTTPServer(("127.0.0.1", 8765), Capture).serve_forever()
