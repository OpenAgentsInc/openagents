#!/usr/bin/env python3
"""A minimal backend for the fresh-install verification.

Publishes one model card on `GET /v1/models` — the digest the gateway's
registry binding must pin — and answers `POST /v1/systemone` with a
typed `noul` answer. It exists so `scripts/verify-gateway-install.sh`
can prove a real bounded call without model weights. It is not a door
you deploy.
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

DIGEST = sys.argv[2] if len(sys.argv) > 2 else "sha256:" + "0" * 64


class Backend(BaseHTTPRequestHandler):
    def _send(self, status, body):
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path == "/v1/models":
            self._send(200, {
                "models": [{
                    "id": "kev-stub",
                    "name": "kev-stub",
                    "artifact_identity": {"digest": DIGEST},
                    "execution": {},
                }]
            })
        else:
            self._send(404, {"error": {"code": "not_found", "message": self.path}})

    def do_POST(self):
        length = int(self.headers.get("content-length", 0))
        body = json.loads(self.rfile.read(length) or b"{}")
        if self.path == "/v1/systemone":
            questions = body.get("questions", {})
            answers = {
                name: {"type": "noul", "answer": True, "probability": 0.99}
                for name in questions
            }
            self._send(200, {"answers": answers})
        else:
            self._send(404, {"error": {"code": "not_found", "message": self.path}})

    def log_message(self, *args):
        pass


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 19080
    HTTPServer(("127.0.0.1", port), Backend).serve_forever()
