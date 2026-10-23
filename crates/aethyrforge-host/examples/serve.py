#!/usr/bin/env python3
"""Local conformance server. Never serve private output from this directory."""
import argparse
import functools
import http.server
import html
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("directory")
parser.add_argument("--port", type=int, default=0)
parser.add_argument("--host-origin", default="http://127.0.0.1:55306")
args = parser.parse_args()

class Handler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path in ["/mock.html", *[f"/forged-{v}.html" for v in ["session", "version", "payload", "bytes", "object"]]]:
            content = (Path(args.directory) / self.path.lstrip("/")).read_text().replace('content="http://127.0.0.1:55306"', 'content="' + html.escape(args.host_origin, quote=True) + '"').encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(content)))
            self.end_headers()
            self.wfile.write(content)
        else:
            super().do_GET()

    def end_headers(self):
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

server = http.server.ThreadingHTTPServer(("127.0.0.1", args.port), functools.partial(Handler, directory=args.directory))
print(f"http://127.0.0.1:{server.server_port}", flush=True)
server.serve_forever()
