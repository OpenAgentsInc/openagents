#!/usr/bin/env python3
"""Compare uncached System One latency under a caller-held quiet lease.

Run: openagents lease quiet -- python3 mac_quiet_bench.py config.json
The harness never acquires a lease or stops an existing server. Example config:
{
  "model_path": "/models/Clef-Flash-Q4_K_M.gguf",
  "requests": ["/fixtures/len1k.json", "/fixtures/len4k.json"],
  "output_dir": "/scratch/clef-comparison",
  "variants": [
    {"name": "metal", "binary": "/bin/psionic-openai-server", "chunk": 2048},
    {"name": "ollama", "url": "http://127.0.0.1:11434/v1/systemone"}
  ]
}

Defaults: three rounds, five measured runs per workload per round, one warmup,
600 continuous seconds below one-minute load 5 and GPU utilization at most 10%,
a 1,800-second wait deadline, and rejection above load 8. Leave applications and display settings
alone; this harness manages only its own child servers. SIGTERM runs cleanup.
Each round reverses the previous variant order. A sample's nonce-prefixed state
is identical across variants; model names may differ. This measures uncached
requests, not prefix reuse. Paths are relative to the config file. Server env
overrides are optional; inherited PSIONIC_CLEF_* overrides are removed. External
server settings cannot be controlled and are recorded as unknown. Use only
non-sensitive fixtures: complete responses are retained. --self-test uses only
a small local mock HTTP server and does not inspect the GPU.
External variants must not share a response cache. Duplicate URLs are rejected;
different URLs backed by the same cache cannot be detected. PROFILE or SKIP
environment overrides produce diagnostic-only summaries.
"""

import argparse
import contextlib
import datetime
import hashlib
import http.server
import json
import math
import os
from pathlib import Path
import plistlib
import signal
import socket
import statistics
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid


HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def file_sha(path):
    digest = hashlib.sha256()
    with open(path, "rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def emit(file, row):
    file.write(json.dumps(row, sort_keys=True) + "\n")
    file.flush()


def resolve(base, path):
    return str((base / Path(path).expanduser()).resolve())


def load_config(path):
    path = Path(path).resolve()
    config = json.loads(path.read_text())
    for key, value in {"rounds": 3, "runs": 5, "settle_seconds": 600,
                       "max_wait_seconds": 1800,
                       "max_start_gpu_percent": 10,
                       "max_start_load": 5, "max_run_load": 8,
                       "request_timeout": 600}.items():
        config.setdefault(key, value)
    for key in ("rounds", "runs"):
        if type(config[key]) is not int or config[key] < 1:
            raise ValueError(f"{key} must be a positive integer")
    for key in ("settle_seconds", "max_wait_seconds", "max_start_load", "max_run_load", "request_timeout"):
        if (type(config[key]) not in (int, float) or not math.isfinite(config[key])
                or config[key] < 0):
            raise ValueError(f"{key} must be a finite nonnegative number")
    if config["max_start_load"] <= 0 or config["max_run_load"] < config["max_start_load"]:
        raise ValueError("load thresholds must be positive and start must not exceed run")
    if config["request_timeout"] <= 0:
        raise ValueError("request_timeout must be positive")
    if config["max_wait_seconds"] <= config["settle_seconds"]:
        raise ValueError("max_wait_seconds must exceed settle_seconds")
    gpu_limit = config["max_start_gpu_percent"]
    if type(gpu_limit) not in (int, float) or not 0 <= gpu_limit <= 100:
        raise ValueError("max_start_gpu_percent must be between 0 and 100")
    config["output_dir"] = resolve(path.parent, config["output_dir"])
    config["requests"] = [resolve(path.parent, p) for p in config["requests"]]
    if not config["requests"] or len(set(config["requests"])) != len(config["requests"]):
        raise ValueError("requests must contain distinct paths")
    if "model_path" in config:
        config["model_path"] = resolve(path.parent, config["model_path"])
    if isinstance(config["variants"], dict):
        config["variants"] = [dict(v, name=k) for k, v in config["variants"].items()]
    names = set()
    external_urls = set()
    for variant in config["variants"]:
        name = variant["name"]
        if not name or not all(c.isalnum() or c in "-_" for c in name) or name in names:
            raise ValueError("variant names must be unique, using letters, digits, '-' or '_'")
        names.add(name)
        if ("binary" in variant) == ("url" in variant):
            raise ValueError("each variant needs exactly one binary or URL")
        if "binary" in variant:
            variant["binary"] = resolve(path.parent, variant["binary"])
            if "model_path" not in config or not Path(config["model_path"]).is_file():
                raise ValueError("a local server needs an existing model_path")
            variant.setdefault("chunk", 2048)
            if type(variant["chunk"]) is not int or variant["chunk"] < 1:
                raise ValueError("chunk must be a positive integer")
        else:
            parsed = urllib.parse.urlsplit(variant["url"])
            if (parsed.scheme != "http" or parsed.hostname not in ("127.0.0.1", "localhost", "::1")
                    or parsed.username or parsed.password or parsed.query or parsed.fragment):
                raise ValueError("external URL must be a credential-free loopback HTTP URL")
            if parsed.path in ("", "/"):
                variant["url"] = variant["url"].rstrip("/") + "/v1/systemone"
            parsed = urllib.parse.urlsplit(variant["url"])
            address = "127.0.0.1" if parsed.hostname == "localhost" else parsed.hostname
            endpoint = (address, parsed.port or 80, parsed.path.rstrip("/"))
            if endpoint in external_urls:
                raise ValueError("external variants must use separate servers and response caches")
            external_urls.add(endpoint)
            if variant.get("env"):
                raise ValueError("external server environment cannot be configured here")
        variant.setdefault("model", config.get("request_model", "clef-flash"))
        for key, value in variant.get("env", {}).items():
            if not (key.startswith("PSIONIC_CLEF_") or key in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS")):
                raise ValueError("env overrides support only PSIONIC_CLEF_*, RAYON_NUM_THREADS and OMP_NUM_THREADS")
            if any(word in key.upper() for word in ("KEY", "TOKEN", "SECRET", "PASSWORD")):
                raise ValueError("secret environment overrides are not supported")
            if not isinstance(value, str):
                raise ValueError("environment values must be strings")
    if not names:
        raise ValueError("at least one variant is required")
    return config


def gpu_utilization():
    if sys.platform != "darwin":
        return None
    try:
        data = subprocess.run(["ioreg", "-r", "-c", "AGXAccelerator", "-d", "1", "-a"],
                              capture_output=True, timeout=2, check=True).stdout
        values = {}

        def visit(node):
            if isinstance(node, dict):
                for key, value in node.items():
                    if "utilization" in key.lower() and isinstance(value, (int, float)):
                        values[key] = value
                    elif isinstance(value, (dict, list)):
                        visit(value)
            elif isinstance(node, list):
                for value in node:
                    visit(value)

        visit(plistlib.loads(data))
        return values or None
    except (OSError, subprocess.SubprocessError, plistlib.InvalidFileException, ValueError):
        return None


class Host:
    def __init__(self, config, output, load=os.getloadavg, gpu=True):
        self.config, self.output, self.load, self.gpu = config, output, load, gpu
        self.phase = "settle"
        self.stop = threading.Event()
        self.overloaded = threading.Event()
        self.monitor_error = None
        self.last_gpu = None
        self.thread = threading.Thread(target=self.monitor, daemon=True)

    def sample(self):
        loads = list(self.load())
        if len(loads) != 3 or not all(math.isfinite(value) for value in loads):
            raise RuntimeError("host load measurement unavailable")
        if self.phase != "settle" and loads[0] > self.config["max_run_load"]:
            self.overloaded.set()
        return loads

    def guard(self):
        if self.monitor_error:
            raise RuntimeError("host monitoring failed; comparison rejected")
        loads = self.sample()
        if self.overloaded.is_set():
            raise RuntimeError("host load exceeded max_run_load; comparison rejected")
        return loads

    def monitor(self):
        try:
            self.monitor_loop()
        except Exception as error:
            self.monitor_error = type(error).__name__

    def monitor_loop(self):
        with open(self.output / "host.jsonl", "a", encoding="utf-8") as file:
            heartbeat = 0
            while not self.stop.is_set():
                row = {"at": now(), "phase": self.phase, "load_1_5_15": self.sample()}
                if self.gpu:
                    row["agx_utilization"] = gpu_utilization()
                    if not row["agx_utilization"] or "Device Utilization %" not in row["agx_utilization"]:
                        raise RuntimeError("GPU utilization measurement unavailable")
                    self.last_gpu = row["agx_utilization"]["Device Utilization %"]
                emit(file, row)
                if time.monotonic() - heartbeat >= 30:
                    print(f"{row['at']} {self.phase}: load {row['load_1_5_15'][0]:.2f}", flush=True)
                    heartbeat = time.monotonic()
                self.stop.wait(5)

    def settle(self):
        since = None
        deadline = time.monotonic() + self.config["max_wait_seconds"]
        while True:
            if self.monitor_error:
                raise RuntimeError("host monitoring failed while waiting for quiet")
            stamp = time.monotonic()
            if stamp >= deadline:
                raise RuntimeError("machine did not become quiet before max_wait_seconds")
            gpu_quiet = (not self.gpu or (self.last_gpu is not None
                         and self.last_gpu <= self.config["max_start_gpu_percent"]))
            if self.sample()[0] < self.config["max_start_load"] and gpu_quiet:
                since = stamp if since is None else since
                if stamp - since >= self.config["settle_seconds"]:
                    self.phase = "run"
                    self.guard()
                    return
            else:
                since = None
            time.sleep(1)


def request(url, body, timeout):
    req = urllib.request.Request(url, data=encoded(body), headers={"Content-Type": "application/json"})
    began = time.perf_counter()
    try:
        with HTTP.open(req, timeout=timeout) as response:
            raw, status = response.read(), response.status
    except urllib.error.HTTPError as error:
        raw, status = error.read(), error.code
    elapsed = time.perf_counter() - began
    try:
        response = json.loads(raw)
    except (ValueError, UnicodeDecodeError):
        response = {"raw_response": raw.decode("utf-8", errors="replace")}
    return elapsed, status, response


@contextlib.contextmanager
def server(config, variant, round_id, output, records, host, mock=False):
    if "url" in variant:
        emit(records, {"kind": "startup", "variant": variant["name"], "round": round_id,
                       "at": now(), "owned": False, "startup_seconds": None})
        yield variant["url"]
        return
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    command = [variant["binary"], "-m", config["model_path"], "--host", "127.0.0.1",
               "--port", str(port), "--decision-device", "metal", "--decision-chunk", str(variant["chunk"])]
    if mock:
        command = [sys.executable, str(Path(__file__).resolve()), "--_mock-port", str(port)]
    env = {k: v for k, v in os.environ.items() if not k.startswith("PSIONIC_CLEF_")}
    env.update(variant.get("env", {}))
    path = output / f"server-r{round_id}-{variant['name']}.log"
    row = {"kind": "startup", "variant": variant["name"], "round": round_id,
           "at": now(), "owned": True, "command": command, "log": path.name}
    process = None
    began = time.perf_counter()
    try:
        host.guard()
        with open(path, "wb") as log:
            process = subprocess.Popen(command, env=env, stdin=subprocess.DEVNULL, stdout=log, stderr=log)
            deadline = time.monotonic() + 120
            while True:
                host.guard()
                if process.poll() is not None:
                    raise RuntimeError(f"{variant['name']} exited before readiness; see server log")
                try:
                    with HTTP.open(f"http://127.0.0.1:{port}/health", timeout=1) as response:
                        if response.status == 200:
                            break
                except (OSError, urllib.error.URLError):
                    pass
                if time.monotonic() >= deadline:
                    raise RuntimeError(f"{variant['name']} did not become ready within 120 seconds")
                time.sleep(0.2)
            row.update(startup_seconds=time.perf_counter() - began, status="ready")
            emit(records, row)
            yield f"http://127.0.0.1:{port}/v1/systemone"
    except BaseException:
        if "status" not in row:
            row.update(startup_seconds=time.perf_counter() - began, status="failed")
            emit(records, row)
        raise
    finally:
        if process is not None:
            process.terminate() if process.poll() is None else None
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def run(config, *, load=os.getloadavg, mock=False):
    if not mock and "quiet" not in os.environ.get("OPENAGENTS_LEASES", "").split(","):
        raise ValueError("run this benchmark through openagents lease quiet")
    output = Path(config["output_dir"])
    output.mkdir(parents=True, exist_ok=True)
    if any(output.iterdir()):
        raise ValueError("output_dir must be empty; existing evidence is never overwritten")
    workloads = []
    for path in config["requests"]:
        raw = Path(path).read_bytes()
        body = json.loads(raw)
        if not isinstance(body, dict) or "state" not in body:
            raise ValueError("each workload must be a System One JSON object with state")
        workloads.append((path, body, sha(raw)))
    metadata = {"schema": "clef.mac-quiet-bench.v1", "created_at": now(), "config": config,
                "config_sha256": sha(encoded(config)), "python": sys.version.split()[0],
                "platform": sys.platform, "load_metric": "one-minute system load average",
                "quiet_lease": os.environ.get("OPENAGENTS_LEASE_ID"), "self_test": mock,
                "removed_inherited_env_names": sorted(k for k in os.environ if k.startswith("PSIONIC_CLEF_")),
                "variants": [], "workloads": [{"path": p, "sha256": h} for p, _, h in workloads]}
    git = subprocess.run(["git", "-C", str(Path(__file__).parent), "rev-parse", "HEAD"],
                         capture_output=True, text=True)
    metadata["checkout_commit"] = git.stdout.strip() if git.returncode == 0 else None
    for variant in config["variants"]:
        metadata["variants"].append({"name": variant["name"],
            "binary_sha256": file_sha(variant["binary"]) if "binary" in variant else None,
            "binary_commit": variant.get("commit"),
            "env": ({**{k: os.environ[k] for k in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS") if k in os.environ},
                     **variant.get("env", {})} if "binary" in variant else None),
            "profile_mode": ("enabled (diagnostic)" if "PSIONIC_CLEF_PROFILE" in variant.get("env", {})
                             else "disabled") if "binary" in variant else "external: unknown"})
    (output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    host = Host(config, output, load=load, gpu=not mock)
    samples, status, error = [], "failed", None
    host.thread.start()
    try:
        with open(output / "records.jsonl", "a", encoding="utf-8") as records:
            host.settle()
            for round_id in range(config["rounds"]):
                nonces = {(path, sample): uuid.uuid4().hex for path, _, _ in workloads
                          for sample in range(config["runs"] + 1)}
                order = config["variants"] if round_id % 2 == 0 else list(reversed(config["variants"]))
                for variant in order:
                    with server(config, variant, round_id, output, records, host, mock) as url:
                        for path, base, input_hash in workloads:
                            for sample in range(config["runs"] + 1):
                                body = dict(base)
                                state = base["state"] if isinstance(base["state"], str) else encoded(base["state"]).decode()
                                nonce = nonces[path, sample]
                                body.update(state=f"RUN {nonce}\n\n{state}", model=variant["model"])
                                row = {"kind": "request", "at": now(), "round": round_id,
                                    "variant": variant["name"], "workload": path, "sample": sample,
                                    "warmup": sample == 0, "nonce": nonce, "input_sha256": input_hash,
                                    "request_sha256": sha(encoded(body)), "model": variant["model"],
                                    "load_before": host.sample(), "valid": False}
                                began = time.perf_counter()
                                try:
                                    host.guard()
                                    elapsed, code, response = request(url, body, config["request_timeout"])
                                    row.update(elapsed_seconds=elapsed, http_status=code, response=response,
                                               usage=response.get("usage") if isinstance(response, dict) else None)
                                    if code != 200 or not isinstance(response, dict) or "answers" not in response:
                                        raise RuntimeError("request failed or response has no answers")
                                    host.guard()
                                    row["valid"] = True
                                except BaseException as exc:
                                    row.setdefault("elapsed_seconds", time.perf_counter() - began)
                                    row["error_type"] = type(exc).__name__
                                    raise
                                finally:
                                    try:
                                        row["load_after"] = host.sample()
                                    except Exception as exc:
                                        host.monitor_error = type(exc).__name__
                                        row["load_after"] = None
                                    if host.monitor_error:
                                        row["valid"] = False
                                        row["rejection"] = "host_monitor"
                                    if host.overloaded.is_set():
                                        row["valid"] = False
                                        row["rejection"] = "host_load"
                                    emit(records, row)
                                    samples.append(row)
                                host.guard()
            host.guard()
            status = "complete"
    except BaseException as exc:
        error = type(exc).__name__
        if isinstance(exc, (KeyboardInterrupt, SystemExit)):
            status = "interrupted"
        print(f"Benchmark rejected ({error}); see {output}", file=sys.stderr, flush=True)
    finally:
        try:
            host.sample()
        except Exception as exc:
            host.monitor_error = type(exc).__name__
        host.stop.set()
        host.thread.join(timeout=4)
        if host.overloaded.is_set():
            status = "rejected_host_load"
        if host.monitor_error or host.thread.is_alive():
            status = "rejected_host_monitor"
        diagnostic = any(any("PROFILE" in key or "SKIP" in key for key in variant.get("env", {}))
                         for variant in config["variants"])
        diagnostic = diagnostic or config["max_start_gpu_percent"] > 10 or config["settle_seconds"] < 600
        groups = []
        for variant in config["variants"]:
            for path, _, _ in workloads:
                rows = [r for r in samples if r["variant"] == variant["name"] and r["workload"] == path
                        and not r["warmup"] and r["valid"]]
                groups.append({"variant": variant["name"], "workload": path, "count": len(rows),
                    "expected_count": config["rounds"] * config["runs"],
                    "median_seconds": statistics.median(r["elapsed_seconds"] for r in rows) if rows else None,
                    "usable_comparison": status == "complete" and not diagnostic})
        summary = {"status": status, "error_type": error, "finished_at": now(), "groups": groups,
                   "monitor_error": host.monitor_error, "diagnostic_only": diagnostic,
                   "gpu_interference_detection": "admission only; unavailable during inference",
                   "idle_machine_acceptance": "not established by this harness",
                   "note": "Medians exclude warmups and rejected samples. No quality verdict or tail-latency claim."}
        (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    return summary


def self_test():
    scratch = Path(os.environ.get("OPENAGENTS_SCRATCH", str(Path.home() / ".openagents" / "scratch")))
    scratch.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="clef-harness-test-", dir=scratch) as directory:
        root = Path(directory)
        fixture = root / "request.json"
        fixture.write_text(json.dumps({"state": "test", "questions": []}))
        config_path = root / "config.json"
        config_path.write_text(json.dumps({"model_path": str(fixture), "requests": [str(fixture)],
            "output_dir": str(root / "results"), "rounds": 2, "runs": 2, "settle_seconds": 0,
            "variants": [{"name": name, "binary": sys.executable} for name in ("a", "b")]}))
        config = load_config(config_path)
        summary = run(config, load=lambda: (0, 0, 0), mock=True)
        assert summary["status"] == "complete" and all(g["count"] == 4 for g in summary["groups"])
        rows = [json.loads(line) for line in (root / "results" / "records.jsonl").read_text().splitlines()]
        assert [r["variant"] for r in rows if r["kind"] == "startup"] == ["a", "b", "b", "a"]
        pairs = {}
        for row in rows:
            if row["kind"] == "request":
                pairs.setdefault((row["round"], row["sample"]), []).append(row)
        assert len({rs[0]["nonce"] for rs in pairs.values()}) == 6
        assert all(len(rs) == 2 and rs[0]["nonce"] == rs[1]["nonce"]
                   and rs[0]["request_sha256"] == rs[1]["request_sha256"] for rs in pairs.values())
        for row in rows:
            if row["kind"] == "startup":
                port = int(row["command"][-1])
                with socket.socket() as sock:
                    assert sock.connect_ex(("127.0.0.1", port)) != 0
        host = Host(config, root, load=lambda: (9, 9, 9), gpu=False)
        host.phase = "run"
        try:
            host.guard()
            raise AssertionError("high load accepted")
        except RuntimeError:
            assert host.overloaded.is_set()
        host = Host(dict(config, max_wait_seconds=0.001), root,
                    load=lambda: (9, 9, 9), gpu=False)
        try:
            host.settle()
            raise AssertionError("quiet wait deadline ignored")
        except RuntimeError as error:
            assert "max_wait_seconds" in str(error)
        # Low CPU load alone must not admit a busy GPU. Inject the sample;
        # the self-test never reads a real GPU counter.
        host = Host(dict(config, max_wait_seconds=0.001), root,
                    load=lambda: (0, 0, 0), gpu=True)
        host.last_gpu = 50
        try:
            host.settle()
            raise AssertionError("busy GPU admitted")
        except RuntimeError as error:
            assert "max_wait_seconds" in str(error)
        # Cancellation must close the owned server even before any request.
        host = Host(config, root, load=lambda: (0, 0, 0), gpu=False)
        with (root / "cancellation.jsonl").open("w") as records:
            try:
                with server(config, config["variants"][0], 3, root, records, host, mock=True) as url:
                    port = urllib.parse.urlsplit(url).port
                    raise KeyboardInterrupt
            except KeyboardInterrupt:
                pass
        with socket.socket() as sock:
            assert sock.connect_ex(("127.0.0.1", port)) != 0
    print("Self-test passed: paired nonces, reversed rounds, warmup exclusion, CPU/GPU admission, wait deadline, cancellation cleanup.")


def mock_server(port):
    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"{}")

        def do_POST(self):
            body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(encoded({"answers": [], "usage": {"input_tokens": len(body["state"].split())}}))

        def log_message(self, *args):
            pass

    http.server.HTTPServer(("127.0.0.1", port), Handler).serve_forever()


if __name__ == "__main__":
    def interrupt(signum, frame):
        # Unwind the owned-server context so cancellation leaves no server behind.
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, interrupt)
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("config", nargs="?")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--_mock-port", type=int, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args._mock_port:
        mock_server(args._mock_port)
    elif args.self_test:
        self_test()
    elif args.config:
        sys.exit(0 if run(load_config(args.config))["status"] == "complete" else 1)
    else:
        parser.error("provide a config path or --self-test")
