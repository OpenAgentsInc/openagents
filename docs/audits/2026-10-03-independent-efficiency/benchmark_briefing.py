#!/usr/bin/env python3
"""Measure fresh-process previews; keep cold indexing and network outside warm rows."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import shutil
import subprocess
import time


def digest(data):
    return hashlib.sha256(data).hexdigest()


def run(command, env=None):
    start = time.perf_counter_ns()
    result = subprocess.run(command, check=True, capture_output=True, text=True, env=env)
    return (time.perf_counter_ns() - start) / 1_000_000, result.stdout


def stats(values):
    ordered = sorted(values)
    return {
        "count": len(values),
        "p50_ms": ordered[math.ceil(len(values) * 0.50) - 1],
        "p95_ms": ordered[math.ceil(len(values) * 0.95) - 1],
        "max_ms": max(values),
        "over_one_second": sum(value >= 1000 for value in values),
    }


def verify(brief, issue, repo, revision):
    assert brief["issue"]["title"] == issue["title"]
    assert brief["issue"]["body"] == (issue.get("body") or "")
    assert brief["commit"] == revision
    for item in brief["evidence"]:
        raw = subprocess.check_output(["git", "-C", str(repo), "show", revision + ":" + item["path"]])
        assert digest(raw) == item["file_sha256"], item["path"]
        selected = b"".join(raw.splitlines(keepends=True)[item["start_line"] - 1:item["end_line"]])
        assert selected == item["text"].encode(), item["path"]
        assert digest(selected) == item["excerpt_sha256"], item["path"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--rev", required=True)
    parser.add_argument("--issues", required=True, type=Path, nargs="+")
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--repeats", type=int, default=20)
    args = parser.parse_args()
    if args.repeats < 1:
        parser.error("--repeats must be positive")
    repo = args.repo.resolve()
    output = args.output.resolve()
    if output == repo or repo in output.parents:
        parser.error("--output must be outside the repository")
    output.mkdir(parents=True, exist_ok=True)
    binary = str(args.binary.resolve())
    revision = subprocess.check_output(["git", "-C", str(repo), "rev-parse", args.rev + "^{commit}"], text=True).strip()
    index_path = output / "index.json"
    index_ms, _ = run([binary, "index", "--repo", str(repo), "--rev", revision, "--output", str(index_path)])
    index = json.loads(index_path.read_text())
    source_paths = ["crates/briefing-lab/Cargo.toml", "crates/briefing-lab/src/lib.rs", "crates/briefing-lab/src/main.rs", "scripts/briefing-preview.sh"]
    report = {
        "schema": "openagents.briefing-lab.benchmark.v1",
        "recorded_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "input_commit": revision,
        "binary_sha256": digest(Path(binary).read_bytes()),
        "source_sha256": {path: digest((repo / path).read_bytes()) for path in source_paths},
        "host": {"platform": platform.platform(), "logical_cpus": os.cpu_count(), "cpu_model": next((line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')), 'unknown') if Path('/proc/cpuinfo').exists() else platform.processor()},
        "method": "Fresh CLI process, existing index and cached issue, through output-file completion. OS cache is not flushed. First-use index measured separately. Nearest-rank quantiles. Provenance verification occurs outside the timed interval.",
        "index": {"elapsed_ms": index_ms, "internal_ms": index["index_ms"], "bytes": index_path.stat().st_size, "sha256": digest(index_path.read_bytes()), "indexed_files": len(index["files"]), "known_paths": len(index["known_paths"]), "omissions": index["omissions"]},
        "cases": [],
    }
    variants = {"default": [], "no_symbols": ["--no-symbols"], "no_history": ["--no-history"], "explicit_only": ["--no-symbols", "--no-history", "--no-lexical"]}
    for issue_path in args.issues:
        raw = issue_path.read_bytes()
        issue = json.loads(raw)
        case_dir = output / str(issue.get("number", digest(raw)[:12]))
        case_dir.mkdir(exist_ok=True)
        shutil.copyfile(issue_path, case_dir / "issue.json")
        case = {"number": issue.get("number"), "url": issue.get("url"), "title": issue["title"], "issue_sha256": digest(raw), "issue_updated_at": issue.get("updatedAt"), "variants": {}}
        for variant, flags in variants.items():
            destination = case_dir / variant
            command = [binary, "preview", "--repo", str(repo), "--rev", revision, "--index", str(index_path), "--issue-file", str(issue_path), "--output-dir", str(destination)] + flags
            elapsed = []
            repeat_count = args.repeats if variant == "default" else 1
            for _ in range(repeat_count):
                ms, _ = run(command)
                elapsed.append(ms)
            brief = json.loads((destination / "briefing.json").read_text())
            verify(brief, issue, repo, revision)
            case["variants"][variant] = {**stats(elapsed), "elapsed_ms": elapsed, "last_internal_timings_ms": brief["timings_ms"], "evidence_paths": [item["path"] for item in brief["evidence"]], "candidate_files": brief["candidate_files"], "history_count": len(brief["history"]), "markdown_bytes": (destination / "briefing.md").stat().st_size}
        wrapper_env = os.environ.copy()
        wrapper_env["BRIEFING_LAB_BIN"] = binary
        wrapper_ms, _ = run(["bash", str(repo / "scripts/briefing-preview.sh"), "--repo", str(repo), "--rev", revision, "--index", str(index_path), "--issue-file", str(issue_path), "--output-dir", str(case_dir / "wrapper")], wrapper_env)
        case["cached_wrapper_elapsed_ms"] = wrapper_ms
        report["cases"].append(case)
    (output / "measurements.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"index": report["index"], "cases": [{"number": case["number"], "default": {key: value for key, value in case["variants"]["default"].items() if key in ['count', 'p50_ms', 'p95_ms', 'max_ms', 'over_one_second']}, "wrapper_ms": case["cached_wrapper_elapsed_ms"]} for case in report["cases"]]}, indent=2))


if __name__ == "__main__":
    main()
