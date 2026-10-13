"""The sealed Clef TPU worker: HTTP on localhost behind the Rust door.

  python -I -m clef_tpu.server <clef-flash dir> [--port 8091] [--chunk 0] [--precision bf16|int8] [--scan fused]

GET  /v1/identity  -> backend, device, precision, artifact and head digests
POST /v1/logits    {"input_ids": [...], "questions": [{"type", "question_span", "option_spans"}], "chunk"?}
                   -> {"logits": [[...], ...], "prefill_seconds", "head_seconds"}

It binds 127.0.0.1 only; the door (`psionic-openai-server --decision-worker`)
owns the public edge, parsing, encoding, admission and answers.
"""
import argparse
import hashlib
import json
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from .runner import Runner


def artifact_digest(path: Path):
    h = hashlib.sha256()
    for f in sorted(path.glob("model*.safetensors")):
        with open(f, "rb") as fh:
            while chunk := fh.read(64 << 20):
                h.update(chunk)
    return "sha256:" + h.hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("model")
    ap.add_argument("--port", type=int, default=8091)
    ap.add_argument("--chunk", type=int, default=0, help="prefill chunk; 0 picks 512 or 768 per prompt")
    ap.add_argument("--precision", default="bf16")
    ap.add_argument("--scan", default="fused", help="delta rule: fused (Pallas, default), jax or pallas (XLA prep + scan)")
    ap.add_argument("--prefix-budget", type=float, default=4.0,
                    help="GB of device memory for prefix snapshots (0 turns prefix reuse off)")
    ap.add_argument("--warm", default="500,1500,2300,3500,6000,7500,15000", help="prompt lengths to compile before serving")
    a = ap.parse_args()
    # compiled programs persist across restarts (each new shape compiles once, 10-40 s)
    import jax
    jax.config.update("jax_compilation_cache_dir", str(Path.home() / ".cache" / "clef-tpu-jax"))
    jax.config.update("jax_persistent_cache_min_compile_time_secs", 1.0)
    runner = Runner(a.model, chunk=a.chunk, precision=a.precision, scan=a.scan,
                    prefix_budget=int(a.prefix_budget * 2**30))
    identity = dict(runner.identity(), artifact_digest=artifact_digest(Path(a.model)),
                    artifact_family="bf16-safetensors", model=Path(a.model).name)
    t = time.time()
    runner.warm_head()
    print(f"head warm in {time.time() - t:.0f} s", file=sys.stderr, flush=True)
    t = time.time()
    runner.warm_prefill()
    print(f"prefill warm in {time.time() - t:.0f} s", file=sys.stderr, flush=True)
    for n in [int(x) for x in a.warm.split(",") if x]:
        for nq, no in ((1, 2), (9, 40)):
            qs = [{"type": 0, "question_span": [0, 1], "option_spans": [[1, 2]] * (no // nq)} for _ in range(nq)]
            runner.logits([0] * n, qs)
    lock = threading.Lock()

    class H(BaseHTTPRequestHandler):
        def _send(self, code, body):
            data = json.dumps(body).encode()
            self.send_response(code)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            if self.path == "/v1/identity":
                return self._send(200, identity)
            self._send(404, {"error": "not found"})

        def do_POST(self):
            if self.path != "/v1/logits":
                return self._send(404, {"error": "not found"})
            try:
                req = json.loads(self.rfile.read(int(self.headers["content-length"])))
                ids, qs = req["input_ids"], req["questions"]
                chunk = req.get("chunk") or a.chunk
                with lock:
                    t = time.time()
                    logits = runner.logits(ids, qs, chunk=chunk, prefix_tokens=int(req.get("prefix_tokens") or 0))
                    dt = time.time() - t
                    restored = runner.last_restored
                self._send(200, {"logits": logits, "prefill_seconds": dt, "head_seconds": 0.0,
                                 "cached_tokens": restored})
            except Exception as e:  # the door maps this to an internal error
                self._send(500, {"error": f"{type(e).__name__}: {e}"})

        def log_message(self, *args):
            pass

    print(f"clef-tpu worker on 127.0.0.1:{a.port} {json.dumps(identity)}", file=sys.stderr, flush=True)
    ThreadingHTTPServer(("127.0.0.1", a.port), H).serve_forever()


if __name__ == "__main__":
    main()
