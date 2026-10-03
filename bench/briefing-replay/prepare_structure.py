#!/usr/bin/env python3
"""Prepare a pinned ExplicitStructureV1 payload without model calls or Git probes."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time

BUDGET = 16 * 1024
SCHEMA = "openagents.briefing-lab.explicit-structure.v1"


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha_argument(value: str) -> str:
    if not re.fullmatch(r"[0-9a-f]{64}", value):
        raise argparse.ArgumentTypeError("Expected a lowercase SHA-256 digest.")
    return value


def command(argv: list[str]) -> tuple[subprocess.CompletedProcess[str], float]:
    started = time.perf_counter()
    result = subprocess.run(argv, capture_output=True, text=True, check=False)
    elapsed_ms = (time.perf_counter() - started) * 1000
    if result.returncode:
        raise RuntimeError(
            f"{Path(argv[0]).name} {argv[1]} exited {result.returncode}: "
            f"{result.stderr.strip()[:1000]}"
        )
    return result, elapsed_ms


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--binary-sha256", type=sha_argument, required=True)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--rev", required=True, help="Full pinned Git commit.")
    parser.add_argument("--issue-file", type=Path, required=True)
    parser.add_argument("--issue-sha256", type=sha_argument)
    parser.add_argument("--index", type=Path, required=True, help="Existing index for the warm preview.")
    parser.add_argument("--index-sha256", type=sha_argument)
    parser.add_argument("--output-dir", type=Path, required=True, help="New directory outside the inspected repository.")
    parser.add_argument("--prior-focused", type=Path, help="Optional frozen V2 focused.md for an exact old-policy comparison.")
    return parser.parse_args()


def prepare(args: argparse.Namespace) -> dict:
    binary = args.binary.resolve(strict=True)
    repo = args.repo.resolve(strict=True)
    output = args.output_dir.resolve()
    if not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", args.rev):
        raise ValueError("--rev must be a full lowercase Git commit, not a moving ref.")
    binary_bytes = binary.read_bytes()
    if digest(binary_bytes) != args.binary_sha256:
        raise ValueError("Binary SHA-256 differs from the requested pin.")
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError("The pinned binary must be an executable regular file.")
    top, _ = command(["git", "-C", str(repo), "rev-parse", "--show-toplevel"])
    top = Path(top.stdout.strip()).resolve(strict=True)
    if output == top or top in output.parents:
        raise ValueError("Write preparation artifacts outside the inspected repository.")
    resolved, _ = command(["git", "-C", str(repo), "rev-parse", "--verify", f"{args.rev}^{{commit}}"])
    if resolved.stdout.strip() != args.rev:
        raise ValueError("Resolved Git revision differs from the requested pin.")
    issue_bytes = args.issue_file.read_bytes()
    index_bytes = args.index.read_bytes()
    issue = json.loads(issue_bytes)
    index = json.loads(index_bytes)
    if not isinstance(issue, dict) or not isinstance(issue.get("title"), str):
        raise ValueError("Issue JSON must contain a string title.")
    if index.get("commit") != args.rev:
        raise ValueError("Warm index is not pinned to the requested commit.")
    for name, data, expected in [
        ("issue", issue_bytes, args.issue_sha256),
        ("index", index_bytes, args.index_sha256),
    ]:
        if expected and digest(data) != expected:
            raise ValueError(f"{name.title()} SHA-256 differs from its requested pin.")
    prior = args.prior_focused.read_bytes() if args.prior_focused else None
    output.mkdir(parents=True, exist_ok=False)
    (output / "issue.json").write_bytes(issue_bytes)
    (output / "index.json").write_bytes(index_bytes)
    record = {
        "schema": "openagents.briefing-replay.structure-preparation.v1",
        "policy": "ExplicitStructureV1",
        "status": "preparing",
        "source_commit": args.rev,
        "model_calls": 0,
        "external_git_probes": 0,
        "byte_budget": BUDGET,
        "preview_flags": ["--explicit-structure"],
        "hashes": {
            "binary_sha256": digest(binary_bytes),
            "issue_sha256": digest(issue_bytes),
            "warm_index_sha256": digest(index_bytes),
            "preparation_script_sha256": digest(Path(__file__).read_bytes()),
        },
        "timings_ms": {},
        "notes": [
            "Default lexical and symbol ranking is enabled inside the explicit-file and nearby-test pool. It does not admit unrelated lexical file matches.",
            "The fresh index build is measured separately. Operating-system page caches are not cleared; cold means a new index artifact, not cold storage.",
            "Warm preview uses the supplied immutable index copy after the fresh index build. Subprocess wall time includes process startup and output writes.",
            "treatment.md is exactly focused.md. There is no external probe, additional source appendix, model call, or task execution.",
            "Supply the complete issue and mandatory instructions separately and identically to both arms.",
        ],
    }
    try:
        _, cold_ms = command([
            str(binary), "index", "--repo", str(repo), "--rev", args.rev,
            "--output", str(output / "cold-index.json"),
        ])
        cold_bytes = (output / "cold-index.json").read_bytes()
        cold = json.loads(cold_bytes)
        if cold.get("commit") != args.rev:
            raise ValueError("Fresh index has an unexpected source commit.")
        record["hashes"]["cold_index_sha256"] = digest(cold_bytes)
        record["timings_ms"]["cold_index_build_wall"] = cold_ms
        record["timings_ms"]["cold_index_reported"] = cold.get("index_ms")
        common = [
            str(binary), "preview", "--repo", str(repo), "--rev", args.rev,
            "--issue-file", str(output / "issue.json"), "--index", str(output / "index.json"),
        ]
        result, warm_ms = command(common + ["--output-dir", str(output / "preview"), "--explicit-structure"])
        brief_bytes = (output / "preview" / "briefing.json").read_bytes()
        brief = json.loads(brief_bytes)
        payload = (output / "preview" / "focused.md").read_bytes()
        pack = brief["focused"]
        if pack.get("schema") != SCHEMA or brief.get("commit") != args.rev:
            raise ValueError("Preview policy or source commit differs from its requested pin.")
        if pack.get("byte_budget") != BUDGET or pack.get("packed_bytes") != len(payload):
            raise ValueError("Recorded payload size or byte budget is inconsistent.")
        if pack.get("markdown", "").encode("utf-8") != payload:
            raise ValueError("Structured and rendered payloads differ.")
        if len(payload) > BUDGET:
            raise ValueError(f"Actual treatment has {len(payload)} bytes; maximum is {BUDGET}.")
        record["timings_ms"]["warm_preview_wall"] = warm_ms
        record["timings_ms"]["preview_stages"] = brief.get("timings_ms", {})
        reported = re.search(r"Complete local preview including output: ([0-9.]+) ms", result.stdout)
        record["timings_ms"]["warm_preview_reported_including_output"] = float(reported[1]) if reported else None
        record["hashes"]["structured_preview_sha256"] = digest(brief_bytes)
        record["hashes"]["focused_sha256"] = digest(payload)
        record["payload_bytes"] = len(payload)
        record["source_bytes"] = pack.get("source_bytes")
        record["selected_ranges"] = [
            {key: row[key] for key in ("path", "start_line", "end_line")}
            for row in brief.get("evidence", [])
        ]
        record["selections"] = pack.get("selections", [])
        record["coverage"] = {
            "omissions": pack.get("omissions", []),
            "omitted_coverage_records": pack.get("omitted_coverage_records", 0),
        }
        if prior is not None:
            _, compare_ms = command(common + [
                "--output-dir", str(output / "v2-comparison"),
                "--focused", "--no-lexical", "--no-symbols",
            ])
            compared = (output / "v2-comparison" / "focused.md").read_bytes()
            record["v2_comparison"] = {
                "flags": ["--focused", "--no-lexical", "--no-symbols"],
                "prior_focused_sha256": digest(prior),
                "new_binary_focused_sha256": digest(compared),
                "identical": compared == prior,
            }
            record["timings_ms"]["v2_comparison_wall"] = compare_ms
            if compared != prior:
                raise ValueError("Frozen V2 focused payload changed under the new binary.")
        if digest(binary.read_bytes()) != args.binary_sha256:
            raise ValueError("Pinned binary changed during preparation.")
        if (output / "issue.json").read_bytes() != issue_bytes or (output / "index.json").read_bytes() != index_bytes:
            raise ValueError("A copied preparation input changed during the run.")
        (output / "treatment.md").write_bytes(payload)
        record["hashes"]["treatment_sha256"] = digest(payload)
        record["status"] = "complete"
    except Exception as error:
        record["status"] = "failed"
        record["error"] = str(error)
        raise
    finally:
        (output / "preparation.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    return record


def main() -> None:
    try:
        result = prepare(parse_args())
    except (OSError, ValueError, RuntimeError, KeyError) as error:
        raise SystemExit(f"prepare_structure: {error}") from error
    print(json.dumps({
        "status": result["status"],
        "payload_bytes": result["payload_bytes"],
        "treatment_sha256": result["hashes"]["treatment_sha256"],
        "timings_ms": result["timings_ms"],
        "v2_comparison": result.get("v2_comparison"),
    }, indent=2))


if __name__ == "__main__":
    main()
