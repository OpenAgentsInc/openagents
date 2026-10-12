#!/usr/bin/env python3
"""Check Metal prefix logits and speed with synthetic requests on a shared Mac.

Run under caller-held GPU and memory leases. Config uses mac_quiet_bench.py's
model_path, requests (one synthetic request), output_dir, and binary variant.
The original schema primes each prefix. A changed schema tests reuse. Exported
raw logits disable whole-answer caching in both servers. Only owned children
are stopped. No quiet lease or desktop controls are used.
"""

import argparse
import copy
import json
import math
import os
from pathlib import Path
import signal
import statistics
import uuid

import mac_quiet_bench as bench


def check(config):
    if "gpu" not in os.environ.get("OPENAGENTS_LEASES", "").split(","):
        raise ValueError("run through openagents lease gpu")
    if len(config["requests"]) != 1 or len(config["variants"]) != 1:
        raise ValueError("one synthetic request and one local binary are required")
    variant = config["variants"][0]
    if "binary" not in variant:
        raise ValueError("raw logit export requires an owned server")
    if any("PROFILE" in key or "SKIP" in key for key in variant.get("env", {})):
        raise ValueError("profiling and skipped kernels are not correctness or speed evidence")
    config = dict(config, measurement_mode="shared")
    output = Path(config["output_dir"])
    output.mkdir(parents=True, exist_ok=False)
    base = json.loads(Path(config["requests"][0]).read_text())
    base["model"] = variant["model"]
    if not isinstance(base.get("questions"), dict) or not base["questions"]:
        raise ValueError("fixture needs a nonempty question map")
    pairs = []
    for _ in range(max(5, config["runs"])):
        first = copy.deepcopy(base)
        first["state"] = f"PREFIX CHECK {uuid.uuid4().hex}\n\n{first['state']}"
        second = copy.deepcopy(first)
        question = next(iter(second["questions"].values()))
        question["instructions"] += " Consider the complete state."
        pairs.append((first, second))
    (output / "requests.json").write_text(json.dumps(pairs, indent=2) + "\n")
    metadata = {"created_at": bench.now(), "config": config,
                "binary_sha256": bench.file_sha(variant["binary"]),
                "model_sha256": bench.file_sha(config["model_path"]),
                "measurement_mode": "shared", "quiet_lease": None,
                "note": "Raw-logit export is enabled; these are prefix acceptance timings, not uncached serving benchmarks."}
    (output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    host = bench.Host(config, output)
    host.thread.start()
    samples = []
    try:
        host.settle()
        with (output / "records.jsonl").open("w") as records:
            for mode, capacity in [("disabled", "0"), ("reuse", str(512 * 1024 * 1024))]:
                current = dict(variant, name=mode, env={**variant.get("env", {}),
                    "PSIONIC_CLEF_PREFIX_CACHE_BYTES": capacity})
                with bench.server(config, current, 0, output, records, host,
                                  export_dir=output / mode) as url:
                    def send(body, label, expected_hit):
                        host.guard()
                        elapsed, code, response = bench.request(url, body, config["request_timeout"])
                        cached = response.get("usage", {}).get("cached_input_tokens", 0)
                        row = {"mode": mode, "label": label, "seconds": elapsed,
                               "request_sha256": "sha256:" + bench.sha(bench.encoded(body)),
                               "status": code, "response": response, "load": host.guard()}
                        bench.emit(records, row)
                        samples.append(row)
                        if code != 200 or "answers" not in response:
                            raise AssertionError(f"{mode}/{label}: request failed")
                        if bool(cached) != expected_hit or response["psionic"]["timing"]["cached"]:
                            raise AssertionError(f"{mode}/{label}: unexpected cache usage")
                        if not 0 <= cached < response["usage"]["input_tokens"]:
                            raise AssertionError("invalid cached token count")
                    for index, (first, second) in enumerate(pairs):
                        if mode == "reuse":
                            send(first, f"prime-{index}", False)
                        send(second, f"measure-{index}", mode == "reuse")
                    if mode == "reuse":
                        # The single-entry cache must reject a different state,
                        # then recover the previous schema only after recomputing.
                        changed = copy.deepcopy(pairs[-1][1])
                        changed["state"] += "\nThe customer has resolved the problem."
                        send(changed, "state-change", False)
                        send(pairs[-1][1], "evicted", False)
                        send(pairs[-1][1], "restored", True)
        tables = {}
        for mode in ("disabled", "reuse"):
            table = {}
            for line in (output / mode / "rows.jsonl").read_text().splitlines():
                row = json.loads(line)
                table.setdefault((row["request_sha256"], row["question_index"]), []).append(row["logits"])
            tables[mode] = table
        maximum = 0.0
        compared = 0
        expected_keys = {("sha256:" + bench.sha(bench.encoded(second)), index)
                         for _, second in pairs for index in range(len(second["questions"]))}
        if set(tables["disabled"]) != expected_keys or not expected_keys <= set(tables["reuse"]):
            raise AssertionError("raw-logit export is missing expected requests or questions")
        for key, values in tables["disabled"].items():
            assert len(values) == 1, "duplicate baseline export"
            for actual in tables["reuse"][key]:
                expected = values[0]
                assert len(actual) == len(expected) and actual
                assert all(math.isfinite(value) for value in actual + expected), "nonfinite logits"
                maximum = max(maximum, max(abs(a - b) for a, b in zip(actual, expected)))
                compared += 1
        assert compared >= len(expected_keys) > 0
        medians = {mode: statistics.median(row["seconds"] for row in samples
                   if row["mode"] == mode and row["label"].startswith("measure-"))
                   for mode in ("disabled", "reuse")}
        speedup = medians["disabled"] / medians["reuse"]
        result = {"max_abs_logit_delta": maximum, "compared_questions": compared,
                  "median_seconds": medians, "speedup": speedup,
                  "logit_gate": maximum <= 1e-3, "speed_gate": speedup >= 5,
                  "conditions": "shared machine; no idle-machine claim"}
        (output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result, indent=2))
        return result["logit_gate"] and result["speed_gate"]
    finally:
        host.stop.set()
        host.thread.join(timeout=6)


if __name__ == "__main__":
    def interrupt(signum, frame):
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, interrupt)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config")
    args = parser.parse_args()
    raise SystemExit(0 if check(bench.load_config(args.config)) else 1)
