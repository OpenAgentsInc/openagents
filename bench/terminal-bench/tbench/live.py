"""Follow an episode's invocation log from the host while it runs.

The Harbor adapter downloads the episode bundle once the episode ends, so
the host sees nothing while a trial runs. ``LiveTail`` closes that gap
without a new channel into the container: while the episode's own exec
runs, it asks the environment at a fixed interval for the bytes of
``episode.atif.jsonl`` past what it already holds, and appends them to a
host copy under the trial's logs directory.

Each poll is one bounded exec:

    tail -c +<offset+1> <log> | head -c <chunk> | base64

The host keeps only complete lines, so the copy is always a readable
prefix of the log, and advances its offset by the bytes it kept. A poll
never reads more than ``chunk`` bytes, and the copy stops growing at
``cap`` bytes; the status says when it did. The episode log is
append-only and fsynced line by line, so the copy is a prefix of the same
file the bundle carries at the end.

A Coder One episode that runs Microluna writes each session to its own
log, ``artifacts/microluna-<dispatch>-<n>.atif.jsonl``, and the
acceptance-suite writer writes ``artifacts/accept-writer-<n>.atif.jsonl``.
When ``session_glob`` names them (space-separated globs), every poll also lists the matching files and
copies each one the same way into ``artifacts/`` beside the episode copy,
so a reader can show the sessions while they run.

``status.json`` beside the copy tells a reader where the tail stands:
``following`` while the episode runs, then ``ended``, ``capped``, or
``failed``, with the offset, the poll count, when it last polled, and
when the log last grew. The Gym's live view reads both files.
"""

from __future__ import annotations

import asyncio
import base64
import json
import os
import shlex
import time
from pathlib import Path
from typing import Any

SCHEMA = "openagents.tbench.live-tail.v1"
LOG_NAME = "episode.atif.jsonl"
DEFAULT_INTERVAL_SEC = 10.0
DEFAULT_CHUNK = 256 * 1024
DEFAULT_CAP = 64 * 1024 * 1024


def _iso(seconds: float) -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime(seconds)) + (
        ".%03dZ" % int((seconds % 1) * 1000)
    )


class LiveTail:
    """A bounded, incremental copy of one file inside an environment."""

    def __init__(
        self,
        environment: Any,
        source: str,
        target_dir: Path,
        interval_sec: float = DEFAULT_INTERVAL_SEC,
        chunk: int = DEFAULT_CHUNK,
        cap: int = DEFAULT_CAP,
        name: str = LOG_NAME,
        session_glob: str | None = None,
        status: bool = True,
    ) -> None:
        self.environment = environment
        self.source = source
        self.target_dir = Path(target_dir)
        self.name = name
        self.target = self.target_dir / name
        self.session_glob = session_glob
        self.sessions: dict[str, LiveTail] = {}
        self.status = status
        self.interval_sec = interval_sec
        self.chunk = chunk
        self.cap = cap
        self.offset = 0
        self.polls = 0
        self.errors = 0
        self.last_error: str | None = None
        self.polled_at: float | None = None
        self.grew_at: float | None = None
        self.state = "following"
        self.started_at = time.time()
        self.target_dir.mkdir(parents=True, exist_ok=True)
        self.target.write_bytes(b"")
        self.write_status()

    def command(self) -> str:
        return (
            f"tail -c +{self.offset + 1} {shlex.quote(self.source)} 2>/dev/null"
            f" | head -c {self.chunk} | base64 | tr -d '\\n'"
        )

    async def poll(self) -> int:
        """Reads what the logs gained since the last poll; returns the bytes kept."""
        kept = await self._poll_own()
        return kept + await self._poll_sessions()

    async def _poll_sessions(self) -> int:
        """Finds new session logs, then polls every one; returns the bytes kept."""
        if not self.session_glob:
            return 0
        try:
            result = await self.environment.exec(
                command=f"ls -1 {self.session_glob} 2>/dev/null"
            )
            names = [line.strip() for line in (result.stdout or "").splitlines()]
        except Exception as exc:  # a failed listing is recorded, not fatal
            self.errors += 1
            self.last_error = str(exc)[:500]
            names = []
        for path in names:
            if path and path not in self.sessions:
                self.sessions[path] = LiveTail(
                    self.environment,
                    path,
                    self.target_dir / "artifacts",
                    self.interval_sec,
                    self.chunk,
                    self.cap,
                    name=os.path.basename(path),
                    status=False,
                )
        kept = 0
        for session in self.sessions.values():
            kept += await session._poll_own()
        return kept

    async def _poll_own(self) -> int:
        """Reads what this log gained since the last poll; returns the bytes kept."""
        if self.state != "following":
            return 0
        self.polls += 1
        self.polled_at = time.time()
        try:
            result = await self.environment.exec(command=self.command())
            data = base64.b64decode((result.stdout or "").strip() or b"")
        except Exception as exc:  # a failed poll is recorded, not fatal
            self.errors += 1
            self.last_error = str(exc)[:500]
            self.write_status()
            return 0
        end = data.rfind(b"\n")
        kept = data[: end + 1] if end >= 0 else b""
        if len(data) >= self.chunk and not kept:
            # One line longer than a whole poll can't be copied whole.
            self.state = "capped"
            self.last_error = f"a line at offset {self.offset} is longer than {self.chunk} bytes"
        if kept:
            room = self.cap - self.offset
            if len(kept) > room:
                kept = kept[: kept.rfind(b"\n", 0, room) + 1] if room > 0 else b""
                self.state = "capped"
            with self.target.open("ab") as handle:
                handle.write(kept)
            self.offset += len(kept)
            self.grew_at = self.polled_at
        self.write_status()
        return len(kept)

    async def follow(self) -> None:
        """Polls until cancelled or capped."""
        while self.state == "following":
            await asyncio.sleep(self.interval_sec)
            # A poll that filled a whole chunk may have more waiting.
            while await self.poll() >= self.chunk // 2 and self.state == "following":
                pass

    async def finish(self, state: str = "ended") -> None:
        """One last poll, then the final state."""
        if self.state == "following":
            while await self.poll() > 0 and self.state == "following":
                pass
        if self.state == "following":
            self.state = state
        for session in self.sessions.values():
            await session.finish(state)
        self.write_status()

    def record(self) -> dict[str, Any]:
        return {
            "schema": SCHEMA,
            "source": self.source,
            "copy": self.name,
            "sessions": sorted(os.path.basename(p) for p in self.sessions),
            "state": self.state,
            "offset": self.offset,
            "polls": self.polls,
            "errors": self.errors,
            "last_error": self.last_error,
            "interval_sec": self.interval_sec,
            "chunk": self.chunk,
            "cap": self.cap,
            "started_at": _iso(self.started_at),
            "polled_at": _iso(self.polled_at) if self.polled_at else None,
            "grew_at": _iso(self.grew_at) if self.grew_at else None,
        }

    def write_status(self) -> None:
        if not self.status:
            return
        path = self.target_dir / "status.json"
        temporary = path.with_suffix(".json.tmp")
        temporary.write_text(json.dumps(self.record(), indent=2) + "\n")
        os.replace(temporary, path)
