#!/usr/bin/env python3
"""Pick a bench's issues by a fixed rule, before any trial (#11211).

    select_issues.py --issues closed.json --n 100 --exclude 10074,... --until REV

A closed issue qualifies when:

- exactly one commit on `--until`'s history names it (`#N`), and that
  commit names no other issue;
- the commit changes 1 to 20 files outside lockfiles, no binary file, at
  least one Rust file under `crates/`, in at most four crates;
- it changes 6 to 1,200 lines outside lockfiles, and its diff is at most
  100 KB (the judge reads the whole reference change);
- the issue has a body of at least 80 characters;
- it is not in `--exclude` (issues an earlier round used).

The newest qualifying issues (by the fix commit's date) are taken, `--n` of
them. The rule reads only git history and issue text, never a trial result.
Prints `ISSUE:COMMIT` lines for `prepare.py`.
"""

import argparse
import json
import re
import subprocess
from collections import defaultdict

from common import REPO

REF = re.compile(r"#(\d{3,6})\b")


def git(*args: str) -> str:
    return subprocess.run(["git", "-C", str(REPO), *args], capture_output=True, text=True, check=True).stdout


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--issues", required=True, help="gh issue list --state closed --json number,title,body")
    ap.add_argument("--n", type=int, default=100)
    ap.add_argument("--exclude", default="")
    ap.add_argument("--until", default="origin/main")
    args = ap.parse_args()
    closed = {i["number"]: i for i in json.load(open(args.issues))}
    exclude = {int(x) for x in args.exclude.split(",") if x}
    commits: dict[int, list[str]] = defaultdict(list)
    names: dict[str, set[int]] = {}
    for line in git("log", args.until, "--no-merges", "--format=%H%x09%ct%x09%s").splitlines():
        sha, ts, subject = line.split("\t", 2)
        refs = {int(n) for n in REF.findall(subject)}
        names[sha] = refs
        for n in refs:
            commits[n].append(sha + "\t" + ts)
    picked = []
    for n, shas in commits.items():
        if n not in closed or n in exclude or len(shas) != 1:
            continue
        sha, ts = shas[0].split("\t")
        if names[sha] != {n}:
            continue
        if len((closed[n].get("body") or "").strip()) < 80:
            continue
        files, lines = [], 0
        for row in git("show", "--numstat", "--format=", sha).splitlines():
            parts = row.split("\t")
            if len(parts) != 3:
                continue
            a, d, path = parts
            if path.endswith(".lock"):
                continue
            if a == "-":
                files = None
                break
            files.append(path)
            lines += int(a) + int(d)
        if not files or not 1 <= len(files) <= 20 or not 6 <= lines <= 1200:
            continue
        ok = any(p.startswith("crates/") and p.endswith(".rs") for p in files)
        crates = {p.split("/")[1] for p in files if p.startswith("crates/")}
        if not ok or len(crates) > 4:
            continue
        size = len(subprocess.run(["git", "-C", str(REPO), "show", "--format=", sha, "--", ".", ":!*.lock"],
                                  capture_output=True).stdout)
        if size > 100_000:
            continue
        picked.append((int(ts), n, sha))
    picked.sort(reverse=True)
    for _, n, sha in picked[: args.n]:
        print(f"{n}:{sha[:12]}")


if __name__ == "__main__":
    main()
