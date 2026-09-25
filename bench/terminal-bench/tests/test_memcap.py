"""The harness's own memory caps (#9596)."""

from __future__ import annotations

import os
import resource
import shutil
import signal
import subprocess
import sys

import pytest

from tbench import memcap


def test_byte_counts_read_with_and_without_a_suffix():
    assert memcap.parse_bytes("1048576") == 1 << 20
    assert memcap.parse_bytes("16G") == 16 << 30
    assert memcap.parse_bytes("16GiB") == 16 << 30
    assert memcap.parse_bytes("512m") == 512 << 20
    assert memcap.parse_bytes("64KB") == 64 << 10


def test_none_off_and_zero_mean_no_cap():
    for text in ("none", "OFF", "0", "0G"):
        assert memcap.parse_bytes(text) is None


def test_an_unreadable_override_keeps_the_default():
    assert memcap.cap("X", 5, {"X": "lots"}) == 5
    assert memcap.cap("X", 5, {}) == 5
    assert memcap.cap("X", 5, {"X": "2G"}) == 2 << 30
    assert memcap.cap("X", 5, {"X": "none"}) is None


def test_the_innermost_user_slice_is_found(tmp_path):
    cgroup = tmp_path / "cgroup"
    cgroup.write_text(
        "0::/user.slice/user-1000.slice/user@1000.service/agents.slice/run-x.scope\n"
    )
    assert memcap.own_slice(str(cgroup)) == "agents.slice"
    cgroup.write_text("0::/user.slice/user-1000.slice/session-2.scope\n")
    assert memcap.own_slice(str(cgroup)) is None


def test_without_a_scope_or_a_limit_the_command_is_unchanged():
    argv = ["python", "-m", "tbench", "run"]
    assert memcap.scoped(argv, None) == argv
    assert memcap.scoped(argv, 1 << 30, {memcap.SCOPE_ENV: "off"}) == argv


def test_a_scoped_command_ends_with_the_original(monkeypatch):
    monkeypatch.setattr(memcap, "scopes_available", lambda environ=None: True)
    monkeypatch.setattr(memcap, "own_slice", lambda: "agents.slice")
    argv = ["python", "-m", "tbench", "run"]
    wrapped = memcap.scoped(argv, 1 << 30)
    assert wrapped[:3] == ["systemd-run", "--user", "--scope"]
    assert "--property=MemoryMax=1073741824" in wrapped
    assert "--slice=agents.slice" in wrapped
    assert wrapped[-5:] == ["--", *argv]


@pytest.mark.skipif(
    not (
        sys.platform.startswith("linux")
        and shutil.which("systemd-run")
        and os.environ.get("XDG_RUNTIME_DIR")
    ),
    reason="needs a systemd user manager",
)
def test_a_scoped_process_past_its_cap_is_killed_alone():
    probe = subprocess.run(
        ["systemd-run", "--user", "--scope", "--quiet", "true"], capture_output=True
    )
    if probe.returncode != 0:
        pytest.skip("the user manager does not answer")
    argv = [sys.executable, "-c", "b = bytearray(512 << 20); print('held')"]
    done = subprocess.run(memcap.scoped(argv, 64 << 20), capture_output=True, text=True)
    assert done.returncode != 0
    assert "held" not in done.stdout


@pytest.mark.skipif(sys.platform == "darwin", reason="macOS refuses a useful RLIMIT_DATA")
def test_limit_self_only_lowers_the_data_limit():
    script = (
        "import resource\n"
        "from tbench import memcap\n"
        "memcap.limit_self(256 << 20)\n"
        "memcap.limit_self(1 << 40)\n"
        "print(resource.getrlimit(resource.RLIMIT_DATA)[0])\n"
        "try:\n"
        "    b = bytearray(1 << 30)\n"
        "    print('held')\n"
        "except MemoryError:\n"
        "    print('refused')\n"
    )
    done = subprocess.run(
        [sys.executable, "-c", script],
        capture_output=True,
        text=True,
        cwd=os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    )
    assert done.stdout.split() == [str(256 << 20), "refused"], done.stderr
    assert resource.getrlimit(resource.RLIMIT_DATA)[0] != 256 << 20


@pytest.mark.skipif(sys.platform != "darwin", reason="the watch holds the cap on macOS")
def test_on_macos_a_process_past_its_cap_is_stopped_and_told_why():
    script = (
        "from tbench import memcap\n"
        "memcap.limit_self(256 << 20)\n"
        "memcap.limit_self(1 << 40)\n"
        "held = [b'x' * (1 << 20) for _ in range(1024)]\n"
        "print('held')\n"
    )
    done = subprocess.run(
        [sys.executable, "-c", script],
        capture_output=True,
        text=True,
        cwd=os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
        timeout=60,
    )
    assert "held" not in done.stdout
    assert done.returncode == -signal.SIGKILL, done.stderr
    assert "memory cap of 268435456 bytes" in done.stderr
    assert memcap.ANALYSIS_ENV in done.stderr


@pytest.mark.skipif(sys.platform != "darwin", reason="the watch holds the cap on macOS")
def test_on_macos_a_process_under_its_cap_runs():
    script = (
        "from tbench import memcap\n"
        "memcap.limit_self(1 << 30)\n"
        "b = b'x' * (16 << 20)\n"
        "print('held')\n"
    )
    done = subprocess.run(
        [sys.executable, "-c", script],
        capture_output=True,
        text=True,
        cwd=os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
        timeout=60,
    )
    assert done.stdout.split() == ["held"], done.stderr


def test_a_refused_data_limit_raises_with_the_way_past_it(monkeypatch):
    def refuse(*_):
        raise ValueError("current limit exceeds maximum limit")

    monkeypatch.setattr(memcap.sys, "platform", "linux")
    monkeypatch.setattr(
        memcap.resource,
        "getrlimit",
        lambda _: (memcap.resource.RLIM_INFINITY, memcap.resource.RLIM_INFINITY),
    )
    monkeypatch.setattr(memcap.resource, "setrlimit", refuse)
    with pytest.raises(memcap.CapRefused) as refused:
        memcap.limit_self(256 << 20)
    message = str(refused.value)
    assert "268435456 bytes" in message
    assert memcap.ANALYSIS_ENV in message
    assert "none" in message
