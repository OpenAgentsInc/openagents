#!/usr/bin/env python3
"""Guest turn driver. Runs one Codex or Claude turn against the control-plane
provider broker and records lifecycle events in state.json. Replaces the
retired Node SDK guest (Codex SDK / Claude Agent SDK)."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import sys
import threading
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


TERMINAL_TAGS = {"RuntimeSettled", "RuntimeFailed", "RuntimeInterrupted"}


def digest(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def iso_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3] + "Z"


def network_bytes() -> int:
    total = 0
    observed = False
    for interface_name in os.listdir("/sys/class/net"):
        if interface_name == "lo":
            continue
        observed = True
        for counter in ("rx_bytes", "tx_bytes"):
            value = int(
                Path("/sys/class/net", interface_name, "statistics", counter).read_text(
                    encoding="utf-8"
                )
            )
            if value < 0:
                raise RuntimeError("network_usage_invalid")
            total += value
    if not observed:
        raise RuntimeError("network_usage_unavailable")
    return total


def artifact_bytes(path: Path) -> int:
    if not path.exists():
        return 0
    status = path.lstat()
    if not status.st_size >= 0 or not path.is_file():
        raise RuntimeError("artifact_usage_invalid")
    return status.st_size


def usage_ref(turn_ref: str, usage: Any) -> str:
    return f"provider.usage.sha256.{digest(f'{turn_ref}|{json.dumps(usage, separators=(chr(44), chr(58)))}')}"


def runtime_usage(turn_ref: str, usage: dict[str, Any]) -> dict[str, Any]:
    input_tokens = usage.get("input_tokens", usage.get("inputTokens"))
    output_tokens = usage.get("output_tokens", usage.get("outputTokens"))
    if not isinstance(input_tokens, int) or input_tokens < 0:
        raise RuntimeError("provider_usage_unavailable")
    if not isinstance(output_tokens, int) or output_tokens < 0:
        raise RuntimeError("provider_usage_unavailable")
    recorded = {
        "inputTokens": input_tokens,
        "outputTokens": output_tokens,
        "providerUsageRef": usage_ref(turn_ref, usage),
        "exact": True,
    }
    cached = usage.get("cached_input_tokens")
    if isinstance(cached, int) and cached >= 0:
        recorded["cachedInputTokens"] = cached
    return recorded


def http_json(method: str, url: str, body: dict[str, Any], headers: dict[str, str], timeout: int) -> dict[str, Any]:
    payload = json.dumps(body).encode("utf-8")
    request = urllib.request.Request(url, data=payload, method=method, headers=headers)
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return json.loads(response.read().decode("utf-8"))


def main() -> None:
    if len(sys.argv) < 3:
        raise SystemExit(2)
    request_path = Path(sys.argv[1])
    state_path = Path(sys.argv[2])
    try:
        request = json.loads(request_path.read_text(encoding="utf-8"))
        state = json.loads(state_path.read_text(encoding="utf-8"))
        request_path.unlink(missing_ok=True)
    except (OSError, json.JSONDecodeError):
        raise SystemExit(2)

    if f"sha256:{digest(request['prompt'])}" != request.get("promptDigest"):
        raise SystemExit(2)
    turn_key = digest(request["turnRef"])[:24]
    runtime_home = Path(f"/run/openagents-managed-sandbox/{turn_key}")
    workspace = Path("/workspace")
    artifact_path = workspace / "forensic-artifact.tar.zst"
    runtime_home.mkdir(parents=True, exist_ok=True, mode=0o700)
    workspace.mkdir(parents=True, exist_ok=True, mode=0o700)

    terminal_event_tag = next(
        (event.get("_tag") for event in reversed(state.get("events") or []) if event.get("_tag") in TERMINAL_TAGS),
        None,
    )

    def write_state() -> None:
        temporary = Path(f"{state_path}.tmp-{os.getpid()}")
        temporary.write_text(json.dumps(state), encoding="utf-8")
        os.chmod(temporary, 0o600)
        temporary.replace(state_path)

    def emit(event: dict[str, Any]) -> bool:
        nonlocal terminal_event_tag
        if terminal_event_tag is not None:
            return False
        next_event = {
            **event,
            "turnRef": request["turnRef"],
            "resourceGeneration": request["expectedResourceGeneration"],
            "turnEventSequence": len(state["events"]) + 1,
            "observedAt": iso_now(),
        }
        state["events"].append(next_event)
        if next_event["_tag"] in TERMINAL_TAGS:
            terminal_event_tag = next_event["_tag"]
        write_state()
        return True

    guardrails = request.get("guardrails") or {}
    guarded = (request.get("runtime") or {}).get("harnessRef") == "driver.openagents.forensic-worker.v1"
    if guarded:
        deadline = datetime.fromisoformat(str(guardrails.get("deadlineAt", "")).replace("Z", "+00:00"))
        if (
            guardrails.get("sandboxRef") != request.get("sandboxRef")
            or guardrails.get("resourceGeneration") != request.get("expectedResourceGeneration")
            or not isinstance(guardrails.get("remainingTokens"), int)
            or guardrails["remainingTokens"] < 1
            or not isinstance(guardrails.get("remainingCostMicros"), int)
            or guardrails["remainingCostMicros"] < 1
            or not isinstance(guardrails.get("networkBytesObserved"), int)
            or not isinstance(guardrails.get("remainingNetworkBytes"), int)
            or not isinstance(guardrails.get("artifactBytesObserved"), int)
            or not isinstance(guardrails.get("remainingArtifactBytes"), int)
            or deadline.timestamp() <= time.time()
        ):
            raise SystemExit(2)

    stop = threading.Event()
    budget_exceeded = False

    def enforce_budget() -> None:
        nonlocal budget_exceeded
        if not guarded:
            return
        try:
            deadline = datetime.fromisoformat(str(guardrails["deadlineAt"]).replace("Z", "+00:00"))
            if (
                time.time() >= deadline.timestamp()
                or network_bytes()
                > guardrails["networkBytesObserved"] + guardrails["remainingNetworkBytes"]
                or artifact_bytes(artifact_path)
                > guardrails["artifactBytesObserved"] + guardrails["remainingArtifactBytes"]
            ):
                budget_exceeded = True
                stop.set()
        except (OSError, RuntimeError, ValueError, KeyError):
            budget_exceeded = True
            stop.set()

    def budget_loop() -> None:
        while not stop.wait(0.1):
            enforce_budget()

    enforce_budget()
    budget_thread = threading.Thread(target=budget_loop, daemon=True)
    if guarded:
        budget_thread.start()

    provider = (request.get("runtime") or {}).get("provider")
    token = request["providerCapabilityToken"]
    base = request["providerBaseUrl"].rstrip("/")
    model = request["providerModel"]
    prompt = request["prompt"]

    try:
        if provider == "codex":
            body = {
                "model": model,
                "stream": False,
                "messages": [{"role": "user", "content": prompt}],
            }
            headers = {
                "Authorization": f"Bearer {token}",
                "Content-Type": "application/json",
            }
            if stop.is_set():
                raise RuntimeError("budget_guardrail")
            result = http_json(
                "POST",
                f"{base}/openai/v1/chat/completions",
                body,
                headers,
                timeout=600,
            )
            choice = (result.get("choices") or [{}])[0]
            content = ((choice.get("message") or {}).get("content")) or ""
            if content:
                emit({"_tag": "RuntimeTextDelta", "content": content})
            usage = runtime_usage(request["turnRef"], result.get("usage") or {})
            emit({"_tag": "RuntimeUsageRecorded", "usage": usage})
            emit({"_tag": "RuntimeSettled", "finishReason": "structural_completion", "usage": usage})
        elif provider == "claude":
            body = {
                "model": model,
                "max_tokens": 4096,
                "messages": [{"role": "user", "content": prompt}],
            }
            headers = {
                "x-api-key": token,
                "anthropic-version": "2023-06-01",
                "Content-Type": "application/json",
            }
            if stop.is_set():
                raise RuntimeError("budget_guardrail")
            result = http_json(
                "POST",
                f"{base}/anthropic/v1/messages",
                body,
                headers,
                timeout=600,
            )
            blocks = result.get("content") or []
            text = "".join(
                block.get("text", "")
                for block in blocks
                if isinstance(block, dict) and block.get("type") == "text"
            )
            if text:
                emit({"_tag": "RuntimeTextDelta", "content": text})
            usage_raw = result.get("usage") or {}
            usage = runtime_usage(
                request["turnRef"],
                {
                    "input_tokens": usage_raw.get("input_tokens"),
                    "output_tokens": usage_raw.get("output_tokens"),
                    "cached_input_tokens": usage_raw.get("cache_read_input_tokens"),
                },
            )
            emit({"_tag": "RuntimeUsageRecorded", "usage": usage})
            emit({"_tag": "RuntimeSettled", "finishReason": "structural_completion", "usage": usage})
        else:
            raise RuntimeError("provider_not_admitted")
    except Exception as error:  # noqa: BLE001 — guest turn records a typed failure
        if terminal_event_tag is None:
            if budget_exceeded or str(error) == "budget_guardrail":
                emit({"_tag": "RuntimeSettled", "finishReason": "budget_guardrail"})
            else:
                name = type(error).__name__
                emit(
                    {
                        "_tag": "RuntimeFailed",
                        "errorRef": f"provider.failure.sha256.{digest(name)}",
                        "retryable": True,
                    }
                )
    finally:
        stop.set()
        shutil.rmtree(runtime_home, ignore_errors=True)


if __name__ == "__main__":
    main()
