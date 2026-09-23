"""The host-side live tail: bounded polls, complete lines, and a status."""

import asyncio
import hashlib
import json
import subprocess

from tbench.coder_v05 import CoderV05
from tbench.live import LOG_NAME, LiveTail


class _Result:
    def __init__(self, stdout: str, return_code: int = 0):
        self.stdout = stdout
        self.stderr = ""
        self.return_code = return_code


class _LocalEnvironment:
    """Runs each command in a local shell, as a container exec would."""

    def __init__(self):
        self.commands: list[str] = []

    async def exec(self, command: str, **_: object) -> _Result:
        self.commands.append(command)
        done = subprocess.run(
            ["sh", "-c", command], capture_output=True, text=True, check=False
        )
        return _Result(done.stdout, done.returncode)


def test_polls_copy_complete_lines_only_and_advance_by_what_they_kept(tmp_path):
    log = tmp_path / "container" / LOG_NAME
    log.parent.mkdir()
    log.write_bytes(b'{"n":1}\n{"n":2}\n{"n":')
    environment = _LocalEnvironment()
    tail = LiveTail(environment, str(log), tmp_path / "live", interval_sec=0.01)

    assert asyncio.run(tail.poll()) == 16
    assert (tmp_path / "live" / LOG_NAME).read_bytes() == b'{"n":1}\n{"n":2}\n'
    with log.open("ab") as handle:
        handle.write(b'3}\n{"n":4}\n')
    assert asyncio.run(tail.poll()) == 16
    assert (tmp_path / "live" / LOG_NAME).read_bytes() == log.read_bytes()
    assert "tail -c +17 " in environment.commands[-1]
    status = json.loads((tmp_path / "live" / "status.json").read_text())
    assert status["state"] == "following"
    assert status["offset"] == 32
    assert status["polls"] == 2
    asyncio.run(tail.finish())
    status = json.loads((tmp_path / "live" / "status.json").read_text())
    assert status["state"] == "ended"


def test_a_poll_reads_at_most_one_chunk_and_the_copy_stops_at_its_cap(tmp_path):
    log = tmp_path / LOG_NAME
    log.write_bytes(b"".join(b'{"line":%03d}\n' % i for i in range(100)))
    tail = LiveTail(
        _LocalEnvironment(), str(log), tmp_path / "live", chunk=64, cap=200
    )
    kept = asyncio.run(tail.poll())
    assert 0 < kept <= 64
    asyncio.run(tail.finish())
    copy = (tmp_path / "live" / LOG_NAME).read_bytes()
    assert len(copy) <= 200
    assert copy.endswith(b"\n")
    assert log.read_bytes().startswith(copy)
    assert json.loads((tmp_path / "live" / "status.json").read_text())["state"] == "capped"


def test_a_failed_poll_is_counted_and_following_continues(tmp_path):
    class _Broken:
        async def exec(self, command: str, **_: object):
            raise RuntimeError("the environment is gone")

    tail = LiveTail(_Broken(), "/nowhere", tmp_path / "live")
    assert asyncio.run(tail.poll()) == 0
    status = json.loads((tmp_path / "live" / "status.json").read_text())
    assert status["errors"] == 1
    assert status["state"] == "following"
    assert "gone" in status["last_error"]


class _EpisodeEnvironment(_LocalEnvironment):
    """The episode exec appends to the log over time; tails run locally."""

    def __init__(self, log):
        super().__init__()
        self.log = log
        self.seen_while_running: list[bytes] = []
        self.live = None

    async def upload_file(self, *_: object) -> None:
        pass

    async def download_dir(self, *_: object) -> None:
        pass

    async def exec(self, command: str, **kw: object) -> _Result:
        if "episode run" not in command:
            return await super().exec(command, **kw)
        for n in range(5):
            with self.log.open("ab") as handle:
                handle.write(b'{"step":%d}\n' % n)
            await asyncio.sleep(0.05)
            if self.live.exists():
                self.seen_while_running.append(self.live.read_bytes())
        return _Result("done")


def test_the_adapter_follows_the_log_while_the_episode_runs(tmp_path, monkeypatch):
    payload = b"#!/bin/sh\n"
    binary = tmp_path / "coder-v05"
    binary.write_bytes(payload)
    agent = CoderV05(
        logs_dir=tmp_path / "agent",
        artifact_path=str(binary),
        artifact_sha256=hashlib.sha256(payload).hexdigest(),
        live_interval_sec=0.01,
    )
    log = tmp_path / "container" / LOG_NAME
    log.parent.mkdir()
    log.write_bytes(b"")
    monkeypatch.setattr("tbench.coder_v05.EPISODE_DIR", log.parent)
    environment = _EpisodeEnvironment(log)
    environment.live = tmp_path / "agent" / "live" / LOG_NAME
    asyncio.run(agent.run("Do it.", environment, context=None))

    # The host copy grew while the episode ran, not only at the end.
    assert any(0 < len(seen) < len(log.read_bytes()) for seen in environment.seen_while_running)
    assert environment.live.read_bytes() == log.read_bytes()
    status = json.loads((tmp_path / "agent" / "live" / "status.json").read_text())
    assert status["state"] == "ended"
    assert status["offset"] == len(log.read_bytes())
