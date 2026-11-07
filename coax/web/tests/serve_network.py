# Firm reason: 2 (independent HTTP byte and wall-clock oracle for the Coax/C server)
"""Run after serve.sh has built the server: python3 coax/web/tests/serve_network.py."""
import concurrent.futures
import http.client
import http.server
import pathlib
import os
import socket
import subprocess
import tempfile
import threading
import time

ROOT = pathlib.Path(__file__).resolve().parents[3]
SERVER = os.environ.get("SERVE_BINARY", str(ROOT / "coax/web/build/serve"))


def port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def get(p, path="/data.bin", headers=None):
    c = http.client.HTTPConnection("127.0.0.1", p, timeout=10)
    start = time.monotonic()
    c.request("GET", path, headers=headers or {})
    r = c.getresponse()
    first = time.monotonic() - start
    data = r.read()
    result = (r.status, dict(r.getheaders()), data, first, time.monotonic() - start)
    c.close()
    return result


with tempfile.TemporaryDirectory(prefix="coax-serve-network-") as tmp:
    directory = pathlib.Path(tmp)
    data = bytes(range(256)) * 4096
    (directory / "data.bin").write_bytes(data)
    (directory / "empty.bin").write_bytes(b"")
    with (directory / "large.bin").open("wb") as f:
        f.seek(3 * 1024**3)
        f.write(b"tail")
    class Api(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            self.send_response(200)
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def log_message(self, *args):
            pass

    api = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Api)
    threading.Thread(target=api.serve_forever, daemon=True).start()
    p = port()
    process = subprocess.Popen([SERVER, tmp, "--static", "--host-dir", tmp,
                                "--listen", f"127.0.0.1:{p}", "--api", str(api.server_port), "--throttle-mbps", "8", "--latency-ms", "120"],
                               stdout=subprocess.DEVNULL)
    try:
        for _ in range(100):
            try:
                with socket.create_connection(("127.0.0.1", p), timeout=.1):
                    break
            except OSError:
                time.sleep(.02)
        else:
            raise AssertionError("server did not start")
        cases = [("bytes=0-0", 206, data[:1]), ("bytes=253-259", 206, data[253:260]),
                 ("bytes=1048500-", 206, data[1048500:]), ("bytes=-9", 206, data[-9:]),
                 ("bytes=1048576-", 416, b""), ("bytes=-0", 416, b""),
                 ("ByTeS=2-4", 206, data[2:5]), ("bytes=4-2", 200, data),
                 ("bytes=0-1,4-5", 200, data), ("bytes=9223372036854775808-", 200, data)]
        for value, status, body in cases:
            actual, headers, received, first, elapsed = get(p, headers={"rAnGe": value})
            assert (actual, received) == (status, body), (value, actual, len(received))
            assert headers["Accept-Ranges"] == "bytes"
            assert int(headers["Content-Length"]) == len(body)
            assert headers["Cross-Origin-Embedder-Policy"] == "require-corp"
            if status == 206:
                # Verify the unambiguous authored cases directly below as well.
                assert headers["Content-Range"].endswith("/1048576")
            if status == 416:
                assert headers["Content-Range"] == "bytes */1048576"
            assert first >= .10, first
        r = get(p, headers={"Range": "bytes=253-259"})
        assert r[1]["Content-Range"] == "bytes 253-259/1048576"
        assert get(p, "/empty.bin", {"Range": "bytes=0-0"})[:3:2] == (416, b"")
        assert get(p, headers={"Range": "bytes=0-0", "If-Range": '"old"'})[2] == data
        assert get(p, headers={"Range": "bytes=-2000000"})[2] == data
        assert get(p, "/missing")[0] == 404
        large = get(p, "/large.bin", {"Range": "bytes=-4"})
        assert large[2] == b"tail" and large[1]["Content-Range"] == "bytes 3221225472-3221225475/3221225476"
        # Repeated fields are deliberately ignored; they must not select an
        # accidental first range when a proxy combines or repeats headers.
        c = http.client.HTTPConnection("127.0.0.1", p, timeout=10)
        c.putrequest("GET", "/data.bin")
        c.putheader("Range", "bytes=0-0")
        c.putheader("Range", "bytes=1-1")
        c.endheaders()
        response = c.getresponse()
        assert response.status == 200 and response.read() == data
        c.close()
        start = time.monotonic()
        with concurrent.futures.ThreadPoolExecutor(2) as pool:
            results = list(pool.map(lambda _: get(p), range(2)))
        total = time.monotonic() - start
        for r in results:
            assert r[2] == data and r[3] >= .10
            assert 1.10 <= r[4] <= 1.9, r[4]
        assert total < 1.95, total  # per connection; no serialized download
        forwarded = get(p, "/api/data")
        assert forwarded[0] == 200 and forwarded[2] == data
        assert forwarded[3] >= .10 and 1.10 <= forwarded[4] <= 1.9, forwarded[3:]
        print(f"PASS HTTP bytes/ranges/416/latency; concurrent 1 MiB transfers {total:.3f}s at 8 Mbit/s, 120ms latency")
    finally:
        process.terminate()
        process.wait(timeout=5)
        api.shutdown()
        api.server_close()

for option in ["--throttle-mbps", "--latency-ms"]:
    for value in ["-1", "nan", "inf", "nope", "1000001", "1e-9999", ""]:
        r = subprocess.run([SERVER, ".", option, value], capture_output=True)
        assert r.returncode == 2, (option, value)
r = subprocess.run([SERVER, ".", "--throttle-mbps", "0.000001"], capture_output=True)
assert r.returncode == 2
print("PASS invalid shaping arguments refuse")
