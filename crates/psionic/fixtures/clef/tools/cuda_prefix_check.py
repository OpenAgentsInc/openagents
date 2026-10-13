#!/usr/bin/env python3
"""Check CUDA Clef prefix reuse (#11442) on one dedicated GPU host.

The CUDA counterpart of metal_prefix_check.py: no leases or Mac host
monitor. It starts the given psionic-openai-server four times and sends
synthetic requests with a long shared state (default about 4k tokens) and a
schema of about 1k tokens:

  disabled  PSIONIC_CLEF_PREFIX_CACHE_BYTES=0, raw-logit export: the
            reference logits for every request, and the cold timings.
  reuse     the cache on, raw-logit export: each pair primes its state with
            one schema, then asks a changed schema (a hit); a changed state
            misses; every state asked again hits (the LRU keeps them all).
  eviction  a budget of about 1.5 snapshots: a second state evicts the
            first, which then misses once and hits again.
  default   PSIONIC_CLEF_PREFIX_CACHE_BYTES unset (on by default on sm_90):
            prompts shorter than one chunk that share a state hit too
            (--no-default skips this on devices where reuse is off).
  batch     the cache on with PSIONIC_CLEF_BATCH_REQUESTS=2 (no export, so
            probabilities are compared): concurrent requests restore
            snapshots, concurrent cold requests each publish one, and a
            request cancelled after its prefix was captured publishes none.

Gates: every exported logit equal to the disabled run (max |dlogit| == 0
unless --tolerance is raised), every batched probability equal to the
disabled run, hits and misses where expected with
`usage.cached_input_tokens` == `psionic.timing.restored_tokens`, and the
median repeated-state speedup >= --speedup (client seconds).

    cuda_prefix_check.py --binary B --model M --model-id clef-flash --out DIR
    cuda_prefix_check.py --self-test

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
import uuid

NAMES = ("Ada", "Bruno", "Chen", "Dalia", "Emeka", "Farah", "Goran", "Hana")
NOTES = ("the customer asked for a callback", "the refund is still pending",
         "a replacement part shipped today", "billing confirmed the duplicate charge",
         "the device fails after the latest update", "the account was locked twice")


def encoded(body):
    return json.dumps(body, sort_keys=True, separators=(",", ":")).encode()


def digest(body):
    return "sha256:" + hashlib.sha256(encoded(body)).hexdigest()


def state_text(tag, lines):
    rows = [f"CASE {tag}", "Support log for one customer account, oldest first."]
    for i in range(lines):
        rows.append(f"Entry {i}: ticket {i % 37} moved from queue {i % 5} to queue {(i * 7) % 5}; "
                    f"{NAMES[i % len(NAMES)]} noted that {NOTES[(i * 5) % len(NOTES)]}.")
    rows.append(f"Latest: the customer in case {tag} wrote again this morning.")
    return "\n".join(rows)


def schema(questions):
    result = {}
    for q in range(questions):
        if q % 2:
            result[f"q{q}"] = {"type": "noul", "instructions":
                               f"Question {q}: considering every entry in the log, did the agents "
                               f"promise the customer something in step {q} that is still open, such "
                               f"as a callback, a refund, or a replacement that has not arrived?"}
        else:
            result[f"q{q}"] = {"type": "choice", "instructions":
                               f"Question {q}: what should support do next for this account, judging "
                               f"from the most recent entries and the customer's latest message?",
                               "criteria": {"call": "Call the customer back today about the open ticket.",
                                            "refund": "Issue or chase the pending refund with billing.",
                                            "ship": "Ship or trace the replacement part.",
                                            "close": "Close the ticket; nothing is left to do."}}
    return result


def long_schema(questions, words):
    """A long suffix: `questions` noul questions of about `words` words each."""
    filler = " ".join(f"detail{i % 50}" for i in range(words))
    return {f"long{q}": {"type": "noul", "instructions": f"Question {q}: is this note relevant? {filler}"}
            for q in range(questions)}


def case(model, lines, questions):
    """A (prime, measure) pair: the same state, the measure's schema changed."""
    tag = uuid.uuid4().hex
    first = {"model": model, "state": state_text(tag, lines), "questions": schema(questions)}
    second = copy.deepcopy(first)
    last = sorted(second["questions"])[-1]
    second["questions"][last]["instructions"] += " Consider the complete state."
    return first, second


def finite(value):
    return type(value) in (int, float) and math.isfinite(value)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


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


def probabilities(body, response):
    result = {}
    for identity, question in body["questions"].items():
        answer = response["answers"][identity]
        if question["type"] == "noul":
            result[identity] = {"true": answer["noul"]}
        else:
            result[identity] = dict(answer["probabilities"])
    return result


def restored(response):
    cached = response.get("usage", {}).get("cached_input_tokens")
    timing = response.get("psionic", {}).get("timing", {})
    if cached != timing.get("restored_tokens"):
        raise AssertionError("usage.cached_input_tokens and psionic.timing.restored_tokens differ")
    if timing.get("cached") is not False:
        raise AssertionError("answer came from the whole-answer logit cache")
    if not finite(cached) or not 0 <= cached < response["usage"]["input_tokens"]:
        raise AssertionError("invalid cached token count")
    return cached


class Server:
    def __init__(self, args, name, out, env, export=None):
        environment = dict(os.environ, PSIONIC_CLEF_LOG="0")
        for key in ("PSIONIC_CLEF_PROFILE", "PSIONIC_CLEF_SKIP", "PSIONIC_CLEF_PREFIX_CACHE_BYTES"):
            environment.pop(key, None)
        environment.update(env)
        command = [args.binary, "-m", args.model, "--host", "127.0.0.1", "--port", str(args.port),
                   "--decision-device", "cuda", "--decision-chunk", str(args.chunk),
                   "--decision-max-tokens", "65536", "--decision-max-questions", "64"]
        if export is not None:
            command += ["--decision-export-rows", str(export)]
        self.log = open(out / f"server-{name}.log", "w")
        self.process = subprocess.Popen(command, env=environment, stdout=self.log, stderr=subprocess.STDOUT)
        deadline = time.monotonic() + args.start_timeout
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise RuntimeError(f"server {name} exited with {self.process.returncode}")
            try:
                connection = http.client.HTTPConnection("127.0.0.1", args.port, timeout=2)
                connection.request("GET", "/health")
                if connection.getresponse().status == 200:
                    return
            except OSError:
                pass
            time.sleep(1)
        self.close()
        raise RuntimeError(f"server {name} did not come up")

    def close(self):
        self.process.terminate()
        try:
            self.process.wait(timeout=30)
        except subprocess.TimeoutExpired:
            self.process.kill()
        self.log.close()


def exported(directory):
    table = {}
    for line in (directory / "rows.jsonl").read_text().splitlines():
        row = json.loads(line)
        table.setdefault((row["request_sha256"], row["question_index"]), []).append(row["logits"])
    return table


def logit_delta(reference, actual):
    """Max |dlogit| of every actual export row against the single reference row."""
    maximum, compared = 0.0, 0
    for key, rows in actual.items():
        if key not in reference or len(reference[key]) != 1:
            raise AssertionError(f"missing or duplicate reference logits for {key}")
        expected = reference[key][0]
        for values in rows:
            if len(values) != len(expected) or not values:
                raise AssertionError("logit length changed")
            if not all(math.isfinite(v) for v in values + expected):
                raise AssertionError("nonfinite logits")
            maximum = max(maximum, max(abs(a - b) for a, b in zip(values, expected)))
            compared += 1
    return maximum, compared


def check(args):
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=False)
    model = args.model_id
    pairs = [case(model, args.lines, args.questions) for _ in range(args.pairs)]
    changed = copy.deepcopy(pairs[-1][1])
    # changed near the start: CUDA rounds the prefix down to 16 tokens, so a
    # change in the state's last few tokens can still share (and exactly
    # restore) the aligned prefix
    changed["state"] = "Update: the customer has resolved the problem.\n" + changed["state"]
    evict_a, evict_b = case(model, args.lines, args.questions), case(model, args.lines, args.questions)
    batch_pairs = [case(model, args.lines, args.questions) for _ in range(2)]
    # shorter than one chunk, so the lane clamps their chunk to their length
    short_pairs = [case(model, args.short_lines, 3) for _ in range(2)]
    cancel_body = {"model": model, "state": state_text(uuid.uuid4().hex, args.cancel_lines),
                   "questions": long_schema(args.cancel_questions, args.cancel_words)}
    cancel_follow = copy.deepcopy(cancel_body)
    cancel_follow["questions"] = schema(2)
    # a schema not asked before, so the whole-answer logit cache (on without
    # export) cannot answer it; its state is pairs[0]'s, so it is a hit
    before_cancel = copy.deepcopy(pairs[0][1])
    before_cancel["questions"][sorted(before_cancel["questions"])[0]]["instructions"] += " Answer carefully."
    warmup = case(model, 8, 1)[0]
    records = (out / "records.jsonl").open("w")
    lock = threading.Lock()

    def send(mode, label, body, expect=None):
        seconds, status, response = post(args.port, body, args.timeout)
        row = {"mode": mode, "label": label, "request_sha256": digest(body), "seconds": seconds,
               "status": status, "response": response}
        with lock:
            records.write(json.dumps(row) + "\n")
            records.flush()
        if status != 200 or "answers" not in response:
            raise AssertionError(f"{mode}/{label}: status {status}: {response}")
        cached = restored(response)
        row["cached"] = cached
        if expect is not None and bool(cached) != expect:
            raise AssertionError(f"{mode}/{label}: expected {'a hit' if expect else 'a miss'}, "
                                 f"restored {cached} tokens")
        return row

    summary = {"schema": "clef.cuda-prefix-check.v1", "status": "failed", "model": model,
               "chunk": args.chunk, "budget_bytes": args.budget,
               "binary_sha256": hashlib.sha256(Path(args.binary).read_bytes()).hexdigest()}
    reference_probabilities = {}
    try:
        # disabled: reference logits and cold timings
        server = Server(args, "disabled", out, {"PSIONIC_CLEF_PREFIX_CACHE_BYTES": "0"}, out / "disabled")
        try:
            send("disabled", "warmup", warmup, False)
            cold = []
            bodies = [b for pair in pairs for b in pair] + [changed] + list(evict_a) + list(evict_b)
            bodies += [b for pair in batch_pairs for b in pair] + [cancel_follow]
            bodies += [b for pair in short_pairs for b in pair]
            for index, body in enumerate(bodies):
                row = send("disabled", f"reference-{index}", body, False)
                reference_probabilities[digest(body)] = probabilities(body, row["response"])
                if any(body is second for _, second in pairs):
                    cold.append(row)
            summary["input_tokens"] = cold[0]["response"]["usage"]["input_tokens"]
        finally:
            server.close()

        # reuse: hits after priming, a changed state misses, all states retained
        budget = str(args.budget)
        server = Server(args, "reuse", out, {"PSIONIC_CLEF_PREFIX_CACHE_BYTES": budget}, out / "reuse")
        warm, per_entry = [], None
        try:
            baseline = send("reuse", "warmup", warmup, False)["response"]["psionic"]["prefix_cache"]["bytes"]
            for index, (first, second) in enumerate(pairs):
                row = send("reuse", f"prime-{index}", first, False)
                if per_entry is None:
                    per_entry = row["response"]["psionic"]["prefix_cache"]["bytes"] - baseline
                warm.append(send("reuse", f"measure-{index}", second, True))
            summary["snapshot_bytes"] = per_entry
            summary["prefix_tokens"] = warm[0]["cached"]
            send("reuse", "state-change", changed, False)
            for index, (_, second) in enumerate(pairs):
                send("reuse", f"retained-{index}", second, True)
            stats = send("reuse", "retained-last", pairs[-1][1], True)["response"]["psionic"]["prefix_cache"]
            summary["reuse_cache"] = stats
            if stats["entries"] < len(pairs) + 1:
                raise AssertionError("the LRU did not keep every primed state")
        finally:
            server.close()
        delta, compared = logit_delta(exported(out / "disabled"), exported(out / "reuse"))
        summary["reuse_max_abs_logit_delta"], summary["reuse_compared_questions"] = delta, compared

        # eviction: a budget of about one and a half snapshots
        small = str(int(per_entry * 1.5))
        server = Server(args, "eviction", out, {"PSIONIC_CLEF_PREFIX_CACHE_BYTES": small}, out / "eviction")
        try:
            send("eviction", "prime-a", evict_a[0], False)
            send("eviction", "hit-a", evict_a[1], True)
            send("eviction", "prime-b", evict_b[0], False)
            send("eviction", "evicted-a", evict_a[1], False)
            send("eviction", "restored-a", evict_a[1], True)
            row = send("eviction", "evicted-b", evict_b[1], False)
            if row["response"]["psionic"]["prefix_cache"]["entries"] != 1:
                raise AssertionError("the small budget kept more than one snapshot")
        finally:
            server.close()
        delta_e, compared_e = logit_delta(exported(out / "disabled"), exported(out / "eviction"))
        summary["eviction_budget_bytes"] = int(small)
        summary["eviction_max_abs_logit_delta"], summary["eviction_compared_questions"] = delta_e, compared_e

        # default: no budget set, short prompts
        summary["default_max_abs_logit_delta"] = None
        if not args.no_default:
            server = Server(args, "default", out, {}, out / "default")
            try:
                for index, (first, second) in enumerate(short_pairs):
                    send("default", f"prime-{index}", first, False)
                    send("default", f"measure-{index}", second, True)
            finally:
                server.close()
            delta_d, compared_d = logit_delta(exported(out / "disabled"), exported(out / "default"))
            summary["default_max_abs_logit_delta"], summary["default_compared_questions"] = delta_d, compared_d

        # batch: concurrent restores, concurrent captures, cancellation
        server = Server(args, "batch", out, {"PSIONIC_CLEF_PREFIX_CACHE_BYTES": budget,
                                             "PSIONIC_CLEF_BATCH_REQUESTS": "2"})
        batch_rows = []

        def together(label, bodies, expect):
            barrier = threading.Barrier(len(bodies))

            def one(item):
                index, body = item
                barrier.wait(timeout=60)
                return send("batch", f"{label}-{index}", body, expect)
            with concurrent.futures.ThreadPoolExecutor(max_workers=len(bodies)) as pool:
                rows = list(pool.map(one, enumerate(bodies)))
            batch_rows.extend(rows)
            return rows
        try:
            send("batch", "warmup", warmup, False)
            for index, (first, _) in enumerate(pairs[:2]):
                send("batch", f"prime-{index}", first, False)
            restores = together("restore", [second for _, second in pairs[:2]], True)
            together("capture", [first for first, _ in batch_pairs], False)
            together("restore-captured", [second for _, second in batch_pairs], True)
            before = send("batch", "before-cancel", before_cancel, True)["response"]["psionic"]["prefix_cache"]
            raw = encoded(cancel_body)
            with socket.create_connection(("127.0.0.1", args.port), timeout=10) as client:
                head = ("POST /v1/systemone HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\n"
                        f"Content-Length: {len(raw)}\r\n\r\n")
                client.sendall(head.encode() + raw)
                time.sleep(args.cancel_after)
                client.shutdown(socket.SHUT_RDWR)
            time.sleep(1.0)
            follow = send("batch", "after-cancel", cancel_follow, False)
            after = follow["response"]["psionic"]["prefix_cache"]
            # the follow-up itself publishes one snapshot (or evicts to make room)
            if after["entries"] > before["entries"] + 1:
                raise AssertionError("the cancelled request published its snapshot")
            batch_rows.append(follow)
            batched = [r for r in restores if r["response"]["psionic"].get("batch", {}).get("max_requests", 0) >= 2]
            summary["batched_restores"] = len(batched)
        finally:
            server.close()
        batch_delta = 0.0
        by_digest = {digest(b): b for b in bodies}
        for row in batch_rows:
            reference = reference_probabilities[row["request_sha256"]]
            actual = probabilities(by_digest[row["request_sha256"]], row["response"])
            for identity, values in actual.items():
                for option, p in values.items():
                    batch_delta = max(batch_delta, abs(p - reference[identity][option]))
        summary["batch_max_abs_probability_delta"] = batch_delta
        summary["batch_compared_requests"] = len(batch_rows)

        medians = {"disabled": statistics.median(r["seconds"] for r in cold),
                   "reuse": statistics.median(r["seconds"] for r in warm)}
        server_ms = {"disabled": statistics.median(r["response"]["psionic"]["timing"]["total_ms"] for r in cold),
                     "reuse": statistics.median(r["response"]["psionic"]["timing"]["total_ms"] for r in warm)}
        summary.update(median_seconds=medians, median_server_ms=server_ms,
                       speedup=medians["disabled"] / medians["reuse"],
                       server_speedup=server_ms["disabled"] / server_ms["reuse"])
        worst = max(summary["reuse_max_abs_logit_delta"], summary["eviction_max_abs_logit_delta"],
                    summary["default_max_abs_logit_delta"] or 0.0)
        summary["logit_gate"] = worst <= args.tolerance and batch_delta <= args.tolerance
        summary["speed_gate"] = summary["speedup"] >= args.speedup
        summary["status"] = "passed" if summary["logit_gate"] and summary["speed_gate"] else "gate failed"
    except BaseException as error:
        summary.update(error_type=type(error).__name__, error=str(error))
        raise
    finally:
        records.close()
        (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
        print(json.dumps(summary, indent=2))
    return summary["status"] == "passed"


def self_test():
    first, second = case("clef-flash", 4, 3)
    assert first["state"] == second["state"] and first["questions"] != second["questions"]
    assert digest(first) != digest(second)
    response = {"answers": {"q0": {"probabilities": {"call": 0.7, "refund": 0.1, "ship": 0.1, "close": 0.1}},
                            "q1": {"noul": 0.4}, "q2": {"probabilities": {"call": 1, "refund": 0, "ship": 0, "close": 0}}},
                "usage": {"input_tokens": 100, "cached_input_tokens": 60},
                "psionic": {"timing": {"cached": False, "restored_tokens": 60}}}
    assert restored(response) == 60
    assert probabilities(first, response)["q1"] == {"true": 0.4}
    for mutate in (lambda r: r["psionic"]["timing"].update(restored_tokens=0),
                   lambda r: r["psionic"]["timing"].update(cached=True),
                   lambda r: (r["usage"].update(cached_input_tokens=100),
                              r["psionic"]["timing"].update(restored_tokens=100))):
        bad = copy.deepcopy(response)
        mutate(bad)
        try:
            restored(bad)
        except AssertionError:
            pass
        else:
            raise AssertionError("invalid response accepted")
    reference = {("a", 0): [[1.0, 2.0]]}
    assert logit_delta(reference, {("a", 0): [[1.0, 2.0], [1.0, 2.5]]}) == (0.5, 2)
    try:
        logit_delta(reference, {("b", 0): [[1.0]]})
    except AssertionError:
        pass
    else:
        raise AssertionError("missing reference accepted")
    print("Self-test passed.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--binary")
    parser.add_argument("--model")
    parser.add_argument("--model-id", default="clef-flash")
    parser.add_argument("--out")
    parser.add_argument("--port", type=int, default=18210)
    parser.add_argument("--chunk", type=int, default=2048)
    parser.add_argument("--pairs", type=int, default=5)
    parser.add_argument("--lines", type=int, default=125, help="log lines in the state (125: about 4k tokens)")
    parser.add_argument("--questions", type=int, default=12, help="questions in the schema (12: about 1k tokens)")
    parser.add_argument("--short-lines", type=int, default=30, help="log lines of the short cases (about 1k tokens)")
    parser.add_argument("--no-default", action="store_true", help="skip the default-budget phase")
    parser.add_argument("--cancel-lines", type=int, default=60)
    parser.add_argument("--cancel-questions", type=int, default=64)
    parser.add_argument("--cancel-words", type=int, default=200)
    parser.add_argument("--cancel-after", type=float, default=0.6)
    parser.add_argument("--budget", type=int, default=8 << 30)
    parser.add_argument("--tolerance", type=float, default=0.0)
    parser.add_argument("--speedup", type=float, default=5.0)
    parser.add_argument("--timeout", type=float, default=300)
    parser.add_argument("--start-timeout", type=float, default=300)
    args = parser.parse_args()
    if args.self_test:
        self_test()
    elif args.binary and args.model and args.out:
        raise SystemExit(0 if check(args) else 1)
    else:
        parser.error("provide --binary, --model and --out, or --self-test")
