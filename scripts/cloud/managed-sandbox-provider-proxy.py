#!/usr/bin/env python3
"""Narrow guest-to-Worker provider-capability relay for managed sandboxes."""

from __future__ import annotations

import http.server
import os
import urllib.error
import urllib.request


MAX_BODY_BYTES = 2 * 1024 * 1024
PATHS = {
    "/openai/v1/responses": "/api/internal/managed-sandbox/providers/openai/v1/responses",
    "/anthropic/v1/messages": "/api/internal/managed-sandbox/providers/anthropic/v1/messages",
}


class Handler(http.server.BaseHTTPRequestHandler):
    server_version = "oa-managed-sandbox-provider-proxy/1"

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def _reply(self, status: int, body: bytes) -> None:
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("cache-control", "no-store")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self) -> None:  # noqa: N802
        upstream_path = PATHS.get(self.path.split("?", 1)[0])
        if upstream_path is None:
            self._reply(404, b'{"error":"not_found"}')
            return
        try:
            length = int(self.headers.get("content-length", "0"))
        except ValueError:
            length = 0
        if length <= 0 or length > MAX_BODY_BYTES:
            self._reply(400, b'{"error":"request_out_of_bounds"}')
            return
        capability = self.headers.get("x-api-key")
        authorization = self.headers.get("authorization", "")
        if not capability and authorization.startswith("Bearer "):
            capability = authorization[len("Bearer ") :]
        if not capability or len(capability) > 16384 or any(char.isspace() for char in capability):
            self._reply(401, b'{"error":"unauthorized"}')
            return
        body = self.rfile.read(length)
        base_url = os.environ.get("OA_MANAGED_SANDBOX_PROVIDER_BROKER_URL", "").rstrip("/")
        if not base_url.startswith("https://"):
            self._reply(503, b'{"error":"broker_not_armed"}')
            return
        request = urllib.request.Request(
            f"{base_url}{upstream_path}",
            data=body,
            method="POST",
            headers={
                "authorization": f"Bearer {capability}",
                "content-type": "application/json",
            },
        )
        try:
            with urllib.request.urlopen(request, timeout=600) as response:
                response_body = response.read()
                self.send_response(response.status)
                self.send_header(
                    "content-type",
                    response.headers.get("content-type", "application/json"),
                )
                self.send_header("cache-control", "no-store")
                self.send_header("content-length", str(len(response_body)))
                request_id = response.headers.get("x-request-id")
                if request_id:
                    self.send_header("x-request-id", request_id)
                self.end_headers()
                self.wfile.write(response_body)
        except urllib.error.HTTPError as error:
            response_body = error.read()
            self.send_response(error.code)
            self.send_header(
                "content-type",
                error.headers.get("content-type", "application/json"),
            )
            self.send_header("cache-control", "no-store")
            self.send_header("content-length", str(len(response_body)))
            self.end_headers()
            self.wfile.write(response_body)
        except Exception:
            self._reply(502, b'{"error":"broker_unavailable"}')


def main() -> None:
    port = int(os.environ.get("OA_MANAGED_SANDBOX_PROVIDER_BROKER_PORT", "8790"))
    server = http.server.ThreadingHTTPServer(("0.0.0.0", port), Handler)
    server.serve_forever()


if __name__ == "__main__":
    main()
