"""Claude credentials for trials, and how to recognize a trial they failed.

A subscription access token from the Claude login lasts about eight hours,
and every login refresh revokes the token a running trial holds. A
long-lived token from ``claude setup-token`` doesn't change when the host
logs in again, so targeted experiments use it. The operator creates it and
stores it in ``~/.openagents/claude-setup-token`` with mode 0600; nothing
in this package writes it.

Nothing here prints, logs, or records a token's value. The status helpers
report only whether the file is present, its mode, and its age.

A trial whose session failed to authenticate is lost to credentials, never
a graded result: the verifier's reward says nothing about the agent. The
scan reads the same evidence as ``tbench.usage_limit``.
"""

from __future__ import annotations

import json
import stat
import time
from pathlib import Path
from typing import Any

from .usage_limit import HARBOR_LOGS, _read_json, _read_tail

SETUP_TOKEN = Path.home() / ".openagents" / "claude-setup-token"

# How the token file was made, for the operator's instructions.
SETUP_TOKEN_HINT = (
    "run `claude setup-token`, then save the printed token to "
    "~/.openagents/claude-setup-token and run `chmod 600` on it"
)

# The credential sources a run can record: names, never values.
SOURCE_SETUP_TOKEN = "setup-token"
SOURCE_LOGIN = "login"

# Error text that means a session could not authenticate, lowercased.
# Matched only against a session's final error, never against model text.
PHRASES = (
    "failed to authenticate",
    "oauth access token has been revoked",
    "oauth token has expired",
    "oauth token revoked",
    "invalid api key",
    "invalid bearer token",
    "authentication_error",
    "please run /login",
    "not logged in",
    "refresh token",
    "401 unauthorized",
)

AUTH_STATUSES = frozenset({401, 403})


def setup_token_status(path: Path = SETUP_TOKEN) -> dict[str, Any]:
    """Whether the long-lived token is usable, without reading it aloud.

    Returns ``present``, ``usable``, the file's ``mode`` as octal text, its
    ``age_days``, and a ``problem`` sentence when it can't be used.
    """
    status: dict[str, Any] = {
        "path": str(path),
        "present": False,
        "usable": False,
        "mode": None,
        "age_days": None,
        "problem": None,
    }
    try:
        info = path.stat()
    except FileNotFoundError:
        status["problem"] = f"no long-lived Claude token at {path}"
        return status
    except OSError as exc:
        status["problem"] = f"can't read {path}: {exc.strerror}"
        return status
    status["present"] = True
    status["mode"] = f"{stat.S_IMODE(info.st_mode):04o}"
    status["age_days"] = round((time.time() - info.st_mtime) / 86400, 1)
    if not stat.S_ISREG(info.st_mode):
        status["problem"] = f"{path} isn't a regular file"
    elif info.st_mode & 0o077:
        status["problem"] = (
            f"{path} has mode {status['mode']}; other users can read it "
            "(run `chmod 600` on it)"
        )
    elif not read_setup_token(path):
        status["problem"] = f"{path} is empty"
    else:
        status["usable"] = True
    return status


def read_setup_token(path: Path = SETUP_TOKEN) -> str:
    """The token's value, or an empty string. Never print the result."""
    try:
        return path.read_text().strip()
    except OSError:
        return ""


def login_token(credentials: Path) -> str:
    """The Claude login's current access token, or an empty string."""
    try:
        token = json.loads(credentials.read_text())["claudeAiOauth"]["accessToken"]
    except (OSError, ValueError, KeyError, TypeError):
        return ""
    return token if isinstance(token, str) else ""


def fill_host_credentials(env: dict[str, str], home: Path | None = None) -> list[str]:
    """Set the Coder One arms' credentials from the host's usual files.

    Fills ``OPENAGENTS_API_KEY`` from ``~/.openagents/bearer``,
    ``TYPESAFE_API_KEY`` from ``api_key`` in ``~/.openagents/jev.json``,
    and ``CODEX_AUTH_JSON_PATH`` with ``~/.codex/auth.json``, each only when
    it's unset and the file exists. Returns the names it set, never values.
    """
    home = home or Path.home()
    filled = []
    if not env.get("OPENAGENTS_API_KEY"):
        value = read_setup_token(home / ".openagents" / "bearer")
        if value:
            env["OPENAGENTS_API_KEY"] = value
            filled.append("OPENAGENTS_API_KEY")
    if not env.get("TYPESAFE_API_KEY"):
        try:
            value = json.loads((home / ".openagents" / "jev.json").read_text())["api_key"]
        except (OSError, ValueError, KeyError, TypeError):
            value = None
        if isinstance(value, str) and value:
            env["TYPESAFE_API_KEY"] = value
            filled.append("TYPESAFE_API_KEY")
    codex = home / ".codex" / "auth.json"
    if not env.get("CODEX_AUTH_JSON_PATH") and codex.is_file():
        env["CODEX_AUTH_JSON_PATH"] = str(codex)
        filled.append("CODEX_AUTH_JSON_PATH")
    return filled


def says_unauthenticated(text: str | None) -> bool:
    lower = (text or "").lower()
    return any(phrase in lower for phrase in PHRASES)


def scan_stream(text: str, *, source: str | None = None) -> dict[str, Any] | None:
    """The authentication failure a Claude Code or Codex stream ended on.

    Claude Code ends a session it can't authenticate with a ``result``
    whose ``api_error_status`` is 401 and whose text reads, for example,
    "Failed to authenticate. API Error: 401 OAuth access token has been
    revoked." Codex fails its turn with an unauthorized error.
    """
    found: dict[str, Any] | None = None
    for line in text.splitlines():
        if '"result"' not in line and "turn.failed" not in line and '"error"' not in line:
            continue
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if not isinstance(event, dict):
            continue
        kind = event.get("type")
        if kind == "result":
            said = event.get("result") or ""
            status = event.get("api_error_status")
            if event.get("is_error") and (
                status in AUTH_STATUSES or says_unauthenticated(said)
            ):
                found = {"message": str(said)[:300], "status": status, "source": source}
            elif event.get("is_error") is False:
                found = None
        elif kind in ("turn.failed", "error"):
            said = (event.get("error") or {}).get("message") if kind == "turn.failed" else (
                event.get("message")
            )
            if says_unauthenticated(said):
                found = {"message": str(said)[:300], "status": None, "source": source}
    return found


def trial_credential_failure(
    trial_dir: Path, result: dict[str, Any] | None = None
) -> dict[str, Any] | None:
    """The authentication failure that ended a trial's session, or ``None``.

    Reads Harbor's ``claude-code.txt`` and ``codex.txt``, then every stream
    Coder One and the matched adapters keep, then the trial's exception.
    """
    agent = trial_dir / "agent"
    for name in HARBOR_LOGS:
        text = _read_tail(agent / name)
        if text:
            found = scan_stream(text, source=f"agent/{name}")
            if found:
                return found
    artifacts = agent / "episode" / "artifacts"
    if artifacts.is_dir():
        for stream in sorted(artifacts.glob("*.stream.jsonl")):
            found = scan_stream(
                _read_tail(stream) or "", source=f"agent/episode/artifacts/{stream.name}"
            )
            if found:
                return found
    if result is None:
        result = _read_json(trial_dir / "result.json")
    exception = (result or {}).get("exception_info") or {}
    said = exception.get("exception_message") or ""
    if said and says_unauthenticated(said):
        line = next((l for l in said.splitlines() if says_unauthenticated(l)), said)
        return {
            "message": line[:300],
            "status": None,
            "source": f"exception {exception.get('exception_type')}",
        }
    return None


def _final_result(text: str) -> dict[str, Any] | None:
    last = None
    for line in text.splitlines():
        if '"result"' not in line:
            continue
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if isinstance(event, dict) and event.get("type") == "result":
            last = event
    return last


def trial_claude_usage(trial_dir: Path) -> dict[str, Any]:
    """The Claude subscription quota one trial drew, from its sessions.

    Each Claude Code session's final ``result`` reports ``total_cost_usd``,
    the list-price value of the session's usage. That value is the quota
    proxy: it isn't a cash charge. Returns the session count, the summed
    value, the summed output tokens, and how many sessions reported none.
    """
    agent = trial_dir / "agent"
    streams = [agent / "claude-code.txt"]
    artifacts = agent / "episode" / "artifacts"
    if artifacts.is_dir():
        streams += sorted(artifacts.glob("*.stream.jsonl"))
    usage: dict[str, Any] = {"sessions": 0, "usd": 0.0, "output_tokens": 0, "unpriced": 0}
    for stream in streams:
        if not stream.is_file():
            continue
        text = _read_tail(stream) or ""
        if '"claude_code_version"' not in text and '"total_cost_usd"' not in text:
            # A Codex stream or a stream with no Claude session in its tail.
            continue
        final = _final_result(text)
        usage["sessions"] += 1
        cost = (final or {}).get("total_cost_usd")
        if isinstance(cost, (int, float)):
            usage["usd"] += float(cost)
        else:
            usage["unpriced"] += 1
        tokens = ((final or {}).get("usage") or {}).get("output_tokens")
        if isinstance(tokens, int):
            usage["output_tokens"] += tokens
    usage["usd"] = round(usage["usd"], 6)
    return usage


def job_claude_usage(job_dir: Path) -> dict[str, Any]:
    """Every trial's Claude usage in one job directory, summed."""
    from .runner import trial_dirs

    total: dict[str, Any] = {"sessions": 0, "usd": 0.0, "output_tokens": 0, "unpriced": 0}
    if not job_dir.is_dir():
        return total
    for trial in trial_dirs(job_dir):
        usage = trial_claude_usage(trial)
        for key in total:
            total[key] += usage[key]
    total["usd"] = round(total["usd"], 6)
    return total

