"""Usage- and rate-limited trials: how to recognize one, and what it means.

A subscription account shares one quota across every session that uses
it. When several trials run Claude Code on Opus at once, the quota can run
out mid-suite, and every session after that ends at once: Claude Code's
stream closes with a ``result`` whose ``api_error_status`` is 429 and whose
text reads "You've hit your session limit · resets 11:50am (UTC)", after a
``rate_limit_event`` whose status is ``rejected``. Codex ends its turn with
``error`` and ``turn.failed`` events that say "You've hit your usage limit"
and, when it knows, "try again at 3:04 PM".

A throttled trial is an infrastructure failure, never a graded result: the
verifier's reward says nothing about the agent. This module reads the
evidence a trial leaves (Coder One's episode manifest, Harbor's
``claude-code.txt`` and ``codex.txt`` logs, Coder One's delegate streams,
and the trial's exception) and answers whether a limit stopped it, which
provider's, and when the limit resets. The scheduler, the attempt record,
and the scoreboard all ask it the same question.
"""

from __future__ import annotations

import json
import re
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

# Coder One's exit code and outcome for a throttled delegate session.
USAGE_LIMIT_EXIT_CODE = 6
OUTCOME = "usage_limited"

# The attempt record's terminal status for a throttled trial.
STATUS = "usage_limited"

# Exceptions that mean a provider throttled the agent: this package's own,
# and Harbor's two classifications of a CLI's rate- or usage-limit output.
EXCEPTION_TYPES = frozenset(
    {"UsageLimitError", "ApiRateLimitError", "ApiUsageLimitError"}
)

# How long the scheduler waits when a limit states no reset time.
DEFAULT_BACKOFF_SEC = 1800.0

# Phrases a provider's limit message uses, lowercased. Matched only against
# error text, never against what a model wrote.
PHRASES = (
    "hit your session limit",
    "hit your usage limit",
    "hit your weekly limit",
    "hit your limit",
    "usage limit",
    "rate_limit_error",
    "rate limit reached",
    "rate_limit_exceeded",
    "usage_limit_reached",
    "usage_limit_exceeded",
    "exceeded retry limit, last status: 429",
    "429 too many requests",
)

# Which provider bills each executor CLI.
PROVIDERS = {"claude-code": "anthropic", "codex": "openai"}

# Log files Harbor's own agents write under the trial's ``agent/``.
HARBOR_LOGS = ("claude-code.txt", "codex.txt")


class UsageLimitError(RuntimeError):
    """A provider throttled the agent's session.

    An infrastructure failure, not a result: the adapter raises it so
    Harbor records the trial without running the verifier, and the
    scheduler requeues the trial once the limit resets.
    """


def says_limited(text: str | None) -> bool:
    """Whether error text says a usage or rate limit stopped the session."""
    lower = (text or "").lower()
    return any(phrase in lower for phrase in PHRASES)


_MONTHS = ("jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec")
_UTC = {"utc", "gmt", "etc/utc", "etc/gmt", "z"}


def _clock(word: str, following: str | None) -> int | None:
    """Minutes past midnight of ``11:50am``, ``5pm``, ``3:04 pm``, or ``15:04``."""
    suffix = None
    body = word
    if word.endswith(("am", "pm")):
        body, suffix = word[:-2], word[-2:]
    elif following and following.rstrip(".") in ("am", "pm"):
        suffix = following.rstrip(".")
    if ":" in body:
        hour_text, minute_text = body.split(":", 1)
        if not (hour_text.isdigit() and minute_text.isdigit()):
            return None
        hour, minute = int(hour_text), int(minute_text)
    elif suffix and body.isdigit():
        hour, minute = int(body), 0
    else:
        return None
    if minute > 59:
        return None
    if suffix:
        if not 1 <= hour <= 12:
            return None
        hour = hour % 12 + (12 if suffix == "pm" else 0)
    elif hour > 23:
        return None
    return hour * 60 + minute


def reset_from_message(message: str, now: float) -> float | None:
    """The reset time a limit message states, in epoch seconds.

    Reads Claude Code's ``resets 11:50am (UTC)`` and ``resets Sep 24, 5pm
    (UTC)`` and Codex's ``try again at 3:04 PM`` and ``try again at Sep
    25th, 2026 9:15 AM``. A time without a date is the next such time after
    ``now``. A named zone other than UTC isn't read; a time with no zone is
    read as UTC, the task containers' clock. This is the same reading as
    ``coder_one::limit::reset_from_message``.
    """
    lower = message.lower()
    start = None
    for marker in ("resets at ", "resets ", "try again at "):
        at = lower.find(marker)
        if at >= 0:
            start = at + len(marker)
            break
    if start is None:
        return None
    rest = lower[start:]
    zone = None
    if "(" in rest:
        open_at = rest.index("(")
        close_at = rest.find(")", open_at)
        zone = rest[open_at + 1 : close_at if close_at >= 0 else len(rest)].strip()
        rest = rest[:open_at]
    if zone is not None and zone not in _UTC:
        return None
    spec = re.split(r"[\n·]", rest)[0].strip().rstrip(".")
    words = [w for w in re.split(r"[\s,]+", spec) if w]
    month = day = year = minutes = None
    for index, raw in enumerate(words):
        word = raw.rstrip(".")
        matched = next((i for i, m in enumerate(_MONTHS) if word.startswith(m)), None)
        if matched is not None:
            month = matched + 1
            continue
        clock = _clock(word, words[index + 1] if index + 1 < len(words) else None)
        if clock is not None:
            minutes = clock
            break
        number = re.fullmatch(r"(\d+)(st|nd|rd|th)?", word)
        if not number:
            return None
        value = int(number.group(1))
        if value >= 1000:
            year = value
        elif month is not None and day is None:
            day = value
        else:
            return None
    if minutes is None:
        return None
    today = datetime.fromtimestamp(now, tz=timezone.utc)
    if month is not None and day is not None:
        try:
            at = datetime(year or today.year, month, day, tzinfo=timezone.utc)
        except ValueError:
            return None
        stamp = at.timestamp() + minutes * 60
        if year is None and stamp + 86400 < now:
            stamp = datetime(today.year + 1, month, day, tzinfo=timezone.utc).timestamp() + minutes * 60
        return stamp
    if month is None and day is None:
        midnight = datetime(today.year, today.month, today.day, tzinfo=timezone.utc).timestamp()
        stamp = midnight + minutes * 60
        return stamp + 86400 if stamp <= now else stamp
    return None


def _iso_seconds(text: str | None) -> float | None:
    if not text:
        return None
    try:
        return datetime.fromisoformat(text.replace("Z", "+00:00")).timestamp()
    except ValueError:
        return None


def limit_record(
    provider: str,
    message: str,
    *,
    now: float | None = None,
    resets_at: float | None = None,
    reset_source: str | None = None,
    window: str | None = None,
    status: int | None = None,
    source: str | None = None,
) -> dict[str, Any]:
    """A limit in the shape Coder One's episode manifest records it."""
    if resets_at is None:
        resets_at = reset_from_message(message, time.time() if now is None else now)
        reset_source = "message" if resets_at is not None else None
    return {
        "provider": provider,
        "message": message.strip(),
        "resets_at": int(resets_at) if resets_at is not None else None,
        "resets_at_iso": (
            datetime.fromtimestamp(resets_at, tz=timezone.utc).isoformat(timespec="seconds")
            if resets_at is not None
            else None
        ),
        "reset_source": reset_source,
        "window": window,
        "status": status,
        "source": source,
    }


# Substrings that mark a line worth parsing; the rest of a stream is skipped.
_INTERESTING = ('"result"', "rate_limit", "turn.failed", '"error"', "turn.completed", "thread.started")
# Event types only ``codex exec --json`` writes; a tail may have lost the
# stream's ``thread.started``.
_CODEX_MARKS = ('"type":"thread.', '"type":"turn.', '"type":"item.')


def scan_stream(text: str, *, source: str | None = None, now: float | None = None) -> dict[str, Any] | None:
    """The limit a Claude Code stream-json or ``codex exec --json`` log ended on.

    A limit counts only when the session ended on it: Claude Code's final
    result is an error with status 429 or limit wording, or Codex's turn
    failed with limit wording. A ``rejected`` rate-limit event the session
    recovered from is not a limit.
    """
    message: str | None = None
    status: int | None = None
    at: float | None = None
    reset: tuple[float, str | None] | None = None
    codex = False
    clean = False
    failed = False
    for line in text.splitlines():
        if not codex and any(mark in line for mark in _CODEX_MARKS):
            codex = True
        if not any(mark in line for mark in _INTERESTING):
            continue
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if not isinstance(event, dict):
            continue
        kind = event.get("type")
        if kind == "rate_limit_event":
            info = event.get("rate_limit_info") or {}
            if info.get("status") == "rejected" and isinstance(info.get("resetsAt"), (int, float)):
                reset = (float(info["resetsAt"]), info.get("rateLimitType"))
        elif kind == "assistant" and event.get("error") == "rate_limit":
            content = (event.get("message") or {}).get("content") or []
            said = "\n".join(
                block.get("text", "") for block in content if isinstance(block, dict) and block.get("text")
            )
            message = message or said
            at = _iso_seconds(event.get("timestamp")) or at
        elif kind == "result":
            said = event.get("result") or ""
            error_status = event.get("api_error_status")
            if error_status == 429 or (event.get("is_error") and says_limited(said)):
                message, status, clean = said, error_status, False
            elif event.get("is_error") is False:
                clean = True
        elif kind in ("thread.started", "turn.completed"):
            codex = True
            if kind == "turn.completed":
                clean = True
        elif kind == "turn.failed":
            codex, failed, clean = True, True, False
            said = ((event.get("error") or {}).get("message")) or ""
            if says_limited(said):
                message = message or said
        elif kind == "error":
            said = event.get("message") or ""
            if says_limited(said):
                message = message or said
    if message is None or clean:
        return None
    provider = "openai" if codex else "anthropic"
    record = limit_record(provider, message, now=at if at is not None else now, source=source, status=status)
    if reset is not None:
        record.update(
            limit_record(
                provider,
                message,
                resets_at=reset[0],
                reset_source="rate_limit_event",
                window=reset[1],
                status=status,
                source=source,
            )
        )
    return record


def _read_json(path: Path) -> dict[str, Any] | None:
    try:
        value = json.loads(path.read_text())
    except (OSError, ValueError):
        return None
    return value if isinstance(value, dict) else None


# A session ends on its limit, so only a stream's tail is read: enough for
# the rejected rate-limit event, the error, and the final result.
TAIL_BYTES = 256 * 1024


def _read_tail(path: Path, limit: int = TAIL_BYTES) -> str | None:
    """The last ``limit`` bytes of a file, from its first whole line."""
    try:
        with path.open("rb") as handle:
            handle.seek(0, 2)
            size = handle.tell()
            handle.seek(max(0, size - limit))
            data = handle.read()
    except OSError:
        return None
    if size > limit:
        data = data.split(b"\n", 1)[1] if b"\n" in data else b""
    return data.decode("utf-8", errors="replace")


def _read_text(path: Path) -> str | None:
    try:
        return path.read_text(errors="replace")
    except OSError:
        return None


def trial_usage_limit(
    trial_dir: Path, result: dict[str, Any] | None = None, *, now: float | None = None
) -> dict[str, Any] | None:
    """The usage limit that stopped a trial, or ``None``.

    Reads, in order: Coder One's episode manifest (``usage_limit``, or the
    ``usage_limited`` outcome), Harbor's ``claude-code.txt`` and
    ``codex.txt``, Coder One's delegate streams, and the trial's exception.
    The first that shows a limit answers, with its reset time when known.
    """
    agent = trial_dir / "agent"
    manifest = _read_json(agent / "episode" / "manifest.json") or {}
    recorded = manifest.get("usage_limit")
    if isinstance(recorded, dict):
        return {**recorded, "source": "episode manifest"}
    for name in HARBOR_LOGS:
        text = _read_tail(agent / name)
        if text:
            found = scan_stream(text, source=f"agent/{name}", now=now)
            if found:
                return found
    artifacts = agent / "episode" / "artifacts"
    if artifacts.is_dir():
        for stream in sorted(artifacts.glob("delegate-*.stream.jsonl")):
            text = _read_tail(stream)
            found = scan_stream(text or "", source=f"agent/episode/artifacts/{stream.name}", now=now)
            if found:
                return found
    if result is None:
        result = _read_json(trial_dir / "result.json")
    exception = (result or {}).get("exception_info") or {}
    if exception.get("exception_type") in EXCEPTION_TYPES:
        said = exception.get("exception_message") or exception["exception_type"]
        line = next((l for l in said.splitlines() if says_limited(l)), said.splitlines()[0] if said else "")
        provider = "openai" if "codex" in said.lower() else "anthropic"
        return limit_record(provider, line[:500], now=now, source=f"exception {exception['exception_type']}")
    if manifest.get("outcome") == OUTCOME:
        return limit_record("unknown", "the episode ended usage_limited", now=now, source="episode manifest")
    exit_note = _read_text(agent / "episode-exit.txt") or ""
    if exit_note.startswith(f"exit {USAGE_LIMIT_EXIT_CODE}"):
        return limit_record("unknown", exit_note.strip(), now=now, source="agent/episode-exit.txt")
    return None


def job_usage_limit(job_dir: Path) -> dict[str, Any] | None:
    """The first usage limit any trial of a job hit, or ``None``."""
    for trial in sorted(job_dir.glob("*__*")):
        if trial.is_dir():
            found = trial_usage_limit(trial)
            if found:
                return {**found, "trial": trial.name}
    return None


def arm_providers(profile: Any) -> frozenset[str]:
    """The providers an agent profile's arm draws on: ``anthropic``, ``openai``.

    Harbor's ``claude-code`` and ``codex`` agents bill their own provider.
    A Coder One delegate arm bills its delegate's, and a policy manifest
    arm bills every executor the manifest can dispatch to.
    """
    name = getattr(profile, "harbor_name", None)
    if name in PROVIDERS:
        return frozenset({PROVIDERS[name]})
    kwargs = dict(getattr(profile, "kwargs", None) or {})
    agents: set[str] = set()
    policy = kwargs.get("policy")
    if policy:
        from .coder_one import load_policy, manifest_tiers

        try:
            agents |= {tier.get("agent") for tier in manifest_tiers(load_policy(policy))}
        except Exception:  # an unreadable policy is refused where it runs
            pass
    elif (getattr(profile, "harbor_import_path", None) or "").endswith("CoderOneDelegate"):
        agents.add(kwargs.get("delegate_agent") or "claude-code")
    return frozenset(PROVIDERS[a] for a in agents if a in PROVIDERS)
