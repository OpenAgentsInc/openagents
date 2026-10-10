#!/usr/bin/env python3
"""Data prep and comparison for the head-only file-relevance ranker (#11217, roadmap X2a).

The deterministic finder (scripts/filefind) stays frozen: its candidates and per-candidate
features are the inputs. psionic-train's `psionic-decision-train` learns a head on top; this
script only moves data and scores rankings.

  export   finder features (file-finding-bench.py's features2.pkl, or a `fresh` pickle) ->
           a psionic-decision-train data dir (meta.json, features.f32, rows.tsv). Rows of
           train cases get role `train`, eval cases role `eval`. `baseline` is today's numpy
           ranker's probability. Also writes clef-items.json: the top --clef-k candidates of
           the chosen issues, as per-file Clef prompts in the file-relevance-v1 state format.
  fresh    run the shipped two-stage finder on a small dataset (the newer-fix set) and pickle
           its candidate features, as `file-finding-bench.py check` does
  join     attach Clef results (file-relevance-clef.py run) and the rows psionic-openai-server
           exported (--decision-export-rows) to a data dir
  compare  recall@k of the numpy ranker and the head on the eval rows, the paired difference
           with its standard error over issues, and calibration (ECE, Brier) of both

Every report carries `evidence_class: measured`.
"""
import argparse, hashlib, importlib.util, json, math, os, pickle, struct, sys
from collections import defaultdict

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "filefind"))
import filefind as ff  # noqa: E402


def _load(name, file):
    spec = importlib.util.spec_from_file_location(name, os.path.join(HERE, file))
    mod = importlib.util.module_from_spec(spec)
    sys.argv, argv = [file], sys.argv
    spec.loader.exec_module(mod)
    sys.argv = argv
    return mod


corpus_mod = _load("frc", "file-relevance-corpus.py")
KS = (20, 50, 100, 200, 300, 400)


def cmd_export(a):
    data = json.load(open(a.dataset))["cases"]
    by_issue = {c["issue"]: c for c in data}
    feats = pickle.load(open(a.features, "rb"))
    if a.eval_features:  # e.g. the newer-fix set, run live by `fresh`
        feats.update(pickle.load(open(a.eval_features, "rb")))
        by_issue.update({c["issue"]: c for c in json.load(open(a.eval_dataset))["cases"]})
    model = json.load(open(a.model))
    stage = model[a.stage]
    names = stage["features"]
    train_ids = set(int(x) for x in open(a.train_issues).read().split()) if a.train_issues else set()
    eval_ids = [int(x) for x in open(a.eval_issues).read().split()]
    clef_train = set(int(x) for x in open(a.clef_train_issues).read().split()) if a.clef_train_issues else set()
    os.makedirs(a.out, exist_ok=True)
    fx = open(os.path.join(a.out, "features.f32"), "wb")
    rows = open(os.path.join(a.out, "rows.tsv"), "w")
    rows.write("issue\tpath\tlabel\trole\tset\tbaseline\thidden_offset\tclef_logit\n")
    clef_pairs = []
    n = 0
    order = [i for i in sorted(train_ids) if i in feats] + [i for i in eval_ids if i in feats]
    for issue in order:
        r = feats[issue]
        role = "eval" if issue in eval_ids else "train"
        paths = list(r["feats"])
        X = ff.vectorize([r["feats"][p] for p in paths], names).astype(np.float32)
        base = ff.score(stage, ff.vectorize([r["feats"][p] for p in paths], names))
        hand = set(r["hand"])
        fx.write(X.tobytes())
        for p, b in zip(paths, base):
            rows.write(f"{issue}\t{p}\t{int(p in hand)}\t{role}\t{a.set if role == 'eval' else 'train'}\t{b:.6g}\t-1\tnan\n")
            n += 1
        k = a.clef_k if role == "eval" else (a.clef_train_k if issue in clef_train else 0)
        if k:
            top = np.argsort(-base, kind="stable")[:k]
            clef_pairs += [(issue, paths[i], int(paths[i] in hand)) for i in top]
    fx.close()
    rows.close()
    with open(os.path.join(a.out, "hand.tsv"), "w") as f:
        for issue in order:
            f.write(f"{issue}\t{len(feats[issue]['hand'])}\n")
    json.dump({"features": names, "evidence_class": "measured",
               "source": {"features": os.path.basename(a.features), "baseline_model": os.path.basename(a.model),
                          "stage": a.stage}}, open(os.path.join(a.out, "meta.json"), "w"), indent=1)
    # Clef prompts for the chosen pairs, in the corpus's state format
    items = []
    need = defaultdict(list)
    for issue, p, y in clef_pairs:
        need[issue].append((p, y))
    for issue, ps in need.items():
        c = by_issue[issue]
        tree = ff.ls_tree(a.repo, c["parent"])
        rws = [{"issue": issue, "path": p, "blob": tree[p], "label": "true" if y else "false", "kind": "rerank"}
               for p, y in ps if p in tree]
        heads = corpus_mod.heads_for(a.repo, rws)
        for rw in rws:
            h = heads.get(rw["blob"])
            if h is None:
                continue
            it = corpus_mod.corpus_item(rw, c, h, "eval" if issue in eval_ids else "train")
            it["partition"] = f"rerank-{a.set}" if issue in eval_ids else "rerank-train"
            items.append(it)
    json.dump({"items": items}, open(os.path.join(a.out, "clef-items.json"), "w"), ensure_ascii=False)
    print(f"{n} rows ({len(order)} issues: {sum(1 for i in order if i in eval_ids)} eval); "
          f"{len(items)} Clef prompts -> {a.out}")


def cmd_fresh(a):
    bench = _load("ffb", "file-finding-bench.py")
    live_embed = ff.embed

    def embed_or_zero(texts, key, **kw):  # an issue not in the index needs a live call; without
        try:                                # credit the similarity features are zero, as in `query`
            return live_embed(texts, key, **kw)
        except Exception as e:
            print(f"issue not embedded ({str(e)[:60]}); similarity features are zero", file=sys.stderr)
            return np.zeros((len(texts), ff.DIMS), np.float32)
    bench.ff.embed = ff.embed = embed_or_zero
    ix = ff.Index(ff.default_cache(a.repo)).load()
    model = json.load(open(a.model))
    out = {}
    for c in json.load(open(a.dataset))["cases"]:
        r = bench.run_case(ix, a.repo, c, os.environ.get("OPENROUTER_API_KEY"), keep_query=True)
        q = r.pop("query")
        ranked, f2 = ff.rank_two_stage(model, q, r["feats"])
        r["feats"] = f2
        out[c["issue"]] = r
        hand = set(r["hand"])
        order = [p for p, _ in ranked]
        print(f"#{c['issue']}: {len(hand)} files, @400 {len(hand & set(order[:400]))}", file=sys.stderr)
    pickle.dump(out, open(a.out, "wb"), protocol=4)


def cmd_join(a):
    """rows.tsv gains hidden_offset and clef_logit for every (issue, path) Clef answered."""
    results = {}
    for l in open(a.results):
        r = json.loads(l)
        if "p" in r:
            results[r["request_sha256"]] = r
    export = {}
    blocks = None
    for l in open(os.path.join(a.export_dir, "rows.jsonl")):
        e = json.loads(l)
        export[e["request_sha256"]] = e
        blocks = e["rows"]
        width = e["width"]
    key = {}
    for sha, r in results.items():
        _, issue, path = r["id"].split(":", 2)
        e = export.get(sha)
        p = min(max(r["p"], 1e-6), 1 - 1e-6)
        key[(int(issue), path)] = (e["offset"] if e else -1, math.log(p / (1 - p)))
    src = os.path.join(a.data, "rows.tsv")
    lines = open(src).read().splitlines()
    hit = 0
    with open(src + ".tmp", "w") as f:
        f.write(lines[0] + "\n")
        for line in lines[1:]:
            v = line.split("\t")
            got = key.get((int(v[0]), v[1]))
            if got:
                v[6], v[7] = str(got[0]), f"{got[1]:.6g}"
                hit += 1
            f.write("\t".join(v) + "\n")
    os.replace(src + ".tmp", src)
    meta_path = os.path.join(a.data, "meta.json")
    meta = json.load(open(meta_path))
    meta["hidden"] = {"file": os.path.abspath(os.path.join(a.export_dir, "rows.f16")), "width": width,
                      "blocks": blocks, "use_blocks": a.blocks.split(",") if a.blocks else blocks}
    json.dump(meta, open(meta_path, "w"), indent=1)
    print(f"joined {hit} rows; blocks {blocks}")


def read_preds(path):
    by = defaultdict(list)
    with open(path) as f:
        next(f)
        for line in f:
            issue, p, s, y, b, sc = line.rstrip("\n").split("\t")
            by[(s, int(issue))].append((p, float(y), float(b), float(sc)))
    return by


def ece(ps, ys, bins=10):
    n = len(ps)
    acc = [[0, 0.0, 0.0] for _ in range(bins)]
    for p, y in zip(ps, ys):
        k = min(int(p * bins), bins - 1)
        acc[k][0] += 1
        acc[k][1] += p
        acc[k][2] += y
    return sum(abs(c[1] - c[2]) for c in acc if c[0]) / n


def cmd_compare(a):
    by = read_preds(a.pred)
    hand_total = {int(l.split()[0]): int(l.split()[1]) for l in open(a.hand)}
    out = {"evidence_class": "measured", "predictions_sha256": "sha256:" + hashlib.sha256(
        open(a.pred, "rb").read()).hexdigest(), "rerank_k": a.rerank_k, "sets": {}}
    sets = sorted({s for s, _ in by})
    for s in sets:
        issues = sorted(i for t, i in by if t == s)
        tot = 0
        per = {k: [] for k in KS}
        hits = {"baseline": defaultdict(int), "head": defaultdict(int)}
        ps_b, ps_h, ys = [], [], []
        for i in issues:
            rows = by[(s, i)]
            nh = hand_total[i]  # every existing hand-written fix file, found by the pool or not
            base = sorted(rows, key=lambda r: -r[2])
            if a.rerank_k:
                top = sorted(base[:a.rerank_k], key=lambda r: -r[3])
                head = top + base[a.rerank_k:]
            else:
                head = sorted(rows, key=lambda r: -r[3])
            tot += nh
            for k in KS:
                hb = sum(r[1] for r in base[:k])
                hh = sum(r[1] for r in head[:k])
                hits["baseline"][k] += hb
                hits["head"][k] += hh
                per[k].append((hh - hb, nh))
            for r in rows:
                ps_b.append(r[2])
                ps_h.append(r[3])
                ys.append(r[1])
        rep = {"issues": len(issues), "fix_files": tot}
        for k in KS:
            rb = hits["baseline"][k] / tot
            rh = hits["head"][k] / tot
            # ratio-estimator SE of the recall difference, clustered by issue
            d = np.array([x for x, _ in per[k]], float)
            m = np.array([w for _, w in per[k]], float)
            diff = d.sum() / m.sum()
            resid = d - diff * m
            se = math.sqrt((resid ** 2).sum() * len(d) / max(len(d) - 1, 1)) / m.sum()
            rep[f"@{k}"] = {"baseline": rb, "head": rh, "diff": diff, "se": se,
                            "wins_by_2se": bool(diff > 2 * se)}
        def cal(ps):  # only probabilities are calibrated; a raw score (a logit) is a ranking only
            if min(ps) < 0 or max(ps) > 1:
                return {"ece": float("nan"), "brier": float("nan")}
            return {"ece": ece(ps, ys), "brier": float(np.mean((np.array(ps) - ys) ** 2))}
        rep["calibration"] = {"baseline": cal(ps_b), "head": cal(ps_h)}
        out["sets"][s] = rep
        print(f"\n== {s}: {len(issues)} issues, {tot} existing hand-written fix files ({a.label})")
        print("| | " + " | ".join(f"@{k}" for k in KS) + " |")
        print("|---|" + "---:|" * len(KS))
        print("| numpy ranker | " + " | ".join(f"{rep[f'@{k}']['baseline']:.3f}" for k in KS) + " |")
        print("| head | " + " | ".join(f"{rep[f'@{k}']['head']:.3f}" for k in KS) + " |")
        print("| diff ± SE | " + " | ".join(f"{rep[f'@{k}']['diff']:+.3f} ± {rep[f'@{k}']['se']:.3f}" for k in KS) + " |")
        c = rep["calibration"]
        print(f"calibration on every candidate row: numpy ECE {c['baseline']['ece']:.4f} Brier {c['baseline']['brier']:.4f}; "
              f"head ECE {c['head']['ece']:.4f} Brier {c['head']['brier']:.4f}")
    if a.report:
        json.dump(out, open(a.report, "w"), indent=1)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("cmd", choices=["export", "fresh", "join", "compare"])
    ap.add_argument("--repo", default=".")
    ap.add_argument("--dataset")
    ap.add_argument("--features")
    ap.add_argument("--model")
    ap.add_argument("--stage", default="stage2")
    ap.add_argument("--eval-features")
    ap.add_argument("--eval-dataset")
    ap.add_argument("--train-issues")
    ap.add_argument("--eval-issues")
    ap.add_argument("--clef-train-issues")
    ap.add_argument("--clef-k", type=int, default=0)
    ap.add_argument("--clef-train-k", type=int, default=0)
    ap.add_argument("--set", default="bench")
    ap.add_argument("--out")
    ap.add_argument("--data")
    ap.add_argument("--results")
    ap.add_argument("--export-dir")
    ap.add_argument("--blocks", default="")
    ap.add_argument("--pred")
    ap.add_argument("--hand", help="compare: hand.tsv from export")
    ap.add_argument("--rerank-k", type=int, default=0)
    ap.add_argument("--label", default="")
    ap.add_argument("--report")
    a = ap.parse_args()
    {"export": cmd_export, "fresh": cmd_fresh, "join": cmd_join, "compare": cmd_compare}[a.cmd](a)


if __name__ == "__main__":
    main()
