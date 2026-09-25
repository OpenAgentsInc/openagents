#!/usr/bin/env python3
"""Verify retained inputs, re-extract reports, and replay Jev without a network call."""
import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--coder-one", required=True, type=Path)
    args = parser.parse_args()
    here = Path(__file__).resolve().parent
    repo = here.parents[3]
    manifest = json.loads((here / "manifest.json").read_text())
    expected = [json.loads(line) for line in (here / "records/microluna/rows.jsonl").read_text().splitlines()]
    with tempfile.TemporaryDirectory(prefix="truthful-checks-replay-") as work:
        work = Path(work)
        traces = work / "traces"
        traces.mkdir()
        for trial in manifest["trials"]:
            job = repo / "bench/terminal-bench/traces" / trial["job"]
            episode = job / (trial["trial"] + ".episode")
            for name, digest in trial["files"].items():
                if hashlib.sha256((episode / name).read_bytes()).hexdigest() != digest:
                    raise SystemExit(f"Changed retained input: {episode / name}")
            link = traces / trial["job"]
            if not link.exists():
                link.symlink_to(job, target_is_directory=True)
        out = work / "result"
        out.mkdir()
        shutil.copyfile(here / "records/microluna/jev-recorded.json", out / "jev-recorded.json")
        subprocess.run([str(args.coder_one.resolve()), "checks", "truth", "--jobs", "none",
                        "--traces", str(traces), "--out", str(out), "--jev", "recorded", "--json"],
                       check=True, stdout=subprocess.DEVNULL)
        got = [json.loads(line) for line in (out / "rows.jsonl").read_text().splitlines()]
        if got != expected:
            raise SystemExit("Re-extracted rows differ from the retained measurement")
        print(f"Replayed {len(got)} trials with identical rows and no live model calls.")


if __name__ == "__main__":
    main()
