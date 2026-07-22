#!/usr/bin/env python3
"""Forward local managed-sandbox proof requests with a host-held token."""

from __future__ import annotations

import argparse
import http.client
import http.server
import pathlib
import urllib.parse


class Proxy(http.server.BaseHTTPRequestHandler):
    token: str
    upstream: str

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def do_GET(self) -> None:  # noqa: N802
        self._forward()

    def do_POST(self) -> None:  # noqa: N802
        self._forward()

    def _forward(self) -> None:
        size = int(self.headers.get("content-length", "0"))
        body = self.rfile.read(size) if size else None
        upstream = urllib.parse.urlsplit(self.upstream)
        connection = http.client.HTTPConnection(upstream.hostname, upstream.port, timeout=180)
        connection.request(
            self.command,
            self.path,
            body=body,
            headers={
                "accept": "application/json",
                "cache-control": "no-store",
                "content-type": self.headers.get("content-type", "application/json"),
                "authorization": f"Bearer {self.token}",
            },
        )
        response = connection.getresponse()
        payload = response.read()
        status = response.status
        content_type = response.getheader("content-type", "application/json")
        connection.close()
        self.send_response(status)
        self.send_header("content-type", content_type)
        self.send_header("content-length", str(len(payload)))
        self.send_header("cache-control", "no-store")
        self.end_headers()
        self.wfile.write(payload)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--token-file", required=True)
    parser.add_argument("--listen-port", type=int, default=18788)
    parser.add_argument("--upstream", default="http://127.0.0.1:8787")
    args = parser.parse_args()
    token = pathlib.Path(args.token_file).read_text(encoding="utf-8").strip()
    if not token:
        raise RuntimeError("the token file is empty")
    Proxy.token = token
    Proxy.upstream = args.upstream.rstrip("/")
    server = http.server.ThreadingHTTPServer(("127.0.0.1", args.listen_port), Proxy)
    server.serve_forever()


if __name__ == "__main__":
    main()
