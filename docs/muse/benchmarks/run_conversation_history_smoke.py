#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import platform
import time
from pathlib import Path

from run_llama_cpp_benchmark import (
    MemorySampler,
    command_output,
    get_json,
    streamed_chat,
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default="http://127.0.0.1:8000/v1")
    parser.add_argument("--api-key", default="local")
    parser.add_argument("--model", default="muse-glimmer")
    parser.add_argument(
        "--reasoning-control",
        choices=("api", "template"),
        default="api",
    )
    parser.add_argument("--configuration", required=True)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument(
        "--omit-seed",
        action="store_true",
        help="Omit seed for greedy artifacts that do not export sampling support.",
    )
    parser.add_argument("--server-pid", type=int, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    sampler = MemorySampler(args.server_pid)
    sampler.start()
    first_scenario = {
        "name": "history_turn_1",
        "messages": [
            {
                "role": "user",
                "content": (
                    "Remember that the project codename is cobalt-orchid. "
                    "Reply with exactly: remembered"
                ),
            }
        ],
        "reasoning_effort": "low",
        "max_tokens": 256,
        "expected": "remembered",
    }
    first = streamed_chat(
        args.base_url,
        args.api_key,
        args.model,
        first_scenario,
        1,
        None,
        args.reasoning_control,
        None if args.omit_seed else args.seed,
    )

    assistant_message = {
        "role": "assistant",
        "content": first["visible_output"],
    }
    if first["reasoning_output"]:
        assistant_message["reasoning_content"] = first["reasoning_output"]
    second_scenario = {
        "name": "history_turn_2",
        "messages": [
            first_scenario["messages"][0],
            assistant_message,
            {
                "role": "user",
                "content": "What is the project codename? Return only the codename.",
            },
        ],
        "reasoning_effort": "low",
        "max_tokens": 256,
        "expected": "cobalt-orchid",
    }
    second = streamed_chat(
        args.base_url,
        args.api_key,
        args.model,
        second_scenario,
        2,
        None,
        args.reasoning_control,
        None if args.omit_seed else args.seed,
    )

    document = {
        "schema": "openagents.muse.conversation_history_smoke.v1",
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
        "turns": [first, second],
        "passed": first["expected_observed"] and second["expected_observed"],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(document, indent=2) + "\n")
    print(
        json.dumps(
            {
                "turn_1": first["expected_observed"],
                "turn_2": second["expected_observed"],
                "passed": document["passed"],
            }
        )
    )
    errors = [turn["error"] for turn in document["turns"] if turn["error"]]
    return 1 if errors or not document["passed"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
