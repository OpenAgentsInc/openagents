#!/usr/bin/env python3
"""Score issue_files.py's configurations on closed issues with known fixes.

For each closed issue the finder runs at the fix's parent commit (what the
solver would have seen), the top K candidates are asked about under every
configuration, and the answers are scored against the hand-written files
the fix changed that already existed at the parent (a finder cannot name a
file the fix created).

Cases come from scripts/bench/file-finding-dataset.py's output:

  gh issue list -R OpenAgentsInc/openagents --state closed --limit 600 \\
      --json number,title,body,closedAt,createdAt > closed.json
  python3 -I scripts/bench/file-finding-dataset.py --repo . --issues closed.json --out cases.json --n 60
  python3 -I scripts/clef-lab/eval_closed.py --cases cases.json --issues 11320,11303,...

Every answer is cached in the run folder (answers.jsonl), so a stopped run
picks up where it stopped. The report is report.md there.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import secrets
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import issue_files as lab  # noqa: E402

# The servers: Clef 27B (Ollama on this Mac) and Clef-Flash (Psionic). Point
# them elsewhere, an H100 for instance, with these two variables.
OLLAMA = os.environ.get("CLEF_LAB_27B_URL", "http://127.0.0.1:11434")
PSIONIC = os.environ.get("CLEF_LAB_FLASH_URL", "http://127.0.0.1:18098")

# name -> (base_url, model, shape, tier); shape: batch | single | solver
CONFIGS = {
    "flash-solver": (PSIONIC, "clef-flash", "solver", 0),
    "flash-batch-t3": (PSIONIC, "clef-flash", "batch", 3),
    "27b-solver": (OLLAMA, "clef", "solver", 0),
    "27b-single-t3": (OLLAMA, "clef", "single", 3),
    "27b-batch-t2": (OLLAMA, "clef", "batch", 2),
    "27b-batch-t3": (OLLAMA, "clef", "batch", 3),
}


def solver_request(model: str, issue_doc: dict, repo: Path, rev: str, path: str) -> dict:
    """The solver's request today (decide.rs `file_questions`): the issue,
    the path and the file's first 80 lines, one noul."""
    text = lab.show(repo, rev, path)
    head = "\n".join(text.splitlines()[:80]) if text is not None else "(the file does not exist at this commit)"
    body = (issue_doc.get("body") or "")[:4000]
    state = f"RUN {secrets.token_hex(8)}\n\nIssue #{issue_doc['number']}: {issue_doc['title']}\n\n{body}\n\nFile: {path}\n\n{head}"
    return {"model": model, "state": state,
            "questions": {"relevant": {"type": "noul", "instructions": lab.QUESTION}}}


def answer(config: str, issue_doc: dict, files: list[dict], repo: Path, rev: str, timeout: int) -> dict:
    url, model, shape, tier = CONFIGS[config]
    p, secs, tokens = {}, 0.0, 0
    if shape == "solver":
        for f in files:
            got, s = lab.ask(url, solver_request(model, issue_doc, repo, rev, f["path"]), timeout)
            p[f["path"]] = got["answers"]["relevant"]["noul"]
            secs += s
            tokens += got.get("usage", {}).get("input_tokens") or 0
    else:
        entries, _ = lab.fit(repo, rev, files, tier, model, issue_doc, lab.OLLAMA_BODY_CAP)
        if shape == "batch":
            got, secs = lab.ask(url, lab.request_body(model, issue_doc, entries, secrets.token_hex(8)), timeout)
            tokens = got.get("usage", {}).get("input_tokens") or 0
            p = {e["path"]: got["answers"][e["id"]]["noul"] for e in entries}
        else:
            for e in entries:
                got, s = lab.ask(url, lab.request_body(model, issue_doc, [dict(e, id="f1")], secrets.token_hex(8)), timeout)
                p[e["path"]] = got["answers"]["f1"]["noul"]
                secs += s
                tokens += got.get("usage", {}).get("input_tokens") or 0
    return {"p": p, "seconds": round(secs, 2), "tokens": tokens}


def auc(scores: list[tuple[float, bool]]) -> float | None:
    pos = [s for s, y in scores if y]
    neg = [s for s, y in scores if not y]
    if not pos or not neg:
        return None
    wins = sum((a > b) + 0.5 * (a == b) for a in pos for b in neg)
    return wins / (len(pos) * len(neg))


def prf(tp: int, fp: int, fn: int) -> tuple[float, float, float]:
    pr = tp / (tp + fp) if tp + fp else 0.0
    rc = tp / (tp + fn) if tp + fn else 0.0
    return pr, rc, (2 * pr * rc / (pr + rc) if pr + rc else 0.0)


def main() -> None:
    here = Path(__file__).resolve().parents[2]
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--cases", type=Path, required=True)
    ap.add_argument("--issues", required=True, help="comma-separated issue numbers from --cases")
    ap.add_argument("--configs", default=",".join(CONFIGS))
    ap.add_argument("--repo", type=Path, default=here)
    ap.add_argument("--k", type=int, default=12)
    ap.add_argument("--out", type=Path, default=Path.home() / ".openagents/clef-lab/eval")
    ap.add_argument("--timeout", type=int, default=900)
    args = ap.parse_args()

    repo = args.repo.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    cache_path = args.out / "answers.jsonl"
    cache = {}
    if cache_path.exists():
        for line in cache_path.read_text().splitlines():
            row = json.loads(line)
            cache[(row["issue"], row["config"])] = row
    cases = {c["issue"]: c for c in json.loads(args.cases.read_text())["cases"]}
    configs = args.configs.split(",")
    say = lambda *a: print(*a, flush=True)  # noqa: E731

    per_issue = []
    for number in [int(x) for x in args.issues.split(",")]:
        case = cases[number]
        truth = {h["path"] for h in case["hand"] if h["existing"]}
        rev = case["parent"]
        finder_path = args.out / f"finder-{number}.json"
        if finder_path.exists():
            found = json.loads(finder_path.read_text())
        else:
            found = lab.finder(repo, rev, number)
            finder_path.write_text(json.dumps(found))
        files = found["files"][: args.k]
        issue_doc = {"number": number, "title": case["title"], "body": case["body"]}
        in_k = truth & {f["path"] for f in files}
        say(f"#{number} {case['title'][:60]}  truth {len(truth)}, in top {args.k}: {len(in_k)}")
        result = {"issue": number, "truth": sorted(truth), "files": files, "configs": {}}
        for config in configs:
            row = cache.get((number, config))
            if row is None:
                got = answer(config, issue_doc, files, repo, rev, args.timeout)
                row = {"issue": number, "config": config, **got}
                with cache_path.open("a") as fh:
                    fh.write(json.dumps(row) + "\n")
            result["configs"][config] = row
            hits = sorted(row["p"].items(), key=lambda kv: -kv[1])
            say(f"   {config:15} {row['seconds']:6.1f}s  top3: "
                + ", ".join(f"{'*' if path in truth else ' '}{path.rsplit('/', 1)[-1]} {pv:.2f}" for path, pv in hits[:3]))
        per_issue.append(result)

    # Scoring. "finder" ranks by the finder's own confidence, with no Clef.
    scorers = {"finder": lambda r, f: f["confidence"]}
    for config in configs:
        scorers[config] = lambda r, f, c=config: r["configs"][c]["p"][f["path"]]
        scorers[config + "+fused"] = lambda r, f, c=config: 1 / (1 + math.exp(
            -(lab.logit(f["confidence"]) + lab.logit(r["configs"][c]["p"][f["path"]]))))

    rows = []
    for name, score in scorers.items():
        pooled, aucs, top3, top6, tp, fp, fn = [], [], 0, 0, 0, 0, 0
        n_truth = 0
        secs = 0.0
        for r in per_issue:
            truth = set(r["truth"])
            scored = [(score(r, f), f["path"] in truth) for f in r["files"]]
            pooled += scored
            a = auc(scored)
            if a is not None:
                aucs.append(a)
            order = [f["path"] for f in sorted(r["files"], key=lambda f: -score(r, f))]
            top3 += len(truth & set(order[:3]))
            top6 += len(truth & set(order[:6]))
            n_truth += len(truth)
            # The solver's keep rule on Clef's raw answer (finder: no rule).
            base = name.split("+")[0]
            if base in r["configs"]:
                p = r["configs"][base]["p"]
                keep = set(lab.solver_keep(r["files"], p)) if "+" not in name else set(order[:6])
                secs += r["configs"][base]["seconds"]
            else:
                keep = set(order[:6])
            tp += len(keep & truth)
            fp += len(keep - truth)
            fn += len(truth - keep)
        pr, rc, f1 = prf(tp, fp, fn)
        best = max(
            (prf(sum(1 for s, y in pooled if s >= t and y), sum(1 for s, y in pooled if s >= t and not y),
                 sum(1 for s, y in pooled if s < t and y))[2], t)
            for t in [i / 20 for i in range(1, 20)]
        )
        rows.append({
            "name": name, "auc_pooled": auc(pooled), "auc_mean": sum(aucs) / len(aucs) if aucs else None,
            "recall@3": top3 / n_truth, "recall@6": top6 / n_truth,
            "keep_p": pr, "keep_r": rc, "keep_f1": f1, "best_f1": best[0], "best_t": best[1],
            "seconds_per_issue": secs / len(per_issue) if name in configs else None,
        })

    ceiling = sum(len(set(r["truth"]) & {f["path"] for f in r["files"]}) for r in per_issue)
    total = sum(len(r["truth"]) for r in per_issue)
    head = ("| Scorer | AUC pooled | AUC per issue | recall@3 | recall@6 | kept: P / R / F1 | best F1 (t, in-sample) | s / issue |\n"
            "|---|---:|---:|---:|---:|---|---|---:|")
    fmt = lambda v: "—" if v is None else f"{v:.3f}"  # noqa: E731
    lines = [head] + [
        f"| {r['name']} | {fmt(r['auc_pooled'])} | {fmt(r['auc_mean'])} | {r['recall@3']:.3f} | {r['recall@6']:.3f} | "
        f"{r['keep_p']:.2f} / {r['keep_r']:.2f} / {r['keep_f1']:.2f} | {r['best_f1']:.2f} ({r['best_t']:.2f}) | "
        f"{fmt(r['seconds_per_issue'])} |"
        for r in rows
    ]
    report = (f"# Clef file-relevance configurations on {len(per_issue)} closed issues\n\n"
              f"Truth: {total} pre-existing files the fixes changed; {ceiling} of them are in the finder's top {args.k} "
              f"(the ceiling for every scorer).\n\n"
              "\"kept\" is the solver's rule (Clef ≥ 0.35, at least 3) for a raw config, and the top 6 for "
              "`finder` and `+fused`.\n\n" + "\n".join(lines) + "\n")
    (args.out / "report.md").write_text(report)
    (args.out / "scores.json").write_text(json.dumps(rows, indent=2))
    say("\n" + report)
    say(f"run folder  {args.out}")


if __name__ == "__main__":
    main()
