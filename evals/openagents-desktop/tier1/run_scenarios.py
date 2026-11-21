#!/usr/bin/env python3
# Firm reason: 5 (deterministic scenario driver for Tier 1 Coder Desktop evals)
"""Tier 1 deterministic host & mock-worker scenario runner.

Drives evals/openagents-desktop/host_driver/build/host_driver against
evals/openagents-desktop/mock_worker/mock_worker.py using declarative JSON scenarios
in evals/openagents-desktop/tier1/scenarios/*.json.
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[3]
HOST_DRIVER_DIR = REPO_ROOT / "evals/openagents-desktop/host_driver"
HOST_DRIVER_BIN = HOST_DRIVER_DIR / "build/host_driver"
SCENARIOS_DIR = REPO_ROOT / "evals/openagents-desktop/tier1/scenarios"
RESOURCES_DIR = REPO_ROOT / "components"


def ensure_host_driver() -> None:
    res = subprocess.run(
        ["bash", str(HOST_DRIVER_DIR / "build.sh")],
        cwd=str(REPO_ROOT),
        capture_output=True,
        text=True,
        check=False,
    )
    if res.returncode != 0:
        sys.stderr.write(res.stdout)
        sys.stderr.write(res.stderr)
        raise RuntimeError(f"host_driver build.sh failed with exit {res.returncode}")


def expand_placeholders(
    cmd: str,
    script_paths: dict[str, Path],
    record_paths: dict[str, Path],
    tmp_dir: Path,
) -> str:
    def repl(match: re.Match[str]) -> str:
        kind, key = match.group(1), match.group(2)
        if kind == "script":
            if key not in script_paths:
                raise KeyError(f"unknown worker script: {key}")
            return str(script_paths[key])
        if kind == "record":
            if key not in record_paths:
                raise KeyError(f"unknown worker record: {key}")
            return str(record_paths[key])
        if kind == "tmp":
            return str(tmp_dir / key)
        raise KeyError(f"unknown placeholder kind: {kind}")

    out = cmd.replace("{resources}", str(RESOURCES_DIR))
    out = re.sub(r"\{(script|record|tmp):([^}]+)\}", repl, out)
    return out


def check_expect(resp: dict[str, Any], expect: dict[str, Any], step_idx: int, cmd: str) -> list[str]:
    errs: list[str] = []
    for k, v in expect.items():
        if k.endswith("_gte"):
            field = k[:-4]
            actual = resp.get(field)
            if not isinstance(actual, (int, float)) or actual < v:
                errs.append(f"step {step_idx} ({cmd}): {field}={actual!r} < {v!r}")
        elif k.endswith("_lte"):
            field = k[:-4]
            actual = resp.get(field)
            if not isinstance(actual, (int, float)) or actual > v:
                errs.append(f"step {step_idx} ({cmd}): {field}={actual!r} > {v!r}")
        elif k.endswith("_gt"):
            field = k[:-3]
            actual = resp.get(field)
            if not isinstance(actual, (int, float)) or actual <= v:
                errs.append(f"step {step_idx} ({cmd}): {field}={actual!r} <= {v!r}")
        elif k.endswith("_lt"):
            field = k[:-3]
            actual = resp.get(field)
            if not isinstance(actual, (int, float)) or actual >= v:
                errs.append(f"step {step_idx} ({cmd}): {field}={actual!r} >= {v!r}")
        elif k.endswith("_contains"):
            field = k[:-9]
            actual = resp.get(field)
            if isinstance(actual, list):
                if v not in actual:
                    errs.append(f"step {step_idx} ({cmd}): {field}={actual!r} does not contain {v!r}")
            elif isinstance(actual, str):
                if str(v) not in actual:
                    errs.append(f"step {step_idx} ({cmd}): {field}={actual!r} does not contain {v!r}")
            else:
                errs.append(f"step {step_idx} ({cmd}): {field}={actual!r} cannot contain {v!r}")
        else:
            actual = resp.get(k)
            if actual != v:
                errs.append(f"step {step_idx} ({cmd}): {k}={actual!r}, expected {v!r}")
    return errs


def check_records(
    assertions: list[dict[str, Any]] | dict[str, dict[str, Any]],
    record_paths: dict[str, Path],
) -> list[str]:
    errs: list[str] = []
    rules: list[dict[str, Any]] = []
    if isinstance(assertions, dict):
        for wname, rule in assertions.items():
            r = dict(rule)
            r["worker"] = wname
            rules.append(r)
    else:
        rules = list(assertions)
    for idx, rule in enumerate(rules):
        wname = rule["worker"]
        rpath = record_paths.get(wname)
        if rpath is None:
            errs.append(f"record assertion {idx}: unknown worker {wname!r}")
            continue
        entries: list[dict[str, Any]] = []
        if rpath.exists():
            for raw in rpath.read_text(encoding="utf-8").splitlines():
                if raw.strip():
                    entries.append(json.loads(raw))
        kinds = [e.get("kind") for e in entries]
        all_raw = "\n".join(str(e.get("raw", "")) for e in entries)

        if "min_count" in rule and len(entries) < int(rule["min_count"]):
            errs.append(f"record[{wname}]: count={len(entries)} < {rule['min_count']}")
        if "exact_count" in rule and len(entries) != int(rule["exact_count"]):
            errs.append(f"record[{wname}]: count={len(entries)} != {rule['exact_count']}")
        for k in rule.get("kinds_include", []):
            if k not in kinds:
                errs.append(f"record[{wname}]: missing kind {k!r} in {kinds}")
        for k in rule.get("kinds_exclude", []):
            if k in kinds:
                errs.append(f"record[{wname}]: unexpected kind {k!r} in {kinds}")
        kind_counts = rule.get("kind_counts") or rule.get("count_by_kind") or {}
        for k, expected_n in kind_counts.items():
            actual_n = sum(1 for x in kinds if x == k)
            if actual_n != expected_n:
                errs.append(f"record[{wname}]: kind {k!r} count={actual_n}, expected {expected_n}")
        for k, min_n in rule.get("kind_min_counts", {}).items():
            actual_n = sum(1 for x in kinds if x == k)
            if actual_n < min_n:
                errs.append(f"record[{wname}]: kind {k!r} count={actual_n} < {min_n}")
        for sub in rule.get("must_contain", []) + rule.get("contains", []):
            if sub not in all_raw:
                errs.append(f"record[{wname}]: missing substring {sub!r}")
        for sub in rule.get("must_not_contain", []) + rule.get("not_contains", []):
            if sub in all_raw:
                errs.append(f"record[{wname}]: unexpected substring {sub!r}")
        if "results_count" in rule:
            res_entries = [e for e in entries if e.get("kind") == "results" and isinstance(e.get("json"), dict)]
            if not res_entries:
                errs.append(f"record[{wname}]: no results entry found")
            else:
                rlist = res_entries[-1]["json"].get("results", [])
                if len(rlist) != int(rule["results_count"]):
                    errs.append(f"record[{wname}]: results len={len(rlist)}, expected {rule['results_count']}")
    return errs


def check_files(assertions: list[dict[str, Any]], tmp_dir: Path) -> list[str]:
    errs: list[str] = []
    for idx, rule in enumerate(assertions):
        fpath = tmp_dir / rule["file"]
        if not fpath.exists():
            errs.append(f"file assertion {idx}: missing {fpath.name}")
            continue
        text = fpath.read_text(encoding="utf-8", errors="replace")
        if "min_bytes" in rule and len(text.encode("utf-8")) < int(rule["min_bytes"]):
            errs.append(f"file[{fpath.name}]: too small ({len(text)} bytes)")
        for sub in rule.get("must_contain", []):
            if sub not in text:
                errs.append(f"file[{fpath.name}]: missing substring {sub!r}")
    return errs


def run_scenario(path: Path) -> dict[str, Any]:
    spec = json.loads(path.read_text(encoding="utf-8"))
    sid = spec.get("id") or spec.get("name") or path.stem
    desc = spec.get("description", sid)
    t0 = time.monotonic()
    errors: list[str] = []
    max_vertices_seen = 0
    max_frame_ms = 0.0
    steps_executed = 0
    steps = spec.get("steps") or spec.get("commands") or []
    if not steps:
        errors.append("scenario has no steps/commands")

    with tempfile.TemporaryDirectory(prefix=f"coder_t1_{sid}_") as td:
        tmp_dir = Path(td)
        script_paths: dict[str, Path] = {}
        record_paths: dict[str, Path] = {}
        for wname, wscript in spec.get("workers", {}).items():
            sp = tmp_dir / f"worker_{wname}.json"
            rp = tmp_dir / f"record_{wname}.jsonl"
            sp.write_text(json.dumps(wscript), encoding="utf-8")
            script_paths[wname] = sp
            record_paths[wname] = rp

        proc = subprocess.Popen(
            [str(HOST_DRIVER_BIN)],
            cwd=str(REPO_ROOT),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        assert proc.stdin is not None and proc.stdout is not None and proc.stderr is not None

        sent_quit = False
        try:
            for idx, step in enumerate(steps):
                cmd = expand_placeholders(step["cmd"], script_paths, record_paths, tmp_dir)
                if cmd.strip().upper() == "QUIT":
                    sent_quit = True
                proc.stdin.write(cmd + "\n")
                proc.stdin.flush()
                line = proc.stdout.readline()
                if not line:
                    errors.append(f"step {idx} ({cmd}): host_driver exited prematurely")
                    break
                steps_executed += 1
                try:
                    resp = json.loads(line)
                except json.JSONDecodeError as exc:
                    errors.append(f"step {idx} ({cmd}): invalid JSON from host_driver: {exc}: {line!r}")
                    break
                if isinstance(resp.get("max_vertices_seen"), int):
                    max_vertices_seen = max(max_vertices_seen, resp["max_vertices_seen"])
                if isinstance(resp.get("max_frame_ms"), (int, float)):
                    max_frame_ms = max(max_frame_ms, float(resp["max_frame_ms"]))
                # Enforce global vertex cap whenever reported.
                cap = resp.get("max_vertices_cap")
                if isinstance(cap, int) and cap > 0 and isinstance(resp.get("max_vertices_seen"), int):
                    if resp["max_vertices_seen"] > cap:
                        errors.append(
                            f"step {idx} ({cmd}): max_vertices_seen={resp['max_vertices_seen']} > cap={cap}"
                        )
                if "expect" in step:
                    errors.extend(check_expect(resp, step["expect"], idx, cmd))

            try:
                if not sent_quit:
                    proc.stdin.write("QUIT\n")
                    proc.stdin.flush()
                proc.stdin.close()
            except OSError:
                pass

            try:
                rc = proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
                rc = proc.wait(timeout=5)
                errors.append("host_driver timed out on QUIT")

            stderr_text = proc.stderr.read().strip()
            if rc != 0:
                errors.append(f"host_driver exited with code {rc}: {stderr_text}")
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait(timeout=5)

        rec_rules = spec.get("record_assertions") or spec.get("assert_records") or []
        errors.extend(check_records(rec_rules, record_paths))
        errors.extend(check_files(spec.get("file_assertions", []), tmp_dir))

    wall_ms = int((time.monotonic() - t0) * 1000)
    return {
        "id": sid,
        "description": desc,
        "passed": len(errors) == 0,
        "wall_ms": wall_ms,
        "steps": steps_executed,
        "max_vertices_seen": max_vertices_seen,
        "max_frame_ms": round(max_frame_ms, 3),
        "errors": errors,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description="Run Tier 1 host + mock-worker scenarios.")
    ap.add_argument("--filter", default="", help="Only run scenarios whose filename/id contains this substring")
    ap.add_argument("--repeat", type=int, default=1, help="Repeat the full suite N times (for flake checking)")
    ap.add_argument("--no-build", action="store_true", help="Skip building host_driver")
    ap.add_argument("--jsonl", default="", help="Optional path to write per-scenario JSONL results")
    args = ap.parse_args()

    if not args.no_build:
        ensure_host_driver()

    paths = [Path(p) for p in sorted(glob.glob(str(SCENARIOS_DIR / "*.json")))] 
    if args.filter:
        paths = [p for p in paths if args.filter in p.name]
    if not paths:
        sys.stderr.write(f"No scenarios matched in {SCENARIOS_DIR}\n")
        return 1

    total_passed = 0
    total_failed = 0
    all_results: list[dict[str, Any]] = []

    for run_idx in range(args.repeat):
        for p in paths:
            res = run_scenario(p)
            all_results.append(res)
            if res["passed"]:
                total_passed += 1
                print(
                    f"ok   scenario {res['id']}: {res['description']} "
                    f"({res['wall_ms']} ms, max_v={res['max_vertices_seen']}, max_frame={res['max_frame_ms']}ms)"
                )
            else:
                total_failed += 1
                print(f"FAIL scenario {res['id']}: {res['description']} ({res['wall_ms']} ms)")
                for e in res["errors"]:
                    print(f"     {e}")

    if args.jsonl:
        with open(args.jsonl, "w", encoding="utf-8") as fp:
            for r in all_results:
                fp.write(json.dumps(r, separators=(",", ":")) + "\n")

    total = total_passed + total_failed
    print(f"{total} checks, {total_passed} passed, {total_failed} failed")
    return 0 if total_failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
