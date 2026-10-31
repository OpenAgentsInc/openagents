#!/usr/bin/env python3
"""Source-level return-borrow regressions; maintenance tooling, never runtime."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

from quiet_crash import command as quiet_command

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = Path(__file__).with_name("borrow-summaries")
MANIFEST = FIXTURES / "fixtures.json"


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def fixture(name):
    path = (FIXTURES / name).resolve()
    assert path.is_relative_to(FIXTURES.resolve()) and path.is_file(), name
    return path


def inventory():
    manifest = json.loads(MANIFEST.read_text())
    assert manifest["schema"] == "coax.borrow-summaries.fixtures.v1"
    cases = manifest["cases"]
    assert cases and len({case["id"] for case in cases}) == len(cases)
    sources = [case["source"] for case in cases]
    assert len(set(sources)) == len(sources)
    expected = set(sources) | set(manifest["support"])
    actual = {str(path.relative_to(FIXTURES)) for path in FIXTURES.rglob("*.coax")}
    assert expected == actual, "fixture inventory is not closed"
    for name in expected:
        fixture(name)
    for case in cases:
        assert case["outcome"] in ("accept", "reject", "accept-manual")
        assert bool(case["diagnostics"]) == (case["outcome"] == "reject")
        for diagnostic in case["diagnostics"]:
            lines = fixture(diagnostic["file"]).read_text().splitlines()
            assert type(diagnostic["line"]) is int and 0 < diagnostic["line"] <= len(lines)
            assert type(diagnostic["col"]) is int and 0 < diagnostic["col"] <= len(lines[diagnostic["line"] - 1])
            assert diagnostic["code"] in ("E0383", "E0366", "E0381")
    return cases, sorted(fixture(name) for name in expected)


def snapshot(paths):
    return {str(path): digest(path) for path in paths}


def run(compiler, directory, selected):
    cases, sources = inventory()
    if selected:
        unknown = set(selected) - {case["id"] for case in cases}
        assert not unknown, f"unknown selection: {sorted(unknown)}"
        cases = [case for case in cases if case["id"] in selected]
    inputs = [*sources, MANIFEST, Path(__file__).resolve(), Path(__file__).with_name("quiet_crash.py")]
    before = snapshot(inputs)
    compiler_before = digest(compiler)
    record = {
        "schema": "coax.borrow-summaries.source-qualification.v1",
        "scope": "Original-source diagnostic and acceptance fixtures only. Acceptance does not prove completeness, retained-state safety or completion/cancellation lifetimes.",
        "compiler": str(compiler),
        "compiler_sha256": compiler_before,
        "producer_provenance": "Actual selected producer; matching compiler/source qualification is retained separately by the integrating gate.",
        "source_before": before,
        "selection": [case["id"] for case in cases],
        "cases": [],
        "outcome": "not-checked",
    }

    def save():
        record["source_after"] = snapshot(inputs)
        record["compiler_sha256_after"] = digest(compiler)
        record["inputs_unchanged"] = record["source_after"] == before and record["compiler_sha256_after"] == compiler_before
        if not record["inputs_unchanged"]:
            record["outcome"] = "source-changed"
        (directory / "qualification.json").write_text(json.dumps(record, indent=2) + "\n")

    started = time.monotonic()
    deadline = started + 600
    failed = []
    try:
        for case in cases:
            save()
            assert record["inputs_unchanged"], "fixture/compiler changed before check"
            remaining = deadline - time.monotonic()
            assert remaining > 0, "ten-minute source gate budget exhausted"
            source = fixture(case["source"])
            argv = [str(compiler), "check", "--json", str(source)]
            tick = time.monotonic()
            result = subprocess.run(quiet_command(argv), cwd=ROOT.parent, capture_output=True, timeout=min(60, remaining))
            for stream, data in [("stdout", result.stdout), ("stderr", result.stderr)]:
                (directory / (case["id"] + "." + stream)).write_bytes(data)
            parse_error = None
            try:
                observed = [json.loads(line) for line in result.stderr.decode().splitlines()]
                normalized = [{"file": str(Path(item["file"]).resolve()), "line": item["line"], "col": item["col"], "code": item["code"]} for item in observed]
            except (UnicodeError, ValueError, KeyError, TypeError) as error:
                observed, normalized = [], []
                parse_error = str(error)
            expected = [{**item, "file": str(fixture(item["file"]))} for item in case["diagnostics"]]
            passed = parse_error is None and result.returncode == (1 if expected else 0) and not result.stdout and normalized == expected
            record["cases"].append({
                "id": case["id"], "source": str(source), "expected_scope": case["outcome"],
                "argv": argv, "returncode": result.returncode, "seconds": time.monotonic() - tick,
                "expected_diagnostics": expected, "observed_diagnostics": observed,
                "diagnostic_transport_error": parse_error,
                "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
                "stderr_sha256": hashlib.sha256(result.stderr).hexdigest(),
                "outcome": "passed" if passed else "failed",
            })
            save()
            assert record["inputs_unchanged"], "fixture/compiler changed during check"
            if not passed:
                failed.append(case["id"])
        assert not failed, f"source fixture failures: {failed}; exact observations: {directory / 'qualification.json'}"
        record["outcome"] = "passed"
    except BaseException as error:
        record["outcome"] = "failed"
        record["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        record["elapsed_s"] = time.monotonic() - started
        save()
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", default=os.environ.get("COAX_TEST_COMPILER", str(ROOT / "bin/coax")))
    parser.add_argument("--record-dir", help="new directory for exact commands, diagnostics and source/producer hashes")
    parser.add_argument("--only", action="append", help="run only this fixture ID; repeat for a focused follow-up")
    args = parser.parse_args()
    compiler = Path(args.compiler).resolve()
    assert compiler.is_file(), compiler
    directory = Path(args.record_dir).resolve() if args.record_dir else Path(tempfile.mkdtemp(prefix="coax-return-borrows-"))
    assert not directory.is_relative_to(FIXTURES.resolve()), "records must not replace fixture sources"
    if args.record_dir:
        directory.mkdir(parents=True, exist_ok=False)
    record = run(compiler, directory, args.only)
    print(json.dumps({"outcome": record["outcome"], "fixtures": len(record["cases"]), "record": str(directory / "qualification.json")}))


if __name__ == "__main__":
    main()
