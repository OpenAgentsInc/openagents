#!/usr/bin/env python3
# Firm reason: 5 (unit tests for host_clean.py benchmark cleanliness preflight & interference guard)
"""Unit tests for host_clean.py preflight & mid-run interference guard."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import host_clean as hc


class TestHostClean(unittest.TestCase):
    def test_parse_cputime_ms(self):
        self.assertEqual(hc.parse_cputime_ms("0:00.31"), 310)
        self.assertEqual(hc.parse_cputime_ms("1:54.58"), 114580)
        self.assertEqual(hc.parse_cputime_ms("78:16.05"), 4696050)
        self.assertEqual(hc.parse_cputime_ms("01:02:03"), 3723000)
        self.assertEqual(hc.parse_cputime_ms("1-00:00:01"), 86401000)

    def test_idle_parked_daemons_pass_with_headroom(self):
        before = [
            hc.ProcSample(pid=100, pcpu=0.0, cputime_ms=5000, rss_kb=4 * 1024 * 1024, command="psionic-inkling-serve --port 18795 --ctx 32768"),
            hc.ProcSample(pid=200, pcpu=0.0, cputime_ms=2000, rss_kb=25 * 1024, command="/Applications/Ollama.app/Contents/Resources/ollama serve"),
            hc.ProcSample(pid=300, pcpu=12.0, cputime_ms=9000, rss_kb=4 * 1024 * 1024, command="psionic-inkling-serve --port 18998 --ctx 32768"),
        ]
        after = [
            hc.ProcSample(pid=100, pcpu=0.0, cputime_ms=5000, rss_kb=4 * 1024 * 1024, command="psionic-inkling-serve --port 18795 --ctx 32768"),
            hc.ProcSample(pid=200, pcpu=0.0, cputime_ms=2000, rss_kb=25 * 1024, command="/Applications/Ollama.app/Contents/Resources/ollama serve"),
            hc.ProcSample(pid=300, pcpu=25.0, cputime_ms=9500, rss_kb=4 * 1024 * 1024, command="psionic-inkling-serve --port 18998 --ctx 32768"),
        ]
        res = hc.evaluate_host_cleanliness(
            before, after, own_pid=999, own_port=18998,
            mem_free_pct=80, total_ram_mb=131072, load_avg_1m=3.2, cpu_count=16,
            thermal_speed_limit=100, thermal_level=0,
        )
        self.assertTrue(res.clean, res.reasons)
        self.assertEqual(len(res.active_gpu_procs), 0)
        self.assertEqual(len(res.idle_gpu_procs), 2)

    def test_active_competing_gpu_server_refused(self):
        before = [
            hc.ProcSample(pid=100, pcpu=0.1, cputime_ms=5000, rss_kb=4 * 1024 * 1024, command="psionic-inkling-serve --port 18795 --ctx 32768"),
        ]
        after = [
            hc.ProcSample(pid=100, pcpu=0.4, cputime_ms=5120, rss_kb=4 * 1024 * 1024, command="psionic-inkling-serve --port 18795 --ctx 32768"),
        ]
        res = hc.evaluate_host_cleanliness(
            before, after, own_pid=999, own_port=18998,
            mem_free_pct=80, total_ram_mb=131072, load_avg_1m=3.2, cpu_count=16,
        )
        self.assertFalse(res.clean)
        self.assertEqual(len(res.active_gpu_procs), 1)
        self.assertEqual(res.active_gpu_procs[0]["delta_cputime_ms"], 120)

    def test_low_ram_competing_resident_refused(self):
        procs = [
            hc.ProcSample(pid=100, pcpu=0.0, cputime_ms=1000, rss_kb=12 * 1024 * 1024, command="llama-server --port 18794"),
        ]
        res = hc.evaluate_host_cleanliness(
            procs, procs, own_pid=999, own_port=18998,
            mem_free_pct=45, total_ram_mb=32768, load_avg_1m=2.0, cpu_count=12,
        )
        self.assertFalse(res.clean)
        self.assertTrue(any("remaining" in r for r in res.reasons))

    def test_memory_pressure_and_thermal_refused(self):
        res = hc.evaluate_host_cleanliness(
            [], [], own_pid=999,
            mem_free_pct=22, total_ram_mb=65536, load_avg_1m=2.0, cpu_count=16,
            thermal_speed_limit=72, thermal_level=10,
        )
        self.assertFalse(res.clean)
        self.assertEqual(len(res.reasons), 3)

    def test_concurrent_eval_runner_refused(self):
        procs = [
            hc.ProcSample(pid=555, pcpu=1.2, cputime_ms=300, rss_kb=20000, command="/path/host_driver --worker /path/inky"),
            hc.ProcSample(pid=556, pcpu=0.1, cputime_ms=10, rss_kb=4000, command="zsh -c python3 evals/openagents-desktop/tier2/run_tasks.py"),
        ]
        res = hc.evaluate_host_cleanliness(
            procs, procs, own_pid=999,
            mem_free_pct=75, total_ram_mb=131072, load_avg_1m=2.0, cpu_count=16,
        )
        self.assertFalse(res.clean)
        self.assertEqual(len(res.concurrent_evals), 1)
        self.assertEqual(res.concurrent_evals[0]["pid"], 555)

    def test_midrun_interference_catches_spawn_and_active_delta(self):
        before = [
            hc.ProcSample(pid=100, pcpu=0.0, cputime_ms=5000, rss_kb=4 * 1024 * 1024, command="psionic-inkling-serve --port 18795"),
            hc.ProcSample(pid=300, pcpu=2.0, cputime_ms=1000, rss_kb=4 * 1024 * 1024, command="psionic-inkling-serve --port 18998"),
        ]
        after = [
            hc.ProcSample(pid=100, pcpu=0.0, cputime_ms=5000, rss_kb=4 * 1024 * 1024, command="psionic-inkling-serve --port 18795"),
            hc.ProcSample(pid=300, pcpu=8.0, cputime_ms=4200, rss_kb=4 * 1024 * 1024, command="psionic-inkling-serve --port 18998"),
            hc.ProcSample(pid=400, pcpu=4.2, cputime_ms=420, rss_kb=4 * 1024 * 1024, command="psionic-inkling-serve --port 18799"),
        ]
        hits = hc.detect_midrun_interference(before, after, own_pid=999, own_port=18998)
        self.assertEqual(len(hits), 1)
        self.assertEqual(hits[0]["pid"], 400)
        self.assertTrue(hits[0]["spawned_mid_task"])


if __name__ == "__main__":
    unittest.main()
