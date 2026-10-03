#!/usr/bin/env python3
"""Run the hidden Rust checker outside the candidate checkout (Boat only)."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--run-dir", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=300)
    args = parser.parse_args()
    if os.uname().sysname != "Linux":
        parser.error("Run this checker on the isolated Linux Boat host.")
    repo = args.repo.resolve(strict=True)
    run_dir = args.run_dir.resolve()
    target_dir = args.target_dir.resolve()
    for artifact in (run_dir, target_dir):
        if artifact == repo or repo in artifact.parents:
            parser.error("Keep checker artifacts and Cargo target outside the candidate repository.")
    if not (repo / "crates/background/Cargo.toml").is_file():
        parser.error("Candidate background manifest is missing.")
    run_dir.mkdir(parents=True, exist_ok=False)
    checker = Path(__file__).resolve().with_name("ignored_worktrees.rs")
    quote = json.dumps
    manifest = "\n".join([
        "[package]", 'name = "historical-replay-10166-checker"', 'version = "0.0.0"',
        'edition = "2024"', "[workspace]", "[dependencies]",
        "background = { path = " + quote(str(repo / "crates/background")) + " }",
        "[dev-dependencies]", 'tempfile = "3"', "[[test]]",
        'name = "ignored_worktrees"', "path = " + quote(str(checker)), "",
    ])
    (run_dir / "Cargo.toml").write_text(manifest)
    # Seed dependency versions from the historical checkout. Cargo may rewrite
    # this external lockfile for the small harness; it never edits the input.
    (run_dir / "Cargo.lock").write_bytes((repo / "Cargo.lock").read_bytes())
    env = os.environ.copy()
    env.update({"CARGO_TARGET_DIR": str(target_dir), "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_NOSYSTEM": "1", "GIT_TERMINAL_PROMPT": "0"})
    for key in ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_COMMON_DIR"):
        env.pop(key, None)
    command = ["cargo", "+1.97.1", "test", "--manifest-path", str(run_dir / "Cargo.toml"), "--offline", "--test", "ignored_worktrees", "--", "--test-threads=1"]
    head = "4705102273140a5f381fb75e17a29965662629c8 (export plus candidate patch)"
    started = time.monotonic()
    timed_out = False
    with (run_dir / "checker.log").open("wb") as output:
        try:
            result = subprocess.run(command, cwd=run_dir, env=env, stdout=output, stderr=subprocess.STDOUT, timeout=args.timeout)
            exit_code = result.returncode
        except subprocess.TimeoutExpired:
            timed_out = True
            exit_code = 124
    result = {
        "schema": "openagents.private-historical-replay-check.v1",
        "candidate_head": head,
        "checker_sha256": hashlib.sha256(checker.read_bytes()).hexdigest(),
        "source_git_rs_sha256": hashlib.sha256((repo / "crates/background/src/git.rs").read_bytes()).hexdigest(),
        "argv": command,
        "exit_code": exit_code,
        "timed_out": timed_out,
        "elapsed_seconds": time.monotonic() - started,
        "log": str(run_dir / "checker.log"),
        "scope": "Seven deterministic API regressions; no original agent tests, real host cleanup, deployment, or network. Build time is included.",
    }
    (run_dir / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    raise SystemExit(exit_code)


if __name__ == "__main__":
    main()
