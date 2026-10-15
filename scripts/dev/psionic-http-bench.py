#!/usr/bin/env python3
"""Benchmark the real Psionic HTTP server with synthetic router requests."""

import argparse
import hashlib
import http.client
import json
from pathlib import Path
import re
import selectors
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--server", type=Path, required=True)
parser.add_argument("--model-dir", type=Path, required=True)
parser.add_argument("--ort-library", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--samples", type=int, default=1000)
args = parser.parse_args()
if args.samples < 1:
    parser.error("Use a positive sample count")
args.output.mkdir(parents=True, exist_ok=True)
corpus_path = ROOT / "crates/psionic/crates/psionic-inference/fixtures/route-v1-inputs.json"
rows = json.loads(corpus_path.read_text())["rows"]
library_hash = hashlib.sha256(args.ort_library.read_bytes()).hexdigest()
records = []


def write_records():
    path = args.output / "http-runs.jsonl"
    temporary = path.with_suffix(".part")
    temporary.write_text("".join(json.dumps(r, sort_keys=True) + "\n" for r in records))
    temporary.replace(path)


def call(connection, method, path, value=None):
    body = None if value is None else json.dumps(value).encode()
    connection.request(method, path, body, {"Content-Type": "application/json"})
    response = connection.getresponse()
    data = json.loads(response.read())
    return response.status, data


for round_index, engines in enumerate([["ort", "native"], ["native", "ort"]], 1):
    for engine in engines:
        command = [str(args.server.resolve()), "--router-model-dir", str(args.model_dir.resolve()),
                   "--router-engine", engine, "--router-threads", "4", "--host", "127.0.0.1", "--port", "0"]
        if engine == "ort":
            command += ["--ort-library", str(args.ort_library.resolve()), "--ort-sha256", library_hash]
        stderr = (args.output / f"{round_index}-{engine}.stderr").open("w")
        process = None
        connection = None
        try:
            started = time.perf_counter()
            process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=stderr, text=True)
            selector = selectors.DefaultSelector()
            selector.register(process.stdout, selectors.EVENT_READ)
            address = None
            deadline = time.monotonic() + 120
            while time.monotonic() < deadline:
                if not selector.select(0.5):
                    if process.poll() is not None:
                        break
                    continue
                line = process.stdout.readline()
                if not line:
                    break
                match = re.search(r"http://127\.0\.0\.1:(\d+)", line)
                if match:
                    address = int(match[1])
                    break
            selector.close()
            if address is None:
                raise RuntimeError(f"{engine} server did not become ready; see its stderr file")
            connection = http.client.HTTPConnection("127.0.0.1", address, timeout=30)
            assert call(connection, "GET", "/health")[0] == 200
            startup_ms = (time.perf_counter() - started) * 1000
            status, models = call(connection, "GET", "/v1/models")
            assert status == 200
            model = models["data"][0]
            assert model["execution"]["engine"] == engine
            requests = [{"model": model["id"], "state": {"request": row["request"], "context": row["context"]},
                         "questions": model["questions"]} for row in rows]
            changed = dict(requests[0], questions={"category": {"type": "noul", "instructions": "A different question"}})
            assert call(connection, "POST", "/v1/systemone", changed)[0] == 422
            for i in range(20):
                assert call(connection, "POST", "/v1/systemone", requests[i % len(requests)])[0] == 200
            latencies = []
            begin = time.perf_counter()
            for i in range(args.samples):
                request_start = time.perf_counter()
                status, answer = call(connection, "POST", "/v1/systemone", requests[i % len(requests)])
                latencies.append((time.perf_counter() - request_start) * 1000)
                assert status == 200 and answer["psionic"]["execution"]["engine"] == engine
            elapsed = time.perf_counter() - begin
            latencies.sort()
            percentile = lambda p: latencies[min(len(latencies) - 1, max(0, __import__("math").ceil(p * len(latencies)) - 1))]
            record = {"schema": "psionic.model.http.benchmark.v1", "round": round_index, "engine": model["execution"],
                      "samples": args.samples, "p50_ms": percentile(0.5), "p95_ms": percentile(0.95), "p99_ms": percentile(0.99),
                      "requests_per_second": args.samples / elapsed, "startup_to_health_ms": startup_ms,
                      "corpus_sha256": hashlib.sha256(corpus_path.read_bytes()).hexdigest(),
                      "server_sha256": hashlib.sha256(args.server.read_bytes()).hexdigest(), "command": command,
                      "model_sha256": answer["psionic"]["model_sha256"], "tokenizer_sha256": answer["psionic"]["tokenizer_sha256"],
                      "scope": "Loopback HTTP with a persistent connection; four model CPU threads; synthetic route-v1 requests; warm caches"}
            records.append(record)
            write_records()
            print(f"{round_index} {engine}: HTTP p50={record['p50_ms']:.3f} ms p99={record['p99_ms']:.3f} ms", flush=True)
        finally:
            if connection:
                connection.close()
            if process:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                if process.stdout:
                    process.stdout.close()
            stderr.close()
