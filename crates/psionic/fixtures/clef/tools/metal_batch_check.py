#!/usr/bin/env python3
"""Check real Metal batching on a shared machine with caller-held GPU/memory leases.

Use the same config shape as mac_quiet_bench.py, with one owned Psionic binary
variant. The request files required by that config are not used: this check
creates and records its own synthetic cases. Optional batch_check settings:
  chunk=256, cases=4, repetitions=128, saturation_repetitions=256,
  cancellation_repetitions=1024, saturation_requests=12,
  probability_tolerance=0.001, recovery_timeout=30, cancel_after_seconds=0.1.

The binary runs in three separate processes: serial, batch2, and batch4. Each
response must contain real token counts, no cached tokens/answers, complete
question and option identities, and finite full-precision probabilities.
Every successful measured answer is compared with its same-binary serial
reference. Cohort metadata must prove shared FFN work. Timing is reported as
shared-machine acceptance evidence, never an idle-machine performance claim.
"""

import argparse
import concurrent.futures
import copy
import http.client
import json
import math
import os
from pathlib import Path
import signal
import socket
import statistics
import threading
import time
import urllib.parse

import mac_quiet_bench as bench


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def finite_number(value):
    return type(value) in (int, float) and math.isfinite(value)


def settings(config):
    result = {"chunk": 256, "cases": 4, "repetitions": 128,
              "saturation_repetitions": 256, "cancellation_repetitions": 1024,
              "saturation_requests": 12, "probability_tolerance": 1e-3,
              "recovery_timeout": 30, "cancel_after_seconds": 0.1}
    result.update(config.get("batch_check", {}))
    for key in ("chunk", "cases", "repetitions", "saturation_repetitions",
                "cancellation_repetitions", "saturation_requests"):
        if type(result[key]) is not int or result[key] < 1:
            raise ValueError(f"{key} must be a positive integer")
    if result["cases"] % 4 or result["cases"] > 16:
        raise ValueError("cases must be a multiple of four, at most 16")
    if not 10 <= result["saturation_requests"] <= 16:
        raise ValueError("saturation_requests must be between 10 and 16 (default server queue is eight)")
    if result["chunk"] > 4096 or max(result[k] for k in
            ("repetitions", "saturation_repetitions", "cancellation_repetitions")) > 2048:
        raise ValueError("chunk or synthetic state is too large for this bounded check")
    for key in ("probability_tolerance", "recovery_timeout", "cancel_after_seconds"):
        if not finite_number(result[key]) or result[key] <= 0:
            raise ValueError(f"{key} must be finite and positive")
    if result["probability_tolerance"] > 1e-3 or result["cancel_after_seconds"] > 1:
        raise ValueError("probability tolerance must be <=0.001 and cancel delay <=1 second")
    return result


def synthetic(identity, index, repetitions, model):
    states = ("open", "waiting", "resolved")
    status = states[index % len(states)]
    questions = {
        f"{identity}-status": {"type": "choice", "instructions": "What is this ticket's current status?",
            "criteria": {state: f"The ticket is {state}." for state in states}},
    }
    if index % 2:
        questions[f"{identity}-urgent"] = {"type": "noul", "instructions": "Is this ticket marked urgent?"}
    state = f"CASE {identity}\nTicket status: {status}. Urgent: {'yes' if index % 2 else 'no'}.\n"
    state += "Background: the office keeps a daily record of its support tickets.\n" * repetitions
    state += f"The current ticket {identity} is {status}."
    return {"model": model, "state": state, "questions": questions}


def probabilities(body, code, response):
    bench.validate_uncached_response(code, response)
    answers = response["answers"]
    if not isinstance(answers, dict) or set(answers) != set(body["questions"]):
        raise AssertionError("missing, extra, or mismatched question identities")
    if response.get("model") != body["model"]:
        raise AssertionError("model identity changed")
    psionic = response.get("psionic", {})
    if psionic.get("backend") != "metal" or psionic.get("truncated_state_tokens") != 0:
        raise AssertionError("wrong backend or truncated synthetic state")
    timing = psionic.get("timing", {})
    for key in ("encode_ms", "queue_ms", "prefill_ms", "head_ms", "total_ms"):
        if not finite_number(timing.get(key)) or timing[key] < 0:
            raise AssertionError(f"missing or invalid {key}")
    result = {}
    for identity, question in body["questions"].items():
        answer = answers[identity]
        if answer.get("type") != question["type"]:
            raise AssertionError("answer type changed")
        if question["type"] == "noul":
            p = answer.get("noul")
            if not finite_number(p):
                raise AssertionError("invalid noul probability")
            values = {"false": 1 - p, "true": p}
        else:
            values = answer.get("probabilities")
            if not isinstance(values, dict) or set(values) != set(question["criteria"]):
                raise AssertionError("missing, extra, or mismatched option identities")
            if answer.get("choice") not in values:
                raise AssertionError("invalid choice identity")
        if not all(finite_number(p) and 0 <= p <= 1 for p in values.values()):
            raise AssertionError("invalid option probability")
        if abs(sum(values.values()) - 1) > 1e-6:
            raise AssertionError("probabilities do not sum to one")
        top = max(values, key=values.get)
        if question["type"] == "choice" and answer["choice"] != top:
            raise AssertionError("reported choice does not match probabilities")
        result[identity] = (values, top)
    return result


def compare(body, reference, actual, tolerance):
    expected = probabilities(body, reference["status"], reference["response"])
    observed = probabilities(body, actual["status"], actual["response"])
    if actual["response"]["usage"]["input_tokens"] != reference["response"]["usage"]["input_tokens"]:
        raise AssertionError("serial and batch input token counts differ")
    for field in ("artifact_digest", "head_digest"):
        a = actual["response"]["psionic"].get(field)
        b = reference["response"]["psionic"].get(field)
        if not a or a != b:
            raise AssertionError("serial and batch artifact identity differs")
    maximum = 0.0
    for identity, (values, top) in observed.items():
        original, expected_top = expected[identity]
        if top != expected_top:
            raise AssertionError(f"top answer changed for {identity}")
        maximum = max(maximum, max(abs(p - original[option]) for option, p in values.items()))
    if maximum > tolerance:
        raise AssertionError(f"probability delta {maximum} exceeds {tolerance}")
    return maximum


def post(url, body, timeout):
    address = urllib.parse.urlsplit(url)
    connection = http.client.HTTPConnection(address.hostname, address.port, timeout=timeout)
    began = time.perf_counter()
    try:
        connection.request("POST", address.path, body=bench.encoded(body), headers={"Content-Type": "application/json"})
        response = connection.getresponse()
        raw = response.read()
        value = json.loads(raw, object_pairs_hook=unique_object,
                           parse_constant=lambda token: (_ for _ in ()).throw(ValueError(f"nonfinite JSON {token}")))
        return time.perf_counter() - began, response.status, value
    finally:
        connection.close()


def send(url, body, identity, mode, phase, timeout, host, barrier=None):
    if barrier is not None:
        barrier.wait(timeout=30)
    host.guard()
    began = time.monotonic()
    elapsed, status, response = post(url, body, timeout)
    return {"kind": "request", "at": bench.now(), "mode": mode, "phase": phase,
            "case": identity, "request_sha256": bench.sha(bench.encoded(body)),
            "started_monotonic": began, "finished_monotonic": time.monotonic(),
            "seconds": elapsed, "status": status, "response": response,
            "load_after": host.guard()}


def burst(url, cases, width, mode, phase, config, host, records):
    rows = []
    for first in range(0, len(cases), width):
        group = cases[first:first + width]
        barrier = threading.Barrier(len(group))
        with concurrent.futures.ThreadPoolExecutor(max_workers=len(group)) as pool:
            futures = {pool.submit(send, url, body, identity, mode, phase,
                config["request_timeout"], host, barrier): identity for identity, body in group}
            seen = set()
            for future in concurrent.futures.as_completed(futures):
                row = future.result()
                if row["case"] in seen or row["case"] != futures[future]:
                    raise AssertionError("duplicate or reordered request identity")
                seen.add(row["case"])
                bench.emit(records, row)
                rows.append(row)
            if seen != {identity for identity, _ in group}:
                raise AssertionError("missing request response")
    return rows


def cancel(url, body, delay):
    address = urllib.parse.urlsplit(url)
    raw = bench.encoded(body)
    with socket.create_connection((address.hostname, address.port), timeout=10) as client:
        headers = f"POST {address.path} HTTP/1.1\r\nHost: {address.hostname}\r\nContent-Type: application/json\r\nContent-Length: {len(raw)}\r\n\r\n".encode()
        client.sendall(headers + raw)
        time.sleep(delay)
        client.shutdown(socket.SHUT_RDWR)
    return {"kind": "cancellation", "at": bench.now(), "request_sha256": bench.sha(raw),
            "delay_seconds": delay, "method": "close owned HTTP connection after sending body",
            "device_progress": "not directly observable over HTTP; transport cancellation has a Rust unit test"}


def check(config):
    if config["measurement_mode"] != "shared":
        raise ValueError("this check requires measurement_mode=shared")
    if not {"gpu", "memory"} <= set(os.environ.get("OPENAGENTS_LEASES", "").split(",")):
        raise ValueError("run this check under GPU and memory leases")
    if len(config["variants"]) != 1:
        raise ValueError("provide one binary; the harness creates serial/batch2/batch4 variants")
    variant = config["variants"][0]
    if "binary" not in variant or variant.get("engine", "psionic") != "psionic":
        raise ValueError("use an owned Psionic binary")
    if any("PROFILE" in key or "SKIP" in key for key in variant.get("env", {})):
        raise ValueError("disable profile and skip modes")
    options = settings(config)
    output = Path(config["output_dir"])
    output.mkdir(parents=True, exist_ok=False)
    model = variant["model"]
    core = [(f"core-{i}", synthetic(f"core-{i}", i, options["repetitions"] + i * 3, model)) for i in range(options["cases"])]
    recovery = [(label, synthetic(label, 1, options["repetitions"], model)) for label in ("after-cancel", "after-saturation")]
    saturation = [(f"saturation-{i}", synthetic(f"saturation-{i}", i, options["saturation_repetitions"], model)) for i in range(options["saturation_requests"])]
    baseline_cases = core + recovery + saturation
    by_id = dict(baseline_cases)
    warmup = synthetic("warmup", 0, options["repetitions"], model)
    cancelled = synthetic("cancelled", 1, options["cancellation_repetitions"], model)
    (output / "requests.json").write_text(json.dumps(dict(baseline_cases, warmup=warmup, cancelled=cancelled), indent=2) + "\n")
    metadata = {"schema": "clef.metal-batch-check.v1", "created_at": bench.now(), "config": config,
        "settings": options, "binary_sha256": bench.file_sha(variant["binary"]),
        "model_sha256": bench.file_sha(config["model_path"]), "measurement_mode": "shared",
        "lease_id": os.environ.get("OPENAGENTS_LEASE_ID"), "quiet_lease": None,
        "probability_precision": "full JSON values; no rounding before comparison",
        "conditions": "shared machine; no idle-machine claim"}
    (output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    host = bench.Host(config, output)
    host.thread.start()
    baseline, summaries = {}, []
    summary = {"status": "failed", "conditions": metadata["conditions"], "variants": summaries}
    try:
        host.settle()
        with (output / "records.jsonl").open("w") as records:
            for width in (1, 2, 4):
                mode = "serial" if width == 1 else f"batch{width}"
                current = dict(variant, name=mode, chunk=options["chunk"], env={**variant.get("env", {}),
                    "PSIONIC_CLEF_BATCH_REQUESTS": str(width),
                    "PSIONIC_CLEF_BATCH_TOKENS": str(options["chunk"] * width),
                    "PSIONIC_CLEF_PREFIX_CACHE_BYTES": "0"})
                with bench.server(config, current, 0, output, records, host) as url:
                    row = send(url, warmup, "warmup", mode, "warmup", config["request_timeout"], host)
                    bench.emit(records, row)
                    probabilities(warmup, row["status"], row["response"])
                    if width == 1:
                        for identity, body in baseline_cases:
                            row = send(url, body, identity, mode, "reference", config["request_timeout"], host)
                            probabilities(body, row["status"], row["response"])
                            if "batch" in row["response"]["psionic"]:
                                raise AssertionError("disabled batching reported batch execution")
                            baseline[identity] = row
                            bench.emit(records, row)
                        continue
                    measured = burst(url, core, width, mode, "concurrent", config, host, records)
                    cancellation = cancel(url, cancelled, options["cancel_after_seconds"])
                    bench.emit(records, dict(cancellation, mode=mode))
                    label, body = recovery[0]
                    row = send(url, body, label, mode, "recovery", options["recovery_timeout"], host)
                    bench.emit(records, row)
                    measured.append(row)
                    overloaded = burst(url, saturation, len(saturation), mode, "saturation", config, host, records)
                    refused = []
                    for row in overloaded:
                        if row["status"] == 503 and row["response"].get("error", {}).get("code") == "busy":
                            refused.append(row)
                        elif row["status"] == 200:
                            measured.append(row)
                        else:
                            raise AssertionError("unexpected saturation outcome")
                    if not refused or len(refused) == len(overloaded):
                        raise AssertionError("saturation must both admit work and return busy")
                    label, body = recovery[1]
                    row = send(url, body, label, mode, "recovery", options["recovery_timeout"], host)
                    bench.emit(records, row)
                    measured.append(row)
                    deltas = [compare(by_id[row["case"]], baseline[row["case"]], row,
                                      options["probability_tolerance"]) for row in measured]
                    groups = [row["response"].get("psionic", {}).get("batch", {}) for row in measured if row["phase"] == "concurrent"]
                    if not any(type(group.get("max_requests")) is int and 2 <= group["max_requests"] <= width
                               and group.get("grouped_ffn") is True for group in groups):
                        raise AssertionError("no evidence of actual shared FFN work")
                    if any(type(group.get("chunks")) is not int or group["chunks"] < 1
                           or not 1 <= group.get("max_requests", 0) <= width for group in groups):
                        raise AssertionError("invalid batch cohort metadata")
                    core_rows = [row for row in measured if row["phase"] == "concurrent"]
                    wall = max(row["finished_monotonic"] for row in core_rows) - min(row["started_monotonic"] for row in core_rows)
                    serial = sum(baseline[identity]["seconds"] for identity, _ in core)
                    summaries.append({"mode": mode, "status": "passed", "compared_requests": len(measured),
                        "max_abs_probability_delta": max(deltas), "top_answer_agreement": 1.0,
                        "max_cohort": max(group["max_requests"] for group in groups),
                        "busy_refusals": len(refused), "saturation_requests": len(overloaded),
                        "cancellation_recovery": "passed", "saturation_recovery": "passed",
                        "concurrent_wall_seconds": wall, "serial_core_seconds": serial,
                        "relative_throughput": serial / wall,
                        "median_queue_ms": statistics.median(row["response"]["psionic"]["timing"]["queue_ms"] for row in core_rows),
                        "median_prefill_ms": statistics.median(row["response"]["psionic"]["timing"]["prefill_ms"] for row in core_rows),
                        "median_head_ms": statistics.median(row["response"]["psionic"]["timing"]["head_ms"] for row in core_rows)})
        host.guard()
        summary["status"] = "passed"
    except BaseException as error:
        summary.update(error_type=type(error).__name__, error=str(error))
        raise
    finally:
        host.stop.set()
        host.thread.join(timeout=6)
        if host.monitor_error or host.overloaded.is_set() or host.thread.is_alive():
            summary.update(status="failed", host_monitor_error=host.monitor_error,
                           host_overloaded=host.overloaded.is_set(), monitor_still_running=host.thread.is_alive())
        summary["finished_at"] = bench.now()
        (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
        print(json.dumps(summary, indent=2))
    return summary["status"] == "passed"


def self_test():
    body = synthetic("identity", 1, 1, "clef-flash")
    response = {"model": "clef-flash", "answers": {
        "identity-status": {"type": "choice", "choice": "waiting", "probabilities": {"open": 0.1, "waiting": 0.8, "resolved": 0.1}},
        "identity-urgent": {"type": "noul", "noul": 0.7}},
        "usage": {"input_tokens": 50, "cached_input_tokens": 0},
        "psionic": {"backend": "metal", "truncated_state_tokens": 0, "artifact_digest": "a", "head_digest": "b",
                    "timing": {"encode_ms": 1, "queue_ms": 2, "prefill_ms": 3, "head_ms": 4, "total_ms": 10, "cached": False}}}
    row = {"status": 200, "response": response}
    assert compare(body, row, row, 1e-3) == 0
    mutations = [lambda x: x["answers"].pop("identity-urgent"),
                 lambda x: x["usage"].update(cached_input_tokens=1),
                 lambda x: x["psionic"]["timing"].update(cached=True),
                 lambda x: x["answers"]["identity-urgent"].update(noul=0.7000001 + 1e-3),
                 lambda x: x["answers"]["identity-status"]["probabilities"].update(open=float("nan")),
                 lambda x: x["psionic"].update(artifact_digest="other")]
    for mutate in mutations:
        value = copy.deepcopy(response)
        mutate(value)
        try:
            compare(body, row, {"status": 200, "response": value}, 1e-3)
        except (AssertionError, RuntimeError):
            pass
        else:
            raise AssertionError("invalid response accepted")
    try:
        json.loads('{"answers":{},"answers":{}}', object_pairs_hook=unique_object)
    except ValueError:
        pass
    else:
        raise AssertionError("duplicate response key accepted")
    assert settings({})["cases"] == 4
    print("Self-test passed: identities, complete answers, full-precision probability gate, cache rejection, artifact identity, duplicate JSON detection.")


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, lambda signum, frame: (_ for _ in ()).throw(KeyboardInterrupt()))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config", nargs="?")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
    elif args.config:
        raise SystemExit(0 if check(bench.load_config(args.config)) else 1)
    else:
        parser.error("provide a config or --self-test")
