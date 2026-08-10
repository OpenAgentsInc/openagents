#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import platform
import subprocess
import threading
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any


def long_context() -> str:
    records = []
    for index in range(900):
        marker = "LANTERN-4821" if index == 731 else f"record-{index:04d}"
        records.append(
            f"Entry {index:04d}: status=nominal owner=team-{index % 17:02d} "
            f"verification={marker}."
        )
    return "\n".join(records)


SCENARIOS = [
    {
        "name": "exact_instruction",
        "messages": [
            {"role": "user", "content": "Reply with exactly: muse-ready"},
        ],
        "reasoning_effort": "low",
        "max_tokens": 128,
        "expected": "muse-ready",
    },
    {
        "name": "code_generation",
        "messages": [
            {
                "role": "user",
                "content": (
                    "Write a Python function named stable_unique that returns items "
                    "in first-seen order without using set on the items. Include type "
                    "hints and three assert examples. Return code only."
                ),
            },
        ],
        "reasoning_effort": "medium",
        "max_tokens": 256,
        "expected": "stable_unique",
    },
    {
        "name": "reasoning",
        "messages": [
            {
                "role": "user",
                "content": (
                    "A farmer must move a wolf, a goat, and a cabbage across a river. "
                    "The boat carries the farmer and one item. The wolf cannot stay "
                    "with the goat, and the goat cannot stay with the cabbage. Give "
                    "the shortest valid crossing sequence and state its number of trips."
                ),
            },
        ],
        "reasoning_effort": "high",
        "max_tokens": 384,
        "expected": "7",
    },
    {
        "name": "long_context_retrieval",
        "messages": [
            {
                "role": "user",
                "content": (
                    "Read the records below. Return only the verification value from "
                    "Entry 0731.\n\n" + long_context()
                ),
            },
        ],
        "reasoning_effort": "low",
        "max_tokens": 128,
        "expected": "LANTERN-4821",
    },
    {
        "name": "multi_turn_memory",
        "messages": [
            {
                "role": "user",
                "content": "Remember that this project's codename is cobalt-orchid.",
            },
            {
                "role": "assistant",
                "content": "I will remember the project codename.",
            },
            {
                "role": "user",
                "content": "What is the codename? Return only the codename.",
            },
        ],
        "reasoning_effort": "low",
        "max_tokens": 128,
        "expected": "cobalt-orchid",
    },
    {
        "name": "structured_json",
        "messages": [
            {
                "role": "user",
                "content": (
                    "Return one compact JSON object with keys model, local, and "
                    "capabilities. Set model to muse-glimmer, local to true, and "
                    "capabilities to an array containing chat and reasoning. No prose."
                ),
            },
        ],
        "reasoning_effort": "medium",
        "max_tokens": 192,
        "expected": "muse-glimmer",
    },
]


def command_output(command: list[str]) -> str:
    return subprocess.run(
        command,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


@dataclass
class MemorySampler:
    process_id: int

    def __post_init__(self) -> None:
        self.samples_kib: list[int] = []
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._sample, daemon=True)

    def _sample(self) -> None:
        while not self._stop.wait(1.0):
            result = subprocess.run(
                ["ps", "-axo", "pid=,ppid=,rss="],
                capture_output=True,
                text=True,
            )
            if result.returncode != 0:
                continue
            processes = []
            for line in result.stdout.splitlines():
                fields = line.split()
                if len(fields) == 3 and all(field.isdigit() for field in fields):
                    processes.append(tuple(int(field) for field in fields))
            descendants = {self.process_id}
            while True:
                children = {
                    process_id
                    for process_id, parent_id, _ in processes
                    if parent_id in descendants
                }
                expanded = descendants | children
                if expanded == descendants:
                    break
                descendants = expanded
            rss_kib = sum(
                rss for process_id, _, rss in processes if process_id in descendants
            )
            if rss_kib:
                self.samples_kib.append(rss_kib)

    def start(self) -> None:
        self._thread.start()

    def finish(self) -> dict[str, int | None]:
        self._stop.set()
        self._thread.join()
        return {
            "samples": len(self.samples_kib),
            "peak_rss_kib": max(self.samples_kib) if self.samples_kib else None,
        }


def get_json(url: str, api_key: str) -> Any:
    request = urllib.request.Request(
        url,
        headers={"Authorization": f"Bearer {api_key}"},
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.load(response)


def streamed_chat(
    base_url: str,
    api_key: str,
    model: str,
    scenario: dict[str, Any],
    repetition: int,
    max_tokens: int | None,
    reasoning_control: str,
    seed: int | None,
) -> dict[str, Any]:
    payload = {
        "model": model,
        "messages": scenario["messages"],
        "temperature": 0,
        "max_tokens": (
            max_tokens if max_tokens is not None else scenario["max_tokens"]
        ),
        "stream": True,
        "stream_options": {"include_usage": True},
    }
    if seed is not None:
        payload["seed"] = seed
    if reasoning_control == "api":
        payload["reasoning_effort"] = scenario["reasoning_effort"]
    else:
        payload["chat_template_kwargs"] = {
            "reasoning_strength": scenario["reasoning_effort"],
            "return_reasoning": True,
        }
    request = urllib.request.Request(
        f"{base_url}/chat/completions",
        data=json.dumps(payload).encode(),
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    started = time.perf_counter()
    first_token_at = None
    first_content_at = None
    content: list[str] = []
    reasoning: list[str] = []
    finish_reason = None
    usage = None
    timings = None
    event_count = 0

    try:
        with urllib.request.urlopen(request, timeout=300) as response:
            status = response.status
            for raw_line in response:
                line = raw_line.decode("utf-8").strip()
                if not line.startswith("data: "):
                    continue
                data = line[6:]
                if data == "[DONE]":
                    break
                event = json.loads(data)
                event_count += 1
                choices = event.get("choices") or []
                choice = choices[0] if choices else {}
                delta = choice.get("delta", {})
                content_delta = delta.get("content") or ""
                reasoning_delta = delta.get("reasoning_content") or ""
                if first_token_at is None and (content_delta or reasoning_delta):
                    first_token_at = time.perf_counter()
                if first_content_at is None and content_delta:
                    first_content_at = time.perf_counter()
                content.append(content_delta)
                reasoning.append(reasoning_delta)
                finish_reason = choice.get("finish_reason") or finish_reason
                usage = event.get("usage") or usage
                timings = event.get("timings") or timings
        error = None
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as exception:
        status = getattr(exception, "code", None)
        error = f"{type(exception).__name__}: {exception}"

    finished = time.perf_counter()
    visible_output = "".join(content)
    reasoning_output = "".join(reasoning)
    expected = scenario["expected"]
    completion_tokens = (usage or {}).get("completion_tokens")
    observed_decode_seconds = (
        None if first_token_at is None else finished - first_token_at
    )
    observed_completion_per_second = None
    if completion_tokens is not None and observed_decode_seconds:
        observed_completion_per_second = completion_tokens / observed_decode_seconds
    end_to_end_completion_per_second = None
    if completion_tokens is not None and finished > started:
        end_to_end_completion_per_second = completion_tokens / (finished - started)
    return {
        "scenario": scenario["name"],
        "repetition": repetition,
        "request": payload,
        "http_status": status,
        "error": error,
        "finish_reason": finish_reason,
        "event_count": event_count,
        "ttft_seconds": None if first_token_at is None else first_token_at - started,
        "first_content_seconds": (
            None if first_content_at is None else first_content_at - started
        ),
        "total_seconds": finished - started,
        "observed_decode_seconds": observed_decode_seconds,
        "observed_completion_per_second": observed_completion_per_second,
        "end_to_end_completion_per_second": end_to_end_completion_per_second,
        "visible_output": visible_output,
        "reasoning_output": reasoning_output,
        "expected": expected,
        "expected_observed": expected.lower() in visible_output.lower(),
        "usage": usage,
        "timings": timings,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default="http://127.0.0.1:8000/v1")
    parser.add_argument("--api-key", default="local")
    parser.add_argument("--model", default="muse-glimmer")
    parser.add_argument(
        "--reasoning-control",
        choices=("api", "template"),
        default="api",
        help=(
            "Use reasoning_effort or the canonical template's reasoning_strength "
            "variable. ExecuTorch requires template."
        ),
    )
    parser.add_argument("--configuration", required=True)
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument(
        "--omit-seed",
        action="store_true",
        help="Omit seed for greedy artifacts that do not export sampling support.",
    )
    parser.add_argument(
        "--scenario",
        action="append",
        choices=[scenario["name"] for scenario in SCENARIOS],
        help="Run only this scenario. Repeat the option to select multiple scenarios.",
    )
    parser.add_argument(
        "--max-tokens",
        type=int,
        help="Override the output budget for every selected scenario.",
    )
    parser.add_argument("--server-pid", type=int, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    sampler = MemorySampler(args.server_pid)
    sampler.start()
    results = []
    scenarios = [
        scenario
        for scenario in SCENARIOS
        if args.scenario is None or scenario["name"] in args.scenario
    ]
    for scenario in scenarios:
        for repetition in range(1, args.repetitions + 1):
            result = streamed_chat(
                args.base_url,
                args.api_key,
                args.model,
                scenario,
                repetition,
                args.max_tokens,
                args.reasoning_control,
                None if args.omit_seed else args.seed,
            )
            results.append(result)
            print(
                json.dumps(
                    {
                        "scenario": result["scenario"],
                        "repetition": repetition,
                        "status": result["http_status"],
                        "ttft_seconds": result["ttft_seconds"],
                        "total_seconds": result["total_seconds"],
                        "predicted_per_second": (result["timings"] or {}).get(
                            "predicted_per_second"
                        ),
                        "observed_completion_per_second": result[
                            "observed_completion_per_second"
                        ],
                        "expected_observed": result["expected_observed"],
                    }
                ),
                flush=True,
            )

    document = {
        "schema": "openagents.muse.chat_completions_benchmark.v2",
        "configuration": args.configuration,
        "captured_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "environment": {
            "platform": platform.platform(),
            "macos": command_output(["sw_vers", "-productVersion"]),
            "hardware_model": command_output(["sysctl", "-n", "hw.model"]),
            "memory_bytes": int(command_output(["sysctl", "-n", "hw.memsize"])),
            "server_pid": args.server_pid,
            "models": get_json(f"{args.base_url}/models", args.api_key),
        },
        "memory": sampler.finish(),
        "results": results,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(document, indent=2) + "\n")
    failures = [result for result in results if result["error"] is not None]
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
