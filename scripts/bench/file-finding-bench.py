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

KS = (20, 50, 100, 200, 300, 400)


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


_POS = {}


def split(cases, n_eval, max_hand=25):
    """Eval: the newest n_eval single-commit cases. Train: every case whose last fix
    commit is older than the oldest eval fix, so the scorer never sees the future."""
    if not _POS:
        _POS.update(ff.Index(ff.default_cache(".")).load(need_blobs=False).hist["pos"])
    singles = [c for c in cases if c.get("kind", "single") == "single" and len(c["hand"]) <= max_hand]
    ev = singles[:n_eval]
    oldest = min(_POS[c["commit"]] for c in ev)
    evn = {c["issue"] for c in ev}
    train = [c for c in cases if c["issue"] not in evn and _POS[c.get("last_commit", c["commit"])] < oldest]
    return ev, train


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
    ix.iface.ensure(a.repo, [(p, s) for s, p in union.items()])


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
    stage_report(split(cases, a.eval)[0], out)


STAGE2_TESTS = {
    "ref (stage 2)": lambda x: x.get("ref", 0) > 0,
    "iface (stage 2)": lambda x: x.get("iface", 0) > 0,
    "rule (stage 2)": lambda x: max(x.get("rule_f", 0), x.get("rule_d", 0)) >= 0.3,
    "crate (stage 2)": lambda x: x.get("crate_rank", 9) < 9,
}


def stage_report(cases, feats):
    """Recall per candidate stage alone and in union, on existing hand-written files."""
    tot = 0
    hit = Counter()
    pool_sizes = defaultdict(list)
    lat = defaultdict(list)
    names = list(ff.SOURCES)
    for c in cases:
        r = feats[c["issue"]]
        hand = set(r["hand"])
        tot += len(hand)
        sets = {s: set(r["src"].get(s, [])) for s in ff.SOURCES}
        for name, test in STAGE2_TESTS.items():
            sets[name] = {p for p, x in r["feats"].items() if test(x)}
            if name not in names and any(sets[name] for _ in [0]):
                names.append(name)
        for s, got in sets.items():
            pool_sizes[s].append(len(got))
            hit[s] += len(hand & got)
        allsrc = set(r["feats"])
        hit["union"] += len(hand & allsrc)
        pool_sizes["union"].append(len(allsrc))
        for k, v in r["timing"].items():
            if v is not None:
                lat[k].append(v)
    print(f"\nstage recall over {tot} existing hand-written files in {len(cases)} eval cases")
    print("| Stage | Recall | Median candidates |")
    print("|---|---:|---:|")
    for s in names + ["union"]:
        if pool_sizes[s]:
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
    # keep every positive and a random share of the negatives, reweighted
    rng = np.random.default_rng(7)
    keep = (y > 0) | (rng.random(len(y)) < a.neg_keep)
    sw = np.where(y > 0, 1.0, 1.0 / a.neg_keep)[keep]
    m = ff.fit_gbdt(X[keep], y[keep], trees=a.trees, sample_weight=sw)
    m["features"] = names
    return m


def stage2_features(a, cases, feats1, models_for):
    """Re-run each case's query and add the stage-2 features from its stage-1 model."""
    ix = ff.Index(a.cache or ff.default_cache(a.repo)).load()
    out = {}
    for i, c in enumerate(cases):
        r = run_case(ix, a.repo, c, os.environ.get("OPENROUTER_API_KEY"), keep_query=True)
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
    if a.reuse_stage2:  # features2.pkl already holds the cross-fitted stage-2 features
        feats2 = pickle.load(open(os.path.join(a.work, "features2.pkl"), "rb"))
        m1 = fit(train, feats, a)
        m2 = fit(train, feats2, a)
        save_model(a, train, m1, m2)
        return
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
    save_model(a, train, m1, m2)


def save_model(a, train, m1, m2):
    model = {"schema": "openagents.filefind.model.v1", "stage1": m1, "stage2": m2,
             "trained_on": {"cases": len(train), "issues": f"#{min(c['issue'] for c in train)}.."
                            f"#{max(c['issue'] for c in train)}"}}
    json.dump(model, open(a.model, "w"))
    print(f"trained on {len(train)} cases -> {a.model}")


def map_report(ev, feats, model, repo, title="map"):
    """Coverage of the map (top k + every file of the top c crates) against its size."""
    grid = [(k, c) for k in (50, 100, 150) for c in (0, 1, 2, 3, 4, 6)]
    hit, size = Counter(), defaultdict(list)
    tot = 0
    for case in ev:
        r = feats[case["issue"]]
        ranked = ff.rank(model, r["feats"])
        paths = list(ff.ls_tree(repo, case["parent"]))
        hand = set(r["hand"])
        tot += len(hand)
        for k, c in grid:
            m = [p for _, ps in ff.build_map(ranked, paths, k, c) for p in ps]
            hit[(k, c)] += len(hand & set(m))
            size[(k, c)].append(len(m))
    print(f"\n{title}: top k files + every file of the top c crates (existing hand-written files)")
    print("| k | c | Recall | Median paths | Mean paths | p90 paths |")
    print("|---:|---:|---:|---:|---:|---:|")
    for k, c in grid:
        s = sorted(size[(k, c)])
        print(f"| {k} | {c} | {hit[(k, c)]/tot:.3f} | {statistics.median(s):.0f} | {statistics.mean(s):.0f} | {s[int(0.9*(len(s)-1))]} |")
    return {f"{k},{c}": hit[(k, c)] / tot for k, c in grid}


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
    res["map"] = map_report(ev, f2, model["stage2"], a.repo)
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
            for k in (100, 400):
                if d["path"] in set(order[:k]) or (d["kind"] == "lock" and d["path"].endswith("Cargo.lock")
                                                    and any(p.endswith("Cargo.toml") for p in order[:50])):
                    derived[f"{d['kind']} in top {k} or by the lock rule"] += 1
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


# ---------------------------------------------------------------- model stages

JEV_URL = "https://api.typesafe.ai/v1/systemone"
PLAN_URL = "https://openrouter.ai/api/v1/chat/completions"


def post(url, key, body, timeout=180):
    import urllib.request
    r = urllib.request.Request(url, data=json.dumps(body).encode(), headers={
        "Authorization": f"Bearer {key}", "Content-Type": "application/json",
        "User-Agent": "openagents-filefind-bench/1"})
    for attempt in range(4):
        try:
            with urllib.request.urlopen(r, timeout=timeout) as x:
                return json.load(x)
        except Exception:
            if attempt == 3:
                raise
            time.sleep(2 ** attempt)


def heads(repo, rev, paths, n=1500):
    tree = ff.ls_tree(repo, rev)
    shas = [tree[p] for p in paths if p in tree]
    got = ff.cat_heads(repo, shas, n * 2)
    return {p: (got.get(tree.get(p)) or "")[:n] for p in paths}


def jev_judge(key, c, paths, head):
    """Jev noul per file, batched 25 files per request (the relevance-bench prompt)."""
    probs = {}
    issue = f"ISSUE #{c['issue']}: {c['title']}\n\n{c['body'].strip()[:6000]}\n"
    chunks = [paths[i:i + 25] for i in range(0, len(paths), 25)]

    def one(chunk):
        parts = [issue, "CANDIDATE FILES:\n"]
        qs = {}
        for i, p in enumerate(chunk, 1):
            parts.append(f"FILE f{i}: {p}\n```\n{head.get(p, '')}\n```\n")
            qs[f"f{i}"] = {"type": "noul",
                           "instructions": f"Will the change that resolves this issue edit file f{i} ({p})?"}
        d = post(JEV_URL, key, {"model": "jev-latest", "state": "\n".join(parts), "questions": qs})
        ans = d.get("answers", {})
        return {p: ans.get(f"f{i}", {}).get("noul") for i, p in enumerate(chunk, 1)}, d.get("usage", {})
    usage = Counter()
    with ThreadPoolExecutor(8) as ex:
        for got, u in ex.map(one, chunks):
            probs.update(got)
            for k, v in (u or {}).items():
                if isinstance(v, (int, float)):
                    usage[k] += v
    return probs, usage


def cmd_judge(a):
    """Jev re-judges the files the scorer is unsure about: ranks a.band_lo..a.band_hi."""
    key = os.environ.get("TYPESAFE_API_KEY") or sys.exit("TYPESAFE_API_KEY is not set")
    cases = load(a)
    model = json.load(open(a.model))
    ev, _ = split(cases, a.eval)
    f2 = pickle.load(open(os.path.join(a.work, "features2.pkl"), "rb"))
    out_path = os.path.join(a.work, "judge.json")
    done = json.load(open(out_path)) if os.path.exists(out_path) else {}
    for c in ev[:a.limit or None]:
        if str(c["issue"]) in done:
            continue
        ranked = ff.rank(model["stage2"], f2[c["issue"]]["feats"])
        band = [p for p, _ in ranked[a.band_lo:a.band_hi]]
        t0 = time.perf_counter()
        head = heads(a.repo, c["parent"], band)
        probs, usage = jev_judge(key, c, band, head)
        done[str(c["issue"])] = {"probs": probs, "latency_s": time.perf_counter() - t0, "usage": usage}
        json.dump(done, open(out_path, "w"))
        print(f"#{c['issue']}: judged {len(band)} in {done[str(c['issue'])]['latency_s']:.1f}s", file=sys.stderr)


def combine(ranked, probs, keep, alpha):
    """Keep the top `keep` as ranked; order the rest by scorer logit + alpha * Jev logit."""
    def lg(p):
        p = min(max(p, 1e-4), 1 - 1e-4)
        return math.log(p / (1 - p))
    head = ranked[:keep]
    rest = []
    for p, s in ranked[keep:]:
        j = probs.get(p)
        rest.append((p, lg(s) + (alpha * lg(j) if j is not None else alpha * lg(0.3))))
    rest.sort(key=lambda x: -x[1])
    return head + rest


def cmd_judge_eval(a):
    cases = load(a)
    model = json.load(open(a.model))
    ev, _ = split(cases, a.eval)
    f2 = pickle.load(open(os.path.join(a.work, "features2.pkl"), "rb"))
    jd = json.load(open(os.path.join(a.work, "judge.json")))
    ev = [c for c in ev if str(c["issue"]) in jd]
    print(f"judged cases: {len(ev)}; median Jev latency "
          f"{statistics.median(v['latency_s'] for v in jd.values()):.2f}s")
    print("\n== stage 2 only, same cases")
    evaluate(ev, f2, model["stage2"], a)
    for alpha in (0.5, 1.0, 2.0):
        judged = {c["issue"]: combine(ff.rank(model["stage2"], f2[c["issue"]]["feats"]),
                                      jd[str(c["issue"])]["probs"], a.band_lo, alpha) for c in ev}
        print(f"\n== with Jev on ranks {a.band_lo}..{a.band_hi} for every query, alpha {alpha}")
        evaluate(ev, f2, model["stage2"], a, judged=judged)
    # gate: call Jev only when the scorer itself expects to miss files in its top 50
    for gate in (0.5, 0.6, 0.7):
        judged, n = {}, 0
        for c in ev:
            ranked = ff.rank(model["stage2"], f2[c["issue"]]["feats"])
            if ff.sureness(ranked) < gate:
                n += 1
                judged[c["issue"]] = combine(ranked, jd[str(c["issue"])]["probs"], a.band_lo, 1.0)
        print(f"\n== Jev only when sureness < {gate}: {n}/{len(ev)} queries call the model")
        evaluate(ev, f2, model["stage2"], a, judged=judged)


PLAN_PROMPT = """You are planning the code change that resolves a GitHub issue in the repository below.

{issue}

A file finder ranked these existing files as likely relevant (best first):
{ranked}

Directories and files of the most relevant areas:
{listing}

Write the change plan as a short list of steps. For every step, name every file the step edits or creates:
existing files by their exact path, new files by the path they should have. Include the files a careful
engineer would also touch: tests, module declarations (mod.rs, lib.rs), Cargo.toml, docs, READMEs,
INVARIANTS.md, registries and routers. Reply with JSON only:
{{"steps": [{{"step": "...", "files": ["path", ...]}}]}}"""


def cmd_plan(a):
    """Stage 4 prototype: a model writes the change plan; each step's files join the list."""
    key = os.environ.get("OPENROUTER_API_KEY") or sys.exit("OPENROUTER_API_KEY is not set")
    cases = load(a)
    model = json.load(open(a.model))
    ev, _ = split(cases, a.eval)
    f2 = pickle.load(open(os.path.join(a.work, "features2.pkl"), "rb"))
    out_path = os.path.join(a.work, f"plan-{a.plan_model.replace('/', '_')}.json")
    done = json.load(open(out_path)) if os.path.exists(out_path) else {}
    todo = [c for c in ev[:a.limit or None] if str(c["issue"]) not in done]

    def one(c):
        ranked = ff.rank(model["stage2"], f2[c["issue"]]["feats"])[:a.plan_top]
        tree = ff.ls_tree(a.repo, c["parent"])
        groups = Counter()
        for p, s in ranked[:30]:
            groups[ff.group(p)] += s
        listing = []
        for g, _ in groups.most_common(4):
            fs = sorted(p for p in tree if p.startswith(g + "/"))
            listing.append(f"{g}/ ({len(fs)} files)\n" + "\n".join("  " + p for p in fs[:150]))
        prompt = PLAN_PROMPT.format(
            issue=f"ISSUE #{c['issue']}: {c['title']}\n\n{c['body'].strip()[:8000]}",
            ranked="\n".join(p for p, _ in ranked), listing="\n".join(listing))
        t0 = time.perf_counter()
        d = post(PLAN_URL, key, {"model": a.plan_model, "messages": [{"role": "user", "content": prompt}],
                                 "max_tokens": 3000, "temperature": 0})
        text = d["choices"][0]["message"]["content"]
        m = text[text.find("{"):text.rfind("}") + 1]
        try:
            plan = json.loads(m)
        except Exception:
            plan = {"steps": [], "raw": text[:2000]}
        return c["issue"], {"plan": plan, "latency_s": time.perf_counter() - t0, "usage": d.get("usage")}
    with ThreadPoolExecutor(6) as ex:
        for n, r in ex.map(one, todo):
            done[str(n)] = r
            json.dump(done, open(out_path, "w"))
            print(f"#{n}: {len(r['plan'].get('steps', []))} steps, {r['latency_s']:.0f}s", file=sys.stderr)
    # measure
    ev = [c for c in ev if str(c["issue"]) in done]
    tot_e = tot_add = 0
    hit = Counter()
    planned_n = []
    jd_path = os.path.join(a.work, "judge.json")
    jd = json.load(open(jd_path)) if os.path.exists(jd_path) else {}
    for c in ev:
        ranked = [p for p, _ in ff.rank(model["stage2"], f2[c["issue"]]["feats"])]
        if str(c["issue"]) in jd:
            jranked = [p for p, _ in combine(ff.rank(model["stage2"], f2[c["issue"]]["feats"]),
                                             jd[str(c["issue"])]["probs"], a.band_lo, 1.0)]
        else:
            jranked = ranked
        pf = {f.strip().lstrip("./") for s in done[str(c["issue"])]["plan"].get("steps", [])
              for f in s.get("files", []) if isinstance(f, str)}
        planned_n.append(len(pf))
        ex_ = {h["path"] for h in c["hand"] if h["existing"]}
        add = {h["path"] for h in c["hand"] if not h["existing"]}
        tot_e += len(ex_)
        tot_add += len(add)
        tree_set = set(f2[c["issue"]]["feats"]) | set(ranked)
        plan_existing = [p for p in pf if p in tree_set or p in ex_]
        merged = plan_existing + [p for p in jranked if p not in set(plan_existing)]
        for k in KS:
            hit[f"e{k}"] += len(ex_ & set(ranked[:k]))
            hit[f"e{k}+plan"] += len(ex_ & (set(ranked[:k]) | pf))
            hit[f"e{k}merged"] += len(ex_ & set(merged[:k]))
        hit["plan alone"] += len(ex_ & pf)
        hit["added by plan"] += len(add & pf)
    print(f"\nplan stage ({a.plan_model}) on {len(ev)} eval cases: {tot_e} existing, {tot_add} added files; "
          f"median {statistics.median(planned_n)} files planned")
    print("| Shortlist | Recall (existing), finder | finder top k + plan files | plan files first, then finder (+Jev), cut at k |")
    print("|---|---:|---:|---:|")
    for k in KS:
        print(f"| top {k} | {hit[f'e{k}']/tot_e:.3f} | {hit[f'e{k}+plan']/tot_e:.3f} | {hit[f'e{k}merged']/tot_e:.3f} |")
    print(f"plan alone recall (existing) {hit['plan alone']/tot_e:.3f}; added files named exactly "
          f"{hit['added by plan']}/{tot_add}")


def cmd_check(a):
    """Run the shipped two-stage finder on every case of a small dataset (e.g. fresh fixes)."""
    ix = ff.Index(a.cache or ff.default_cache(a.repo)).load()
    model = json.load(open(a.model))
    key = os.environ.get("OPENROUTER_API_KEY")
    print("| Issue | Hand-written files (existing) | " + " | ".join(f"@{k}" for k in KS) + " | Missed at 400 |")
    print("|---|---:|" + "---:|" * len(KS) + "---|")
    tot = Counter()
    for c in load(a):
        r = run_case(ix, a.repo, c, key, keep_query=True)
        q = r.pop("query")
        ranked, _ = ff.rank_two_stage(model, q, r["feats"])
        order = [p for p, _ in ranked]
        hand = set(r["hand"])
        row = [len(hand & set(order[:k])) for k in KS]
        for k, v in zip(KS, row):
            tot[k] += v
        tot["n"] += len(hand)
        miss = sorted(hand - set(order[:400]))
        print(f"| #{c['issue']} | {len(c['hand'])} ({len(hand)}) | " + " | ".join(map(str, row)) +
              " | " + ", ".join(f"`{m}`" for m in miss) + " |")
    print("| all | " + str(tot["n"]) + " | " + " | ".join(f"{tot[k]/tot['n']:.2f}" for k in KS) + " | |")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("cmd", choices=["prepare", "features", "train", "eval", "judge", "judge-eval", "plan", "check"])
    ap.add_argument("--repo", default=".")
    ap.add_argument("--dataset", required=True)
    ap.add_argument("--work", required=True)
    ap.add_argument("--cache")
    ap.add_argument("--eval", type=int, default=100)
    ap.add_argument("--workers", type=int, default=4)
    ap.add_argument("--fresh", action="store_true")
    ap.add_argument("--model", default=ff.MODEL_PATH)
    ap.add_argument("--trees", type=int, default=200)
    ap.add_argument("--neg-keep", type=float, default=0.3)
    ap.add_argument("--reuse-stage2", action="store_true")
    ap.add_argument("--band-lo", type=int, default=30)
    ap.add_argument("--band-hi", type=int, default=150)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--plan-model", default="anthropic/claude-sonnet-5.5")
    ap.add_argument("--plan-top", type=int, default=60)
    a = ap.parse_args()
    os.makedirs(a.work, exist_ok=True)
    {"prepare": cmd_prepare, "features": cmd_features, "train": cmd_train, "eval": cmd_eval,
     "judge": cmd_judge, "judge-eval": cmd_judge_eval, "plan": cmd_plan, "check": cmd_check}[a.cmd](a)


if __name__ == "__main__":
    main()
