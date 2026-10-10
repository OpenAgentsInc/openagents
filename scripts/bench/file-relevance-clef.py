#!/usr/bin/env python3
"""Clef-Flash on file-relevance-v1: raw scores, a calibration map, and its check (#11216, roadmap X1).

  run        POST every item of the chosen partitions to a /v1/systemone door, one request
             per item (the corpus's per-file prompt), and append {id, p, request_sha256,
             psionic} to --out. Resumable. The request body's sha256 is the key the server's
             --decision-export-rows lines carry, so hidden rows join back to items.
  calibrate  fit a Platt map (and a temperature, for comparison) on the calibration
             partition; report raw and mapped scores on development; write the map file
             psionic-openai-server reads with --decision-calibration
  score      F1 at 0.5, ECE, Brier, log loss and confident errors of raw and mapped
             probabilities on one partition (the locked partition is read with --locked-read)

Every report line carries `evidence_class: measured` and the corpus digest it scored.

    python3 -I scripts/bench/file-relevance-clef.py run --corpus C --partitions calibration,development \\
        --base-url http://127.0.0.1:18095 --out clef-results.jsonl
    python3 -I scripts/bench/file-relevance-clef.py calibrate --corpus C --results clef-results.jsonl \\
        --head-digest sha256:... --out crates/psionic/fixtures/clef/calibration/file-relevance-v1.json
"""
import argparse, hashlib, json, math, os, sys, time, urllib.request
from collections import Counter

QUESTION_ID = "relevant"


def load_items(path, partitions):
    corpus = json.load(open(path))
    want = set(partitions)
    return corpus, [it for it in corpus["items"] if it["partition"] in want]


def body_of(item, model):
    return json.dumps({"model": model, "state": item["state"], "questions": item["question"]},
                      ensure_ascii=False, separators=(",", ":"))


def post(url, body, timeout=600):
    req = urllib.request.Request(url, data=body.encode("utf-8"), headers={"Content-Type": "application/json"})
    for attempt in range(5):
        try:
            with urllib.request.urlopen(req, timeout=timeout) as r:
                return json.load(r)
        except urllib.error.HTTPError as e:
            detail = e.read().decode("utf-8", "replace")
            if e.code in (400, 413, 422):
                return {"error": {"status": e.code, "detail": detail[:400]}}
            if attempt == 4:
                raise
        except Exception:
            if attempt == 4:
                raise
        time.sleep(2 ** attempt)


def cmd_run(a):
    _, items = load_items(a.corpus, a.partitions.split(","))
    done = set()
    if os.path.exists(a.out):
        done = {json.loads(l)["id"] for l in open(a.out)}
    todo = [it for it in items if it["id"] not in done]
    if a.limit:
        todo = todo[:a.limit]
    url = a.base_url.rstrip("/") + "/v1/systemone"
    t0 = time.time()
    with open(a.out, "a") as out:
        for k, it in enumerate(todo):
            body = body_of(it, a.model)
            t = time.perf_counter()
            d = post(url, body)
            row = {"id": it["id"], "partition": it["partition"], "label": it["label"],
                   "request_sha256": "sha256:" + hashlib.sha256(body.encode("utf-8")).hexdigest(),
                   "latency_s": round(time.perf_counter() - t, 4)}
            if "error" in d:
                row["error"] = d["error"]
            else:
                ans = d["answers"][QUESTION_ID]
                row["p"] = ans["noul"]
                row["psionic"] = d.get("psionic", {})
            out.write(json.dumps(row) + "\n")
            out.flush()
            if k % 200 == 0:
                rate = (k + 1) / max(time.time() - t0, 1e-6)
                print(f"{k+1}/{len(todo)} {rate:.2f}/s", file=sys.stderr)
    print(f"done: {len(todo)} requests in {time.time()-t0:.0f}s")


def logit(p):
    p = min(max(p, 1e-6), 1 - 1e-6)
    return math.log(p / (1 - p))


def sig(z):
    return 1 / (1 + math.exp(-z)) if z > -700 else 0.0


def scores(ps, ys):
    """gym::gate Scores for a binary noul, read the way the gate reads a choice: the winning
    option is true when p >= 0.5, its probability max(p, 1-p), correct when it equals the label.
    Plus F1 of `true` at 0.5 and AUC."""
    n = len(ps)
    win = [max(p, 1 - p) for p in ps]
    correct = [(p >= 0.5) == bool(y) for p, y in zip(ps, ys)]
    acc = sum(correct) / n
    bins = [[0, 0.0, 0] for _ in range(10)]
    for w, c in zip(win, correct):
        b = min(int(w * 10), 9)
        bins[b][0] += 1
        bins[b][1] += w
        bins[b][2] += c
    ece = sum(abs(b[1] / b[0] - b[2] / b[0]) * b[0] / n for b in bins if b[0])
    brier = sum((w - c) ** 2 for w, c in zip(win, correct)) / n
    nll = -sum(math.log(min(max(w if c else 1 - w, 1e-12), 1)) for w, c in zip(win, correct)) / n
    conf_err = sum(1 for w, c in zip(win, correct) if w >= 0.9 and not c)
    tp = sum(1 for p, y in zip(ps, ys) if p >= 0.5 and y)
    fp = sum(1 for p, y in zip(ps, ys) if p >= 0.5 and not y)
    fn = sum(1 for p, y in zip(ps, ys) if p < 0.5 and y)
    prec = tp / (tp + fp) if tp + fp else 0.0
    rec = tp / (tp + fn) if tp + fn else 0.0
    f1 = 2 * prec * rec / (prec + rec) if prec + rec else 0.0
    pos = [p for p, y in zip(ps, ys) if y]
    neg = [p for p, y in zip(ps, ys) if not y]
    auc = None
    if pos and neg:
        allp = sorted((p, y) for p, y in zip(ps, ys))
        rank_sum, i = 0.0, 0
        while i < len(allp):
            j = i
            while j < len(allp) and allp[j][0] == allp[i][0]:
                j += 1
            r = (i + j + 1) / 2
            rank_sum += r * sum(1 for k in range(i, j) if allp[k][1])
            i = j
        auc = (rank_sum - len(pos) * (len(pos) + 1) / 2) / (len(pos) * len(neg))
    # probability-of-true calibration (10 bins on p itself), the number a caller thresholds
    pb = [[0, 0.0, 0] for _ in range(10)]
    for p, y in zip(ps, ys):
        b = min(int(p * 10), 9)
        pb[b][0] += 1
        pb[b][1] += p
        pb[b][2] += y
    ece_true = sum(abs(b[1] / b[0] - b[2] / b[0]) * b[0] / n for b in pb if b[0])
    return {"items": n, "accuracy": acc, "ece": ece, "brier": brier, "nll": nll, "confident_errors": conf_err,
            "f1_at_0_5": f1, "precision": prec, "recall": rec, "auc": auc, "ece_p_true": ece_true,
            "mean_p_true": sum(pos) / len(pos) if pos else None, "mean_p_false": sum(neg) / len(neg) if neg else None}


def fit_platt(xs, ys, iters=200):
    """Logistic regression of y on logit(p): Newton steps, a tiny ridge for stability."""
    a, b = 1.0, 0.0
    for _ in range(iters):
        ga = gb = haa = hab = hbb = 0.0
        for x, y in zip(xs, ys):
            p = sig(a * x + b)
            g = p - y
            w = p * (1 - p)
            ga += g * x
            gb += g
            haa += w * x * x
            hab += w * x
            hbb += w
        haa += 1e-6
        hbb += 1e-6
        det = haa * hbb - hab * hab
        da = (hbb * ga - hab * gb) / det
        db = (haa * gb - hab * ga) / det
        a, b = a - da, b - db
        if abs(da) + abs(db) < 1e-10:
            break
    return a, b


def fit_temperature(xs, ys):
    best = None
    for k in range(1, 400):
        t = k / 100
        nll = -sum(math.log(max(sig(x / t) if y else 1 - sig(x / t), 1e-12)) for x, y in zip(xs, ys))
        if best is None or nll < best[0]:
            best = (nll, t)
    return best[1]


def results_for(path, items):
    by_id = {}
    for l in open(path):
        r = json.loads(l)
        if "p" in r:
            by_id[r["id"]] = r
    got = [(by_id[it["id"]]["p"], 1 if it["label"] == "true" else 0) for it in items if it["id"] in by_id]
    return got, by_id


def corpus_digest(path):
    return "sha256:" + hashlib.sha256(open(path, "rb").read()).hexdigest()


def fmt(s):
    return (f"F1@0.5 {s['f1_at_0_5']:.3f} (P {s['precision']:.3f} R {s['recall']:.3f}) acc {s['accuracy']:.3f} "
            f"ECE {s['ece']:.3f} ECE(p) {s['ece_p_true']:.3f} Brier {s['brier']:.3f} NLL {s['nll']:.3f} "
            f"conf.err {s['confident_errors']} AUC {s['auc']:.3f} n {s['items']}")


def cmd_calibrate(a):
    _, cal = load_items(a.corpus, ["calibration"])
    _, dev = load_items(a.corpus, ["development"])
    cal_r, by_id = results_for(a.results, cal)
    dev_r, _ = results_for(a.results, dev)
    if len(cal_r) < len(cal) or len(dev_r) < len(dev):
        print(f"warning: results cover {len(cal_r)}/{len(cal)} calibration and {len(dev_r)}/{len(dev)} "
              "development items", file=sys.stderr)
    xs = [logit(p) for p, _ in cal_r]
    ys = [y for _, y in cal_r]
    pa, pb = fit_platt(xs, ys)
    t = fit_temperature(xs, ys)
    report = {"evidence_class": "measured", "corpus": a.corpus_name, "corpus_file_sha256": corpus_digest(a.corpus),
              "results_file_sha256": corpus_digest(a.results),
              "platt": {"a": pa, "b": pb}, "temperature": t, "fitted_on": {"partition": "calibration",
                                                                         "items": len(cal_r)}}
    for name, items in (("calibration", cal_r), ("development", dev_r)):
        ps = [p for p, _ in items]
        ys_ = [y for _, y in items]
        report[name] = {"raw": scores(ps, ys_),
                        "platt": scores([sig(pa * logit(p) + pb) for p in ps], ys_),
                        "temperature": scores([sig(logit(p) / t) for p in ps], ys_)}
    heads = Counter(r.get("psionic", {}).get("head_digest") for r in by_id.values())
    artifacts = Counter(r.get("psionic", {}).get("artifact_digest") for r in by_id.values())
    head = a.head_digest or heads.most_common(1)[0][0]
    mapping = {
        "schema": "openagents.clef.calibration.v1",
        "name": a.name,
        "version": a.version,
        "evidence_class": "measured",
        "question": {"type": "noul", "instructions": json.load(open(a.corpus))["items"][0]["question"]
                     [QUESTION_ID]["instructions"]},
        "map": {"kind": "platt", "a": round(pa, 6), "b": round(pb, 6)},
        "head_digest": head,
        "artifact_digest": artifacts.most_common(1)[0][0],
        "fitted_on": {"corpus": a.corpus_name, "partition": "calibration", "items": len(cal_r),
                      "prompt": "one request per (issue, file); the corpus state; noul `relevant`"},
        "development": {k: {m: round(v, 4) if isinstance(v, float) else v for m, v in report["development"][k].items()}
                        for k in ("raw", "platt")},
    }
    os.makedirs(os.path.dirname(os.path.abspath(a.out)), exist_ok=True)
    with open(a.out, "w") as f:
        json.dump(mapping, f, indent=1)
        f.write("\n")
    if a.report:
        json.dump(report, open(a.report, "w"), indent=1)
    print(f"Platt a={pa:.4f} b={pb:.4f}; temperature t={t:.2f}; fitted on {len(cal_r)} calibration items")
    for name in ("calibration", "development"):
        for k in ("raw", "platt", "temperature"):
            print(f"{name:12} {k:11} {fmt(report[name][k])}")
    print(f"map -> {a.out}")


def cmd_score(a):
    parts = a.partitions.split(",")
    if "locked" in parts and not a.locked_read:
        sys.exit("the locked partition is read once, by the final check: pass --locked-read")
    _, items = load_items(a.corpus, parts)
    got, _ = results_for(a.results, items)
    m = json.load(open(a.map))["map"]
    ps = [p for p, _ in got]
    ys = [y for _, y in got]
    mapped = [sig(m["a"] * logit(p) + m["b"]) for p in ps] if m["kind"] == "platt" else \
        [sig(logit(p) / m["t"]) for p in ps]
    out = {"evidence_class": "measured", "partitions": parts, "corpus_file_sha256": corpus_digest(a.corpus),
           "map_sha256": corpus_digest(a.map), "raw": scores(ps, ys), "mapped": scores(mapped, ys),
           "covered": f"{len(got)}/{len(items)}"}
    if a.report:
        json.dump(out, open(a.report, "w"), indent=1)
    print(f"{','.join(parts)} ({out['covered']} items)")
    print(f"  raw    {fmt(out['raw'])}")
    print(f"  mapped {fmt(out['mapped'])}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("cmd", choices=["run", "calibrate", "score"])
    ap.add_argument("--corpus", required=True)
    ap.add_argument("--corpus-name", default="file-relevance-v1")
    ap.add_argument("--partitions", default="calibration,development")
    ap.add_argument("--base-url", default="http://127.0.0.1:18095")
    ap.add_argument("--model", default="clef-flash")
    ap.add_argument("--out")
    ap.add_argument("--results")
    ap.add_argument("--report")
    ap.add_argument("--map")
    ap.add_argument("--name", default="file-relevance")
    ap.add_argument("--version", type=int, default=1)
    ap.add_argument("--head-digest")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--locked-read", action="store_true")
    a = ap.parse_args()
    {"run": cmd_run, "calibrate": cmd_calibrate, "score": cmd_score}[a.cmd](a)


if __name__ == "__main__":
    main()
