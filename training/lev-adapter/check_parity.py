#!/usr/bin/env python3
"""Proves the Python renderer matches the Rust one, character for character.

Apple's toolkit is Python, so the training data is rendered by `convert.py`
while serving is rendered by `crates/lev/src/schema.rs`. Two implementations
of one set of rules drift silently, and a model trained on text it never sees
at inference is the worst kind of bug: everything runs, the numbers are just
quietly worse.

    python3 check_parity.py

Non-zero exit on any disagreement.
"""

import json
import os
import pathlib
import shlex
import subprocess
import sys

import convert

REPO = pathlib.Path(__file__).resolve().parents[2]


def main():
    # The workspace needs rustc 1.95+; set CARGO to pick a toolchain, for
    # example CARGO="cargo +1.97.1".
    cargo = shlex.split(os.environ.get("CARGO", "cargo"))
    result = subprocess.run(
        cargo + ["run", "--quiet", "-p", "lev", "--bin", "lev-render"],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        print(result.stderr, file=sys.stderr)
        return 2

    reference = {}
    for line in result.stdout.splitlines():
        if not line.strip():
            continue
        row = json.loads(line)
        reference[row["id"]] = row

    suite = json.loads((REPO / "crates/lev/suites/support-v2.json").read_text())
    failures = []
    for item in suite["items"]:
        rust = reference.get(item["id"])
        if rust is None:
            failures.append(f"{item['id']}: the Rust side rendered nothing")
            continue
        mine_instructions = convert.instructions_text(item["question"])
        mine_prompt = convert.state_prompt(item["state"])
        mine_options, _ = convert.options_and_legend(item["question"])
        if mine_instructions != rust["instructions"]:
            failures.append(
                f"{item['id']}: instructions differ\n"
                f"  rust:   {rust['instructions']!r}\n"
                f"  python: {mine_instructions!r}"
            )
        if mine_prompt != rust["prompt"]:
            failures.append(
                f"{item['id']}: prompt differs\n"
                f"  rust:   {rust['prompt']!r}\n"
                f"  python: {mine_prompt!r}"
            )
        if mine_options != rust["options"]:
            failures.append(
                f"{item['id']}: options differ: rust {rust['options']} python {mine_options}"
            )

    if failures:
        for failure in failures[:10]:
            print(failure, file=sys.stderr)
        print(
            f"\n{len(failures)} disagreement(s) between the Python and Rust renderers",
            file=sys.stderr,
        )
        return 1

    print(f"renderers agree on all {len(suite['items'])} items")
    return 0


if __name__ == "__main__":
    sys.exit(main())
