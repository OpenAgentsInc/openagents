#!/usr/bin/env python3
"""Build the file-relevance dataset for scripts/bench/clef-relevance-bench.py.

For each closed issue, the state is the issue title and body, and the
candidate files are the files its fixing commit changed (relevant) plus an
equal number of same-directory (else same-crate) .rs files that no commit
mentioning the issue touched (not relevant). Contents are read at the fix
commit's parent and truncated to --cap bytes. Open issues are added as
unlabeled demo cases with hand-picked candidate files at origin/main.

The output holds repository file contents: write it to a scratch directory,
never commit it.

    python3 -I scripts/bench/clef-relevance-dataset.py --repo . --out $SCRATCH/dataset.json
"""
import argparse, json, random, subprocess

# issue -> fixing commit (subject carries "(#N)")
CLOSED = {
    11123: "76f465cf9d", 11117: "7344d6fb52", 10998: "c8abfacec9", 10295: "4d194518f5",
    10179: "41e0b9ce4a", 10283: "17edcc548a", 10177: "4b6495605c", 10206: "7310fc3bc1",
    10201: "74498aeb6a", 10170: "cdcc111e85",
}
# open issue -> candidate files at origin/main (no ground truth)
OPEN = {
    11190: ["crates/gateway/src/serve.rs", "crates/gateway/src/config.rs"],
    11159: ["crates/openagents-web/src/upstream.rs", "crates/openagents-web/src/lib.rs",
            "crates/openagents-web/src/coder_sync.rs"],
}
OPEN_FILL = 8  # pad open-issue candidates with same-directory files up to this many


def git(repo, *args):
    return subprocess.run(["git", "-C", repo, *args], check=True, capture_output=True).stdout


def gh_issue(repo, n):
    out = subprocess.run(["gh", "issue", "view", str(n), "--repo", "OpenAgentsInc/openagents",
                          "--json", "title,body,state"], check=True, capture_output=True, cwd=repo).stdout
    return json.loads(out)


def ls_rs(repo, rev, d):
    names = git(repo, "ls-tree", "--name-only", f"{rev}:{d}").decode().split()
    return [f"{d}/{n}" for n in names if n.endswith(".rs")]


def show(repo, rev, path, cap):
    raw = git(repo, "show", f"{rev}:{path}")
    text = raw.decode("utf-8", "replace")
    truncated = len(raw) > cap
    if truncated:
        text = raw[:cap].decode("utf-8", "ignore") + "\n/* [truncated] */"
    return text, len(raw), truncated


def touched_by_issue(repo, n):
    out = git(repo, "log", "origin/main", f"--grep=#{n}\\b", "-E", "--format=", "--name-only").decode()
    return {l for l in out.splitlines() if l.strip()}


def pick_negatives(repo, rev, positives, exclude, k, rng):
    dirs = sorted({p.rsplit("/", 1)[0] for p in positives})
    pool = sorted({f for d in dirs for f in ls_rs(repo, rev, d)} - exclude)
    if len(pool) < k:  # widen to the crate's src tree
        crates = sorted({"/".join(p.split("/")[:2]) for p in positives})
        more = git(repo, "ls-tree", "-r", "--name-only", rev, *crates).decode().split()
        pool = sorted(set(pool) | ({m for m in more if m.endswith(".rs")} - exclude))
    pool = [p for p in pool if int(git(repo, "cat-file", "-s", f"{rev}:{p}")) > 400]
    rng.shuffle(pool)
    return sorted(pool[:k])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default=".")
    ap.add_argument("--out", required=True)
    ap.add_argument("--cap", type=int, default=4096)
    ap.add_argument("--seed", type=int, default=11)
    a = ap.parse_args()
    rng = random.Random(a.seed)
    cases = []
    for n, commit in CLOSED.items():
        iss = gh_issue(a.repo, n)
        parent = f"{commit}^"
        pos = sorted(l for l in git(a.repo, "show", "--format=", "--name-only", commit).decode().splitlines() if l)
        neg = pick_negatives(a.repo, parent, pos, touched_by_issue(a.repo, n) | set(pos), len(pos), rng)
        files = []
        for p, label in [(p, True) for p in pos] + [(p, False) for p in neg]:
            try:
                text, size, tr = show(a.repo, parent, p, a.cap)
            except subprocess.CalledProcessError:  # file added by the fix: show it as committed
                text, size, tr = show(a.repo, commit, p, a.cap)
            files.append({"path": p, "relevant": label, "content": text, "bytes": size, "truncated": tr})
        cases.append({"issue": n, "state": iss["state"], "commit": commit, "title": iss["title"],
                      "body": iss["body"], "files": files})
    for n, given in OPEN.items():
        iss = gh_issue(a.repo, n)
        rev = "origin/main"
        extra = pick_negatives(a.repo, rev, given, set(given), OPEN_FILL - len(given), rng)
        files = []
        for p in sorted(given + extra):
            text, size, tr = show(a.repo, rev, p, a.cap)
            files.append({"path": p, "relevant": None, "content": text, "bytes": size, "truncated": tr})
        cases.append({"issue": n, "state": iss["state"], "commit": None, "title": iss["title"],
                      "body": iss["body"], "files": files})
    json.dump({"cap_bytes": a.cap, "cases": cases}, open(a.out, "w"), indent=1)
    for c in cases:
        lab = sum(1 for f in c["files"] if f["relevant"])
        print(c["issue"], c["state"], len(c["files"]), "relevant", lab,
              "truncated", sum(f["truncated"] for f in c["files"]))


main()
