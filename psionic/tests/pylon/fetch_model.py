#!/usr/bin/env python3
"""Installer HTTPS/range/checksum and cache-boundary tests, no model downloads."""
import fcntl
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import ssl
import subprocess
import tempfile
import threading
import time
import unittest

ROOT = Path(__file__).resolve().parents[3]
FETCH = ROOT / "scripts/pylon-fetch-model.py"
PAYLOAD = bytes(range(256)) * 1024 + b"independent HTTPS artifact fixture"


class Fetch(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="pylon-fetch-https-")
        cls.directory = Path(cls.temp.name)
        cert, key = cls.directory / "cert.pem", cls.directory / "key.pem"
        subprocess.run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                        "-subj", "/CN=127.0.0.1", "-addext", "subjectAltName=IP:127.0.0.1",
                        "-keyout", str(key), "-out", str(cert)], check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        cls.requests = []
        cls.body = PAYLOAD

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                cls.requests.append((self.path, self.headers.get("Range")))
                if self.path == "/downgrade":
                    self.send_response(302)
                    self.send_header("Location", "http://127.0.0.1:9/never-send-plaintext")
                    self.end_headers()
                    return
                start = 0
                requested = self.headers.get("Range")
                if requested:
                    start = int(requested.removeprefix("bytes=").removesuffix("-"))
                body = cls.body[start:]
                self.send_response(206 if requested else 200)
                self.send_header("Content-Length", str(len(body)))
                if requested:
                    self.send_header("Content-Range", f"bytes {start}-{len(cls.body)-1}/{len(cls.body)}")
                self.end_headers()
                if self.path == "/slow":
                    self.wfile.write(body[:32768])
                    self.wfile.flush()
                    time.sleep(2)
                    try:self.wfile.write(body[32768:])
                    except (BrokenPipeError, ConnectionResetError, ssl.SSLError):pass
                else:self.wfile.write(body)

            def log_message(self, *unused):
                pass

        cls.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(cert, key)
        cls.server.socket = context.wrap_socket(cls.server.socket, server_side=True)
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        cls.env = {**os.environ, "CURL_CA_BUNDLE": str(cert)}

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join()
        cls.temp.cleanup()

    def setUp(self):
        self.case = tempfile.TemporaryDirectory(dir=self.directory)
        self.work = Path(self.case.name)
        self.cache = self.work / "cache"
        self.manifest = self.work / "manifest.json"
        type(self).requests.clear()
        type(self).body = PAYLOAD
        self.files = [{"name": "weights.bin", "sha256": hashlib.sha256(PAYLOAD).hexdigest(),
                       "bytes": len(PAYLOAD), "url": f"https://127.0.0.1:{self.server.server_port}/weights"}]

    def tearDown(self):
        self.case.cleanup()

    def run_fetch(self, success=True, extra=()):
        self.manifest.write_text(json.dumps({"files": self.files}))
        result = subprocess.run(["python3", str(FETCH), str(self.manifest), str(self.cache), *extra],
                                env=self.env, capture_output=True, text=True, timeout=20)
        if success:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0)
        return result

    def test_real_https_then_checksum_cache_hit_and_corruption_repair(self):
        self.run_fetch()
        self.assertEqual((self.cache / "weights.bin").read_bytes(), PAYLOAD)
        self.assertEqual(len(self.requests), 1)
        self.run_fetch()
        self.assertEqual(len(self.requests), 1)
        changed = bytearray(PAYLOAD)
        changed[7] ^= 1
        (self.cache / "weights.bin").write_bytes(changed)
        self.run_fetch()
        self.assertEqual(len(self.requests), 2)
        self.assertEqual((self.cache / "weights.bin").read_bytes(), PAYLOAD)

    def test_range_resume_and_completed_partial_promotion(self):
        self.cache.mkdir()
        (self.cache / "weights.bin.part").write_bytes(PAYLOAD[:17001])
        self.run_fetch()
        self.assertIn(("/weights", "bytes=17001-"), self.requests)
        self.assertEqual((self.cache / "weights.bin").read_bytes(), PAYLOAD)
        (self.cache / "weights.bin").rename(self.cache / "weights.bin.part")
        self.run_fetch()
        self.assertEqual(len(self.requests), 1)

    def test_bad_complete_download_is_never_promoted_and_can_be_retried(self):
        type(self).body = b"x" * len(PAYLOAD)
        result = self.run_fetch(False)
        self.assertIn("SHA-256/length mismatch", result.stderr)
        self.assertFalse((self.cache / "weights.bin").exists())
        self.assertFalse((self.cache / "weights.bin.part").exists())
        type(self).body = PAYLOAD
        self.run_fetch()
        self.assertEqual((self.cache / "weights.bin").read_bytes(), PAYLOAD)

    def test_metadata_without_size_still_requires_the_full_checksum(self):
        self.files[0]["bytes"] = 0
        self.run_fetch()
        type(self).body = b"altered metadata"
        (self.cache / "weights.bin").unlink()
        self.run_fetch(False)
        self.assertFalse((self.cache / "weights.bin").exists())

    def test_timeout_keeps_all_written_progress_and_next_call_resumes_it(self):
        self.files[0]["url"] = f"https://127.0.0.1:{self.server.server_port}/slow"
        result = self.run_fetch(False, ("--timeout-seconds", "1"))
        self.assertIn("partial progress retained", result.stderr)
        partial = self.cache / "weights.bin.part"
        retained = partial.stat().st_size
        self.assertGreater(retained, 0)
        self.assertLess(retained, len(PAYLOAD))
        self.assertEqual(partial.read_bytes(), PAYLOAD[:retained])
        self.assertEqual(self.requests, [("/slow", None)])
        self.files[0]["url"] = f"https://127.0.0.1:{self.server.server_port}/weights"
        self.run_fetch()
        self.assertIn(("/weights", f"bytes={retained}-"), self.requests)
        self.assertEqual((self.cache / "weights.bin").read_bytes(), PAYLOAD)

    def test_no_plaintext_or_redirect_downgrade(self):
        self.files[0]["url"] = f"http://127.0.0.1:{self.server.server_port}/weights"
        self.run_fetch(False)
        self.assertEqual(self.requests, [])
        self.files[0]["url"] = f"https://127.0.0.1:{self.server.server_port}/downgrade"
        self.run_fetch(False)
        self.assertEqual(self.requests, [("/downgrade", None)])
        self.assertFalse((self.cache / "weights.bin").exists())

    def test_symlinks_and_foreign_cache_paths_never_change_their_targets(self):
        self.cache.mkdir()
        keep = self.work / "keep"
        keep.write_text("keep this data")
        for name in ("weights.bin", "weights.bin.part", "source.json", ".fetch.lock"):
            path = self.cache / name
            if path.exists():
                path.unlink()
            path.symlink_to(keep)
            self.run_fetch(False)
            self.assertEqual(keep.read_text(), "keep this data")
            path.unlink()
        self.assertEqual(self.requests, [])
        self.files[0]["name"] = "../escape"
        self.run_fetch(False)
        self.assertFalse((self.work / "escape").exists())

    def test_cache_lock_is_exclusive(self):
        self.cache.mkdir()
        with (self.cache / ".fetch.lock").open("w") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            result = self.run_fetch(False)
            self.assertIn("another model download", result.stderr)
        self.assertEqual(self.requests, [])


if __name__ == "__main__":
    unittest.main()
