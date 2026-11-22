#!/usr/bin/env python3
# Firm reason: 5 (local Metal benchmark host-cleanliness preflight & mid-run interference guard for run_tasks.py)
"""Host cleanliness preflight & mid-run interference guard for local Metal/GPU benchmarks.

Prevents noisy or contaminated local inference benchmarks by checking:
  1. Active competing GPU/LLM inference servers (instant CPU% or CPU-time delta over a
     sampling window, excluding our own target port/PID).
  2. Concurrent eval runners (other host_driver / run_tasks.py processes) or heavy
     compiler/build storms.
  3. Unified memory pressure (macOS memory_pressure free% < 30% or available RAM < 32 GB).
  4. macOS thermal throttling (pmset -g therm CPU_Speed_Limit < 85 or Thermal_Level > 0).
  5. Excess CPU load average (1-minute load > max(8, cpu_count)).

Idle parked daemons (e.g. menu-bar Ollama or an idle resident server with 0 CPU-time delta
and ample unified memory headroom) are logged in metadata without causing false positives;
the moment any competing server burns CPU/GPU time before or during a benchmark task, the
guard flags interference and aborts the benchmark (unless --allow-dirty-host is passed).
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass, field
from typing import Any

GPU_SERVER_PATTERNS = (
    "psionic-inkling-serve",
    "psionic-openai-server",
    "psionic-serve",
    "llama-server",
    "ollama serve",
    "mlx_lm",
    "vllm",
    "lmstudio",
)

EVAL_RUNNER_PATTERNS = (
    "host_driver",
    "tier2/run_tasks.py",
)

BUILD_STORM_PATTERNS = (
    "swiftc",
    "swift-frontend",
    "clang",
    "xcodebuild",
    "cargo build",
    "coax mutate",
)


@dataclass
class ProcSample:
    pid: int
    pcpu: float
    cputime_ms: int
    rss_kb: int
    command: str

    @property
    def rss_mb(self) -> int:
        return self.rss_kb // 1024


@dataclass
class HostCheckResult:
    clean: bool
    reasons: list[str] = field(default_factory=list)
    active_gpu_procs: list[dict[str, Any]] = field(default_factory=list)
    idle_gpu_procs: list[dict[str, Any]] = field(default_factory=list)
    concurrent_evals: list[dict[str, Any]] = field(default_factory=list)
    build_storms: list[dict[str, Any]] = field(default_factory=list)
    mem_free_pct: int | None = None
    total_ram_mb: int = 0
    competing_gpu_rss_mb: int = 0
    load_avg_1m: float = 0.0
    cpu_count: int = 1
    thermal_speed_limit: int | None = None
    thermal_level: int | None = None

    def to_dict(self) -> dict[str, Any]:
        return {
            "clean": self.clean,
            "reasons": self.reasons,
            "active_gpu_procs": self.active_gpu_procs,
            "idle_gpu_procs": self.idle_gpu_procs,
            "concurrent_evals": self.concurrent_evals,
            "build_storms": self.build_storms,
            "mem_free_pct": self.mem_free_pct,
            "total_ram_mb": self.total_ram_mb,
            "competing_gpu_rss_mb": self.competing_gpu_rss_mb,
            "load_avg_1m": round(self.load_avg_1m, 2),
            "cpu_count": self.cpu_count,
            "thermal_speed_limit": self.thermal_speed_limit,
            "thermal_level": self.thermal_level,
        }


def parse_cputime_ms(s: str) -> int:
    """Parse ps `time` column: `MM:SS.cc`, `HH:MM:SS`, or `D-HH:MM:SS` -> milliseconds."""
    s = s.strip()
    if not s:
        return 0
    days = 0
    if "-" in s:
        d_part, s = s.split("-", 1)
        try:
            days = int(d_part)
        except ValueError:
            days = 0
    parts = s.split(":")
    try:
        if len(parts) == 2:
            mins = int(parts[0])
            secs = float(parts[1])
            return int(round((days * 86400 + mins * 60 + secs) * 1000))
        if len(parts) == 3:
            hrs = int(parts[0])
            mins = int(parts[1])
            secs = float(parts[2])
            return int(round((days * 86400 + hrs * 3600 + mins * 60 + secs) * 1000))
    except ValueError:
        return 0
    return 0


def parse_ps_output(text: str) -> list[ProcSample]:
    samples: list[ProcSample] = []
    for line in text.splitlines()[1:]:
        parts = line.strip().split(None, 4)
        if len(parts) < 5:
            continue
        try:
            pid = int(parts[0])
            pcpu = float(parts[1])
            cputime_ms = parse_cputime_ms(parts[2])
            rss_kb = int(parts[3])
            cmd = parts[4]
        except ValueError:
            continue
        samples.append(ProcSample(pid=pid, pcpu=pcpu, cputime_ms=cputime_ms, rss_kb=rss_kb, command=cmd))
    return samples


def sample_processes() -> list[ProcSample]:
    r = subprocess.run(
        ["ps", "-axo", "pid,pcpu,time,rss,command"],
        capture_output=True, text=True, timeout=10,
    )
    if r.returncode != 0:
        return []
    return parse_ps_output(r.stdout)


def extract_port(cmd: str) -> int | None:
    m = re.search(r"--port(?:=|\s+)(\d+)", cmd)
    if m:
        return int(m.group(1))
    m = re.search(r"-p\s+(\d+)", cmd)
    if m:
        return int(m.group(1))
    return None


def is_gpu_server_cmd(cmd: str) -> bool:
    low = cmd.lower()
    return any(pat in low for pat in GPU_SERVER_PATTERNS)


def is_eval_runner_cmd(cmd: str) -> bool:
    low = cmd.lower().strip()
    if low.startswith(("sh -c", "zsh -c", "bash -c", "/bin/sh", "/bin/zsh", "/bin/bash")):
        return False
    return any(pat in low for pat in EVAL_RUNNER_PATTERNS)


def is_build_storm_cmd(cmd: str) -> bool:
    low = cmd.lower()
    return any(pat in low for pat in BUILD_STORM_PATTERNS)


def is_own_server(p: ProcSample, own_pid: int, own_port: int | None, own_server_pid: int | None, own_ppid: int | None = None) -> bool:
    if p.pid == own_pid:
        return True
    if own_ppid is not None and p.pid == own_ppid:
        return True
    if own_server_pid is not None and p.pid == own_server_pid:
        return True
    if own_port is not None and own_port > 0:
        port = extract_port(p.command)
        if port == own_port and "psionic-inkling-serve" in p.command.lower():
            return True
    return False


def probe_memory_free_pct() -> int | None:
    if sys.platform != "darwin":
        return None
    try:
        r = subprocess.run(["memory_pressure", "-Q"], capture_output=True, text=True, timeout=5)
        m = re.search(r"System-wide memory free percentage:\s*(\d+)%", r.stdout)
        if m:
            return int(m.group(1))
    except Exception:
        pass
    return None


def probe_total_ram_mb() -> int:
    try:
        if sys.platform == "darwin":
            r = subprocess.run(["sysctl", "-n", "hw.memsize"], capture_output=True, text=True, timeout=5)
            return int(r.stdout.strip()) // (1024 * 1024)
        pages = os.sysconf("SC_PHYS_PAGES")
        psize = os.sysconf("SC_PAGE_SIZE")
        return (pages * psize) // (1024 * 1024)
    except Exception:
        return 0


def probe_thermal_mac() -> tuple[int | None, int | None]:
    if sys.platform != "darwin":
        return None, None
    try:
        r = subprocess.run(["pmset", "-g", "therm"], capture_output=True, text=True, timeout=5)
        spd = None
        lvl = None
        m_spd = re.search(r"CPU_Speed_Limit\s*=\s*(\d+)", r.stdout)
        if m_spd:
            spd = int(m_spd.group(1))
        m_lvl = re.search(r"Thermal_Level\s*=\s*(\d+)", r.stdout)
        if m_lvl:
            lvl = int(m_lvl.group(1))
        return spd, lvl
    except Exception:
        return None, None


def evaluate_host_cleanliness(
    before: list[ProcSample],
    after: list[ProcSample],
    *,
    own_pid: int,
    own_ppid: int | None = None,
    own_port: int | None = None,
    own_server_pid: int | None = None,
    mem_free_pct: int | None = None,
    total_ram_mb: int = 0,
    load_avg_1m: float = 0.0,
    cpu_count: int = 1,
    thermal_speed_limit: int | None = None,
    thermal_level: int | None = None,
    active_delta_ms: int = 30,
    active_pcpu: float = 1.5,
    min_free_pct: int = 30,
) -> HostCheckResult:
    """Pure decision function classifying whether competing host activity would interfere."""
    before_by_pid = {p.pid: p for p in before}
    reasons: list[str] = []
    active_gpu: list[dict[str, Any]] = []
    idle_gpu: list[dict[str, Any]] = []
    evals: list[dict[str, Any]] = []
    storms: list[dict[str, Any]] = []
    competing_rss_mb = 0

    for p in after:
        if is_own_server(p, own_pid=own_pid, own_port=own_port, own_server_pid=own_server_pid, own_ppid=own_ppid):
            continue
        prev = before_by_pid.get(p.pid)
        delta_ms = (p.cputime_ms - prev.cputime_ms) if prev is not None else p.cputime_ms

        if is_eval_runner_cmd(p.command):
            evals.append({"pid": p.pid, "pcpu": p.pcpu, "rss_mb": p.rss_mb, "cmd": p.command[:140]})
            continue

        if is_gpu_server_cmd(p.command):
            competing_rss_mb += p.rss_mb
            port = extract_port(p.command)
            entry = {
                "pid": p.pid,
                "port": port,
                "pcpu": p.pcpu,
                "delta_cputime_ms": delta_ms,
                "rss_mb": p.rss_mb,
                "cmd": p.command[:140],
            }
            if p.pcpu >= active_pcpu or delta_ms >= active_delta_ms:
                active_gpu.append(entry)
            else:
                idle_gpu.append(entry)
            continue

        if is_build_storm_cmd(p.command) and (p.pcpu >= 25.0 or delta_ms >= 350):
            storms.append({"pid": p.pid, "pcpu": p.pcpu, "delta_cputime_ms": delta_ms, "cmd": p.command[:140]})

    if active_gpu:
        desc = ", ".join(f"pid={x['pid']} port={x['port']} cpu={x['pcpu']:.1f}% dcpu={x['delta_cputime_ms']}ms rss={x['rss_mb']}MB" for x in active_gpu)
        reasons.append(f"active competing GPU/LLM server(s): {desc}")
    if evals:
        desc = ", ".join(f"pid={x['pid']} cmd={x['cmd'][:60]}" for x in evals)
        reasons.append(f"concurrent eval runner(s) detected: {desc}")
    if storms:
        desc = ", ".join(f"pid={x['pid']} cpu={x['pcpu']:.1f}% cmd={x['cmd'][:50]}" for x in storms)
        reasons.append(f"heavy concurrent build/compile activity: {desc}")
    if mem_free_pct is not None and mem_free_pct < min_free_pct:
        reasons.append(f"unified memory pressure: only {mem_free_pct}% free (< {min_free_pct}% floor)")
    if total_ram_mb > 0 and competing_rss_mb > 0:
        # Reserve 28 GB for Inkling 2 Mini 32K + OS; block if competing servers push remaining RAM below 28 GB.
        if total_ram_mb - competing_rss_mb < 28672:
            reasons.append(
                f"competing GPU servers hold {competing_rss_mb} MB RSS on {total_ram_mb} MB host (< 28 GB remaining for 32K Inkling)"
            )
    load_limit = max(8.0, float(cpu_count) * 1.25)
    if load_avg_1m > load_limit:
        reasons.append(f"high CPU load average {load_avg_1m:.2f} (> {load_limit:.1f} limit for {cpu_count} cores)")
    if thermal_speed_limit is not None and thermal_speed_limit < 85:
        reasons.append(f"thermal throttling active: CPU_Speed_Limit={thermal_speed_limit}% (< 85%)")
    if thermal_level is not None and thermal_level > 0:
        reasons.append(f"thermal warning active: Thermal_Level={thermal_level}")

    return HostCheckResult(
        clean=len(reasons) == 0,
        reasons=reasons,
        active_gpu_procs=active_gpu,
        idle_gpu_procs=idle_gpu,
        concurrent_evals=evals,
        build_storms=storms,
        mem_free_pct=mem_free_pct,
        total_ram_mb=total_ram_mb,
        competing_gpu_rss_mb=competing_rss_mb,
        load_avg_1m=load_avg_1m,
        cpu_count=cpu_count,
        thermal_speed_limit=thermal_speed_limit,
        thermal_level=thermal_level,
    )


def check_host_preflight(
    *,
    own_port: int | None = None,
    own_server_pid: int | None = None,
    sample_window_s: float = 1.5,
) -> HostCheckResult:
    before = sample_processes()
    if sample_window_s > 0:
        time.sleep(sample_window_s)
    after = sample_processes()
    load_1m = os.getloadavg()[0] if hasattr(os, "getloadavg") else 0.0
    cpus = os.cpu_count() or 1
    mem_free = probe_memory_free_pct()
    ram_mb = probe_total_ram_mb()
    spd, lvl = probe_thermal_mac()
    return evaluate_host_cleanliness(
        before, after,
        own_pid=os.getpid(),
        own_ppid=os.getppid(),
        own_port=own_port,
        own_server_pid=own_server_pid,
        mem_free_pct=mem_free,
        total_ram_mb=ram_mb,
        load_avg_1m=load_1m,
        cpu_count=cpus,
        thermal_speed_limit=spd,
        thermal_level=lvl,
    )


def detect_midrun_interference(
    before: list[ProcSample],
    after: list[ProcSample],
    *,
    own_pid: int,
    own_ppid: int | None = None,
    own_port: int | None = None,
    own_server_pid: int | None = None,
    delta_threshold_ms: int = 150,
) -> list[dict[str, Any]]:
    """Checks whether any competing GPU/LLM server actively burned CPU/GPU time during a task."""
    before_by_pid = {p.pid: p for p in before}
    hits: list[dict[str, Any]] = []
    for p in after:
        if is_own_server(p, own_pid=own_pid, own_port=own_port, own_server_pid=own_server_pid, own_ppid=own_ppid):
            continue
        if not is_gpu_server_cmd(p.command):
            continue
        prev = before_by_pid.get(p.pid)
        delta_ms = (p.cputime_ms - prev.cputime_ms) if prev is not None else p.cputime_ms
        if delta_ms >= delta_threshold_ms or p.pcpu >= 3.0:
            hits.append({
                "pid": p.pid,
                "port": extract_port(p.command),
                "pcpu": p.pcpu,
                "delta_cputime_ms": delta_ms,
                "rss_mb": p.rss_mb,
                "spawned_mid_task": prev is None,
                "cmd": p.command[:140],
            })
    return hits


def format_preflight_summary(res: HostCheckResult) -> str:
    status = "CLEAN" if res.clean else "DIRTY (REFUSED)"
    mem_s = f"{res.mem_free_pct}% free" if res.mem_free_pct is not None else "n/a"
    lines = [
        f"[host-preflight] {status} | mem={mem_s} (total={res.total_ram_mb}MB, other_gpu_rss={res.competing_gpu_rss_mb}MB) "
        f"| load={res.load_avg_1m:.2f}/{res.cpu_count}c | active_gpu={len(res.active_gpu_procs)} idle_gpu={len(res.idle_gpu_procs)}"
    ]
    for reason in res.reasons:
        lines.append(f"  BLOCK: {reason}")
    return "\n".join(lines)


def main() -> int:
    ap = argparse.ArgumentParser(description="Check if this host is clean enough for local Metal/GPU benchmarks.")
    ap.add_argument("--port", type=int, default=0, help="exclude our own psionic-inkling-serve on this port")
    ap.add_argument("--window", type=float, default=1.5, help="CPU-time delta sampling window in seconds")
    ap.add_argument("--json", action="store_true", help="emit JSON report")
    args = ap.parse_args()
    res = check_host_preflight(own_port=args.port or None, sample_window_s=args.window)
    if args.json:
        print(json.dumps(res.to_dict(), indent=2))
    else:
        print(format_preflight_summary(res))
    return 0 if res.clean else 2


if __name__ == "__main__":
    sys.exit(main())
