#!/usr/bin/env python3
"""W3 (#12390): derive the measured placement input from chain reports.

    placement_memory.py --commit REV --binary-sha256 HEX REPORT.json [...] > placement.json

Each REPORT.json is a `psionic-v41-stage chain` report from one machine.
Together they must load every decoder layer, the embeddings, and the head.
The output is the `psionic.deepseek_v41.placement_memory.v1` file that
`psionic_models::deepseek_v41_stage::MeasuredMemory` reads:

  - per layer: the GPU weight and state bytes the stage's allocation ledger
    recorded, and the Engram table's page-locked host bytes;
  - the runtime model: bytes per stored KV-source row (KV plus indexer
    keys), the compressor's pending group per unit of ratio, the scratch per
    row of the largest source, and the step buffers per token, each from
    the ledger; and a per-stage constant, the largest residual between a
    stage's measured peak (`cudaMemGetInfo`) and the modeled bytes;
  - checks: every stage's measured peak, which the model must cover.
"""

import argparse
import json
import math
import sys

SCHEMA = "psionic.deepseek_v41.placement_memory.v1"
REPO = "deepseek-ai/DeepSeek-V4.1-Flash-1017"
REVISION = "1bae0e1088993b9b82de08f59de5be5a34e42170"
INDEX_SHA256 = "74b0686a3d2891980d5e303251b075a3bccae2c2ff650747db2620a649b98fa8"


def ratio(layer):
    return 0 if layer <= 1 else 2 if layer <= 19 else 1


def kv_owner(layer):
    owners = [s for s in (2, 8, 14, 20) if s <= layer]
    return owners[-1] if owners else None


def modeled(stage_layers, layers, embed, head, rt, context, step):
    first, end = stage_layers
    sources = []
    for layer in range(first, end):
        if ratio(layer) == 0:
            continue
        owner = kv_owner(layer)
        if owner is not None and owner not in sources:
            sources.append(owner)
    total = sum(layers[l]["gpu_weight_bytes"] + layers[l]["gpu_state_bytes"] for l in range(first, end))
    if first == 0:
        total += embed
    if end == 40:
        total += head
    largest = 0
    for owner in sources:
        r = max(ratio(owner), 1)
        rows = context // r + 1
        largest = max(largest, rows)
        total += rows * rt["source_row_bytes"]
        if first <= owner < end:
            total += r * rt["compressor_bytes_per_ratio"]
    total += max(largest, 1) * rt["score_bytes_per_row"]
    total += step * rt["step_token_bytes"]
    return total


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--commit", required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--gpu", default="NVIDIA RTX PRO 6000 Blackwell Server Edition (sm_120, 96 GB)")
    parser.add_argument("--served-max-prompt-tokens", type=int, required=True)
    parser.add_argument("reports", nargs="+")
    args = parser.parse_args()
    reports = [json.load(open(p)) for p in args.reports]

    layers, embed, head = {}, None, None
    row_bytes, compressor, step_bytes = set(), set(), set()
    score_points = []
    stages = []
    for report in reports:
        for stage in report["stages"]:
            ledger = stage["ledger"]
            for l in ledger["layers"]:
                entry = {
                    "layer": l["layer"],
                    "gpu_weight_bytes": l["weight_bytes"],
                    "gpu_state_bytes": l["state_bytes"],
                    "engram_host_bytes": 0,
                }
                if l["layer"] in layers and layers[l["layer"]]["gpu_weight_bytes"] != entry["gpu_weight_bytes"]:
                    sys.exit(f"layer {l['layer']} loaded with different weight bytes")
                layers[l["layer"]] = entry
            if ledger["embedding_bytes"]:
                embed = ledger["embedding_bytes"]
            if ledger["head_bytes"]:
                head = ledger["head_bytes"]
            for e in stage["engram"]:
                layers.setdefault(e["layer"], {})["engram_host_bytes"] = e["host_bytes"]
            caps = []
            for s in ledger["sources"]:
                row_bytes.add((s["kv_bytes"] + s["index_key_bytes"]) / s["capacity"])
                if s["compressor_state_bytes"]:
                    compressor.add(s["compressor_state_bytes"] / s["ratio"])
                caps.append(s["capacity"])
            step_bytes.add(ledger["work_step_bytes"] / ledger["max_step_tokens"])
            fixed_work = ledger["work_bytes"] - ledger["work_step_bytes"]
            score_points.append((max(caps) if caps else 1, fixed_work, tuple(stage["layers"])))
            stages.append((stage, report))
    if sorted(layers) != list(range(40)) or embed is None or head is None:
        sys.exit("the reports don't cover layers 0-39, the embeddings, and the head")
    if len(row_bytes) != 1 or len(step_bytes) != 1 or len(compressor) > 1:
        sys.exit(f"inconsistent ledgers: rows {row_bytes}, steps {step_bytes}, compressor {compressor}")
    # Scratch that grows with the largest source: the slope of the fixed work
    # over capacity, between loads of the same layers at different contexts.
    slopes = []
    by_layers = {}
    for cap, work, key in score_points:
        by_layers.setdefault(key, []).append((cap, work))
    for points in by_layers.values():
        points = sorted(set(points))
        for (c0, w0), (c1, w1) in zip(points, points[1:]):
            if c1 != c0:
                slopes.append((w1 - w0) / (c1 - c0))
    if not slopes:
        sys.exit("need loads of the same layers at two contexts")
    rt = {
        "per_stage_bytes": 0,
        "source_row_bytes": int(row_bytes.pop()),
        "compressor_bytes_per_ratio": int(compressor.pop()) if compressor else 0,
        "score_bytes_per_row": math.ceil(max(slopes)),
        "step_token_bytes": int(step_bytes.pop()),
    }
    checks, residuals = [], []
    for stage, report in stages:
        first, end = stage["layers"]
        context = stage["ledger"]["max_context"]
        step = stage["ledger"]["max_step_tokens"]
        run = stage.get("run") or {}
        peak = run.get("peak_used_total")
        if peak is None:
            continue
        residuals.append(peak - modeled((first, end), layers, embed, head, rt, context, step))
        checks.append(
            {
                "first_layer": first,
                "end_layer": end,
                "max_context": context,
                "max_step_tokens": step,
                "peak_used_bytes": peak,
            }
        )
    rt["per_stage_bytes"] = max(residuals)
    contexts = sorted({c["max_context"] for c in checks})
    out = {
        "schema": SCHEMA,
        "status": "provisional",
        "runtime": {
            "name": "psionic-v41-stage/1+cuda",
            "commit": args.commit,
            "binary_sha256": args.binary_sha256,
            "gpu": args.gpu,
            "precision": "FP8 E4M3 dense and FP4 E2M1 routed-expert weights with UE8M0 scales, "
            "some small BF16 weights widened to F32 on the device; BF16 KV and indexer keys; "
            "F32 residual streams; ReferenceBf16 reduction order",
        },
        "repo": REPO,
        "revision": REVISION,
        "index_sha256": INDEX_SHA256,
        "host_gpu_split": "every decoder layer's weights, caches, and scratch on its stage's GPU; "
        "the Engram tables of layers 1 and 14 in page-locked host memory of the machine "
        "running that layer, read over PCIe row by row",
        "measured_contexts": contexts,
        "served_max_prompt_tokens": args.served_max_prompt_tokens,
        "layers": [layers[i] for i in range(40)],
        "embeddings_bytes": embed,
        "head_bytes": head,
        "runtime_bytes": rt,
        "checks": checks,
    }
    json.dump(out, sys.stdout, indent=1)
    print()


if __name__ == "__main__":
    main()
