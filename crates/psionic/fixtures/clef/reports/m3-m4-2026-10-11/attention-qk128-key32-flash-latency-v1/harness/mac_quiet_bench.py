#!/usr/bin/env python3
"""Compare uncached System One latency under a caller-held resource lease.

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

For a shared machine, set "measurement_mode": "shared" and run under a GPU
lease. This mode starts without waiting for a quiet machine and records load
throughout; its results cannot establish idle-machine performance targets.
An owned Ollama variant uses "engine": "ollama", "binary", "models_dir",
and "model". It starts on a fresh loopback port with model pruning disabled,
then stops its own process group before the next variant starts.
Owned servers default to a 36 GiB process-group RSS limit and a 120-second
startup deadline. The bounded 27B configuration uses "max_rss_gib": 52,
"startup_timeout": 600, and PSIONIC_CLEF_METAL_FFN_WEIGHTS=q4k-on-demand.
Resident f16 weights require a separate memory estimate. These settings also
accept config-wide defaults. RSS is sampled
every 0.5 seconds; it is not a hard allocation limit or a system memory cap.
On macOS, the same cap also covers the sum of each member's larger RSS or
physical footprint, which includes task-charged IOKit mappings and compressed
memory. The host margin remains necessary for memory charged elsewhere.
Set "minimum_available_memory_gib": 12 to stop the owned group when macOS
free, inactive, and speculative pages total less than 12 GiB. Before starting
the child, this option requires max_rss_gib plus that margin to be available.

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
Owned Psionic latency variants explicitly disable prefix caching; nonzero
PSIONIC_CLEF_PREFIX_CACHE_BYTES settings are refused by this latency runner.
The shared server helper still accepts them for the separate prefix check.
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
import re
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
    for key, value in {"measurement_mode": "quiet", "rounds": 3, "runs": 5, "settle_seconds": 600,
                       "max_wait_seconds": 1800,
                       "max_start_gpu_percent": 10,
                       "max_start_load": 5, "max_run_load": 8,
                       "request_timeout": 600, "max_rss_gib": 36,
                       "startup_timeout": 120}.items():
        config.setdefault(key, value)
    if config["measurement_mode"] not in ("quiet", "shared"):
        raise ValueError("measurement_mode must be quiet or shared")
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
    owned_process_limits(config, {})
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
    if "clef_head" in config:
        config["clef_head"] = resolve(path.parent, config["clef_head"])
        if not Path(config["clef_head"]).is_file():
            raise ValueError("clef_head must name an existing safetensors file")
    if isinstance(config["variants"], dict):
        config["variants"] = [dict(v, name=k) for k, v in config["variants"].items()]
    names = set()
    external_urls = set()
    for variant in config["variants"]:
        name = variant["name"]
        if not name or not all(c.isalnum() or c in "-_" for c in name) or name in names:
            raise ValueError("variant names must be unique, using letters, digits, '-' or '_'")
        names.add(name)
        variant.setdefault("engine", "psionic")
        if variant["engine"] not in ("psionic", "ollama"):
            raise ValueError("engine must be psionic or ollama")
        if ("binary" in variant) == ("url" in variant):
            raise ValueError("each variant needs exactly one binary or URL")
        if "binary" in variant:
            owned_process_limits(config, variant)
            variant["binary"] = resolve(path.parent, variant["binary"])
            if variant["engine"] == "ollama":
                variant["models_dir"] = resolve(path.parent, variant["models_dir"])
                if not Path(variant["models_dir"]).is_dir():
                    raise ValueError("an owned Ollama server needs an existing models_dir")
            elif "model_path" not in config or not Path(config["model_path"]).is_file():
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
            if (any(word in key.upper() for word in ("KEY", "TOKEN", "SECRET", "PASSWORD"))
                    and key != "PSIONIC_CLEF_BATCH_TOKENS"):
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
        if self.config["measurement_mode"] == "shared":
            self.phase = "run"
            self.guard()
            return
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


def validate_uncached_response(code, response):
    if code != 200 or not isinstance(response, dict) or "answers" not in response:
        raise RuntimeError("request failed or response has no answers")
    usage = response.get("usage")
    if (not isinstance(usage, dict) or type(usage.get("input_tokens")) is not int
            or usage["input_tokens"] <= 0):
        raise RuntimeError("response must report a positive actual input token count")
    cached = usage.get("cached_input_tokens", 0)
    if type(cached) is not int or cached != 0:
        raise RuntimeError("cached input invalidates an uncached comparison")
    timing = response.get("psionic", {}).get("timing", {})
    if timing.get("cached"):
        raise RuntimeError("cached answers invalidate an uncached comparison")


def stop_owned_group(process, grace=10):
    """Reap the group created by this Popen, including an exited leader's runners."""
    observations = []
    def signal_group(value):
        try:
            os.killpg(process.pid, value)
            return True
        except ProcessLookupError:
            return False
        except PermissionError as error:
            # Darwin can report EPERM while an exited group disappears. An
            # exited leader alone is insufficient: runners may still remain.
            try:
                sample = owned_group_rss(process)
                exit_code = owned_exit_code(process) if not sample["members"] else None
            except Exception:
                raise error
            if not sample["members"] and exit_code is not None:
                observations.append({"owned_pid": process.pid, "signal": int(value),
                                     "error_type": "PermissionError", "errno": error.errno,
                                     "leader_exit_code": exit_code,
                                     "verified_group_members": 0})
                return False
            raise

    def wait_group(seconds):
        deadline = time.monotonic() + seconds
        while True:
            process.poll()  # Reap the leader before checking for remaining children.
            if not signal_group(0):
                return True
            if time.monotonic() >= deadline:
                return False
            time.sleep(0.05)

    if signal_group(signal.SIGTERM) and not wait_group(grace):
        signal_group(signal.SIGKILL)
        if not wait_group(5):
            raise RuntimeError("the owned server group did not stop; comparison rejected")
    process.wait(timeout=1)
    return observations


def owned_process_limits(config, variant):
    limits = {key: variant.get(key, config.get(key, default)) for key, default in
              (("max_rss_gib", 36), ("startup_timeout", 120))}
    for key, maximum in (("max_rss_gib", 128), ("startup_timeout", 1800)):
        value = limits[key]
        if type(value) not in (int, float) or not math.isfinite(value) or not 0 < value <= maximum:
            raise ValueError(f"{key} must be finite, positive, and at most {maximum}")
    limits["max_rss_bytes"] = int(limits["max_rss_gib"] * (1024 ** 3))
    if limits["max_rss_bytes"] < 1:
        raise ValueError("max_rss_gib must allow at least one byte")
    margin = variant.get("minimum_available_memory_gib", config.get("minimum_available_memory_gib"))
    if margin is not None and (type(margin) not in (int, float) or not math.isfinite(margin)
                               or not 0 < margin <= 128):
        raise ValueError("minimum_available_memory_gib must be finite, positive, and at most 128")
    limits["minimum_available_memory_bytes"] = int(margin * (1024 ** 3)) if margin is not None else None
    return limits


class MemorySamplingError(RuntimeError):
    def __init__(self, sampler, command, error):
        self.details = {"sampler": sampler, "command": command,
                        "error_type": type(error).__name__}
        if isinstance(error, subprocess.TimeoutExpired):
            self.details["timeout_seconds"] = error.timeout
        elif isinstance(error, subprocess.CalledProcessError):
            self.details["exit_code"] = error.returncode
            self.details["stderr"] = (error.stderr or "")[-1024:]
        super().__init__(f"{sampler} memory sampler failed: {type(error).__name__}")


def memory_command(command, sampler):
    try:
        return subprocess.run(command, capture_output=True, text=True, timeout=5, check=True)
    except (subprocess.TimeoutExpired, subprocess.CalledProcessError, OSError) as error:
        raise MemorySamplingError(sampler, command, error) from error


def available_memory():
    """Return the configured macOS margin metric, not a claim of allocatable RAM."""
    if sys.platform != "darwin":
        raise RuntimeError("available-memory guard requires macOS vm_stat")
    result = memory_command(["vm_stat"], "vm_stat")
    page_size = re.search(r"page size of (\d+) bytes", result.stdout)
    if page_size is None or int(page_size.group(1)) <= 0:
        raise RuntimeError("vm_stat page size unavailable")
    pages = {}
    for name in ("free", "inactive", "speculative"):
        value = re.search(rf"^Pages {name}:\s+(\d+)\.?\s*$", result.stdout, re.MULTILINE)
        if value is None:
            raise RuntimeError(f"vm_stat {name} pages unavailable")
        pages[name] = int(value.group(1))
    return {"available_memory_bytes": sum(pages.values()) * int(page_size.group(1)),
            "page_size_bytes": int(page_size.group(1)), "pages": pages,
            "available_memory_formula": "vm_stat (free + inactive + speculative) * page_size"}


def read_owned_group_memory(process):
    """Read process metadata; inspect only the group created by this Popen."""
    if sys.platform == "darwin":
        result = memory_command([sys.executable, str(Path(__file__).with_name("mac_memory.py")),
                                 str(process.pid)], "libproc_owned_group")
        row = json.loads(result.stdout)
        if row["process_group"] != process.pid:
            raise RuntimeError("owned memory sample has the wrong process group")
        for key in ("rss_bytes", "physical_footprint_bytes", "accounted_bytes"):
            if type(row[key]) is not int or row[key] < 0 or any(
                type(member[key]) is not int or member[key] < 0 for member in row["members"]
            ) or row[key] != sum(member[key] for member in row["members"]):
                raise RuntimeError("invalid owned process memory sample")
        if any(type(member["pid"]) is not int or member["pid"] <= 0
               or member["accounted_bytes"] != max(member["rss_bytes"], member["physical_footprint_bytes"])
               for member in row["members"]):
            raise RuntimeError("invalid owned process memory member")
        return {"at": now(), **row}
    result = memory_command(["ps", "-axo", "pid=,pgid=,rss="], "ps_owned_group")
    members = []
    for line in result.stdout.splitlines():
        pid, group, kib = map(int, line.split())
        if group == process.pid:
            if pid <= 0 or kib < 0:
                raise RuntimeError("invalid owned process memory sample")
            members.append({"pid": pid, "rss_bytes": kib * 1024})
    return {"at": now(), "process_group": process.pid, "members": members,
            "rss_bytes": sum(member["rss_bytes"] for member in members)}


def owned_exit_code(process):
    code = process.poll()
    if code is None:
        try:
            code = process.wait(timeout=1)
        except subprocess.TimeoutExpired:
            return None
    return code


def owned_group_rss(process):
    """Allow a bounded gap between group disappearance and a waitable exit."""
    row = read_owned_group_memory(process)
    if not row["members"]:
        was_running = process.poll() is None
        code = owned_exit_code(process)
        if code is None:
            raise RuntimeError("owned group is empty but its leader did not exit within one second")
        if was_running:
            # A runner may have appeared during the exit wait. Keep observing
            # that group instead of treating the leader's exit as group exit.
            row = read_owned_group_memory(process)
            row["exit_grace_seconds"] = 1
        row["leader_exit_code"] = code
    return row


def recorded_server_env(env):
    def allowed(key):
        if key in ("RAYON_NUM_THREADS", "OMP_NUM_THREADS", "PSIONIC_CLEF_BATCH_TOKENS"):
            return True
        return key.startswith("PSIONIC_CLEF_") and not any(
            word in key.upper() for word in ("KEY", "TOKEN", "SECRET", "PASSWORD"))
    return {key: value for key, value in sorted(env.items()) if allowed(key)}


def owned_process_diagnostics(process):
    """Read identity details after cleanup fails; never retry a signal here."""
    details = {"owned_pid": process.pid, "guard_pgid": os.getpgrp(),
               "guard_uid": os.getuid(), "process_exit_code": process.poll()}
    try:
        details["observed_pgid"] = os.getpgid(process.pid)
    except OSError as error:
        details["observed_pgid_error"] = {"type": type(error).__name__, "errno": error.errno}
    try:
        result = subprocess.run(["ps", "-p", str(process.pid), "-o", "uid=,pgid="],
                                capture_output=True, text=True, timeout=5, check=False)
        fields = result.stdout.split()
        if len(fields) == 2:
            details["observed_uid"], details["observed_ps_pgid"] = map(int, fields)
        else:
            details["observed_uid_available"] = False
    except (OSError, subprocess.TimeoutExpired, ValueError) as error:
        details["observed_uid_error"] = type(error).__name__
    return details


class OwnedProcessGuard:
    """Bound a task-owned new-session child, including its same-group runners.

    Call close() in a finally block. The child must use start_new_session=True;
    this class never discovers or controls an existing application or service.
    """
    def __init__(self, process, max_rss_bytes, output, sample=owned_group_rss,
                 minimum_available_memory_bytes=None, memory_sample=available_memory):
        if type(max_rss_bytes) is not int or max_rss_bytes < 1:
            raise ValueError("max_rss_bytes must be a positive integer")
        if minimum_available_memory_bytes is not None and (type(minimum_available_memory_bytes) is not int
                                                           or minimum_available_memory_bytes < 1):
            raise ValueError("minimum_available_memory_bytes must be a positive integer")
        try:
            if os.getpgid(process.pid) != process.pid or process.pid == os.getpgrp():
                raise ValueError("the owned child must start a new process group")
        except ProcessLookupError:
            # The leader may exit before the monitor starts. Cleanup must still
            # cover runners left in the group created by this exact Popen.
            if process.poll() is None:
                raise
        self.process, self.max_rss_bytes, self.output = process, max_rss_bytes, Path(output)
        self.sample = sample
        self.minimum_available_memory_bytes = minimum_available_memory_bytes
        self.memory_sample = memory_sample
        self.lowest_available_memory_bytes = None
        self.stop = threading.Event()
        self.kill_lock = threading.Lock()
        self.stopped = False
        self.failure = None
        self.cleanup_error = None
        self.cleanup_diagnostics = None
        self.cleanup_observations = []
        self.sampling_error = None
        self.peak_rss_bytes = 0
        self.peak_physical_footprint_bytes = 0
        self.peak_accounted_bytes = 0
        self.sample_count = 0
        self.thread = threading.Thread(target=self.monitor, daemon=True)
        self.thread.start()

    def stop_process(self, grace=10):
        with self.kill_lock:
            if not self.stopped:
                try:
                    self.cleanup_observations = stop_owned_group(self.process, grace=grace) or []
                except BaseException as error:
                    self.cleanup_error = type(error).__name__
                    self.cleanup_diagnostics = owned_process_diagnostics(self.process)
                    raise
                self.stopped = True

    def monitor(self):
        try:
            with self.output.open("a", encoding="utf-8") as file:
                while not self.stop.is_set():
                    started = time.monotonic()
                    row = self.sample(self.process)
                    self.sample_count += 1
                    self.peak_rss_bytes = max(self.peak_rss_bytes, row["rss_bytes"])
                    self.peak_physical_footprint_bytes = max(self.peak_physical_footprint_bytes,
                                                           row.get("physical_footprint_bytes", 0))
                    accounted = row.get("accounted_bytes", row["rss_bytes"])
                    self.peak_accounted_bytes = max(self.peak_accounted_bytes, accounted)
                    row.update(max_rss_bytes=self.max_rss_bytes, sample_interval_seconds=0.5)
                    if row["rss_bytes"] > self.max_rss_bytes:
                        self.failure = "rss_limit"
                    elif accounted > self.max_rss_bytes:
                        self.failure = "physical_footprint_limit"
                    if self.minimum_available_memory_bytes is not None and not self.failure:
                        row.update(self.memory_sample())
                        available = row["available_memory_bytes"]
                        self.lowest_available_memory_bytes = (available if self.lowest_available_memory_bytes is None
                                                             else min(self.lowest_available_memory_bytes, available))
                        row["minimum_available_memory_bytes"] = self.minimum_available_memory_bytes
                        if available < self.minimum_available_memory_bytes:
                            self.failure = self.failure or "available_memory_margin"
                    row["rejection"] = self.failure
                    row["sampling_seconds"] = time.monotonic() - started
                    emit(file, row)
                    if self.failure:
                        self.stop_process(grace=1)
                        return
                    if not row["members"]:
                        return
                    self.stop.wait(0.5)
        except Exception as error:
            self.failure = self.failure or f"rss_monitor:{type(error).__name__}"
            self.sampling_error = (error.details if isinstance(error, MemorySamplingError)
                                   else {"error_type": type(error).__name__, "message": str(error)})
            try:
                with self.output.open("a", encoding="utf-8") as file:
                    emit(file, {"at": now(), "process_group": self.process.pid,
                                "rejection": self.failure, "sampling_error": self.sampling_error})
            except OSError:
                pass
            try:
                self.stop_process(grace=1)
            except Exception as cleanup:
                self.cleanup_error = type(cleanup).__name__

    def guard(self):
        if self.failure:
            raise RuntimeError(f"owned process memory guard rejected this run: {self.failure}")

    def close(self):
        self.stop.set()
        self.thread.join(timeout=10)
        if self.thread.is_alive():
            self.failure = self.failure or "rss_monitor:join_timeout"
        self.stop_process()
        if self.thread.is_alive() or self.cleanup_error:
            raise RuntimeError("owned process memory monitor cleanup failed")

    def receipt(self):
        return {"rss_log": self.output.name, "process_group": self.process.pid,
                "max_rss_bytes": self.max_rss_bytes, "peak_rss_bytes": self.peak_rss_bytes,
                "peak_physical_footprint_bytes": self.peak_physical_footprint_bytes,
                "peak_accounted_bytes": self.peak_accounted_bytes,
                "sampling_error": self.sampling_error, "sampler_timeout_seconds": 5,
                "rss_samples": self.sample_count, "sample_interval_seconds": 0.5,
                "minimum_available_memory_bytes": self.minimum_available_memory_bytes,
                "lowest_available_memory_bytes": self.lowest_available_memory_bytes,
                "available_memory_formula": ("vm_stat (free + inactive + speculative) * page_size"
                                             if self.minimum_available_memory_bytes is not None else None),
                "memory_rejection": self.failure, "cleanup_error": self.cleanup_error,
                "cleanup_diagnostics": self.cleanup_diagnostics,
                "cleanup_observations": self.cleanup_observations,
                "owned_group_stopped": self.stopped,
                "memory_limit_kind": "sampled per-member max(RSS, physical footprint) on macOS; RSS elsewhere; not a hard allocation limit"}


@contextlib.contextmanager
def server(config, variant, round_id, output, records, host, mock=False, export_dir=None):
    if "url" in variant:
        emit(records, {"kind": "startup", "variant": variant["name"], "round": round_id,
                       "at": now(), "owned": False, "startup_seconds": None})
        yield variant["url"]
        return
    limits = owned_process_limits(config, variant)
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    ollama = variant.get("engine", "psionic") == "ollama"
    command = ([variant["binary"], "serve"] if ollama else
               [variant["binary"], "-m", config["model_path"], "--host", "127.0.0.1",
                "--port", str(port), "--decision-device", "metal", "--decision-chunk", str(variant["chunk"])])
    head = None
    if not ollama and "clef_head" in config:
        command.extend(["--clef-head", config["clef_head"]])
        head = {"path": config["clef_head"], "sha256": file_sha(config["clef_head"]),
                "bytes": Path(config["clef_head"]).stat().st_size}
    if export_dir is not None:
        if ollama:
            raise ValueError("raw-logit export requires Psionic")
        command.extend(["--decision-export-rows", str(export_dir)])
    if mock:
        command = [sys.executable, str(Path(__file__).resolve()), "--_mock-port", str(port)]
    env = {k: v for k, v in os.environ.items() if not k.startswith(("PSIONIC_CLEF_", "OLLAMA_"))}
    env.update(variant.get("env", {}))
    owned_env = {}
    ollama_state_dir = None
    if ollama and not mock:
        scratch = Path(os.environ.get("OPENAGENTS_SCRATCH", str(Path.home() / ".openagents" / "scratch")))
        scratch.mkdir(parents=True, exist_ok=True)
        ollama_state_dir = tempfile.TemporaryDirectory(prefix="clef-ollama-state-", dir=scratch)
        owned_env = {"OLLAMA_HOST": f"127.0.0.1:{port}", "OLLAMA_MODELS": variant["models_dir"],
                     "OLLAMA_NOPRUNE": "true", "OLLAMA_NO_CLOUD": "true",
                     "OLLAMA_MAX_LOADED_MODELS": "1", "OLLAMA_NUM_PARALLEL": "1",
                     "OLLAMA_CONTEXT_LENGTH": "32768", "OLLAMA_KEEP_ALIVE": "10m",
                     "HOME": ollama_state_dir.name}
        env.update(owned_env)
    path = output / f"server-r{round_id}-{variant['name']}.log"
    row = {"kind": "startup", "variant": variant["name"], "round": round_id,
           "at": now(), "owned": True, "command": command, "log": path.name,
           "engine": variant.get("engine", "psionic"), "owned_server_env": owned_env,
           "effective_env": recorded_server_env(env),
           "external_head": head,
           "startup_timeout": limits["startup_timeout"], "max_rss_bytes": limits["max_rss_bytes"],
           "minimum_available_memory_bytes": limits["minimum_available_memory_bytes"]}
    process, memory = None, None
    began = time.perf_counter()
    try:
        host.guard()
        if limits["minimum_available_memory_bytes"] is not None:
            row.update(available_memory())
            required = limits["max_rss_bytes"] + limits["minimum_available_memory_bytes"]
            row["startup_required_available_memory_bytes"] = required
            if row["available_memory_bytes"] < required:
                row["memory_rejection"] = "startup_available_memory"
                raise RuntimeError("available memory is below the owned process budget plus margin")
        with open(path, "wb") as log:
            process = subprocess.Popen(command, env=env, stdin=subprocess.DEVNULL,
                                       stdout=log, stderr=log, start_new_session=True)
            memory = OwnedProcessGuard(process, limits["max_rss_bytes"],
                                       output / f"rss-r{round_id}-{variant['name']}.jsonl",
                                       minimum_available_memory_bytes=limits["minimum_available_memory_bytes"])
            deadline = time.monotonic() + limits["startup_timeout"]
            while True:
                memory.guard()
                host.guard()
                if process.poll() is not None:
                    raise RuntimeError(f"{variant['name']} exited before readiness; see server log")
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise RuntimeError(f"{variant['name']} exceeded its startup_timeout")
                try:
                    health_path = "/api/version" if ollama and not mock else "/health"
                    with HTTP.open(f"http://127.0.0.1:{port}{health_path}", timeout=min(1, remaining)) as response:
                        if response.status == 200:
                            if ollama and not mock:
                                row["version"] = json.loads(response.read()).get("version")
                            if time.monotonic() >= deadline:
                                raise RuntimeError(f"{variant['name']} exceeded its startup_timeout")
                            break
                except (OSError, urllib.error.URLError):
                    pass
                if time.monotonic() >= deadline:
                    raise RuntimeError(f"{variant['name']} exceeded its startup_timeout")
                time.sleep(0.2)
            row.update(startup_seconds=time.perf_counter() - began, status="ready")
            emit(records, row)
            yield f"http://127.0.0.1:{port}/v1/systemone"
            memory.guard()
    except BaseException as error:
        if "status" not in row:
            row.update(startup_seconds=time.perf_counter() - began, status="failed",
                       error_type=type(error).__name__)
            emit(records, row)
        raise
    finally:
        try:
            if memory is not None:
                try:
                    memory.close()
                finally:
                    emit(records, {"kind": "shutdown", "variant": variant["name"],
                                   "round": round_id, "at": now(), **memory.receipt()})
                memory.guard()
            elif process is not None:
                # This group was created by this exact Popen. It contains only
                # the owned server and its runners, never the desktop service.
                stop_owned_group(process)
        finally:
            if ollama_state_dir is not None:
                ollama_state_dir.cleanup()


def uncached_config(config):
    """Pin latency settings without changing callers of the shared server helper."""
    variants = []
    for original in config["variants"]:
        variant = dict(original)
        if "binary" in variant and variant.get("engine", "psionic") == "psionic":
            env = dict(variant.get("env", {}))
            if env.get("PSIONIC_CLEF_PREFIX_CACHE_BYTES", "0") != "0":
                raise ValueError("uncached latency requires PSIONIC_CLEF_PREFIX_CACHE_BYTES=0")
            env["PSIONIC_CLEF_PREFIX_CACHE_BYTES"] = "0"
            variant["env"] = env
        variants.append(variant)
    return dict(config, variants=variants)


def run(config, *, load=os.getloadavg, mock=False):
    config = uncached_config(config)
    lease_kind = "gpu" if config["measurement_mode"] == "shared" else "quiet"
    leases = os.environ.get("OPENAGENTS_LEASES", "").split(",")
    if not mock and lease_kind not in leases:
        raise ValueError(f"run this measurement through openagents lease {lease_kind}")
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
                "measurement_mode": config["measurement_mode"],
                "quiet_lease": os.environ.get("OPENAGENTS_LEASE_ID") if "quiet" in leases else None,
                "lease_id": os.environ.get("OPENAGENTS_LEASE_ID"), "lease_resources": leases,
                "self_test": mock,
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
                                    validate_uncached_response(code, response)
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
        shared = config["measurement_mode"] == "shared"
        groups = []
        for variant in config["variants"]:
            for path, _, _ in workloads:
                rows = [r for r in samples if r["variant"] == variant["name"] and r["workload"] == path
                        and not r["warmup"] and r["valid"]]
                groups.append({"variant": variant["name"], "workload": path, "count": len(rows),
                    "expected_count": config["rounds"] * config["runs"],
                    "median_seconds": statistics.median(r["elapsed_seconds"] for r in rows) if rows else None,
                    "input_tokens": sorted({r["usage"]["input_tokens"] for r in rows}),
                    "comparison_conditions": "shared machine" if shared else "quiet admission",
                    "usable_comparison": status == "complete" and not diagnostic})
        summary = {"status": status, "error_type": error, "finished_at": now(), "groups": groups,
                   "measurement_mode": config["measurement_mode"],
                   "monitor_error": host.monitor_error, "diagnostic_only": diagnostic,
                   "gpu_interference_detection": ("not enforced in shared mode; total GPU activity sampled" if shared
                                                  else "admission only; unavailable during inference"),
                   "idle_machine_acceptance": ("not applicable: shared machine" if shared
                                               else "not established by this harness"),
                   "note": "Medians exclude warmups and rejected samples. No quality verdict or tail-latency claim."}
        (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    return summary


def memory_self_test():
    """Exercise rejection paths without starting or signaling a process."""
    from types import SimpleNamespace
    from unittest.mock import patch
    from mac_memory import self_test as abi_test
    abi_test()
    scratch = Path(os.environ.get("OPENAGENTS_SCRATCH", str(Path.home() / ".openagents" / "scratch")))
    scratch.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="clef-memory-test-", dir=scratch) as directory:
        for case in ("rss_limit", "physical_footprint_limit", "available_memory_margin", "sampler_timeout"):
            process = SimpleNamespace(pid=101, poll=lambda: None)
            stopped = []
            def sample(owned):
                assert owned is process
                if case == "sampler_timeout":
                    raise MemorySamplingError("libproc_owned_group", ["python", "mac_memory.py", "101"],
                                              subprocess.TimeoutExpired(["python"], 5))
                return {"members": [{"pid": 101}], "rss_bytes": 101 if case == "rss_limit" else 1,
                        "physical_footprint_bytes": 101 if case == "physical_footprint_limit" else 1,
                        "accounted_bytes": 101 if case == "physical_footprint_limit" else 1}
            kwargs = {"minimum_available_memory_bytes": 1,
                      "memory_sample": lambda: {"available_memory_bytes": 0 if case == "available_memory_margin" else 10}}
            with patch.object(os, "getpgid", return_value=101), patch.object(os, "getpgrp", return_value=100), patch(__name__ + ".stop_owned_group", side_effect=lambda owned, grace: stopped.append(owned)):
                guard = OwnedProcessGuard(process, 100, Path(directory) / f"{case}.jsonl", sample=sample, **kwargs)
                guard.thread.join(timeout=2)
                guard.close()
            assert not guard.thread.is_alive() and stopped == [process]
            assert guard.failure == ("rss_monitor:MemorySamplingError" if case == "sampler_timeout" else case)
            if case == "sampler_timeout":
                assert guard.receipt()["sampling_error"]["sampler"] == "libproc_owned_group"
                assert guard.receipt()["sampling_error"]["timeout_seconds"] == 5
            try:
                guard.guard()
                raise AssertionError("memory failure accepted")
            except RuntimeError:
                pass
        process = SimpleNamespace(pid=101, poll=lambda: None)
        with patch.object(os, "getpgid", return_value=101), patch.object(os, "getpgrp", return_value=100), patch.object(os, "getuid", return_value=501), patch.object(subprocess, "run", return_value=SimpleNamespace(stdout="501 101\n")):
            details = owned_process_diagnostics(process)
        assert details == {"owned_pid": 101, "guard_pgid": 100, "guard_uid": 501,
                           "process_exit_code": None, "observed_pgid": 101,
                           "observed_uid": 501, "observed_ps_pgid": 101}
        guard = OwnedProcessGuard.__new__(OwnedProcessGuard)
        guard.process, guard.kill_lock, guard.stopped = process, threading.Lock(), False
        with patch(__name__ + ".stop_owned_group", side_effect=PermissionError(1, "injected refusal")) as stop, patch(__name__ + ".owned_process_diagnostics", return_value=details):
            try:
                guard.stop_process()
                raise AssertionError("cleanup refusal accepted")
            except PermissionError:
                pass
        assert stop.call_count == 1 and not guard.stopped
        assert guard.cleanup_error == "PermissionError" and guard.cleanup_diagnostics == details
        for exited, members, sample_error, accepted in [
            (True, [], None, True),
            (False, [], None, False),
            (True, [{"pid": 102}], None, False),
            (True, [], RuntimeError("unreadable group"), False),
        ]:
            def wait_for_exit(timeout):
                if not exited:
                    raise subprocess.TimeoutExpired(["owned child"], timeout)
                return -15
            process = SimpleNamespace(pid=101, poll=lambda: -15 if exited else None, wait=wait_for_exit)
            with patch.object(os, "killpg", side_effect=PermissionError(1, "injected refusal")) as signal_group, patch(__name__ + ".owned_group_rss", return_value={"members": members}, side_effect=sample_error) as sample_group:
                try:
                    observations = stop_owned_group(process)
                    assert accepted and all(row["verified_group_members"] == 0 for row in observations)
                except PermissionError:
                    assert not accepted
            assert all(call.args[0] == 101 for call in signal_group.call_args_list)
            assert all(call.args[1] != signal.SIGKILL for call in signal_group.call_args_list)
        process = SimpleNamespace(pid=101, poll=lambda: -15, wait=lambda timeout: -15)
        with patch.object(os, "killpg", side_effect=[None, PermissionError(1, "exited group")]) as signal_group, patch(__name__ + ".owned_group_rss", return_value={"members": []}):
            observations = stop_owned_group(process)
        assert [call.args for call in signal_group.call_args_list] == [(101, signal.SIGTERM), (101, 0)]
        assert len(observations) == 1 and observations[0]["signal"] == 0
        for completes, runners_after_exit in [(True, []), (False, []), (True, [{"pid": 102}])]:
            status = {"code": None, "waits": []}
            def wait_for_exit(timeout):
                status["waits"].append(timeout)
                if not completes:
                    raise subprocess.TimeoutExpired(["owned child"], timeout)
                status["code"] = 0
                return 0
            process = SimpleNamespace(pid=101, poll=lambda: status["code"], wait=wait_for_exit)
            with patch(__name__ + ".read_owned_group_memory", side_effect=[{"members": []}, {"members": runners_after_exit}]):
                try:
                    row = owned_group_rss(process)
                    assert completes and row["members"] == runners_after_exit and row["leader_exit_code"] == 0
                except RuntimeError:
                    assert not completes
            assert status["waits"] == [1]
        for completes in (True, False):
            status = {"code": None}
            def wait_for_exit(timeout):
                assert timeout == 1
                if not completes:
                    raise subprocess.TimeoutExpired(["owned child"], timeout)
                status["code"] = -15
                return -15
            process = SimpleNamespace(pid=101, poll=lambda: status["code"], wait=wait_for_exit)
            with patch.object(os, "killpg", side_effect=PermissionError(1, "exit race")) as signals, patch(__name__ + ".read_owned_group_memory", return_value={"members": []}):
                try:
                    observations = stop_owned_group(process)
                    assert completes and observations[0]["leader_exit_code"] == -15
                except PermissionError:
                    assert not completes
            assert signals.call_count == 1
    print("memory cap, footprint cap, host floor, and contextual timeout checks passed; no processes started or signaled")


def self_test():
    from types import SimpleNamespace
    from unittest.mock import patch

    scratch = Path(os.environ.get("OPENAGENTS_SCRATCH", str(Path.home() / ".openagents" / "scratch")))
    scratch.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="clef-harness-test-", dir=scratch) as directory:
        root = Path(directory)
        fixture = root / "request.json"
        fixture.write_text(json.dumps({"state": "test", "questions": []}))
        config_path = root / "config.json"
        config_path.write_text(json.dumps({"model_path": str(fixture), "clef_head": str(fixture), "requests": [str(fixture)],
            "output_dir": str(root / "results"), "rounds": 2, "runs": 2, "settle_seconds": 0,
            "variants": [{"name": name, "binary": sys.executable} for name in ("a", "b")]}))
        config = load_config(config_path)
        assert config["clef_head"] == str(fixture)
        pinned = uncached_config(config)
        assert all(variant["env"]["PSIONIC_CLEF_PREFIX_CACHE_BYTES"] == "0" for variant in pinned["variants"])
        assert all("env" not in variant for variant in config["variants"])
        try:
            uncached_config(dict(config, variants=[dict(config["variants"][0],
                env={"PSIONIC_CLEF_PREFIX_CACHE_BYTES": "512"})]))
            raise AssertionError("nonzero prefix cache accepted for uncached latency")
        except ValueError:
            pass
        assert recorded_server_env({"PSIONIC_CLEF_BATCH_TOKENS": "512",
            "PSIONIC_CLEF_METAL_FFN_WEIGHTS": "q4k-on-demand", "RAYON_NUM_THREADS": "2",
            "OMP_NUM_THREADS": "1", "PSIONIC_CLEF_SECRET": "do-not-record", "HOME": "do-not-record"}) == {
            "PSIONIC_CLEF_BATCH_TOKENS": "512", "PSIONIC_CLEF_METAL_FFN_WEIGHTS": "q4k-on-demand",
            "RAYON_NUM_THREADS": "2", "OMP_NUM_THREADS": "1"}
        summary = run(config, load=lambda: (0, 0, 0), mock=True)
        assert summary["status"] == "complete" and all(g["count"] == 4 for g in summary["groups"])
        rows = [json.loads(line) for line in (root / "results" / "records.jsonl").read_text().splitlines()]
        assert [r["variant"] for r in rows if r["kind"] == "startup"] == ["a", "b", "b", "a"]
        assert all(r["effective_env"]["PSIONIC_CLEF_PREFIX_CACHE_BYTES"] == "0"
                   for r in rows if r["kind"] == "startup")
        shutdowns = [r for r in rows if r["kind"] == "shutdown"]
        assert len(shutdowns) == 4 and all(r["owned_group_stopped"] and not r["memory_rejection"]
                                         and r["rss_samples"] > 0 for r in shutdowns)
        assert owned_process_limits(config, {"max_rss_gib": 84, "startup_timeout": 600,
                                            "minimum_available_memory_gib": 12}) == {
            "max_rss_gib": 84, "startup_timeout": 600, "max_rss_bytes": 84 * 1024 ** 3,
            "minimum_available_memory_bytes": 12 * 1024 ** 3}
        for key in ("max_rss_gib", "startup_timeout", "minimum_available_memory_gib"):
            for value in (0, -1, True, float("inf"), float("nan")):
                try:
                    owned_process_limits(config, {key: value})
                    raise AssertionError(f"invalid {key} accepted")
                except ValueError:
                    pass
        with patch.object(sys, "platform", "linux"), patch.object(subprocess, "run", return_value=SimpleNamespace(stdout="101 101 10\n102 101 30\n103 100 99\n")):
            sample = owned_group_rss(SimpleNamespace(pid=101))
            assert sample["rss_bytes"] == 40 * 1024
            assert [member["pid"] for member in sample["members"]] == [101, 102]
        import ctypes
        from mac_memory import RusageInfoV0
        assert ctypes.sizeof(RusageInfoV0) == 96 and ctypes.alignment(RusageInfoV0) == 8
        assert RusageInfoV0.ri_resident_size.offset == 64 and RusageInfoV0.ri_phys_footprint.offset == 72
        footprint = {"process_group": 101, "members": [
            {"pid": 101, "rss_bytes": 10, "physical_footprint_bytes": 40, "accounted_bytes": 40},
            {"pid": 102, "rss_bytes": 30, "physical_footprint_bytes": 20, "accounted_bytes": 30}],
            "rss_bytes": 40, "physical_footprint_bytes": 60, "accounted_bytes": 70}
        with patch.object(sys, "platform", "darwin"), patch.object(subprocess, "run", return_value=SimpleNamespace(stdout=json.dumps(footprint))) as read:
            sample = owned_group_rss(SimpleNamespace(pid=101))
            assert sample["accounted_bytes"] == 70 and sample["physical_footprint_bytes"] == 60
            assert read.call_args.kwargs["timeout"] == 5
            assert read.call_args.args[0][-1] == "101"
        for sampler, command in [("vm_stat", ["vm_stat"]), ("libproc_owned_group", ["python", "mac_memory.py", "101"])]:
            with patch.object(subprocess, "run", side_effect=subprocess.TimeoutExpired(command, 5)):
                try:
                    memory_command(command, sampler)
                    raise AssertionError("sampler timeout ignored")
                except MemorySamplingError as error:
                    assert error.details == {"sampler": sampler, "command": command,
                                             "error_type": "TimeoutExpired", "timeout_seconds": 5}
        vm_stat = ("Mach Virtual Memory Statistics: (page size of 16384 bytes)\n"
                   "Pages free: 10.\nPages inactive: 20.\nPages speculative: 30.\nPages wired down: 999.\n")
        with patch.object(sys, "platform", "darwin"), patch.object(subprocess, "run", return_value=SimpleNamespace(stdout=vm_stat)):
            assert available_memory()["available_memory_bytes"] == 60 * 16384
        with patch.object(sys, "platform", "darwin"), patch.object(subprocess, "run", return_value=SimpleNamespace(stdout="unavailable")):
            try:
                available_memory()
                raise AssertionError("missing vm_stat accepted")
            except RuntimeError:
                pass
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
        shared = Host(dict(config, measurement_mode="shared", settle_seconds=600), root,
                      load=lambda: (7, 7, 7), gpu=True)
        shared.last_gpu = 50
        shared.settle()
        assert shared.phase == "run"
        valid = {"answers": [], "usage": {"input_tokens": 10, "cached_input_tokens": 0}}
        validate_uncached_response(200, valid)
        invalid = [
            {"answers": [], "usage": {"input_tokens": 0}},
            {"answers": [], "usage": {"input_tokens": True}},
            {"answers": [], "usage": {"input_tokens": 10, "cached_input_tokens": 1}},
            dict(valid, psionic={"timing": {"cached": True}}),
        ]
        for response in invalid:
            try:
                validate_uncached_response(200, response)
                raise AssertionError("invalid uncached response accepted")
            except RuntimeError:
                pass
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
                variant = dict(config["variants"][0], env={"PSIONIC_CLEF_PREFIX_CACHE_BYTES": "512",
                    "PSIONIC_CLEF_BATCH_TOKENS": "256", "PSIONIC_CLEF_METAL_FFN_WEIGHTS": "q4k-on-demand",
                    "RAYON_NUM_THREADS": "2", "OMP_NUM_THREADS": "1"})
                with server(config, variant, 3, root, records, host, mock=True) as url:
                    port = urllib.parse.urlsplit(url).port
                    raise KeyboardInterrupt
            except KeyboardInterrupt:
                pass
        startup = json.loads((root / "cancellation.jsonl").read_text().splitlines()[0])
        assert startup["external_head"] == {"path": str(fixture),
            "sha256": file_sha(fixture), "bytes": fixture.stat().st_size}
        assert startup["effective_env"] == variant["env"]
        with socket.socket() as sock:
            assert sock.connect_ex(("127.0.0.1", port)) != 0
        # A startup timeout and a failed host-margin check must stop or avoid
        # starting the owned child; neither test controls another process.
        with (root / "startup-timeout.jsonl").open("w") as records:
            try:
                variant = dict(config["variants"][0], startup_timeout=0.001)
                with server(config, variant, 4, root, records, host, mock=True):
                    raise AssertionError("startup deadline ignored")
            except RuntimeError as error:
                assert "startup_timeout" in str(error)
        with patch(__name__ + ".available_memory", return_value={"available_memory_bytes": 95 * 1024 ** 3}):
            with (root / "startup-memory.jsonl").open("w") as records:
                try:
                    variant = dict(config["variants"][0], max_rss_gib=84, minimum_available_memory_gib=12)
                    with server(config, variant, 5, root, records, host, mock=True):
                        raise AssertionError("insufficient startup memory accepted")
                except RuntimeError as error:
                    assert "budget plus margin" in str(error)
        assert not (root / "server-r5-a.log").exists()
        for case in ("rss_limit", "physical_footprint_limit", "available_memory_margin", "monitor_failure", "sampler_timeout"):
            child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"],
                                     start_new_session=True, stdin=subprocess.DEVNULL,
                                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            kwargs = {}
            if case == "available_memory_margin":
                kwargs = {"minimum_available_memory_bytes": 1,
                          "memory_sample": lambda: {"available_memory_bytes": 0}}
            elif case == "physical_footprint_limit":
                def footprint_sample(process):
                    return {"members": [{"pid": process.pid}], "rss_bytes": 10,
                            "physical_footprint_bytes": 37 * 1024 ** 3,
                            "accounted_bytes": 37 * 1024 ** 3}
                kwargs = {"sample": footprint_sample}
            elif case == "monitor_failure":
                def fail_sample(process):
                    raise RuntimeError("injected sample failure")
                kwargs = {"sample": fail_sample}
            elif case == "sampler_timeout":
                def timeout_sample(process):
                    raise MemorySamplingError("vm_stat", ["vm_stat"], subprocess.TimeoutExpired(["vm_stat"], 5))
                kwargs = {"sample": timeout_sample}
            guard = OwnedProcessGuard(child, 1 if case == "rss_limit" else 36 * 1024 ** 3,
                                      root / f"guard-{case}.jsonl", **kwargs)
            try:
                guard.thread.join(timeout=5)
                assert not guard.thread.is_alive()
                try:
                    guard.guard()
                    raise AssertionError("memory failure accepted")
                except RuntimeError:
                    pass
            finally:
                try:
                    guard.close()
                except BaseException:
                    print(json.dumps({"self_test_case": case, "memory": guard.receipt()}), file=sys.stderr)
                    raise
            assert guard.stopped and child.poll() is not None
            if case not in ("monitor_failure", "sampler_timeout"):
                assert guard.failure == case
            if case == "sampler_timeout":
                assert guard.receipt()["sampling_error"]["sampler"] == "vm_stat"
                failure = json.loads((root / f"guard-{case}.jsonl").read_text().splitlines()[-1])
                assert failure["sampling_error"]["timeout_seconds"] == 5
        # An exited leader must not leave a runner behind. The synthetic child
        # deliberately ignores SIGTERM; cleanup must finish before proceeding.
        ready = root / "orphan-ready"
        child_code = ("import pathlib,signal,sys,time; "
                      "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                      "pathlib.Path(sys.argv[1]).write_text('ready'); time.sleep(60)")
        leader_code = ("import subprocess,sys; "
                       "subprocess.Popen([sys.executable,'-c',sys.argv[1],sys.argv[2]], "
                       "stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)")
        leader = subprocess.Popen([sys.executable, "-c", leader_code, child_code, str(ready)],
                                  start_new_session=True)
        try:
            leader.wait(timeout=5)
            deadline = time.monotonic() + 5
            while not ready.exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            assert ready.exists(), "synthetic runner did not start"
        finally:
            stop_owned_group(leader, grace=0.05)
    print("Self-test passed: paired requests, cache rejection, shared mode, admission, startup deadline, process-group RSS, host-memory margin, monitoring failure, cancellation, and orphan cleanup.")


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
    parser.add_argument("--self-test-memory", action="store_true")
    parser.add_argument("--_mock-port", type=int, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args._mock_port:
        mock_server(args._mock_port)
    elif args.self_test_memory:
        memory_self_test()
    elif args.self_test:
        self_test()
    elif args.config:
        sys.exit(0 if run(load_config(args.config))["status"] == "complete" else 1)
    else:
        parser.error("provide a config path or --self-test")
