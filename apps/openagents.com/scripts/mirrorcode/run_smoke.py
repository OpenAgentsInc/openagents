#!/usr/bin/env python3
"""Run a single MirrorCode (Epoch Research) public task against openagents/khala.

This is the Phase-0 smoke runner described in
`docs/benchmarks/2026-06-27-mirrorcode-khala-gym-integration-analysis.md`
(epic #6376 / issue #6377). It treats the upstream Inspect harness as the
executor and Khala as a plain OpenAI-compatible model (Option A: zero provider
code). It does NOT modify the read-only MirrorCode clone and never trains/RAGs
on tasks.

It points Inspect's `openai` provider at Khala
(`OPENAI_BASE_URL=https://openagents.com/api/v1`, `OPENAI_API_KEY=<khala key>`,
model id `openai/openagents/khala`), runs ONE chosen public task with hard
token + wall-clock caps far below the paper's 1B/10B limits, and writes a
public-safe result JSON in the shared gym contract:

    { runId, model, taskId, bucket, status, passRate, tokens, startedAt,
      finishedAt, summary }

Requests are tagged `internal` / `gym_mirrorcode` via the Khala demand-attribution
headers (#6298) so the load is auditably an eval and stays preemptible (#6318).

Invoked by `run.sh`; can also be run directly inside a configured venv with
the MirrorCode clone on PYTHONPATH. See README.md.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
import sys
import uuid
from typing import Any


# Buckets per the paper (Table 3), mirrored from scripts/run_mirrorcode.py in the
# clone. Trivial targets (false, dirname, cal_simple, rev) are excluded from the
# benchmark proper; we allow them ONLY as the cheapest possible smoke target.
S_TARGETS = {
    "qsv_select", "jq_simple", "gron", "bitwise", "hexyl",
    "uuidparse", "numfmt", "cal", "choose",
}
M_TARGETS = {
    "giac", "tex", "gotree", "mailauth", "brotli",
    "wren_cli", "nonogrid", "sed", "tssql", "bib2json",
}
L_TARGETS = {"ruff", "pkl", "cprepro"}
TRIVIAL_EXCLUDED = {"false", "dirname", "cal_simple", "rev"}


def bucket_for_target(target: str) -> str:
    if target in S_TARGETS:
        return "S"
    if target in M_TARGETS:
        return "M"
    if target in L_TARGETS:
        return "L"
    if target in TRIVIAL_EXCLUDED:
        return "trivial_excluded"
    return "unknown"


def _iso(value: Any) -> str | None:
    if value is None:
        return None
    if isinstance(value, str):
        return value
    if isinstance(value, (_dt.datetime, _dt.date)):
        return value.isoformat()
    return str(value)


def _aggregate_tokens(model_usage: Any) -> dict[str, int]:
    """Sum Inspect ModelUsage records across models into a flat token dict."""
    totals = {
        "input": 0,
        "output": 0,
        "total": 0,
        "reasoning": 0,
        "cacheRead": 0,
        "cacheWrite": 0,
    }
    if not model_usage:
        return totals
    for usage in model_usage.values():
        totals["input"] += int(getattr(usage, "input_tokens", 0) or 0)
        totals["output"] += int(getattr(usage, "output_tokens", 0) or 0)
        totals["total"] += int(getattr(usage, "total_tokens", 0) or 0)
        totals["reasoning"] += int(getattr(usage, "reasoning_tokens", 0) or 0)
        totals["cacheRead"] += int(getattr(usage, "input_tokens_cache_read", 0) or 0)
        totals["cacheWrite"] += int(getattr(usage, "input_tokens_cache_write", 0) or 0)
    if totals["total"] == 0:
        totals["total"] = totals["input"] + totals["output"]
    return totals


def _score_value(sample: Any) -> dict[str, Any]:
    """Pull the MirrorCode per-group pass rates out of a scored sample."""
    scores = getattr(sample, "scores", None) or {}
    for name, score in scores.items():
        value = getattr(score, "value", None)
        if isinstance(value, dict):
            return {"scorer": name, "groups": value}
    return {"scorer": None, "groups": {}}


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description="MirrorCode x Khala Phase-0 smoke runner")
    p.add_argument(
        "--task",
        default=os.environ.get("MC_TASK_ID", "false_c"),
        help="Inspect sample id '<target>_<language>' (default: false_c).",
    )
    p.add_argument(
        "--model",
        default=os.environ.get("MC_MODEL", "openai/openagents/khala"),
        help="Inspect model id (default: openai/openagents/khala).",
    )
    p.add_argument(
        "--token-limit",
        type=int,
        default=int(os.environ.get("MC_TOKEN_LIMIT", "250000")),
        help="Hard per-sample token cap (default 250000; far below paper 1B).",
    )
    p.add_argument(
        "--time-limit",
        type=int,
        default=int(os.environ.get("MC_TIME_LIMIT", "1800")),
        help="Hard per-sample wall-clock cap in seconds (default 1800).",
    )
    p.add_argument(
        "--message-limit",
        type=int,
        default=int(os.environ.get("MC_MESSAGE_LIMIT", "120")),
        help="Hard per-sample message cap (default 120).",
    )
    p.add_argument(
        "--out",
        default=os.environ.get("MC_OUT", "mirrorcode-phase0-result.json"),
        help="Path to write the public-safe result JSON.",
    )
    p.add_argument(
        "--log-dir",
        default=os.environ.get("MC_LOG_DIR", None),
        help="Inspect log dir (default: Inspect's ./logs).",
    )
    p.add_argument(
        "--demand-source",
        default=os.environ.get("MC_DEMAND_SOURCE", "gym_mirrorcode"),
    )
    p.add_argument(
        "--demand-client",
        default=os.environ.get("MC_DEMAND_CLIENT", "mirrorcode-phase0"),
    )
    args = p.parse_args(argv)

    if not os.environ.get("OPENAI_API_KEY"):
        print("ERROR: OPENAI_API_KEY (a Khala key) is not set.", file=sys.stderr)
        return 2
    os.environ.setdefault("OPENAI_BASE_URL", "https://openagents.com/api/v1")

    import inspect_ai
    from mc.task import mirrorcode

    target = args.task.rsplit("_", 1)[0]
    bucket = bucket_for_target(target)
    run_id = (
        "mc-phase0-"
        + _dt.datetime.now(_dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        + "-"
        + uuid.uuid4().hex[:8]
    )

    # Tag Khala's own eval load as internal + segmented (#6298), preemptible (#6318).
    default_headers = {
        "x-openagents-demand-kind": "internal",
        "x-openagents-demand-source": args.demand_source,
        "x-openagents-client": args.demand_client,
    }

    # gated_submit=None so `submit` is always available (keeps the bounded smoke
    # short); the rest of the task config matches the paper defaults.
    task = mirrorcode(gated_submit=None)

    print("=" * 64)
    print(" MirrorCode x Khala Phase-0 smoke")
    print("=" * 64)
    print(f" runId        : {run_id}")
    print(f" model        : {args.model}")
    print(f" task         : {args.task} (bucket {bucket})")
    print(f" token_limit  : {args.token_limit:,}")
    print(f" time_limit   : {args.time_limit}s")
    print(f" message_limit: {args.message_limit}")
    print(f" base_url     : {os.environ['OPENAI_BASE_URL']}")
    print(f" demand tags  : internal / {args.demand_source} / {args.demand_client}")
    print("-" * 64)

    status = "error"
    summary = ""
    pass_rate: float | None = None
    tokens = _aggregate_tokens(None)
    started_at = _iso(_dt.datetime.now(_dt.timezone.utc))
    finished_at = started_at
    score_groups: dict[str, Any] = {}

    try:
        logs = inspect_ai.eval(
            task,
            model=args.model,
            model_args={"default_headers": default_headers},
            sample_id=[args.task],
            token_limit=args.token_limit,
            time_limit=args.time_limit,
            message_limit=args.message_limit,
            log_dir=args.log_dir,
            display="plain",
            fail_on_error=False,
        )
        log = logs[0]
        stats = getattr(log, "stats", None)
        if stats is not None:
            tokens = _aggregate_tokens(getattr(stats, "model_usage", None))
            started_at = _iso(getattr(stats, "started_at", None)) or started_at
            finished_at = _iso(getattr(stats, "completed_at", None)) or finished_at

        log_status = getattr(log, "status", "unknown")
        samples = getattr(log, "samples", None) or []
        if log_status == "error" and not samples:
            err = getattr(log, "error", None)
            status = "error"
            summary = f"eval errored: {getattr(err, 'message', err)}"
        elif samples:
            sample = samples[0]
            sv = _score_value(sample)
            score_groups = sv["groups"]
            # Headline pass rate: the held-out (hidden+ablated) reproduction rate
            # if present, else "all", else "visible". This is the anti-contamination
            # number that matters for the gym rung.
            for key in ("withheld", "all", "visible"):
                if key in score_groups and score_groups[key] is not None:
                    try:
                        candidate = float(score_groups[key])
                    except (TypeError, ValueError):
                        continue
                    if candidate == candidate:  # not NaN
                        pass_rate = candidate
                        break
            sample_err = getattr(sample, "error", None)
            if sample_err is not None:
                status = "error"
                summary = f"sample errored: {getattr(sample_err, 'message', sample_err)}"
            elif pass_rate is not None and pass_rate >= 0.999:
                status = "passed"
                summary = "Khala completed the MirrorCode agent loop and solved the target."
            elif pass_rate is not None:
                status = "failed"
                summary = (
                    "Khala completed the MirrorCode agent loop; implementation did not "
                    "fully reproduce the target behavior."
                )
            else:
                status = "failed"
                summary = "Khala ran the agent loop; no usable pass rate was produced."
        else:
            status = "error"
            summary = f"no samples produced (log status={log_status})."
    except Exception as exc:  # noqa: BLE001 - smoke must always emit a result
        status = "error"
        summary = f"runner exception: {type(exc).__name__}: {exc}"
        import traceback

        traceback.print_exc()

    result = {
        "runId": run_id,
        "model": "openagents/khala",
        "taskId": args.task,
        "bucket": bucket,
        "status": status,
        "passRate": pass_rate,
        "tokens": tokens,
        "startedAt": started_at,
        "finishedAt": finished_at,
        "summary": summary,
        # Public-safe extras (not part of the minimal contract, but useful for the
        # service lane and honest reporting). Public-task only.
        "benchmark": "Epoch Research MirrorCode (public tasks only; private set excluded)",
        "scoreGroups": score_groups,
        "caps": {
            "tokenLimit": args.token_limit,
            "timeLimitSeconds": args.time_limit,
            "messageLimit": args.message_limit,
        },
        "demand": {
            "kind": "internal",
            "source": args.demand_source,
            "client": args.demand_client,
        },
    }

    with open(args.out, "w") as f:
        json.dump(result, f, indent=2)
        f.write("\n")

    print("-" * 64)
    print(json.dumps(result, indent=2))
    print("-" * 64)
    print(f"Result JSON written to: {args.out}")
    return 0 if status in ("passed", "failed") else 1


if __name__ == "__main__":
    raise SystemExit(main())
