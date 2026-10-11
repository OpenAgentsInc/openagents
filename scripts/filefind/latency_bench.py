"""Latency bench for `filefind.py query` (#11269): per-stage timing on a fixed query set,
cold and warm page cache, and a check that two finder versions rank identically.

    # the fixed query set: issue text plus its query embedding, so no run needs the network
    python3 scripts/filefind/latency_bench.py queries --from 11213 --to 11262
    # one process per query, as the CLI runs; cold evicts the cache files from the page cache first
    python3 scripts/filefind/latency_bench.py run --ff scripts/filefind --mode warm --out /tmp/new-warm.jsonl
    python3 scripts/filefind/latency_bench.py stats /tmp/base-warm.jsonl /tmp/new-warm.jsonl
    python3 scripts/filefind/latency_bench.py same /tmp/base-warm.jsonl /tmp/new-warm.jsonl

`--ff` may be any directory holding a filefind.py (e.g. an older version from
`git show REV:scripts/filefind/filefind.py`). Every run reads the same per-repository
cache at the same revision; the outputs (ranked list, map, stages, reasons) are compared
by digest, timing excluded. Runs are niced and sequential, so the bench stays quiet.
"""

import argparse
import contextlib
import hashlib
import io
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
QUERIES = os.path.join(HERE, "fixtures", "latency-queries.json")


def load_queries(path=QUERIES):
    return json.load(open(path))["queries"]


def cmd_queries(a):
    sys.path.insert(0, HERE)
    import numpy as np
    import filefind as ff
    rows = json.loads(subprocess.run(
        ["gh", "issue", "list", "--state", "all", "--limit", "300", "--search", "sort:created-desc",
         "--json", "number,title,body"], cwd=a.repo, check=True, capture_output=True).stdout)
    qs = sorted((r for r in rows if a.lo <= r["number"] <= a.hi), key=lambda r: r["number"])
    cache = ff.default_cache(a.repo)
    key = ff.embed_key(required=True, cache=cache)
    vecs = ff.embed([ff.issue_text(r["title"], r["body"] or "") for r in qs], key, task="RETRIEVAL_QUERY")
    out = {"schema": "openagents.filefind.latency-queries.v1", "embedder": ff.emb_tag(key) or "openrouter",
           "queries": [{"issue": r["number"], "title": r["title"], "body": r["body"] or "",
                        "qvec": [round(float(x), 7) for x in np.asarray(v, np.float32)]}
                       for r, v in zip(qs, vecs)]}
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    with open(a.out, "w") as f:
        json.dump(out, f, indent=0)
    print(f"{len(qs)} queries -> {a.out}", file=sys.stderr)


def run_one(ffdir, repo, rev, q, profile=None, cache=None):
    """One query through filefind's own `query` path, issue text and embedding in hand.
    Returns its --json output. Only the query's embedding is stubbed; index refreshes are real."""
    sys.path.insert(0, ffdir)
    import numpy as np
    import filefind as ff
    real = ff.embed

    def embed(texts, key, **kw):
        if kw.get("task") == "RETRIEVAL_QUERY" and len(texts) == 1:
            return np.array([q["qvec"]], np.float32)
        return real(texts, key, **kw)
    ff.embed = embed
    ff.issue_from_gh = lambda repo, n: (q["title"], q["body"])
    a = argparse.Namespace(repo=repo, rev=rev, issue=q["issue"], text=None, k=100, map=400, json=True,
                           cache=cache, workspace=None, model=os.path.join(ffdir, "model.json"), no_cards=False)
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        if profile:
            import cProfile
            cProfile.runctx("ff.cmd_query(a)", globals(), {"ff": ff, "a": a}, profile)
        else:
            ff.cmd_query(a)
    return json.loads(buf.getvalue())


def cmd_one(a):
    q = load_queries(a.queries)[a.index]
    print(json.dumps(run_one(os.path.abspath(a.ff), a.repo, a.rev, q, a.profile, a.cache)))


def evict(path):
    """Drop a file from the page cache (mmap + msync MS_INVALIDATE, as vmtouch -e does)."""
    import ctypes
    import ctypes.util
    n = os.path.getsize(path)
    if not n:
        return
    libc = ctypes.CDLL(ctypes.util.find_library("c"), use_errno=True)
    libc.mmap.restype = ctypes.c_void_p
    libc.mmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                          ctypes.c_longlong]
    libc.msync.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int]
    libc.munmap.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    ms_invalidate = 2  # MS_INVALIDATE: drops clean pages on macOS (Linux keeps them: drop_caches there)
    fd = os.open(path, os.O_RDONLY)
    try:
        addr = libc.mmap(None, n, 1, 1, fd, 0)  # PROT_READ, MAP_SHARED
        if addr in (None, ctypes.c_void_p(-1).value):
            return
        libc.msync(addr, n, ms_invalidate)
        libc.munmap(addr, n)
    finally:
        os.close(fd)


def cmd_run(a):
    sys.path.insert(0, HERE)
    import filefind as ff
    qs = load_queries(a.queries)
    cache = a.cache or ff.default_cache(a.repo)
    rev = subprocess.run(["git", "-C", a.repo, "rev-parse", a.rev], check=True, capture_output=True,
                         text=True).stdout.strip()
    n = min(a.n or len(qs), len(qs))
    outdir = a.out + ".d"
    os.makedirs(outdir, exist_ok=True)
    with open(a.out, "w") as out:
        for i in range(n):
            if a.mode == "cold":
                for name in os.listdir(cache):
                    p = os.path.join(cache, name)
                    if os.path.isfile(p):
                        evict(p)
            env = dict(os.environ, PYTHONHASHSEED=str(a.hashseed))
            r = subprocess.run(["nice", "-n", "19", sys.executable, os.path.abspath(__file__), "one",
                                "--ff", a.ff, "--repo", a.repo, "--rev", rev, "--queries", a.queries,
                                "--index", str(i)] + (["--cache", a.cache] if a.cache else []), capture_output=True, text=True, env=env)
            if r.returncode:
                sys.exit(r.stderr[-3000:])
            d = json.loads(r.stdout)
            t = d.pop("timing_ms")
            body = json.dumps(d, sort_keys=True)
            with open(os.path.join(outdir, f"{d['issue']}.json"), "w") as f:
                f.write(body)
            out.write(json.dumps({"issue": d["issue"], "timing": t,
                                  "digest": hashlib.sha256(body.encode()).hexdigest()}) + "\n")
            out.flush()
            print(f"{i:3d} #{d['issue']} {t['total']} ms", file=sys.stderr)


def pct(xs, p):
    xs = sorted(xs)
    if not xs:
        return 0.0
    k = (len(xs) - 1) * p / 100
    lo, hi = int(k), min(int(k) + 1, len(xs) - 1)
    return xs[lo] + (xs[hi] - xs[lo]) * (k - lo)


def cmd_stats(a):
    for fn in a.files:
        rows = [json.loads(l) for l in open(fn)]
        tot = [r["timing"]["total"] for r in rows]
        print(f"{fn}: n={len(rows)} p50 {pct(tot, 50):.0f}  p95 {pct(tot, 95):.0f}  p99 {pct(tot, 99):.0f}  "
              f"max {max(tot):.0f} ms")
        keys = [k for k in rows[0]["timing"] if k not in ("fetch_issue", "total")]
        print("  stage p50/p95: " + "  ".join(
            f"{k} {pct([r['timing'].get(k, 0) for r in rows], 50):.0f}/"
            f"{pct([r['timing'].get(k, 0) for r in rows], 95):.0f}" for k in keys))


def cmd_same(a):
    base = {json.loads(l)["issue"]: json.loads(l)["digest"] for l in open(a.a)}
    new = {json.loads(l)["issue"]: json.loads(l)["digest"] for l in open(a.b)}
    diff = sorted(n for n in base.keys() & new.keys() if base[n] != new[n])
    print(f"{len(base.keys() & new.keys())} queries compared, {len(diff)} differ" +
          (f": {diff}" if diff else ""))
    sys.exit(1 if diff else 0)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    q = sub.add_parser("queries")
    q.add_argument("--repo", default=".")
    q.add_argument("--from", dest="lo", type=int, required=True)
    q.add_argument("--to", dest="hi", type=int, required=True)
    q.add_argument("--out", default=QUERIES)
    o = sub.add_parser("one")
    r = sub.add_parser("run")
    for p in (o, r):
        p.add_argument("--ff", default=HERE, help="directory holding the filefind.py to run")
        p.add_argument("--repo", default=".")
        p.add_argument("--rev", default="HEAD")
        p.add_argument("--queries", default=QUERIES)
        p.add_argument("--cache", help="a frozen copy of the cache (e.g. `cp -cR`), so runs compare "
                       "like for like while the shared cache keeps refreshing")
    o.add_argument("--index", type=int, required=True)
    o.add_argument("--profile")
    r.add_argument("--mode", choices=("warm", "cold"), default="warm")
    r.add_argument("--n", type=int)
    r.add_argument("--hashseed", type=int, default=0,
                   help="PYTHONHASHSEED for every query: tied scores and rank features follow set order, "
                        "which follows the string hash, so outputs compare only under one seed")
    r.add_argument("--out", required=True)
    s = sub.add_parser("stats")
    s.add_argument("files", nargs="+")
    m = sub.add_parser("same")
    m.add_argument("a")
    m.add_argument("b")
    a = ap.parse_args()
    {"queries": cmd_queries, "one": cmd_one, "run": cmd_run, "stats": cmd_stats, "same": cmd_same}[a.cmd](a)


if __name__ == "__main__":
    main()
