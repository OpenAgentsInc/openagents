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

    { runId, model, taskId, bucket, language, status, passRate, tokens,
      startedAt, finishedAt, summary, grade, decisionGrade }

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
# benchmark proper; keep them behind an explicit opt-in so the default Phase-0
# path remains a real public S-target as required by #6377.
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
LANGUAGES = {"python", "c", "go", "rust", "ocaml", "ada"}


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


def split_sample_id(sample_id: str) -> tuple[str, str | None]:
    """Split '<target>_<language>' without breaking targets like qsv_select."""
    for suffix in sorted(LANGUAGES, key=len, reverse=True):
        language_suffix = "_" + suffix
        if sample_id.endswith(language_suffix):
            return sample_id[: -len(language_suffix)], suffix
    return sample_id, None


def validate_sample_id(sample_id: str, allow_trivial_smoke: bool) -> tuple[str, str | None, str]:
    target, language = split_sample_id(sample_id)
    bucket = bucket_for_target(target)
    if language is None:
        raise ValueError(
            f"task '{sample_id}' must use '<target>_<language>' with language in {sorted(LANGUAGES)}"
        )
    if bucket == "unknown":
        raise ValueError(f"task '{sample_id}' is not a known MirrorCode target")
    if bucket == "L":
        raise ValueError("L-bucket targets are off-limits for this bounded Phase-0 runner")
    if bucket == "trivial_excluded" and not allow_trivial_smoke:
        raise ValueError(
            "trivial excluded targets require --allow-trivial-smoke; use an S-target "
            "such as cal_python for issue #6377"
        )
    return target, language, bucket


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


def _load_fixture_result(path: str) -> dict[str, Any]:
    with open(path) as f:
        payload = json.load(f)
    if not isinstance(payload, dict):
        raise ValueError("fixture result must be a JSON object")
    return payload


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description="MirrorCode x Khala Phase-0 smoke runner")
    p.add_argument(
        "--task",
        default=os.environ.get("MC_TASK_ID", "cal_python"),
        help="Inspect sample id '<target>_<language>' (default: cal_python).",
    )
    p.add_argument(
        "--model",
        default=os.environ.get("MC_MODEL", "openai/openagents/khala"),
        help="Inspect model id (default: openai/openagents/khala).",
    )
    p.add_argument(
        "--token-limit",
        type=int,
        default=int(os.environ.get("MC_TOKEN_LIMIT", "20000000")),
        help="Hard per-sample token cap (default 20000000; far below paper 1B).",
    )
    p.add_argument(
        "--time-limit",
        type=int,
        default=int(os.environ.get("MC_TIME_LIMIT", "7200")),
        help="Hard per-sample wall-clock cap in seconds (default 7200).",
    )
    p.add_argument(
        "--message-limit",
        type=int,
        default=int(os.environ.get("MC_MESSAGE_LIMIT", "250")),
        help="Hard per-sample message cap (default 250).",
    )
    p.add_argument(
        "--out",
        default=os.environ.get("MC_OUT", "mirrorcode-phase0-result.json"),
        help="Path to write the public-safe result JSON.",
    )
    p.add_argument(
        "--allow-trivial-smoke",
        action="store_true",
        default=os.environ.get("MC_ALLOW_TRIVIAL_SMOKE", "0") == "1",
        help="Allow benchmark-excluded trivial targets such as false_c.",
    )
    p.add_argument(
        "--fixture-result",
        default=os.environ.get("MC_FIXTURE_RESULT", None),
        help="Use an existing public-safe result JSON instead of running Inspect.",
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

    if not args.fixture_result and not os.environ.get("OPENAI_API_KEY"):
        print("ERROR: OPENAI_API_KEY (a Khala key) is not set.", file=sys.stderr)
        return 2
    os.environ.setdefault("OPENAI_BASE_URL", "https://openagents.com/api/v1")

    try:
        target, language, bucket = validate_sample_id(
            args.task,
            allow_trivial_smoke=args.allow_trivial_smoke,
        )
    except ValueError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2

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

    print("=" * 64)
    print(" MirrorCode x Khala Phase-0 smoke")
    print("=" * 64)
    print(f" runId        : {run_id}")
    print(f" model        : {args.model}")
    print(f" task         : {args.task} (target {target}, language {language}, bucket {bucket})")
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
    caps = {
        "tokenLimit": args.token_limit,
        "timeLimitSeconds": args.time_limit,
        "messageLimit": args.message_limit,
    }

    try:
        if args.fixture_result:
            fixture = _load_fixture_result(args.fixture_result)
            status = str(fixture.get("status", "error"))
            pass_rate = fixture.get("passRate")
            tokens = fixture.get("tokens") or tokens
            started_at = _iso(fixture.get("startedAt")) or started_at
            finished_at = _iso(fixture.get("finishedAt")) or finished_at
            summary = str(fixture.get("summary", "Loaded public-safe fixture result."))
            score_groups = fixture.get("scoreGroups") or {}
            run_id = str(fixture.get("runId") or run_id)
            caps = fixture.get("caps") or caps
        else:
            import inspect_ai
            from mc.task import mirrorcode

            # gated_submit=None so `submit` is always available (keeps the bounded
            # smoke short); the rest of the task config matches the paper defaults.
            task = mirrorcode(gated_submit=None)
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
        "language": language,
        "status": status,
        "passRate": pass_rate,
        "tokens": tokens,
        "startedAt": started_at,
        "finishedAt": finished_at,
        "summary": summary,
        "grade": "smoke",
        "decisionGrade": False,
        # Public-safe extras (not part of the minimal contract, but useful for the
        # service lane and honest reporting). Public-task only.
        "benchmark": "Epoch Research MirrorCode (public tasks only; private set excluded)",
        "scoreGroups": score_groups,
        "caps": caps,
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
