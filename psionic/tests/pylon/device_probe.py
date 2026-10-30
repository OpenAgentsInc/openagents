#!/usr/bin/env python3
"""Actual GPU preflight must compile kernels without reading a model/listening.

Usage: device_probe.py GPU_BINARY; optional CPU_BINARY checks no CPU fallback.
Run on a qualified native GPU host, not as a fake driver/source-string test.
"""
from pathlib import Path
import socket
import subprocess
import sys
import tempfile

gpu = str(Path(sys.argv[1]).resolve())
with tempfile.TemporaryDirectory(prefix="pylon-device-probe-") as temporary:
    missing = str(Path(temporary) / "no-checkpoint-here")
    with socket.socket() as guard:
        guard.bind(("127.0.0.1", 0))
        port = guard.getsockname()[1]
        # An occupied port and nonexistent model/table prove this command
        # never opens a listener or checkpoints, even if normal flags occur.
        result = subprocess.run([gpu, "--check-device", "--capacity", "512", "--port", str(port),
                                 "--table", missing], capture_output=True, text=True, timeout=120)
        assert result.returncode == 0, result.stdout + result.stderr
        assert "device preflight passed" in result.stdout, result.stdout
        assert "no checkpoint opened" in result.stdout, result.stdout
    for args in (("--device-mib", "1"), ("--capacity", "513")):
        result = subprocess.run([gpu, "--check-device", *args], capture_output=True, text=True, timeout=120)
        assert result.returncode != 0, result.stdout
        assert "device preflight refused" in result.stderr, result.stderr
if len(sys.argv) > 2:
    result = subprocess.run([str(Path(sys.argv[2]).resolve()), "--check-device"], capture_output=True, text=True, timeout=10)
    assert result.returncode != 0 and "no CPU fallback" in result.stderr, result.stdout + result.stderr
print("device preflight: actual generated kernels, memory refusal, shape refusal; no model/listener or CPU fallback")
