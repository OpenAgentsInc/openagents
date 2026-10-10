#!/usr/bin/env python3
"""file-relevance-v1: the file finder's issue -> fix cases as a decision corpus (#11215, roadmap N8).

One item asks Clef's file-relevance question about one (issue, file) pair:

    state     "ISSUE #N: title\\n\\nbody\\n\\nFILE: path\\n```\\n<first 2,048 bytes at the fix's parent>\\n```"
    question  noul "Is this file relevant to solving the issue?"
    label     "true" when the issue's fix commit(s) changed the file, else "false"

Labels come from outcomes (git), never from a model: `label_source` is `measurement`, every
item carries `evidence_class: measured`, and `teacher` is reserved for a judge's answer
(Jev), kept apart from the label. Cases come from scripts/bench/file-finding-dataset.py
(the #11210 finder bench's dataset code), replayed at the fix's parent commit.

Candidates per issue, chosen with a seed derived from the issue number:
  positive  the fix's existing hand-written files (at most 8)
  sibling   files in a positive's directory the fix did not change (3)
  crate     files of a positive's crate outside that directory (2)
  recent    one of the files most touched in the 400 commits before the parent (1)
  random    one text file anywhere in the parent tree (1)

Partitions are split by time: issues ordered by their last fix commit, oldest first, give
training < calibration < development < locked, so no fix sits in an earlier partition than
an older fix, and the newest fixes are held out. Two issue sets are excluded from every
partition because they were already used in-sample: the 10 issues of the Clef/Jev
relevance bench (they tuned its thresholds) and the 8 "newer-fix" issues #11210 checks the
finder on. A later-partition item whose text near-duplicates an earlier-partition item
(token-set Jaccard >= 0.8, the tenancy::training leakage rule) is dropped and counted.

Commands:

  build        cases -> manifest (committed: items.tsv.gz, issues.tsv, manifest.json: commits,
               paths, blob and text digests, labels, no file contents) + corpus (scratch: full text, tenancy format)
  materialize  committed manifest + git + closed-issues JSON -> the same corpus, checked
               against every digest in the manifest

One command rebuilds it from git (scripts/bench/file-relevance-corpus.sh); the corpus
itself holds file contents and issue text, so it stays out of git.
"""
import argparse, gzip, hashlib, io, json, os, random, re, subprocess, sys
from collections import Counter, defaultdict

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "filefind"))
import filefind as ff  # noqa: E402

NAME = "file-relevance-v1"
QUESTION_ID = "relevant"
INSTRUCTIONS = "Is this file relevant to solving the issue?"
HEAD_BYTES = 2048
ISSUE_CHARS = 2500
MAX_POS = 8
TAKE = {"sibling": 3, "crate": 2, "recent": 1, "random": 1}
LEAK_JACCARD = 0.8
CLEF_BENCH = {11123, 11117, 10998, 10295, 10179, 10283, 10177, 10206, 10201, 10170}
NEWER_FIX = {11134, 11156, 11158, 11159, 11160, 11177, 11182, 11132}
TEXT_EXT = {"rs", "md", "toml", "py", "sh", "js", "mjs", "ts", "tsx", "json", "yaml", "yml", "css", "html",
            "swift", "kt", "wgsl", "sql", "txt", "garden", "c", "cu", "h", "go", "nix"}
LABEL_RULE = ("true when a commit that fixed the issue (subject carries (#N)) changed this file, "
              "read from git at the fix; false when no such commit changed it")
PERMISSION = ("OpenAgents-owned repository history and issue tracker, used by OpenAgents for its own "
              "decision models (training-system audit roadmap N8, openagents#11215)")


def sha(text):
    return "sha256:" + hashlib.sha256(text.encode("utf-8")).hexdigest()


def seed_of(issue):
    return int(hashlib.sha256(f"{NAME}:{issue}".encode()).hexdigest()[:16], 16)


def issue_block(n, title, body):
    body = (body or "").strip()
    if len(body) > ISSUE_CHARS:
        body = body[:ISSUE_CHARS] + "\n[...]"
    return f"ISSUE #{n}: {title.strip()}\n\n{body}"


def state_text(n, title, body, path, head):
    return f"{issue_block(n, title, body)}\n\nFILE: {path}\n```\n{head}\n```"


def text_file(path):
    name = path.rsplit("/", 1)[-1]
    ext = name.rsplit(".", 1)[-1].lower() if "." in name else ""
    return ext in TEXT_EXT or name in ("Cargo.toml", "Makefile", "Dockerfile")


def group(path):
    parts = path.split("/")
    return "/".join(parts[:2]) if parts[0] in ("crates", "apps", "packages", "docs", "scripts") and len(parts) > 2 \
        else parts[0]


def normalized_tokens(text):
    """tenancy::training::normalized + tokens: lowercase, non-alphanumerics to spaces, token set."""
    return set("".join(c if c.isalnum() else " " for c in text.lower()).split())


def leak_pairs(items):
    """Exact cross-partition pairs at Jaccard >= LEAK_JACCARD (prefix-filtered all-pairs join).

    Two sets with J >= t share a token within their first |s| - ceil(t*|s|) + 1 tokens when
    tokens are ordered by global frequency (rarest first), so only those prefixes are indexed."""
    import math
    toks = [normalized_tokens(it["state"]) for it in items]
    df = Counter(t for s in toks for t in s)
    order = [sorted(s, key=lambda t: (df[t], t)) for s in toks]
    index = defaultdict(list)
    found = []
    by_size = sorted(range(len(items)), key=lambda i: len(toks[i]))
    for i in by_size:
        s = order[i]
        if not s:
            continue
        pref = len(s) - math.ceil(LEAK_JACCARD * len(s)) + 1
        cands = set()
        for t in s[:pref]:
            cands.update(index[t])
        for j in cands:
            if items[j]["partition"] == items[i]["partition"]:
                continue
            a, b = toks[i], toks[j]
            if len(b) < LEAK_JACCARD * len(a):
                continue
            inter = len(a & b)
            if inter / (len(a) + len(b) - inter) >= LEAK_JACCARD:
                found.append((j, i))
        for t in s[:pref]:
            index[t].append(i)
    return found


def recent_files(repo, parent, n=400):
    out = ff.git(repo, "log", f"-{n}", "--no-merges", "--name-only", "--format=", parent).decode("utf-8", "replace")
    return Counter(p for p in out.splitlines() if p)


def build_rows(repo, cases):
    """Per case: (candidate path, blob, label, kind). Deterministic for a given git history."""
    rows = []
    for c in cases:
        n = c["issue"]
        rng = random.Random(seed_of(n))
        tree = ff.ls_tree(repo, c["parent"])
        changed = {h["path"] for h in c["hand"]} | {d["path"] for d in c["derived"]}
        pos = sorted(h["path"] for h in c["hand"] if h["existing"] and h["path"] in tree and text_file(h["path"]))
        if not pos:
            continue
        if len(pos) > MAX_POS:
            pos = sorted(rng.sample(pos, MAX_POS))
        pool = sorted(p for p in tree if text_file(p) and p not in changed)
        dirs = {p.rsplit("/", 1)[0] if "/" in p else "" for p in pos}
        groups = {group(p) for p in pos}
        sib = [p for p in pool if (p.rsplit("/", 1)[0] if "/" in p else "") in dirs]
        crate = [p for p in pool if group(p) in groups and p not in set(sib)]
        rec = [p for p, _ in recent_files(repo, c["parent"]).most_common(60) if p in tree and p not in changed
               and text_file(p)]
        picked = [(p, "positive") for p in pos]
        seen = set(pos)
        for kind, cand in (("sibling", sib), ("crate", crate), ("recent", rec), ("random", pool)):
            cand = [p for p in cand if p not in seen]
            for p in rng.sample(cand, min(TAKE[kind], len(cand))):
                picked.append((p, kind))
                seen.add(p)
        for p, kind in picked:
            rows.append({"issue": n, "path": p, "blob": tree[p], "label": "true" if kind == "positive" else "false",
                         "kind": kind})
    return rows


def heads_for(repo, rows):
    shas = sorted({r["blob"] for r in rows})
    got = ff.cat_heads(repo, shas, n=HEAD_BYTES * 2, cap=10 ** 9)
    out = {}
    for s, text in got.items():
        if text is None:
            out[s] = None
            continue
        out[s] = text.encode("utf-8")[:HEAD_BYTES].decode("utf-8", "ignore")
    return out


def corpus_item(r, case, head, partition):
    state = state_text(case["issue"], case["title"], case["body"], r["path"], head)
    return {
        "id": f"{NAME}:{case['issue']}:{r['path']}",
        "group": f"issue-{case['issue']}",
        "partition": partition,
        "state": state,
        "question": {QUESTION_ID: {"type": "noul", "instructions": INSTRUCTIONS}},
        "label": r["label"],
        "label_source": "measurement",
        "label_rule": LABEL_RULE,
        "provenance": {
            "source": f"github.com/OpenAgentsInc/openagents issue #{case['issue']}; fix "
                      f"{(case.get('last_commit') or case['commit'])[:12]}; file {r['path']} at {case['parent'][:12]}",
            "license": "Apache-2.0 (repository LICENSE)",
            "permission": PERMISSION,
        },
        "evidence_class": "measured",
    }


def assign(cases, pos, dev_from, locked_after, n_c, n_t):
    """Partition by the last fix commit's place in history (pos 0 = newest).

    locked       fixes newer than `locked_after` (the finder ranker's training cutoff)
    development  fixes from `dev_from` (the oldest #11210 bench fix) through `locked_after`:
                 the finder bench's window, which X2a compares on
    calibration  the n_c issues fixed just before that window
    training     the n_t issues before those"""
    last = lambda c: pos[c.get("last_commit", c["commit"])]
    order = sorted(cases, key=lambda c: -last(c))  # oldest first
    part = {}
    older = []
    for c in order:
        if last(c) < pos[locked_after]:
            part[c["issue"]] = "locked"
        elif last(c) <= pos[dev_from]:
            part[c["issue"]] = "development"
        else:
            older.append(c)
    for c in older[-n_c:]:
        part[c["issue"]] = "calibration"
    for c in older[-(n_c + n_t):-n_c]:
        part[c["issue"]] = "training"
    return part, [c for c in order if c["issue"] in part]


def cmd_build(a):
    data = json.load(open(a.dataset))
    rev = ff.git(a.repo, "rev-parse", data["rev"]).decode().strip()
    log = ff.git(a.repo, "log", rev, "--no-merges", "--format=%H").decode().split()
    pos = {h: i for i, h in enumerate(log)}  # 0 = newest
    cases = [c for c in data["cases"] if c["issue"] not in CLEF_BENCH | NEWER_FIX]
    excluded = sorted(c["issue"] for c in data["cases"] if c["issue"] in CLEF_BENCH | NEWER_FIX)
    # one positive at least: drop cases whose fix changed no existing text file before sizing
    usable = [c for c in cases if any(h["existing"] and text_file(h["path"]) for h in c["hand"])]
    dev_from = ff.git(a.repo, "rev-parse", a.dev_from).decode().strip()
    locked_after = ff.git(a.repo, "rev-parse", a.locked_after).decode().strip()
    part_of, order = assign(usable, pos, dev_from, locked_after, a.calibration, a.training)
    by_issue = {c["issue"]: c for c in order}
    print(f"{len(order)} issues: " + ", ".join(f"{k} {v}" for k, v in Counter(part_of.values()).items()),
          file=sys.stderr)
    rows = build_rows(a.repo, order)
    heads = heads_for(a.repo, rows)
    rows = [r for r in rows if heads.get(r["blob"]) is not None and len(heads[r["blob"]].strip()) >= 40]
    items, meta = [], []
    for r in rows:
        c = by_issue[r["issue"]]
        it = corpus_item(r, c, heads[r["blob"]], part_of[r["issue"]])
        items.append(it)
        meta.append(r)
    # leakage: drop the later-partition item of every cross-partition near-duplicate pair
    rank = {"training": 0, "calibration": 1, "development": 2, "locked": 3}
    drop = set()
    for i, j in leak_pairs(items):
        drop.add(i if rank[items[i]["partition"]] > rank[items[j]["partition"]] else j)
    keep = [k for k in range(len(items)) if k not in drop]
    items = [items[k] for k in keep]
    meta = [meta[k] for k in keep]
    # an issue left without a positive or a negative is no longer a decision; drop it whole
    lab = defaultdict(set)
    for it in items:
        lab[it["group"]].add(it["label"])
    whole = {g for g, s in lab.items() if s != {"true", "false"}}
    sel = [k for k, it in enumerate(items) if it["group"] not in whole]
    items = [items[k] for k in sel]
    meta = [meta[k] for k in sel]
    os.makedirs(a.out_dir, exist_ok=True)
    corpus = {"v": "openagents.tenant_training.corpus.v1", "workspace": "openagents", "name": NAME,
              "created": a.created, "label_rules": LABEL_RULE,
              "retention": {"days": 0, "access": "operator", "artifacts": "digests-only"},
              "digest": "", "items": items}
    corpus_path = os.path.join(a.out_dir, f"{NAME}.corpus.json")
    with open(corpus_path, "w") as f:
        json.dump(corpus, f, ensure_ascii=False)
    manifest_rows = []
    for it, r in zip(items, meta):
        c = by_issue[r["issue"]]
        manifest_rows.append({"issue": r["issue"], "partition": it["partition"], "fix": c.get("last_commit") or c["commit"],
                              "parent": c["parent"], "path": r["path"], "blob": r["blob"], "label": r["label"],
                              "kind": r["kind"], "issue_sha256": sha(c["title"] + "\n\n" + (c["body"] or "")),
                              "state_sha256": sha(it["state"])})
    write_manifest(a.out_dir, manifest_rows)
    counts = defaultdict(Counter)
    issues = defaultdict(set)
    for it in items:
        counts[it["partition"]][it["label"]] += 1
        issues[it["partition"]].add(it["group"])
    span = {}
    for p in ("training", "calibration", "development", "locked"):
        ns = sorted(int(g.split("-")[1]) for g in issues[p])
        fixes = [m["fix"] for m in manifest_rows if m["partition"] == p]
        span[p] = {"issues": len(ns), "items": sum(counts[p].values()), "true": counts[p]["true"],
                   "false": counts[p]["false"],
                   "oldest_fix": max(fixes, key=lambda h: pos[h]) if fixes else None,
                   "newest_fix": min(fixes, key=lambda h: pos[h]) if fixes else None}
    summary = {
        "schema": "openagents.gym.file-relevance-manifest.v1", "name": NAME, "evidence_class": "measured",
        "rev": rev, "dataset_rule": "scripts/bench/file-finding-dataset.py --multi --max-hand 40",
        "question": {"id": QUESTION_ID, "type": "noul", "instructions": INSTRUCTIONS},
        "state_rule": {"issue_chars": ISSUE_CHARS, "head_bytes": HEAD_BYTES,
                       "format": "ISSUE #N: title\\n\\nbody\\n\\nFILE: path\\n```\\nhead\\n```"},
        "label_rule": LABEL_RULE, "label_source": "measurement", "teacher": "none recorded (field reserved)",
        "candidates": {"positive_max": MAX_POS, **TAKE},
        "split_rule": ("time: issues ordered by last fix commit; training < calibration < development < "
                       "locked. locked = fixes after the finder ranker's training cutoff; development = the "
                       "#11210 bench window (from its oldest fix through that cutoff)"),
        "split_commits": {"development_from": dev_from, "locked_after": locked_after},
        "sizes": {"training": a.training, "calibration": a.calibration},
        "excluded": {"clef_relevance_bench": sorted(CLEF_BENCH), "newer_fix_check_set": sorted(NEWER_FIX),
                     "present_in_dataset": excluded},
        "leak_dropped": len(drop), "issues_dropped_unbalanced": len(whole),
        "partitions": span,
        "totals": {"issues": sum(v["issues"] for v in span.values()), "items": len(items),
                   "true": sum(v["true"] for v in span.values())},
        "digests": {name: "sha256:" + hashlib.sha256(open(os.path.join(a.out_dir, name), "rb").read()).hexdigest()
                    for name in ("items.tsv.gz", "issues.tsv")},
        "locked_reads": 0,
    }
    with open(os.path.join(a.out_dir, "manifest.json"), "w") as f:
        json.dump(summary, f, indent=1, sort_keys=True)
        f.write("\n")
    print(json.dumps({k: summary[k] for k in ("partitions", "totals", "leak_dropped", "issues_dropped_unbalanced")},
                     indent=1))
    print(f"corpus: {corpus_path}")


ITEM_COLS = ("issue", "partition", "path", "blob", "label", "kind", "state_sha256")
ISSUE_COLS = ("issue", "partition", "fix", "parent", "issue_sha256")


def write_manifest(out_dir, rows):
    """items.tsv (one line per item) and issues.tsv (one per issue): ids, commits, digests, labels.
    Every item carries evidence class `measured` (manifest.json says so once)."""
    # gzip with mtime 0, so the same rows give the same bytes
    with gzip.GzipFile(os.path.join(out_dir, "items.tsv.gz"), "wb", mtime=0) as gz, \
            io.TextIOWrapper(gz, encoding="utf-8") as f:
        f.write("\t".join(ITEM_COLS) + "\n")
        for r in rows:
            f.write("\t".join(str(r[c]) for c in ITEM_COLS) + "\n")
    seen = {}
    for r in rows:
        seen.setdefault(r["issue"], r)
    with open(os.path.join(out_dir, "issues.tsv"), "w") as f:
        f.write("\t".join(ISSUE_COLS) + "\n")
        for n in sorted(seen):
            f.write("\t".join(str(seen[n][c]) for c in ISSUE_COLS) + "\n")


def read_manifest(suite_dir):
    def tsv(name):
        path = os.path.join(suite_dir, name)
        raw = gzip.open(path, "rt", encoding="utf-8").read() if name.endswith(".gz") else open(path).read()
        lines = raw.splitlines()
        cols = lines[0].split("\t")
        return [dict(zip(cols, l.split("\t"))) for l in lines[1:]]
    issues = {int(r["issue"]): r for r in tsv("issues.tsv")}
    rows = []
    for r in tsv("items.tsv.gz"):
        r["issue"] = int(r["issue"])
        r.update({k: issues[r["issue"]][k] for k in ("fix", "parent", "issue_sha256")})
        rows.append(r)
    return rows


def cmd_materialize(a):
    rows = read_manifest(a.manifest)
    issues = {i["number"]: i for i in json.load(open(a.issues))}
    heads = heads_for(a.repo, rows)
    items, bad = [], Counter()
    for r in rows:
        iss = issues.get(r["issue"])
        if iss is None:
            bad["issue missing"] += 1
            continue
        if sha(iss["title"] + "\n\n" + (iss["body"] or "")) != r["issue_sha256"]:
            bad["issue text changed since the manifest"] += 1
        case = {"issue": r["issue"], "title": iss["title"], "body": iss["body"] or "", "parent": r["parent"],
                "last_commit": r["fix"]}
        it = corpus_item(r, case, heads[r["blob"]], r["partition"])
        if sha(it["state"]) != r["state_sha256"]:
            bad["state digest mismatch"] += 1
        items.append(it)
    corpus = {"v": "openagents.tenant_training.corpus.v1", "workspace": "openagents", "name": NAME,
              "created": a.created, "label_rules": LABEL_RULE,
              "retention": {"days": 0, "access": "operator", "artifacts": "digests-only"},
              "digest": "", "items": items}
    with open(a.out, "w") as f:
        json.dump(corpus, f, ensure_ascii=False)
    print(f"{len(items)} items -> {a.out}; mismatches: {dict(bad) or 'none'}")
    if bad:
        sys.exit(1)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("cmd", choices=["build", "materialize"])
    ap.add_argument("--repo", default=".")
    ap.add_argument("--dataset", help="build: file-finding-dataset.py output (--multi)")
    ap.add_argument("--out-dir", help="build: where the manifest and corpus go")
    ap.add_argument("--dev-from", default="9f5f8ad756", help="oldest fix of the #11210 bench (100 newest single-commit cases)")
    ap.add_argument("--locked-after", default="63a5197dfb", help="the finder ranker's training cutoff (its dataset rev)")
    ap.add_argument("--calibration", type=int, default=150, help="issues in the calibration partition")
    ap.add_argument("--training", type=int, default=450, help="issues in the training partition")
    ap.add_argument("--manifest", help="materialize: the suite directory holding items.tsv and issues.tsv")
    ap.add_argument("--issues", help="materialize: closed-issues JSON (gh issue list ... --json number,title,body)")
    ap.add_argument("--out", help="materialize: corpus path")
    ap.add_argument("--created", default="2026-10-10")
    a = ap.parse_args()
    {"build": cmd_build, "materialize": cmd_materialize}[a.cmd](a)


if __name__ == "__main__":
    main()
