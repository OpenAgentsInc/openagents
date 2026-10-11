#!/usr/bin/env python3
"""End-to-end file-finding bench for scripts/filefind/filefind.py (#11210).

Every case is replayed at its fix's parent commit: the tree, the co-change
history, and the past issues are all cut at that commit, so nothing the fix
did leaks into the query. The newest --eval cases are the bench; the older
cases train the scorer.

Steps (each writes into --work, a scratch directory):

  prepare   embed every blob of every case's parent tree (cached per blob)
  features  run the candidate stages per case; per-stage recall and latency
  train     fit the scorer on the train cases -> --model (a candidate file; the
            active scripts/filefind/model.json changes only through
            scripts/filefind/ranker_gate.py promote)
  eval      rank the eval cases; recall / precision at 20, 50, 100; misses.
            Refuses a model trained on any eval case (--allow-overlap labels
            the numbers as development instead)
  compare   the active model against a candidate on the eval cases under the
            gate's frozen plan; writes the receipt promote needs
  judge     ask our decision API (batch nouls; connected Pylons first, #11225)
            about the files the scorer is unsure of
  plan      stage 4: a model writes the change plan; map steps to files

    export GOOGLE_APPLICATION_CREDENTIALS=...  # embeddings on Vertex AI (filefind.embed_key)
    export OPENROUTER_API_KEY=...   # the plan model; embeddings when no Google credential
    export TYPESAFE_API_KEY=...     # judge step: Jev directly; without a key,
    # our decision API at openagents.com, keyless (OPENAGENTS_DECISIONS=pylon
    # asks its Pylons first; OPENAGENTS_DECISIONS_URL names another)
    python3 scripts/bench/file-finding-bench.py prepare --repo . --dataset D --work W
    python3 scripts/bench/file-finding-bench.py features --repo . --dataset D --work W
    python3 scripts/bench/file-finding-bench.py train --dataset D --work W
    python3 scripts/bench/file-finding-bench.py eval --dataset D --work W
"""
import argparse, json, math, os, pickle, re, statistics, sys, time
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "filefind"))
import filefind as ff  # noqa: E402
import ranker_gate as gate  # noqa: E402
import cards as fc  # noqa: E402

# Cards (#11249): with --cards every replay runs the card stage, with the issue's Jev
# profile from the cache (`profiles`) and the tower that never saw the case's issue.
CARDS = None
CORPUS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "crates", "gym", "suites",
                      "file-relevance-v1")


class BenchCards:
    def __init__(self, a, ix=None):
        cache = ff.cache_for(a)
        self.store = fc.CardStore(cache, ix.key if ix else None).load()
        if not self.store.vocab or (a.cmd not in ("profiles", "cards") and self.store.tags is None):
            sys.exit("--cards: this cache has no card vocabulary or compiled tags (cards.py vocab / index)")
        self.profiles = fc.Profiles(cache, self.store.vocab)
        self.towers = json.load(open(a.towers)) if a.towers and os.path.exists(a.towers) else None
        self.partition = corpus_partition()
        self.no_tower = a.no_tower

    def tower_for(self, issue):
        """Out of sample for every case: a training-partition issue gets the fold tower
        that did not train on it; every other issue the tower trained on all of them."""
        t = self.towers
        if not t or self.no_tower:
            return None
        if self.partition.get(issue) == "training":
            return t["B"] if issue % 2 == 0 else t["A"]
        return t["all"]

    def ctx(self, c):
        prof = self.profiles.get(c["title"], c["body"], fetch=False)
        return {"store": self.store, "profile": prof, "tower": self.tower_for(c["issue"]) if prof else None,
                "tower_cache": None}


def corpus_partition():
    out = {}
    with open(os.path.join(CORPUS, "issues.tsv")) as f:
        next(f)
        for line in f:
            n, part = line.split("\t")[:2]
            out[int(n)] = part
    return out

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
    ix = ff.Index(ff.cache_for(a)).load()
    key = ix.key or ff.embed_key()
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
    ix.feedback = {}  # run feedback is newer than the case: never use it in a replay
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
        qvec = ff.embed([ff.issue_text(c["title"], c["body"])], key, task="RETRIEVAL_QUERY")[0]
        timing["embed_query"] = time.perf_counter() - t0
    q = ff.Query(ix, repo, c["parent"], tree, cutoff=pos, exclude_issue=c["issue"], timing=timing,
                 cards=CARDS.ctx(c) if CARDS else None)
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
    ix = ff.Index(ff.cache_for(a)).load()
    key = ix.key
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
    if names is None and a.drop:
        allnames = sorted({k for c in cases for f in feats[c["issue"]]["feats"].values() for k in f})
        names = [n for n in allnames if not any(n.startswith(d) for d in a.drop.split(","))]
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
    ix = ff.Index(ff.cache_for(a)).load()
    out = {}
    for i, c in enumerate(cases):
        r = run_case(ix, a.repo, c, ix.key, keep_query=True)
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
    if a.all:  # the production ranker: every case, eval included (its stage-2 inputs are out of sample)
        train = cases
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
    if os.path.abspath(a.model) == os.path.abspath(ff.MODEL_PATH):
        sys.exit("train writes a candidate, never the active model: pass --model elsewhere and "
                 "promote it with scripts/filefind/ranker_gate.py (LEARN-01)")
    issues = sorted({c["issue"] for c in train})
    with open(a.dataset, "rb") as f:
        dataset_digest = "sha256:" + __import__("hashlib").sha256(f.read()).hexdigest()
    model = {"schema": "openagents.filefind.model.v1", "stage1": m1, "stage2": m2,
             **({"cards": cards_card(a)} if CARDS else {}),
             "trained_on": {"cases": len(train), "issues": f"#{min(issues)}..#{max(issues)}",
                            "issue_list": issues, "dataset": dataset_digest,
                            "includes_eval": bool(a.all)}}
    tmp = f"{a.model}.{os.getpid()}.tmp"
    with open(tmp, "w") as f:
        json.dump(model, f)
    os.replace(tmp, a.model)
    print(f"trained on {len(train)} cases -> {a.model}")


def cards_card(a):
    """The model card's cards block: the vocabulary its tag features were trained on and
    the distilled tower that ran on its eval cases and runs at query time."""
    t = CARDS.towers["all"] if CARDS.towers and not CARDS.no_tower else None
    return {"vocab_digest": CARDS.store.vocab["digest"], "vocab_rev": CARDS.store.vocab["rev"],
            "card_model": fc.CARD_MODEL, "profile_model": fc.JEV_MODEL, "tower": t,
            "source": "openagents#11249"}


def cmd_cards(a):
    """Card, embed and compile every blob of every replayed parent tree (and HEAD)."""
    cases = load(a)
    revs = sorted({c["parent"] for c in cases}) + ["HEAD"]
    blobs = {}
    with ThreadPoolExecutor(8) as ex:
        for t in ex.map(lambda r: ff.ls_tree(a.repo, r), revs):
            for p, s_ in t.items():
                blobs.setdefault(s_, p)
    st = CARDS.store
    print(json.dumps(st.make(a.repo, blobs, workers=64)))
    print(json.dumps(st.embed(a.repo, blobs, tokens=ff.TokenIndex(ff.cache_for(a)))))
    st.compile_tags()


def cmd_teacher(a):
    """Clef's answer on every training-partition item of file-relevance-v1, through the
    fusion card's door (answers count only from the card's Clef artifact and head). They
    are written as a teacher field (--teacher JSONL); labels stay the corpus outcomes."""
    import gzip
    cases = {c["issue"]: c for c in load(a)}
    card = json.load(open(a.candidate))
    door = ff.ClefDoor(card["fusion"], os.path.join(a.work, "clef-teacher-cache.jsonl"))
    rows = [l.rstrip("\n").split("\t") for l in gzip.open(os.path.join(CORPUS, "items.tsv.gz"), "rt")]
    h = rows[0]
    rows = [dict(zip(h, r)) for r in rows[1:] if r[1] == "training" and int(r[0]) in cases]
    heads = ff.clef_heads(a.repo, [r["blob"] for r in rows])
    jobs = [(r, ff.clef_state(int(r["issue"]), cases[int(r["issue"])]["title"], cases[int(r["issue"])]["body"],
                              r["path"], heads[r["blob"]])) for r in rows if heads.get(r["blob"]) is not None]

    def one(j):
        for attempt in range(8):
            try:
                return j[0], door.ask(j[1], timeout=120)
            except Exception:  # noqa: BLE001 - a benched pylon: wait and ask again
                time.sleep(20 * (attempt + 1))
        return j[0], None
    with open(a.teacher, "a") as out, ThreadPoolExecutor(a.clef_workers) as ex:
        for r, p in ex.map(one, jobs):
            if p is not None:
                out.write(json.dumps({"issue": int(r["issue"]), "path": r["path"], "blob": r["blob"],
                                      "label": r["label"], "clef_p": p}) + "\n")
    print(f"teacher answers for {len(jobs)} items -> {a.teacher}")


def cmd_profiles(a):
    """Jev profiles of every case's issue (and the corpus training issues), cached."""
    cases = load(a)
    t0 = time.perf_counter()
    lat = []

    def one(c):
        t = time.perf_counter()
        p = CARDS.profiles.get(c["title"], c["body"])
        lat.append(time.perf_counter() - t)
        return p is not None
    with ThreadPoolExecutor(a.workers) as ex:
        ok = sum(ex.map(one, cases))
    print(f"profiles: {ok}/{len(cases)} in {time.perf_counter() - t0:.0f}s")


def cmd_tower(a):
    """Distil the two-tower projection on the file-relevance-v1 training partition (#11215):
    the corpus items (outcome labels, Clef's answer as a teacher field where asked) plus,
    per issue, every existing fix file and 30 other files of the parent tree as outcome
    pairs. Trains `all` and the two issue-parity folds `A` (even) and `B` (odd)."""
    import gzip, random
    cases = {c["issue"]: c for c in load(a)}
    ix = ff.Index(ff.cache_for(a)).load()
    st = CARDS.store
    teacher = {}
    if a.teacher and os.path.exists(a.teacher):
        for line in open(a.teacher):
            r = json.loads(line)
            teacher[(r["issue"], r["path"])] = ff._logit(r["clef_p"])
    rows = [l.rstrip("\n").split("\t") for l in gzip.open(os.path.join(CORPUS, "items.tsv.gz"), "rt")]
    h = rows[0]
    items = defaultdict(list)
    for r in rows[1:]:
        d = dict(zip(h, r))
        if d["partition"] == "training":
            items[int(d["issue"])].append(d)
    isx = ix.issues
    inum = {int(n): i for i, n in enumerate(isx["numbers"])}
    Q, D, Y, T, G = [], [], [], [], []
    tags = st.tags
    rng = random.Random(11249)
    skipped = Counter()
    for n in sorted(items):
        c = cases.get(n)
        prof = CARDS.profiles.get(c["title"], c["body"], fetch=False) if c else None
        if c is None or prof is None or n not in inum:
            skipped["issue"] += 1
            continue
        qin = fc.tower_inputs_query(isx["vecs"][inum[n]], fc.profile_vector(st.vocab, prof))
        tree = ff.ls_tree(a.repo, c["parent"])
        pairs = {(d["path"], d["blob"]): (int(d["label"] == "true"), teacher.get((n, d["path"]), float("nan")))
                 for d in items[n]}
        hand = {x["path"] for x in c["hand"] if x["existing"]}
        for p in hand:
            if p in tree:
                pairs.setdefault((p, tree[p]), (1, float("nan")))
        others = [p for p in tree if p not in hand]
        for p in rng.sample(others, min(30, len(others))):
            pairs.setdefault((p, tree[p]), (0, float("nan")))
        for (p, sha), (y, t) in pairs.items():
            cr = st.rows.get(sha, st.rows.get(sha.encode()))
            br = ix.blob_rows.get(sha.encode())
            tr = tags["rows"].get(sha.encode())
            if cr is None or br is None or tr is None:
                skipped["file"] += 1
                continue
            Q.append(qin)
            D.append(fc.tower_inputs_doc(st.vecs[cr:cr + 1], ix.blob_mat[br:br + 1], tags["hot"][tr:tr + 1])[0])
            Y.append(y)
            T.append(t)
            G.append(n)
    Q, D, Y, T, G = (np.array(Q, np.float32), np.array(D, np.float32), np.array(Y, np.float32),
                     np.array(T, np.float32), np.array(G))
    print(f"tower rows {len(Y)} ({int(Y.sum())} positive, {int(np.isfinite(T).sum())} with Clef), "
          f"{len(set(G.tolist()))} issues; skipped {dict(skipped)}", file=sys.stderr)
    out = {"source": "file-relevance-v1 training partition", "rows": int(len(Y)),
           "teacher_rows": int(np.isfinite(T).sum()), "issues": sorted(set(G.tolist()))}
    for name, sel in (("all", np.ones(len(G), bool)), ("A", G % 2 == 0), ("B", G % 2 == 1)):
        t0 = time.time()
        out[name] = fc.train_tower(Q[sel], D[sel], Y[sel], T[sel], G[sel], k=a.tower_k, lam=a.tower_lam)
        out[name]["seconds"] = round(time.time() - t0, 1)
        print(f"tower {name}: val loss {out[name]['val_loss']:.4f} in {out[name]['epochs_run']} epochs", file=sys.stderr)
    with open(a.towers, "w") as f:
        json.dump(out, f)
    print(f"towers -> {a.towers}")


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
    seen = gate.overlap(model, [c["issue"] for c in ev])
    evidence = "held-out"
    if seen:
        if not a.allow_overlap:
            sys.exit(f"{a.model} was trained on {len(seen)} of the {len(ev)} eval cases (e.g. #{seen[0]}): "
                     "its numbers would not be held-out. Evaluate the work dir's held-out model, or pass "
                     "--allow-overlap to label them development")
        evidence = f"development (trained on {len(seen)} of {len(ev)} eval cases)"
        print(f"== DEVELOPMENT NUMBERS: {evidence}")
    f1 = pickle.load(open(os.path.join(a.work, "features.pkl"), "rb"))
    f2 = pickle.load(open(os.path.join(a.work, "features2.pkl"), "rb"))
    print("== stage 1 candidates")
    stage_report(ev, f2)
    print("\n== stage 1 scorer only")
    evaluate(ev, f1, model["stage1"], a)
    print("\n== stage 1 + stage 2 (propagation)")
    res = evaluate(ev, f2, model["stage2"], a)
    res["map"] = map_report(ev, f2, model["stage2"], a.repo)
    res["evidence"] = evidence
    res["model_digest"] = gate.digest_file(a.model)
    json.dump(res, open(os.path.join(a.work, "eval.json"), "w"), indent=1)


def fused_rankings(a, ev, feats, card):
    """issue -> the card's ranking with its Clef fusion applied; exits when any
    case could not be fused (the comparison needs every case measured)."""
    door = ff.ClefDoor(card["fusion"], os.path.join(a.work, "clef-cache.jsonl"))
    out, t0 = {}, time.perf_counter()
    for i, c in enumerate(ev):
        ranked = ff.rank(card["stage2"], feats[c["issue"]]["feats"])
        tree = ff.ls_tree(a.repo, c["parent"])
        for attempt in range(6):  # a pylon benched for a minute, or busy: wait and ask again
            fused, info = ff.clef_fuse(card["fusion"], ranked, c["issue"], c["title"], c["body"], tree, a.repo,
                                       door=door, budget=3600, workers=a.clef_workers)
            if info["fused"]:
                break
            print(f"#{c['issue']}: Clef fusion missed ({info['why']}); asking again in 90 s", file=sys.stderr)
            time.sleep(90)
        if not info["fused"]:
            sys.exit(f"#{c['issue']}: Clef fusion failed ({info['why']}); no receipt written")
        out[c["issue"]] = fused
        print(f"fused {i + 1}/{len(ev)} #{c['issue']}: {info['answered']} answers in {info['seconds']} s "
              f"({time.perf_counter() - t0:.0f} s total)", file=sys.stderr)
    return out


def cmd_fusion_card(a):
    """A candidate card: --model's numpy ranker (its stages and training record,
    unchanged) plus the frozen Clef fusion of #11217 over its top --clef-k. It has
    no learned parameter of its own, so its overlap with the eval cases is the
    base model's."""
    card = json.load(open(a.model))
    if card.get("fusion"):
        sys.exit("the base model already carries a fusion")
    base_digest = gate.digest_file(a.model)
    card["fusion"] = {"kind": ff.FUSION_KIND, "k": a.clef_k, "door": a.clef_door, "model": "clef-flash",
                      "question": "Is this file relevant to solving the issue?",
                      "artifact_digest": a.clef_artifact, "head_digest": a.clef_head,
                      "budget_s": 20, "workers": 8, "base_digest": base_digest,
                      "source": "openagents#11217 (e1f070d566): numpy logit + Clef logit in the numpy top K"}
    tmp = os.path.join(a.work, "fusion-card.json")
    with open(tmp, "w") as f:
        json.dump(card, f, indent=1, sort_keys=True)
    print(gate.immutable_copy(tmp, os.path.join(a.work, "candidates")))


def gate_rows(ev, feats, model, judged=None):
    """Per-case rows for the gate: hits at 20/50/100 and the Brier score of the
    top 100 confidences against whether each file is in the fix."""
    rows = []
    for c in ev:
        r = feats[c["issue"]]
        ranked = judged[c["issue"]] if judged else ff.rank(model, r["feats"])
        hand = set(r["hand"])
        order = [p for p, _ in ranked]
        row = {"issue": c["issue"], "n": len(hand)}
        for k in (20, 50, 100):
            row[f"r{k}"] = len(hand & set(order[:k]))
        top = ranked[:100]
        row["brier_top100"] = sum((s - (p in hand)) ** 2 for p, s in top) / max(1, len(top))
        rows.append(row)
    return rows


def cmd_compare(a):
    """The active model (--baseline) against --candidate on the eval cases; the
    receipt goes to WORK/compare-<candidate digest>.json. Never promotes."""
    cases = load(a)
    ev, _ = split(cases, a.eval)
    issues = [c["issue"] for c in ev]
    base, cand = json.load(open(a.baseline)), json.load(open(a.candidate))
    seen = gate.overlap(cand, issues)
    if seen:
        sys.exit(f"refused: the candidate was trained on {len(seen)} of the {len(ev)} eval cases "
                 f"(e.g. #{seen[0]}); a comparison on them is not held-out")
    base_seen = gate.overlap(base, issues)
    if base_seen:
        print(f"note: the baseline was trained on {len(base_seen)} of the {len(ev)} eval cases; its numbers "
              "are optimistic, which only makes the gate harder to pass", file=sys.stderr)
    f2 = pickle.load(open(os.path.join(a.work, "features2.pkl"), "rb"))
    judged = fused_rankings(a, ev, f2, cand) if cand.get("fusion") else None
    rec = gate.receipt(a.baseline, a.candidate, issues, gate_rows(ev, f2, base["stage2"]),
                       gate_rows(ev, f2, cand["stage2"], judged), base_seen)
    if cand.get("fusion"):
        rec["fusion"] = cand["fusion"]
    out = os.path.join(a.work, f"compare-{rec['candidate']['digest'].split(':')[1][:12]}.json")
    with open(out, "w") as f:
        json.dump(rec, f, indent=1, sort_keys=True)
    for name, m in rec["metrics"].items():
        print(f"{name}: baseline {m['baseline']:.4f} candidate {m['candidate']:.4f} "
              f"diff {m['diff']:+.4f} (SE {m['se']:.4f})")
    print(("PASS" if rec["pass"] else "FAIL: " + "; ".join(rec["why"])) + f" -> {out}")


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
# Our decision API (#11225): connected Pylons first, then Gemini on Vertex.
OURS_URL = os.environ.get("OPENAGENTS_DECISIONS_URL", "https://openagents.com/api").rstrip("/") + "/v1/systemone"


def decision_door():
    """(url, key): Jev directly with a TypeSafe key, else our API keyless (#11225)."""
    key = os.environ.get("TYPESAFE_API_KEY")
    if key and os.environ.get("OPENAGENTS_DECISIONS") != "pylon":
        return JEV_URL, key
    return OURS_URL, None
PLAN_URL = "https://openrouter.ai/api/v1/chat/completions"


def post(url, key, body, timeout=180):
    import urllib.request
    headers = {"Content-Type": "application/json", "User-Agent": "openagents-filefind-bench/1"}
    if key:
        headers["Authorization"] = f"Bearer {key}"
    r = urllib.request.Request(url, data=json.dumps(body).encode(), headers=headers)
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
        d = post(key[0], key[1], {"model": "jev-latest", "state": "\n".join(parts), "questions": qs})
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
    key = decision_door()
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


RERANK_PROMPT = """You are choosing which files a code change needs. Here is a GitHub issue, then {n}
files of this repository that a file finder ranked for it (best guess first), each with a
short summary.

{issue}

FILES
{files}

Pick the files the change resolving this issue will most likely edit or must read: the code
that changes, its tests, the modules and registries that wire it in, docs kept in sync, and
every client or consumer of what changes. Return up to {k} file numbers, most likely first,
as JSON only: {{"files": [12, 3, ...]}}"""


def summary_line(text):
    for line in (text or "").splitlines():
        t = line.strip().lstrip("/!#*-> ").strip()
        if len(t) >= 12 and not t.startswith(("use ", "import ", "package ", "[", "{", "<")):
            return t[:80]
    return ""


def planner_rerank(a, key, c, ranked, n=1000, k=150):
    """The planner re-orders the finder's top n: its picks first, then the rest as ranked."""
    top = [p for p, _ in ranked[:n]]
    tree = ff.ls_tree(a.repo, c["parent"])
    heads = ff.cat_heads(a.repo, [tree[p] for p in top if p in tree], n=600, cap=600)
    lines = [f"{i}. {p} — {summary_line(heads.get(tree.get(p)))}" for i, p in enumerate(top, 1)]
    prompt = RERANK_PROMPT.format(n=len(top), k=k, files="\n".join(lines),
                                  issue=f"ISSUE #{c['issue']}: {c['title']}\n\n{c['body'].strip()[:8000]}")
    t0 = time.perf_counter()
    if a.plan_backend == "claude-cli":  # this computer's Claude Code login
        import subprocess as sp
        r = sp.run(["claude", "-p", "--model", a.plan_cli_model, "--output-format", "json", "--max-turns", "1"],
                   input=prompt, capture_output=True, text=True, timeout=600)
        d = json.loads(r.stdout or "{}")
        text = d.get("result") or ""
        usage = {"cost": d.get("total_cost_usd"), **(d.get("usage") or {})}
    else:
        d = post(PLAN_URL, key, {"model": a.plan_model, "messages": [{"role": "user", "content": prompt}],
                                 "max_tokens": 4000, "temperature": 0})
        text = d["choices"][0]["message"]["content"]
        usage = d.get("usage")
    tail = text[text.find('"files"'):] if '"files"' in text else text
    picks = [int(x) for x in re.findall(r"\d+", tail)]  # tolerant of a cut-off or chatty reply
    chosen = []
    for i in picks:
        if isinstance(i, int) and 1 <= i <= len(top) and top[i - 1] not in chosen:
            chosen.append(top[i - 1])
    rest = [p for p, _ in ranked if p not in set(chosen)]
    return chosen + rest, {"latency_s": time.perf_counter() - t0, "usage": usage, "picked": len(chosen),
                           "raw": text[:400]}


def cmd_rerank(a):
    """Planner re-rank on the finder's top 1,000, for unsure queries (sureness < 0.7)."""
    key = os.environ.get("OPENROUTER_API_KEY")
    model = json.load(open(a.model))
    cases = load(a)
    f2p = os.path.join(a.work, "features2.pkl")
    f2 = pickle.load(open(f2p, "rb")) if os.path.exists(f2p) and not a.live else {}
    if f2:
        cases, _ = split(cases, a.eval)
    ix = None if f2 else ff.Index(ff.cache_for(a)).load()
    out_path = os.path.join(a.work, f"rerank-{os.path.basename(a.dataset)}.json")
    done = json.load(open(out_path)) if os.path.exists(out_path) else {}

    def ranking(c):
        if f2:
            return ff.rank(model["stage2"], f2[c["issue"]]["feats"]), set(f2[c["issue"]]["hand"])
        r = run_case(ix, a.repo, c, key, keep_query=True)
        q = r.pop("query")
        ranked, _ = ff.rank_two_stage(model, q, r["feats"])
        return ranked, set(r["hand"])
    rows = []
    todo = []
    for c in cases:
        ranked, hand = ranking(c)
        rows.append((c, ranked, hand))
        if str(c["issue"]) not in done:
            todo.append((c, ranked))

    def one(x):
        c, ranked = x
        order, info = planner_rerank(a, key, c, ranked)
        return c["issue"], {"order": order[:1000], **info}
    with ThreadPoolExecutor(4) as ex:
        for n, r in ex.map(one, todo):
            done[str(n)] = r
            json.dump(done, open(out_path, "w"))
            print(f"#{n}: picked {r['picked']} in {r['latency_s']:.0f}s", file=sys.stderr)
    KK = (100, 400)
    tot = Counter()
    lat, cost, gated = [], 0.0, 0
    for c, ranked, hand in rows:
        order = [p for p, _ in ranked]
        unsure = ff.sureness(ranked) < 0.7
        rr = done[str(c["issue"])]["order"]
        gated += unsure
        tot["n"] += len(hand)
        for k in KK:
            tot[f"base{k}"] += len(hand & set(order[:k]))
            tot[f"all{k}"] += len(hand & set(rr[:k]))
            tot[f"gated{k}"] += len(hand & set((rr if unsure else order)[:k]))
        lat.append(done[str(c["issue"])]["latency_s"])
        cost += (done[str(c["issue"])].get("usage") or {}).get("cost", 0) or 0
    print(f"{len(rows)} cases, {tot['n']} existing files; {gated} unsure (sureness < 0.7)")
    print("| Ranking | @100 | @400 |")
    print("|---|---:|---:|")
    for name, lab in (("base", "finder"), ("gated", "planner re-rank, unsure queries only"),
                      ("all", "planner re-rank, every query")):
        print(f"| {lab} | {tot[name + '100']/tot['n']:.3f} | {tot[name + '400']/tot['n']:.3f} |")
    print(f"planner latency median {statistics.median(lat):.1f}s p90 {sorted(lat)[int(0.9*(len(lat)-1))]:.1f}s; "
          f"cost ${cost:.2f} total, ${cost/len(rows):.3f} per query")


def cmd_check(a):
    """Run the shipped two-stage finder on every case of a small dataset (e.g. fresh fixes)."""
    ix = ff.Index(ff.cache_for(a)).load()
    model = json.load(open(a.model))
    key = ix.key
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
    ap.add_argument("cmd", choices=["prepare", "features", "train", "eval", "compare", "judge", "judge-eval", "plan",
                                    "check", "rerank", "fusion-card", "profiles", "tower", "cards", "teacher"])
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
    ap.add_argument("--all", action="store_true", help="train on every case (with --reuse-stage2); the model "
                    "is marked as trained on the eval cases, so eval and compare refuse it")
    ap.add_argument("--allow-overlap", action="store_true",
                    help="eval: report a model trained on eval cases, labelled development")
    ap.add_argument("--baseline", default=ff.MODEL_PATH, help="compare: the active model")
    ap.add_argument("--candidate", help="compare: the candidate model")
    ap.add_argument("--plan-backend", choices=["openrouter", "claude-cli"], default="claude-cli")
    ap.add_argument("--plan-cli-model", default="sonnet")
    ap.add_argument("--live", action="store_true", help="rerank: run the finder live instead of features2.pkl")
    ap.add_argument("--drop", default="", help="comma-separated feature-name prefixes to leave out (ablation)")
    ap.add_argument("--band-lo", type=int, default=30)
    ap.add_argument("--band-hi", type=int, default=150)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--plan-model", default="anthropic/claude-sonnet-5.5")
    ap.add_argument("--plan-top", type=int, default=60)
    ap.add_argument("--clef-k", type=int, default=100, help="fusion-card: files Clef re-ranks")
    ap.add_argument("--clef-door", default="https://openagents.com/api/v1/systemone")
    ap.add_argument("--clef-artifact",
                    default="sha256:fd3e90605e8103307dca37cb5a8cdb036267e2fe3cb2d908d80a8ceb9ec0638c")
    ap.add_argument("--clef-head",
                    default="sha256:6e4699704c24e9f04f3e9270e7fe9c72fdb7c72086e30648d4ac4a39e5e6b041")
    ap.add_argument("--clef-workers", type=int, default=3, help="compare: concurrent Clef requests")
    ap.add_argument("--cards", action="store_true", help="run the card stage (#11249) in every replay")
    ap.add_argument("--towers", help="--cards: the distilled towers (tower writes it)")
    ap.add_argument("--no-tower", action="store_true", help="--cards without the distilled score (ablation)")
    ap.add_argument("--teacher", help="tower: Clef answers on corpus items (JSONL issue, path, clef_p)")
    ap.add_argument("--tower-k", type=int, default=32)
    ap.add_argument("--tower-lam", type=float, default=0.5)
    a = ap.parse_args()
    os.makedirs(a.work, exist_ok=True)
    global CARDS
    if a.cards or a.cmd in ("profiles", "tower", "cards"):
        CARDS = BenchCards(a)
    {"prepare": cmd_prepare, "features": cmd_features, "train": cmd_train, "eval": cmd_eval, "compare": cmd_compare,
     "judge": cmd_judge, "judge-eval": cmd_judge_eval, "plan": cmd_plan, "check": cmd_check, "rerank": cmd_rerank,
     "fusion-card": cmd_fusion_card, "profiles": cmd_profiles, "tower": cmd_tower,
     "cards": cmd_cards, "teacher": cmd_teacher}[a.cmd](a)


if __name__ == "__main__":
    main()
