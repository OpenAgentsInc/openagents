#!/usr/bin/env python3
"""Measures git lock contention between parallel runs sharing one repository.

Each round advances a bare origin, then N workers sharing one clone (its
linked worktrees) act at the same moment. Scenarios:

  fetch-raw        every worker runs `git fetch origin main` unserialized
  fetch-locked     every worker takes Coder's fetch lock first
                   (openagents-fetch.lock in the common git dir, as
                   crates/coder/src/task/landing.rs does)
  fetch-mixed      N-1 workers use the lock, one is an outside `git fetch`
                   (a person's `git pull`, oa-terminal-dev, another tool)
  worktree-add     every worker adds a new linked worktree
  commit           every worker commits in its own worktree

Usage: git_contention.py [workers] [rounds]
"""
import fcntl, os, subprocess, sys, tempfile, threading, time

N = int(sys.argv[1]) if len(sys.argv) > 1 else 6
ROUNDS = int(sys.argv[2]) if len(sys.argv) > 2 else 30


def git(cwd, *args):
    return subprocess.run(["git", "-C", cwd, *args], capture_output=True, text=True)


def must(cwd, *args):
    r = git(cwd, *args)
    if r.returncode:
        raise SystemExit(f"git {args}: {r.stderr}")
    return r.stdout.strip()


def setup(root):
    origin = os.path.join(root, "origin.git")
    must(root, "init", "-q", "--bare", "-b", "main", origin)
    seed = os.path.join(root, "seed")
    must(root, "init", "-q", "-b", "main", seed)
    for k, v in (("user.name", "b"), ("user.email", "b@x"), ("commit.gpgsign", "false")):
        must(seed, "config", k, v)
    open(os.path.join(seed, "f"), "w").write("0")
    must(seed, "add", "f")
    must(seed, "commit", "-qm", "0")
    must(seed, "push", "-q", origin, "HEAD:refs/heads/main")
    clone = os.path.join(root, "clone")
    must(root, "clone", "-q", origin, clone)
    for k, v in (("user.name", "b"), ("user.email", "b@x"), ("commit.gpgsign", "false")):
        must(clone, "config", k, v)
    trees = [clone]
    for i in range(1, N):
        t = os.path.join(root, f"wt{i}")
        must(clone, "worktree", "add", "-q", "--detach", t)
        trees.append(t)
    common = must(clone, "rev-parse", "--path-format=absolute", "--git-common-dir")
    return origin, seed, clone, trees, common


def advance(seed, origin, n):
    open(os.path.join(seed, "f"), "w").write(str(n))
    must(seed, "commit", "-qam", str(n))
    must(seed, "push", "-q", origin, "HEAD:refs/heads/main")


def locked_fetch(tree, common):
    with open(os.path.join(common, "openagents-fetch.lock"), "a+") as f:
        fcntl.flock(f, fcntl.LOCK_EX)
        try:
            return git(tree, "fetch", "-q", "origin", "main")
        finally:
            fcntl.flock(f, fcntl.LOCK_UN)


def together(jobs):
    barrier = threading.Barrier(len(jobs))
    out = [None] * len(jobs)

    def run(i, job):
        barrier.wait()
        out[i] = job()

    ts = [threading.Thread(target=run, args=(i, j)) for i, j in enumerate(jobs)]
    [t.start() for t in ts]
    [t.join() for t in ts]
    return out


def scenario(name):
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as root:
        origin, seed, clone, trees, common = setup(root)
        failures, samples, kinds = 0, 0, {}
        start = time.monotonic()
        for r in range(ROUNDS):
            advance(seed, origin, r + 1)
            if name == "fetch-raw":
                jobs = [lambda t=t: git(t, "fetch", "-q", "origin", "main") for t in trees]
            elif name == "fetch-locked":
                jobs = [lambda t=t: locked_fetch(t, common) for t in trees]
            elif name == "fetch-mixed":
                jobs = [lambda t=t: locked_fetch(t, common) for t in trees[1:]]
                jobs.append(lambda: git(clone, "fetch", "-q", "origin", "main"))
            elif name == "worktree-add":
                jobs = [lambda i=i: git(clone, "worktree", "add", "-q", "--detach",
                                        os.path.join(root, f"add{r}-{i}")) for i in range(N)]
            elif name == "commit":
                def commit(t, i):
                    open(os.path.join(t, f"c{i}"), "w").write(str(r))
                    git(t, "add", f"c{i}")
                    return git(t, "commit", "-qm", f"r{r}")
                jobs = [lambda t=t, i=i: commit(t, i) for i, t in enumerate(trees)]
            for res in together(jobs):
                samples += 1
                if res.returncode:
                    failures += 1
                    line = res.stderr.strip().splitlines()[0][:60] if res.stderr.strip() else "?"
                    kind = ("cannot lock ref" if "cannot lock ref" in res.stderr
                            else "couldn't write" if "couldn't write" in res.stderr
                            else "index.lock" if "index.lock" in res.stderr else line)
                    kinds[kind] = kinds.get(kind, 0) + 1
        secs = time.monotonic() - start
        print(f"{name:13} {failures:3}/{samples:<4} failed  {secs:5.1f}s  {kinds or ''}")


if __name__ == "__main__":
    print(f"git {must('.', 'version')}; {N} workers x {ROUNDS} rounds; {sys.platform}")
    for s in ("fetch-raw", "fetch-locked", "fetch-mixed", "worktree-add", "commit"):
        scenario(s)
