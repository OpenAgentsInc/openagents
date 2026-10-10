#!/usr/bin/env python3
"""End-to-end file-finding bench for scripts/filefind/filefind.py (#11210).

Every case is replayed at its fix's parent commit: the tree, the co-change
history, and the past issues are all cut at that commit, so nothing the fix
did leaks into the query. The newest --eval cases are the bench; the older
cases train the scorer.

Steps (each writes into --work, a scratch directory):

  prepare   embed every blob of every case's parent tree (cached per blob)
  features  run the candidate stages per case; per-stage recall and latency
  train     fit the scorer on the train cases -> scripts/filefind/model.json
  eval      rank the eval cases; recall / precision at 20, 50, 100; misses
  judge     ask Jev (batch nouls) about the files the scorer is unsure of
  plan      stage 4: a model writes the change plan; map steps to files

    export OPENROUTER_API_KEY=...   # embeddings and the plan model
    export TYPESAFE_API_KEY=...     # Jev, judge step only
    python3 scripts/bench/file-finding-bench.py prepare --repo . --dataset D --work W
    python3 scripts/bench/file-finding-bench.py features --repo . --dataset D --work W
    python3 scripts/bench/file-finding-bench.py train --dataset D --work W
    python3 scripts/bench/file-finding-bench.py eval --dataset D --work W
"""
import argparse, json, math, os, pickle, statistics, sys, time
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "filefind"))
import filefind as ff  # noqa: E402

KS = (20, 50, 100)


def kind(p):
    name = p.rsplit("/", 1)[-1]
    ext = name.rsplit(".", 1)[-1] if "." in name else ""
    if ext == "rs":
        if "/tests/" in p or name in ("tests.rs",) or name.endswith(("_tests.rs", "_test.rs")):
            return "rust test"
        if name in ("lib.rs", "main.rs", "mod.rs"):
            return "rust lib/main/mod"
        return "rust source"
    if name == "Cargo.toml":
        return "Cargo.toml"
    if ext == "md":
        if name in ("README.md", "INVARIANTS.md", "AGENTS.md", "CLAUDE.md") or name.isupper():
            return "README / INVARIANTS / AGENTS"
        return "docs (.md)"
    if ext in ("sh", "py", "mjs", "js") and p.startswith("scripts/"):
        return "script"
    if ext in ("json", "jsonl", "sse", "txt", "garden", "yaml", "yml") or "/fixtures/" in p:
        return "fixture / data"
    if ext in ("swift", "kt", "h", "m"):
        return "mobile (Swift/Kotlin)"
    if ext in ("css", "js", "mjs", "ts", "tsx", "html", "wgsl"):
        return "web / shader"
    return "other"


def split(cases, n_eval):
    return cases[:n_eval], cases[n_eval:]  # dataset is newest-first


def load(a):
    d = json.load(open(a.dataset))
    return d["cases"]


def cmd_prepare(a):
    ix = ff.Index(a.cache or ff.default_cache(a.repo)).load()
    key = ff.embed_key()
    cases = load(a)
    total = 0
    union = {}
    for i, c in enumerate(cases):
        tree = ff.ls_tree(a.repo, c["parent"])
        union.update({s: p for p, s in tree.items()})
        n = ix.ensure_blobs(a.repo, tree, key, log=False)
        total += n
        if n:
            print(f"[{i+1}/{len(cases)}] #{c['issue']}: +{n} blobs", file=sys.stderr)
        if n and i % 20 == 0:
            ix.save_blobs()
    ix.save_blobs()
    print(f"embedded {total} new blobs; cache has {len(ix.blob_rows)}")
    ix.tokens.ensure(a.repo, [(p, s) for s, p in union.items()])


def run_case(ix, repo, c, key, keep_query=False):
    timing = {}
    t0 = time.perf_counter()
    tree = ff.ls_tree(repo, c["parent"])
    timing["tree"] = time.perf_counter() - t0
    pos = ix.hist["pos"][c["commit"]]
    isx = ix.issues
    t0 = time.perf_counter()
    j = np.searchsorted(isx["numbers"], c["issue"])
    if j < len(isx["numbers"]) and isx["numbers"][j] == c["issue"]:
        qvec = isx["vecs"][j]
        timing["embed_query"] = None  # measured separately (live call)
    else:
        qvec = ff.embed([ff.issue_text(c["title"], c["body"])], key)[0]
        timing["embed_query"] = time.perf_counter() - t0
    q = ff.Query(ix, repo, c["parent"], tree, cutoff=pos, exclude_issue=c["issue"], timing=timing)
    feats = q.run(c["title"], c["body"], qvec)
    hand = {h["path"] for h in c["hand"] if h["existing"]}
    if keep_query:
        return {"issue": c["issue"], "feats": feats, "query": q, "hand": sorted(hand), "tree_n": len(tree),
                "tree_dirs": sorted({p.rsplit("/", 1)[0] for p in tree if "/" in p})}
    return {"issue": c["issue"], "feats": feats, "src": {k: sorted(v) for k, v in q.src.items()},
            "reason": {p: q.reason.get(p, [])[:4] for p in feats}, "timing": timing,
            "hand": sorted(hand), "tree_n": len(tree),
            "tree_dirs": sorted({p.rsplit("/", 1)[0] for p in tree if "/" in p})}


def cmd_features(a):
    ix = ff.Index(a.cache or ff.default_cache(a.repo)).load()
    key = os.environ.get("OPENROUTER_API_KEY")
    cases = load(a)
    out = {}
    path = os.path.join(a.work, "features.pkl")
    if os.path.exists(path) and not a.fresh:
        out = pickle.load(open(path, "rb"))
    todo = [c for c in cases if c["issue"] not in out]

    def go(c):
        return run_case(ix, a.repo, c, key)
    with ThreadPoolExecutor(a.workers) as ex:
        for i, r in enumerate(ex.map(go, todo)):
            out[r["issue"]] = r
            if i % 25 == 0:
                print(f"features {i+1}/{len(todo)}", file=sys.stderr)
    pickle.dump(out, open(path, "wb"), protocol=4)
    stage_report(cases[:a.eval], out)


def stage_report(cases, feats):
    """Recall per candidate stage alone and in union, on existing hand-written files."""
    rows = []
    tot = 0
    hit = Counter()
    pool_sizes = defaultdict(list)
    lat = defaultdict(list)
    for c in cases:
        r = feats[c["issue"]]
        hand = set(r["hand"])
        tot += len(hand)
        allsrc = set()
        for s in ff.SOURCES:
            got = set(r["src"].get(s, []))
            pool_sizes[s].append(len(got))
            hit[s] += len(hand & got)
            allsrc |= got
        hit["union"] += len(hand & allsrc)
        pool_sizes["union"].append(len(allsrc))
        for k, v in r["timing"].items():
            if v is not None:
                lat[k].append(v)
    print(f"\nstage recall over {tot} existing hand-written files in {len(cases)} eval cases")
    print("| Stage | Recall | Median candidates |")
    print("|---|---:|---:|")
    for s in ff.SOURCES + ["union"]:
        print(f"| {s} | {hit[s]/tot:.3f} ({hit[s]}) | {statistics.median(pool_sizes[s]):.0f} |")
    print("\nlatency per stage (median / p90 ms):")
    for k, v in lat.items():
        v = sorted(v)
        print(f"  {k}: {1000*statistics.median(v):.0f} / {1000*v[int(0.9*(len(v)-1))]:.0f}")


def rows_of(cases, feats, names=None):
    rows, y, grp = [], [], []
    for c in cases:
        r = feats[c["issue"]]
        hand = set(r["hand"])
        for p, f in r["feats"].items():
            rows.append(f)
            y.append(1.0 if p in hand else 0.0)
            grp.append(c["issue"])
    if names is None:
        names = sorted({k for f in rows for k in f})
    return ff.vectorize(rows, names), np.array(y), names


def fit(cases, feats, a, names=None):
    X, y, names = rows_of(cases, feats, names)
    m = ff.fit_gbdt(X, y, trees=a.trees)
    m["features"] = names
    return m


def stage2_features(a, cases, feats1, models_for):
    """Re-run each case's query and add the stage-2 features from its stage-1 model."""
    ix = ff.Index(a.cache or ff.default_cache(a.repo)).load()
    out = {}
    for i, c in enumerate(cases):
        r = run_case(ix, a.repo, c, None, keep_query=True)
        q = r.pop("query")
        first = ff.rank(models_for(c["issue"]), r["feats"])
        r["feats"] = q.stage2(r["feats"], first)
        r["src"] = {k: sorted(v) for k, v in q.src.items()}
        r["reason"] = {p: q.reason.get(p, [])[:5] for p in r["feats"]}
        r["timing"] = q.t
        out[c["issue"]] = r
        if i % 50 == 0:
            print(f"stage2 {i+1}/{len(cases)}", file=sys.stderr)
    return out


def cmd_train(a):
    cases = load(a)
    feats = pickle.load(open(os.path.join(a.work, "features.pkl"), "rb"))
    ev, train = split(cases, a.eval)
    # stage 1: cross-fit on two folds so the train cases' stage-2 inputs are out of sample
    fa, fb = train[0::2], train[1::2]
    t0 = time.time()
    m_a = fit(fa, feats, a)
    m_b = fit(fb, feats, a, m_a["features"])
    m1 = fit(train, feats, a, m_a["features"])
    print(f"stage 1 fitted ({time.time()-t0:.0f}s)", file=sys.stderr)
    in_a = {c["issue"] for c in fa}
    in_b = {c["issue"] for c in fb}

    def models_for(n):
        return m_b if n in in_a else m_a if n in in_b else m1
    feats2 = stage2_features(a, cases, feats, models_for)
    pickle.dump(feats2, open(os.path.join(a.work, "features2.pkl"), "wb"), protocol=4)
    m2 = fit(train, feats2, a)
    model = {"schema": "openagents.filefind.model.v1", "stage1": m1, "stage2": m2,
             "trained_on": {"cases": len(train), "issues": f"#{train[-1]['issue']}..#{train[0]['issue']}",
                            "fix_commits_before": train[0]["commit"]}}
    json.dump(model, open(a.model, "w"))
    print(f"trained on {len(train)} cases -> {a.model}")


def cmd_eval(a):
    cases = load(a)
    model = json.load(open(a.model))
    ev, _ = split(cases, a.eval)
    f1 = pickle.load(open(os.path.join(a.work, "features.pkl"), "rb"))
    f2 = pickle.load(open(os.path.join(a.work, "features2.pkl"), "rb"))
    print("== stage 1 candidates")
    stage_report(ev, f2)
    print("\n== stage 1 scorer only")
    evaluate(ev, f1, model["stage1"], a)
    print("\n== stage 1 + stage 2 (propagation)")
    res = evaluate(ev, f2, model["stage2"], a)
    json.dump(res, open(os.path.join(a.work, "eval.json"), "w"), indent=1)


def evaluate(ev, feats, model, a, judged=None):
    tot_e = sum(len(feats[c["issue"]]["hand"]) for c in ev)
    tot_all = sum(len(c["hand"]) for c in ev)
    hits = Counter()
    prec = defaultdict(list)
    miss_kind = {k: Counter() for k in KS}
    all_kind = Counter()
    per_case = []
    added_dir = Counter()
    added_tot = 0
    derived = Counter()
    for c in ev:
        r = feats[c["issue"]]
        ranked = ff.rank(model, r["feats"])
        if judged and c["issue"] in judged:
            ranked = judged[c["issue"]]
        order = [p for p, _ in ranked]
        hand = set(r["hand"])
        for h in hand:
            all_kind[kind(h)] += 1
        row = {"issue": c["issue"], "n": len(hand)}
        for k in KS:
            top = set(order[:k])
            got = hand & top
            hits[k] += len(got)
            prec[k].append(len(got) / k)
            row[f"r{k}"] = len(got)
            for m in hand - top:
                miss_kind[k][kind(m)] += 1
        row["pool"] = len(order)
        row["union_hit"] = len(hand & set(order))
        per_case.append(row)
        # added files: is the directory among the top-50 files' directories?
        top_dirs = {p.rsplit("/", 1)[0] for p in order[:50]}
        for h in c["hand"]:
            if not h["existing"]:
                added_tot += 1
                d = h["path"].rsplit("/", 1)[0]
                if d in top_dirs:
                    added_dir["dir_in_top50"] += 1
                if d in set(r["tree_dirs"]):
                    added_dir["dir_exists"] += 1
        # derived: Cargo.lock predicted when a Cargo.toml is in the top 50
        for d in c["derived"]:
            derived[d["kind"] + " total"] += 1
            if d["kind"] == "lock" and any(p.endswith("Cargo.toml") for p in order[:50]):
                derived["lock predicted"] += 1
            if d["kind"] == "generated" and d["path"].rsplit("/", 1)[0] in top_dirs:
                derived["generated dir in top50"] += 1
    print(f"\nshortlist over {len(ev)} eval cases; {tot_e} existing hand-written files "
          f"({tot_all - tot_e} added by the fix, counted separately)")
    print("| Shortlist | Recall (existing) | Recall (all hand-written) | Precision | Cases with every existing file |")
    print("|---|---:|---:|---:|---:|")
    for k in KS:
        full = sum(1 for r in per_case if r[f"r{k}"] == r["n"])
        print(f"| top {k} | {hits[k]/tot_e:.3f} | {hits[k]/tot_all:.3f} | {statistics.mean(prec[k]):.3f} | {full}/{len(ev)} |")
    print("\nmissed existing files by kind (top 20 / 50 / 100), of all such files:")
    print("| Kind | Files | Missed @20 | Missed @50 | Missed @100 |")
    print("|---|---:|---:|---:|---:|")
    for kd, n in all_kind.most_common():
        print(f"| {kd} | {n} | {miss_kind[20][kd]} | {miss_kind[50][kd]} | {miss_kind[100][kd]} |")
    print(f"\nadded files: {added_tot}; directory exists at parent {added_dir['dir_exists']}, "
          f"directory among top-50 files' directories {added_dir['dir_in_top50']}")
    print("derived:", dict(derived))
    return {"per_case": per_case, "hits": dict(hits), "tot_existing": tot_e, "tot_all": tot_all,
            "miss_kind": {k: dict(v) for k, v in miss_kind.items()}, "kinds": dict(all_kind),
            "added": dict(added_dir), "added_total": added_tot, "derived": dict(derived)}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("cmd", choices=["prepare", "features", "train", "eval"])
    ap.add_argument("--repo", default=".")
    ap.add_argument("--dataset", required=True)
    ap.add_argument("--work", required=True)
    ap.add_argument("--cache")
    ap.add_argument("--eval", type=int, default=100)
    ap.add_argument("--workers", type=int, default=4)
    ap.add_argument("--fresh", action="store_true")
    ap.add_argument("--model", default=ff.MODEL_PATH)
    ap.add_argument("--trees", type=int, default=200)
    a = ap.parse_args()
    os.makedirs(a.work, exist_ok=True)
    {"prepare": cmd_prepare, "features": cmd_features, "train": cmd_train, "eval": cmd_eval}[a.cmd](a)


if __name__ == "__main__":
    main()
