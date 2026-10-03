#!/usr/bin/env python3
"""Measure fresh-process briefing preparation without executing proposed checks."""
import argparse
import json
import math
import pathlib
import statistics
import subprocess
import time


def invoke(binary, arguments):
    started = time.perf_counter()
    result = subprocess.run([str(binary), *map(str, arguments)], check=True, capture_output=True, text=True)
    return (time.perf_counter() - started) * 1000, result.stdout


def summary(values):
    return {
        "samples": len(values),
        "p50_ms": statistics.median(values),
        "p95_ms": sorted(values)[math.ceil(0.95 * len(values)) - 1],
        "max_ms": max(values),
        "at_or_over_one_second": sum(value >= 1000 for value in values),
        "elapsed_ms": values,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=pathlib.Path, required=True)
    parser.add_argument("--repo", type=pathlib.Path, required=True)
    parser.add_argument("--rev", required=True)
    parser.add_argument("--issue-file", type=pathlib.Path, required=True)
    parser.add_argument("--output-dir", type=pathlib.Path, required=True)
    parser.add_argument("--repeats", type=int, default=20)
    args = parser.parse_args()
    if args.repeats < 2:
        parser.error("Use at least two repeats.")
    out = args.output_dir.resolve()
    repo = args.repo.resolve()
    if out == repo or repo in out.parents or out.exists():
        parser.error("Choose a new output directory outside the repository.")
    out.mkdir(parents=True)
    issue = out / "issue.json"
    issue.write_bytes(args.issue_file.read_bytes())
    index = out / "index.json"
    base = ["--repo", repo, "--rev", args.rev]
    index_ms, _ = invoke(args.binary, ["index", *base, "--output", index])
    execution = ["--manifest", "crates/openagents-mobile/Cargo.toml", "--environment-id", "briefing-experiment"]
    prepare_args = ["prepare-run", *base, "--issue-file", issue, *execution, "--artifact-root", out / "runs"]
    _, stdout = invoke(args.binary, prepare_args)
    prior = json.loads(stdout)
    # This is explicitly synthetic evidence; no Cargo command is run.
    invoke(args.binary, ["record-result", "--run-dir", prior["run_dir"], "--request-sha256", prior["request_sha256"], "--exit-code", "101", "--summary", "Synthetic workspace-command failure for the preview experiment.", "--next-action", "Inspect the proposed manifest-path command before a separately authorized check."])
    variants = {
        "source_only": [],
        "execution": ["--execution", *execution],
        "execution_with_attempt": ["--execution", *execution, "--attempt-dir", prior["run_dir"]],
    }
    elapsed = {name: [] for name in variants}
    preparation = []
    run_ids = set()
    for repeat in range(args.repeats):
        names = list(variants)
        offset = repeat % len(names)
        for name in names[offset:] + names[:offset]:
            ms, _ = invoke(args.binary, ["preview", *base, "--index", index, "--issue-file", issue, "--output-dir", out / name, *variants[name]])
            elapsed[name].append(ms)
        ms, stdout = invoke(args.binary, prepare_args)
        preparation.append(ms)
        request = json.loads(stdout)
        assert request["run_id"] not in run_ids
        run_ids.add(request["run_id"])
    briefs = {name: json.loads((out / name / "briefing.json").read_text()) for name in variants}
    for name in ("execution", "execution_with_attempt"):
        for field in ("issue", "commit", "components", "evidence", "history", "notes", "index_omissions"):
            assert briefs[name][field] == briefs["source_only"][field], (name, field)
    augmented = briefs["execution_with_attempt"]
    record = augmented["execution"]["attempts"][0]
    assert record["status"] == "same_inputs"
    assert record["result"]["exit_code"] == 101
    assert record["result"]["status"] == "caller_reported"
    plan = augmented["execution"]["manifest"]["packages"][0]
    assert plan["test_argv"] == ["cargo", "test", "--manifest-path", "./crates/openagents-mobile/Cargo.toml"]
    assert plan["workspace"]["kind"] == "package_workspace_root"
    data = {
        "schema": "openagents.briefing-lab.execution-measurement.v1",
        "source_commit": augmented["commit"],
        "implementation_sources_sha256": {str(path.relative_to(repo)): __import__("hashlib").sha256(path.read_bytes()).hexdigest() for path in sorted((repo / "crates/briefing-lab/src").glob("*.rs"))},
        "binary_sha256": __import__("hashlib").sha256(args.binary.read_bytes()).hexdigest(),
        "issue_sha256": __import__("hashlib").sha256(issue.read_bytes()).hexdigest(),
        "method": "Fresh CLI process per sample; rotated preview arms; external wall time includes loading, Git checks, environment inspection, attempt validation, rendering, and writes. Indexing, build, and network acquisition are separate. prepare-run allocation is measured separately. Summed component times are arithmetic totals, not an observed sequential workflow. The prior attempt is synthetic caller-reported evidence; no proposed command executes.",
        "index_ms_separate": index_ms,
        "variants": {name: summary(values) for name, values in elapsed.items()},
        "prepare_run": summary(preparation),
        "sum_of_same_iteration_component_times": summary([a+b for a,b in zip(preparation, elapsed["execution_with_attempt"])]),
        "distinct_prepared_runs": len(run_ids),
        "example": {"manifest": plan, "attempt_status": record["status"], "reported_exit_code": record["result"]["exit_code"]},
        "internal_timings_ms": {name: brief["timings_ms"] for name, brief in briefs.items()},
    }
    (out / "measurements.json").write_text(json.dumps(data, indent=2) + "\n")
    (out / "execution-example.json").write_text(json.dumps(augmented["execution"], indent=2) + "\n")
    print(json.dumps({"index_ms": index_ms, "variants": {name: {k: v for k, v in value.items() if k != "elapsed_ms"} for name, value in data["variants"].items()}, "prepare_run": {k: v for k, v in data["prepare_run"].items() if k != "elapsed_ms"}}, indent=2))


if __name__ == "__main__":
    main()
