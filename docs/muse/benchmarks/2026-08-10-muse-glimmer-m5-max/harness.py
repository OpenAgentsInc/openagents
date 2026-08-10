#!/usr/bin/env python3
import argparse
import hashlib
import json
import math
import os
import platform
import signal
import subprocess
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path


PROMPTS = [
    "Reply with exactly: muse-ready",
    "Return only the integer result of 37 multiplied by 19.",
    "Return exactly these three lowercase words separated by one space: amber cedar quartz",
]


def request(url, api_key, payload=None, timeout=600):
    data = None if payload is None else json.dumps(payload).encode()
    headers = {"Authorization": f"Bearer {api_key}"}
    if payload is not None:
        headers["Content-Type"] = "application/json"
    return urllib.request.urlopen(
        urllib.request.Request(url, data=data, headers=headers), timeout=timeout
    )


def stream_chat(base_url, api_key, messages, destination, max_tokens=128):
    payload = {
        "model": "muse-glimmer",
        "messages": messages,
        "temperature": 0,
        "seed": 42,
        "max_tokens": max_tokens,
        "stream": True,
        "stream_options": {"include_usage": True},
    }
    started = time.perf_counter()
    first_token = None
    content = []
    reasoning_content = []
    chunks = []
    with request(f"{base_url}/chat/completions", api_key, payload) as response:
        while True:
            line = response.readline()
            if not line:
                break
            decoded = line.decode("utf-8")
            chunks.append(decoded)
            if not decoded.startswith("data: "):
                continue
            event = decoded[6:].strip()
            if event == "[DONE]":
                continue
            parsed = json.loads(event)
            choices = parsed.get("choices") or []
            if choices:
                delta = choices[0].get("delta") or {}
                token = delta.get("content")
                reasoning_token = delta.get("reasoning_content")
                if token or reasoning_token:
                    if first_token is None:
                        first_token = time.perf_counter()
                if token:
                    content.append(token)
                if reasoning_token:
                    reasoning_content.append(reasoning_token)
    ended = time.perf_counter()
    raw = "".join(chunks)
    destination.write_text(raw)
    parsed_events = []
    for line in chunks:
        if line.startswith("data: ") and line[6:].strip() != "[DONE]":
            parsed_events.append(json.loads(line[6:]))
    usage = next(
        (event.get("usage") for event in reversed(parsed_events) if event.get("usage")),
        {},
    )
    timings = next(
        (event.get("timings") for event in reversed(parsed_events) if event.get("timings")),
        {},
    )
    result = {
        "request": payload,
        "content": "".join(content),
        "reasoning_content": "".join(reasoning_content),
        "ttft_seconds": None if first_token is None else first_token - started,
        "end_to_end_seconds": ended - started,
        "usage": usage,
        "timings": timings,
        "sse_event_count": len(parsed_events),
        "malformed_sse_events": 0,
    }
    for number in numeric_values(result):
        if not math.isfinite(number):
            raise RuntimeError(f"non-finite timing or count: {number}")
    return result


def numeric_values(value):
    if isinstance(value, bool):
        return
    if isinstance(value, (int, float)):
        yield float(value)
    elif isinstance(value, dict):
        for child in value.values():
            yield from numeric_values(child)
    elif isinstance(value, list):
        for child in value:
            yield from numeric_values(child)


def rss_sampler(process, samples, stop):
    while not stop.wait(0.1):
        completed = subprocess.run(
            ["ps", "-o", "rss=", "-p", str(process.pid)],
            capture_output=True,
            text=True,
            check=False,
        )
        value = completed.stdout.strip()
        if value.isdigit():
            samples.append({"monotonic": time.perf_counter(), "rss_kib": int(value)})


def wait_for_server(base_url, api_key, process, timeout=300):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"llama-server exited during startup: {process.returncode}")
        try:
            with request(f"{base_url}/models", api_key, timeout=2) as response:
                return json.loads(response.read())
        except (urllib.error.URLError, TimeoutError):
            time.sleep(0.25)
    raise RuntimeError("llama-server did not become ready")


def run_configuration(args, name, dflash):
    run_dir = args.output / name
    run_dir.mkdir(parents=True, exist_ok=False)
    log_path = run_dir / "server.log"
    command = [
        str(args.server),
        "--model", str(args.model),
        "--alias", "muse-glimmer",
        "--ctx-size", "32768",
        "--parallel", "1",
        "--seed", "42",
        "--api-key", args.api_key,
        "--host", "127.0.0.1",
        "--port", str(args.port),
        "--metrics",
        "--log-timestamps",
        "--log-file", str(log_path),
        "--no-webui",
    ]
    if dflash:
        command[3:3] = [
            "--spec-draft-model", str(args.draft),
            "--spec-type", "draft-dflash",
        ]
    (run_dir / "launch-command.json").write_text(json.dumps(command, indent=2) + "\n")
    started = time.perf_counter()
    process = subprocess.Popen(
        command,
        cwd=args.server.parent.parent.parent,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    samples = []
    stop = threading.Event()
    sampler = threading.Thread(target=rss_sampler, args=(process, samples, stop), daemon=True)
    sampler.start()
    base_url = f"http://127.0.0.1:{args.port}/v1"
    result = {"configuration": name, "launch_command": command}
    try:
        models = wait_for_server(base_url, args.api_key, process)
        result["startup_seconds_to_models"] = time.perf_counter() - started
        result["models"] = models
        (run_dir / "models.json").write_text(json.dumps(models, indent=2) + "\n")
        measurements = []
        if args.mode == "all":
            for repetition in range(1, 4):
                for prompt_index, prompt in enumerate(PROMPTS, 1):
                    measurement = stream_chat(
                        base_url,
                        args.api_key,
                        [{"role": "user", "content": prompt}],
                        run_dir / f"prompt-{prompt_index}-repeat-{repetition}.sse",
                    )
                    measurement["prompt_index"] = prompt_index
                    measurement["repetition"] = repetition
                    measurements.append(measurement)
        first_turn = stream_chat(
            base_url,
            args.api_key,
            [{"role": "user", "content": "Remember nonce COBALT-731. Reply exactly: acknowledged"}],
            run_dir / "history-turn-1.sse",
            max_tokens=512,
        )
        second_messages = [
            {"role": "user", "content": "Remember nonce COBALT-731. Reply exactly: acknowledged"},
            {"role": "assistant", "content": first_turn["content"]},
            {"role": "user", "content": "Return only the nonce I asked you to remember."},
        ]
        second_turn = stream_chat(
            base_url,
            args.api_key,
            second_messages,
            run_dir / "history-turn-2.sse",
            max_tokens=512,
        )
        result["measurements"] = measurements
        result["history"] = {"turn_1": first_turn, "turn_2": second_turn}
        with request(f"http://127.0.0.1:{args.port}/metrics", args.api_key) as response:
            (run_dir / "metrics.prom").write_bytes(response.read())
    finally:
        if process.poll() is None:
            process.send_signal(signal.SIGINT)
            try:
                process.wait(timeout=60)
            except subprocess.TimeoutExpired:
                process.terminate()
                process.wait(timeout=30)
        stop.set()
        sampler.join(timeout=2)
        result["server_exit_code"] = process.returncode
        result["rss_samples"] = samples
        result["peak_rss_kib"] = max((sample["rss_kib"] for sample in samples), default=None)
        (run_dir / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as file:
        while chunk := file.read(8 * 1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--server", type=Path, required=True)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--draft", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--port", type=int, default=18000)
    parser.add_argument("--api-key", default="local")
    parser.add_argument(
        "--configuration",
        choices=("all", "target-only", "dflash"),
        default="all",
    )
    parser.add_argument("--mode", choices=("all", "history"), default="all")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    metadata = {
        "uname": platform.uname()._asdict(),
        "macos": platform.mac_ver()[0],
        "model_sha256": sha256(args.model),
        "draft_sha256": sha256(args.draft),
        "model_bytes": args.model.stat().st_size,
        "draft_bytes": args.draft.stat().st_size,
        "port": args.port,
        "prompt_set": PROMPTS,
    }
    (args.output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    results = []
    if args.configuration in ("all", "target-only"):
        results.append(run_configuration(args, "target-only", False))
    if args.configuration in ("all", "dflash"):
        results.append(run_configuration(args, "dflash", True))
    (args.output / "all-results.json").write_text(json.dumps(results, indent=2) + "\n")


if __name__ == "__main__":
    main()
