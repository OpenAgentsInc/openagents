#!/usr/bin/env python3
"""Check CUDA Clef batching (#11441) against serial answers on one GPU host.

The CUDA counterpart of metal_batch_check.py, for a dedicated GPU VM: no
leases or Mac host monitor. It starts the given psionic-openai-server three
times (serial, two slots, four slots) with
PSIONIC_CLEF_BATCH_TOKENS = chunk x slots, sends the same synthetic cases
(unique text, so the logit cache never answers), and requires for every
successful batched answer:

  - the same input token count, artifact and head digests as serial;
  - the same top answer for every question;
  - max |dp| <= --tolerance against the serial probabilities (full JSON
    precision); the summary reports whether every answer was bitwise
    identical (max |dp| == 0);
  - batch metadata proving shared FFN work (max_requests >= 2, grouped_ffn);
  - recovery after a disconnected request and after an overload burst that
    must return some `503 busy` and admit the rest.

    cuda_batch_check.py --binary B --model M --out DIR [--chunk 256]
    cuda_batch_check.py --self-test

Writes DIR/records.jsonl (every request and response) and DIR/summary.json.
"""

import argparse
import concurrent.futures
import copy
import hashlib
import http.client
import json
import math
import os
from pathlib import Path
import socket
import statistics
import subprocess
import threading
import time


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def finite(value):
    return type(value) in (int, float) and math.isfinite(value)


def synthetic(identity, index, repetitions, model):
    states = ("open", "waiting", "resolved")
    status = states[index % len(states)]
    questions = {f"{identity}-status": {"type": "choice", "instructions": "What is this ticket's current status?",
                                         "criteria": {s: f"The ticket is {s}." for s in states}}}
    if index % 2:
        questions[f"{identity}-urgent"] = {"type": "noul", "instructions": "Is this ticket marked urgent?"}
    state = f"CASE {identity}\nTicket status: {status}. Urgent: {'yes' if index % 2 else 'no'}.\n"
    state += "Background: the office keeps a daily record of its support tickets.\n" * repetitions
    state += f"The current ticket {identity} is {status}."
    return {"model": model, "state": state, "questions": questions}


def probabilities(body, status, response, backend="cuda"):
    if status != 200:
        raise AssertionError(f"status {status}: {response}")
    if response.get("usage", {}).get("cached_input_tokens") != 0:
        raise AssertionError("cached input tokens")
    psionic = response.get("psionic", {})
    timing = psionic.get("timing", {})
    if timing.get("cached") is not False:
        raise AssertionError("answer came from the logit cache")
    for key in ("encode_ms", "queue_ms", "prefill_ms", "head_ms", "total_ms"):
        if not finite(timing.get(key)) or timing[key] < 0:
            raise AssertionError(f"missing or invalid {key}")
    if psionic.get("backend") != backend or psionic.get("truncated_state_tokens") != 0:
        raise AssertionError("wrong backend or truncated synthetic state")
    if response.get("model") != body["model"]:
        raise AssertionError("model identity changed")
    answers = response.get("answers")
    if not isinstance(answers, dict) or set(answers) != set(body["questions"]):
        raise AssertionError("missing, extra, or mismatched question identities")
    result = {}
    for identity, question in body["questions"].items():
        answer = answers[identity]
        if answer.get("type") != question["type"]:
            raise AssertionError("answer type changed")
        if question["type"] == "noul":
            p = answer.get("noul")
            if not finite(p):
                raise AssertionError("invalid noul probability")
            values = {"false": 1 - p, "true": p}
        else:
            values = answer.get("probabilities")
            if not isinstance(values, dict) or set(values) != set(question["criteria"]):
                raise AssertionError("missing, extra, or mismatched option identities")
        if not all(finite(p) and 0 <= p <= 1 for p in values.values()):
            raise AssertionError("invalid option probability")
        if abs(sum(values.values()) - 1) > 1e-6:
            raise AssertionError("probabilities do not sum to one")
        top = max(values, key=values.get)
        if question["type"] == "choice" and answer.get("choice") != top:
            raise AssertionError("reported choice does not match probabilities")
        result[identity] = (values, top)
    return result


def compare(body, reference, actual, tolerance):
    expected = probabilities(body, reference["status"], reference["response"])
    observed = probabilities(body, actual["status"], actual["response"])
    if actual["response"]["usage"]["input_tokens"] != reference["response"]["usage"]["input_tokens"]:
        raise AssertionError("serial and batch input token counts differ")
    for field in ("artifact_digest", "head_digest"):
        a, b = actual["response"]["psionic"].get(field), reference["response"]["psionic"].get(field)
        if not a or a != b:
            raise AssertionError("serial and batch artifact identity differs")
    maximum = 0.0
    for identity, (values, top) in observed.items():
        original, expected_top = expected[identity]
        if top != expected_top:
            raise AssertionError(f"top answer changed for {identity}")
        maximum = max(maximum, max(abs(p - original[o]) for o, p in values.items()))
    if maximum > tolerance:
        raise AssertionError(f"probability delta {maximum} exceeds {tolerance}")
    return maximum


def encoded(body):
    return json.dumps(body, separators=(",", ":")).encode()


def post(port, body, timeout):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=timeout)
    began = time.perf_counter()
    try:
        connection.request("POST", "/v1/systemone", body=encoded(body), headers={"Content-Type": "application/json"})
        response = connection.getresponse()
        value = json.loads(response.read(), object_pairs_hook=unique_object)
        return time.perf_counter() - began, response.status, value
    finally:
        connection.close()


def send(port, body, identity, mode, phase, timeout, barrier=None):
    if barrier is not None:
        barrier.wait(timeout=60)
    began = time.monotonic()
    seconds, status, response = post(port, body, timeout)
    return {"mode": mode, "phase": phase, "case": identity, "request_sha256": hashlib.sha256(encoded(body)).hexdigest(),
            "started": began, "finished": time.monotonic(), "seconds": seconds, "status": status, "response": response}


def burst(port, cases, width, mode, phase, timeout, emit):
    rows = []
    for first in range(0, len(cases), width):
        group = cases[first:first + width]
        barrier = threading.Barrier(len(group))
        with concurrent.futures.ThreadPoolExecutor(max_workers=len(group)) as pool:
            futures = [pool.submit(send, port, body, identity, mode, phase, timeout, barrier) for identity, body in group]
            for future in futures:
                row = future.result()
                emit(row)
                rows.append(row)
    return rows


def cancel(port, body, delay):
    raw = encoded(body)
    with socket.create_connection(("127.0.0.1", port), timeout=10) as client:
        head = f"POST /v1/systemone HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {len(raw)}\r\n\r\n"
        client.sendall(head.encode() + raw)
        time.sleep(delay)
        client.shutdown(socket.SHUT_RDWR)
    return {"phase": "cancellation", "delay_seconds": delay, "request_sha256": hashlib.sha256(raw).hexdigest()}


class Server:
    def __init__(self, args, slots, log):
        env = dict(os.environ, PSIONIC_CLEF_BATCH_REQUESTS=str(slots),
                   PSIONIC_CLEF_BATCH_TOKENS=str(args.chunk * slots), PSIONIC_CLEF_LOG="0")
        for key in ("PSIONIC_CLEF_PROFILE", "PSIONIC_CLEF_SKIP"):
            env.pop(key, None)
        command = [args.binary, "-m", args.model, "--host", "127.0.0.1", "--port", str(args.port),
                   "--decision-device", "cuda", "--decision-chunk", str(args.chunk),
                   "--decision-max-tokens", "65536", "--decision-max-queue", str(args.max_queue)]
        self.log = open(log, "w")
        self.process = subprocess.Popen(command, env=env, stdout=self.log, stderr=subprocess.STDOUT)
        deadline = time.monotonic() + args.start_timeout
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise RuntimeError(f"server exited with {self.process.returncode}; see {log}")
            try:
                connection = http.client.HTTPConnection("127.0.0.1", args.port, timeout=2)
                connection.request("GET", "/health")
                if connection.getresponse().status == 200:
                    return
            except OSError:
                pass
            time.sleep(1)
        self.close()
        raise RuntimeError("server did not come up")

    def close(self):
        self.process.terminate()
        try:
            self.process.wait(timeout=30)
        except subprocess.TimeoutExpired:
            self.process.kill()
        self.log.close()


def check(args):
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=False)
    model = args.model_id
    core = [(f"core-{i}", synthetic(f"core-{i}", i, args.repetitions + i * 3, model)) for i in range(args.cases)]
    recovery = [(label, synthetic(label, 1, args.repetitions, model)) for label in ("after-cancel", "after-saturation")]
    saturation = [(f"saturation-{i}", synthetic(f"saturation-{i}", i, args.saturation_repetitions, model))
                  for i in range(args.saturation_requests)]
    cases = core + recovery + saturation
    by_id = dict(cases)
    warmup = synthetic("warmup", 0, args.repetitions, model)
    cancelled = synthetic("cancelled", 1, args.cancel_repetitions, model)
    records = (out / "records.jsonl").open("w")

    def emit(row):
        records.write(json.dumps(row) + "\n")
        records.flush()

    baseline, modes = {}, []
    summary = {"schema": "clef.cuda-batch-check.v1", "status": "failed", "chunk": args.chunk,
               "tolerance": args.tolerance, "binary_sha256": hashlib.sha256(Path(args.binary).read_bytes()).hexdigest(),
               "modes": modes}
    try:
        for slots in (1, 2, 4):
            mode = "serial" if slots == 1 else f"batch{slots}"
            server = Server(args, slots, out / f"server-{mode}.log")
            try:
                row = send(args.port, warmup, "warmup", mode, "warmup", args.timeout)
                emit(row)
                probabilities(warmup, row["status"], row["response"])
                if slots == 1:
                    for identity, body in cases:
                        row = send(args.port, body, identity, mode, "reference", args.timeout)
                        probabilities(body, row["status"], row["response"])
                        if "batch" in row["response"]["psionic"]:
                            raise AssertionError("serial mode reported batch execution")
                        baseline[identity] = row
                        emit(row)
                    continue
                # All core cases at once: requests that fit one chunk only share
                # FFN work when they wait together behind a running step.
                measured = burst(args.port, core, len(core), mode, "concurrent", args.timeout, emit)
                emit(dict(cancel(args.port, cancelled, args.cancel_after), mode=mode))
                label, body = recovery[0]
                row = send(args.port, body, label, mode, "recovery", args.timeout)
                emit(row)
                measured.append(row)
                overloaded = burst(args.port, saturation, len(saturation), mode, "saturation", args.timeout, emit)
                refused = [r for r in overloaded if r["status"] == 503 and r["response"].get("error", {}).get("code") == "busy"]
                for r in overloaded:
                    if r["status"] == 200:
                        measured.append(r)
                    elif r not in refused:
                        raise AssertionError(f"unexpected saturation outcome {r['status']}: {r['response']}")
                if not refused or len(refused) == len(overloaded):
                    raise AssertionError("saturation must both admit work and return busy")
                label, body = recovery[1]
                row = send(args.port, body, label, mode, "recovery", args.timeout)
                emit(row)
                measured.append(row)
                deltas = [compare(by_id[r["case"]], baseline[r["case"]], r, args.tolerance) for r in measured]
                groups = [r["response"]["psionic"].get("batch", {}) for r in measured if r["phase"] == "concurrent"]
                if not any(type(g.get("max_requests")) is int and 2 <= g["max_requests"] <= slots
                           and g.get("grouped_ffn") is True for g in groups):
                    raise AssertionError("no evidence of actual shared FFN work")
                core_rows = [r for r in measured if r["phase"] == "concurrent"]
                wall = max(r["finished"] for r in core_rows) - min(r["started"] for r in core_rows)
                serial = sum(baseline[i]["seconds"] for i, _ in core)
                modes.append({"mode": mode, "status": "passed", "compared_requests": len(measured),
                              "max_abs_probability_delta": max(deltas), "bitwise_identical": max(deltas) == 0,
                              "top_answer_agreement": 1.0,
                              "max_cohort": max(g.get("max_requests", 0) for g in groups),
                              "busy_refusals": len(refused), "saturation_requests": len(overloaded),
                              "cancellation_recovery": "passed", "saturation_recovery": "passed",
                              "concurrent_wall_seconds": wall, "serial_core_seconds": serial,
                              "relative_throughput": serial / wall,
                              "median_queue_ms": statistics.median(r["response"]["psionic"]["timing"]["queue_ms"] for r in core_rows),
                              "median_prefill_ms": statistics.median(r["response"]["psionic"]["timing"]["prefill_ms"] for r in core_rows),
                              "input_tokens": sorted({baseline[i]["response"]["usage"]["input_tokens"] for i, _ in core})})
            finally:
                server.close()
        summary["status"] = "passed"
    except BaseException as error:
        summary.update(error_type=type(error).__name__, error=str(error))
        raise
    finally:
        records.close()
        (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
        print(json.dumps(summary, indent=2))
    return True


def self_test():
    body = synthetic("identity", 1, 1, "clef-flash")
    response = {"model": "clef-flash", "answers": {
        "identity-status": {"type": "choice", "choice": "waiting", "probabilities": {"open": 0.1, "waiting": 0.8, "resolved": 0.1}},
        "identity-urgent": {"type": "noul", "noul": 0.7}},
        "usage": {"input_tokens": 50, "cached_input_tokens": 0},
        "psionic": {"backend": "cuda", "truncated_state_tokens": 0, "artifact_digest": "a", "head_digest": "b",
                    "timing": {"encode_ms": 1, "queue_ms": 2, "prefill_ms": 3, "head_ms": 4, "total_ms": 10, "cached": False}}}
    row = {"status": 200, "response": response}
    assert compare(body, row, row, 1e-3) == 0
    mutations = [lambda x: x["answers"].pop("identity-urgent"),
                 lambda x: x["usage"].update(cached_input_tokens=1),
                 lambda x: x["psionic"]["timing"].update(cached=True),
                 lambda x: x["psionic"].update(backend="cpu"),
                 lambda x: x["answers"]["identity-urgent"].update(noul=0.7000001 + 1e-3),
                 lambda x: x["answers"]["identity-status"]["probabilities"].update(open=float("nan")),
                 lambda x: x["psionic"].update(artifact_digest="other")]
    for mutate in mutations:
        value = copy.deepcopy(response)
        mutate(value)
        try:
            compare(body, row, {"status": 200, "response": value}, 1e-3)
        except AssertionError:
            pass
        else:
            raise AssertionError("invalid response accepted")
    print("Self-test passed.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--binary")
    parser.add_argument("--model")
    parser.add_argument("--model-id", default="clef-flash")
    parser.add_argument("--out")
    parser.add_argument("--port", type=int, default=18200)
    parser.add_argument("--chunk", type=int, default=256)
    parser.add_argument("--cases", type=int, default=8)
    parser.add_argument("--repetitions", type=int, default=96)
    parser.add_argument("--saturation-repetitions", type=int, default=128)
    parser.add_argument("--saturation-requests", type=int, default=16)
    parser.add_argument("--cancel-repetitions", type=int, default=1024)
    parser.add_argument("--cancel-after", type=float, default=0.05)
    parser.add_argument("--max-queue", type=int, default=8)
    parser.add_argument("--tolerance", type=float, default=1e-3)
    parser.add_argument("--timeout", type=float, default=300)
    parser.add_argument("--start-timeout", type=float, default=300)
    args = parser.parse_args()
    if args.self_test:
        self_test()
    elif args.binary and args.model and args.out:
        check(args)
    else:
        parser.error("provide --binary, --model and --out, or --self-test")
