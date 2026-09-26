"""Write MANIFEST.json for a retained directory of Microcoder runs.

Usage, from the repository root:

    python3 bench/terminal-bench/tools/microcoder_manifest.py \
        bench/terminal-bench/microcoder-runs/coderos-4080 --host coderos-4080

The manifest names every retained file's SHA-256, so `gym runs` can check
the records it reads, and attributes each run to a commit. Microcoder's
records name no commit, so the attribution follows one rule: the last
commit to `crates/microcoder` on `main` whose committer time is at or
before the run's start (the run directory's stamp). `--commit RUN=SHA`
overrides a run whose binary was built from work committed later, and
`--commit-note RUN=TEXT` says why. The Gym prints an attributed commit as
attributed, never as recorded. `--mixed RUN=TEXT` marks a directory two
runs wrote whose records don't show it themselves; the Gym leaves such a
run out of claims.
"""

import argparse
import datetime
import hashlib
import json
import os
import subprocess

RULE = (
    "attributed, not recorded: the last commit to crates/microcoder on main "
    "at or before the run's start"
)


def digest(path):
    with open(path, "rb") as f:
        return "sha256:" + hashlib.sha256(f.read()).hexdigest()


def started_seconds(name):
    stamp = name.rsplit("-", 1)[-1]
    if not stamp.isdigit():
        return None
    value = int(stamp)
    return value / 1000 if len(stamp) >= 13 else value


def microcoder_commits(ref):
    out = subprocess.run(
        ["git", "log", ref, "--format=%ct %H", "--", "crates/microcoder"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    commits = []
    for line in out.splitlines():
        when, sha = line.split()
        commits.append((int(when), sha))
    return commits


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("dir")
    parser.add_argument("--host", required=True)
    parser.add_argument("--ref", default="origin/main")
    parser.add_argument("--commit", action="append", default=[])
    parser.add_argument("--commit-note", action="append", default=[])
    parser.add_argument("--mixed", action="append", default=[])
    args = parser.parse_args()
    mixed = dict(item.split("=", 1) for item in args.mixed)
    overrides = dict(item.split("=", 1) for item in args.commit)
    notes = dict(item.split("=", 1) for item in args.commit_note)
    commits = microcoder_commits(args.ref)
    runs = []
    for name in sorted(os.listdir(args.dir)):
        path = os.path.join(args.dir, name)
        if not os.path.isdir(path):
            continue
        files = {
            file: digest(os.path.join(path, file))
            for file in ("summary.json", "events.jsonl")
            if os.path.isfile(os.path.join(path, file))
        }
        if not files:
            continue
        run = {"name": name, "files": files}
        start = started_seconds(name)
        if name in overrides:
            run["commit"] = overrides[name]
            run["commit_source"] = notes.get(name, "attributed by hand")
        elif start is not None:
            before = [sha for when, sha in commits if when <= start]
            if before:
                run["commit"] = before[0][:10]
        if name in mixed:
            run["mixed"] = mixed[name]
        runs.append(run)
    manifest = {
        "schema": "openagents.gym.microcoder-retained.v1",
        "host": args.host,
        "source": "~/.openagents/microcoder/runs on " + args.host,
        "copied_at": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "files": "summary.json and events.jsonl only; artifacts/ is not retained",
        "commit_rule": RULE,
        "runs": runs,
    }
    with open(os.path.join(args.dir, "MANIFEST.json"), "w") as f:
        json.dump(manifest, f, indent=1)
        f.write("\n")
    print(f"{len(runs)} runs in {args.dir}/MANIFEST.json")


if __name__ == "__main__":
    main()
