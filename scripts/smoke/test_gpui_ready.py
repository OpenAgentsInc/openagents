"""Exercise the startup gate in fresh Chrome against broken HTTP-success pages.

Run with OPENAGENTS_SCRATCH set. Requires openagents, Chrome, and websocket-client.
"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import unittest


CHECK = Path(__file__).with_name("gpui-ready.py")


class StartupGate(unittest.TestCase):
    def probe(self, page, expected, *flags):
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                self.send_response(200 if self.path == "/" else 500 if self.path.startswith("/gpui-fonts/") else 404)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                self.end_headers()
                self.wfile.write(page.encode() if self.path == "/" else b"Missing asset")

            def log_message(self, *_args):
                pass

        with ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server:
            worker = threading.Thread(target=server.serve_forever, daemon=True)
            worker.start()
            try:
                with tempfile.TemporaryDirectory(dir=os.environ["OPENAGENTS_SCRATCH"]) as directory:
                    result = subprocess.run([
                        sys.executable, str(CHECK), "--base",
                        f"http://127.0.0.1:{server.server_port}", "--timeout", "2",
                        "--out", directory, *flags,
                    ], capture_output=True, text=True, timeout=30,
                        env={**os.environ, "BROWSER": "/usr/bin/true"})
                    self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
                    out = Path(directory)
                    self.assertTrue((out / "page.png").read_bytes().startswith(b"\x89PNG"))
                    state = json.loads((out / "state.json").read_text())
                    events = json.loads((out / "events.json").read_text())
                    return state, events
            finally:
                server.shutdown()
                worker.join()

    def test_http_success_with_missing_start_module_fails_and_keeps_the_404(self):
        _, events = self.probe(
            '<div id="gpui-shell">Loading</div><script type="module" src="/missing-start.js"></script>', 1)
        self.assertTrue(any(event.get("status") == 404
                            and event.get("url", "").endswith("/missing-start.js") for event in events))

    def test_a_caught_font_failure_still_blocks_startup(self):
        _, events = self.probe(
            '<div id="gpui-shell">Loading</div><script>fetch("/gpui-fonts/Geist.ttf")'
            '.then(r=>{if(!r.ok)throw new Error("font: "+r.status)}).catch(console.error)</script>', 1)
        self.assertTrue(any(event.get("status") == 500
                            and event.get("url", "").endswith("/gpui-fonts/Geist.ttf") for event in events))

    def test_a_startup_exception_fails_and_is_saved(self):
        _, events = self.probe('<script>throw new Error("startup fixture exception")</script>', 1)
        self.assertTrue(any(event["event"] == "Runtime.exceptionThrown" for event in events))

    def test_a_ready_marker_without_a_canvas_cannot_pass(self):
        state, _ = self.probe('<script>document.documentElement.dataset.gpui="ready"</script>', 1)
        self.assertFalse(state["canvas"])

    def test_an_explicitly_disabled_feature_can_skip(self):
        self.probe('<meta name="openagents-features" content=""><p>HTML chat</p>', 0, "--if-enabled")


if __name__ == "__main__":
    unittest.main()
