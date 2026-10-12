#!/usr/bin/env python3
"""The briefing for one issue (#11211): a complete hand-off, made
deterministically from the repository at the issue's base commit.

    briefing.py ISSUE [--finder lite|oracle|finder] [--out DIR] [lever=value ...]

It writes `briefing.json` and `briefing.md`:

- **Plan**: the issue restated as a short change plan.
- **Files**: each file to change, with excerpts and why it is listed.
  `lite` finds them from the issue text alone (paths, symbols and quoted
  messages it names, searched across the repo at the base commit and
  weighted by rarity). `oracle` lists the fix's real files: an upper
  bound, never a result. `finder` runs the context finder of #11210 when
  its CLI is present and falls back to `lite`.
- **History**: the most recent small past changes to the top files, as a
  pattern to copy.
- **Checks**: the exact commands, for the packages the files belong to.
- **Rules**: the repo rules that apply (AGENTS.md's core and the crate's
  own entry).

Everything is read from git objects (`git grep REV`, `git show REV:path`),
so no checkout is needed and nothing after the base commit is visible.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import shutil
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path

from common import HERE, LEVERS, REPO, WORK, dump_json, git, is_test_file, load_json, package_of, run, show

TASKS = HERE / "tasks"
SKIP = (":!bench/", ":!assets/", ":!**/*.lock", ":!**/*.json", ":!**/*.svg", ":!**/*.png")
STOP = {
    "the", "and", "that", "this", "with", "from", "when", "into", "issue", "should", "would",
    "there", "their", "which", "while", "where", "about", "after", "before", "still", "every",
    "error", "test", "tests", "fails", "failed", "cargo", "crate", "crates", "main", "true",
    "false", "none", "some", "self", "string", "result", "option", "value", "openagents",
}

CORE_RULES = [
    "Product code is Rust. Do not add TypeScript.",
    "The check for a change is `cargo test -p` for the crates you edited plus `cargo fmt`. "
    "Do not run clippy, release gates, or other crates' tests.",
    "Never put machine talk (internal words like retained, projection, canonical, digest, lane) "
    "in text a user sees; say what happened in plain words. Each surface's tests run the "
    "`oa-copy` guard over user-visible text.",
    "When a test fails only because a checked-in generated file is stale, regenerate it.",
    "No new INVARIANTS rows, design notes, or long docs for a small change.",
    "Fix stale or false user-facing copy you touch in the same change.",
]


def tokens_of(text: str) -> dict[str, float]:
    """Search terms from the issue text, with a base weight each."""
    terms: dict[str, float] = {}
    for span in re.findall(r"`([^`\n]{3,80})`", text):
        span = span.strip()
        for part in re.split(r"[\s(),;]+", span):
            part = part.strip(".:'\"")
            if len(part) >= 4 and part.lower() not in STOP:
                terms[part] = max(terms.get(part, 0), 3.0)
        if 6 <= len(span) <= 60 and " " in span:
            terms[span] = max(terms.get(span, 0), 4.0)
    for quoted in re.findall(r"[\"“]([^\"”\n]{12,90})[\"”]", text):
        terms[quoted.strip()] = max(terms.get(quoted.strip(), 0), 5.0)
    for ident in re.findall(r"\b[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)+\b", text):
        for part in ident.split("::"):
            if len(part) >= 4 and part.lower() not in STOP:
                terms[part] = max(terms.get(part, 0), 3.0)
    for ident in re.findall(r"\b(?:[a-z0-9]+_[a-z0-9_]+|[A-Z][a-z0-9]+(?:[A-Z][a-z0-9]+)+)\b", text):
        if len(ident) >= 5:
            terms[ident] = max(terms.get(ident, 0), 2.5)
    for path in re.findall(r"\b[\w./-]+\.(?:rs|toml|md|sh|py)\b", text):
        terms[path] = max(terms.get(path, 0), 6.0)
    for word in re.findall(r"\b[a-z][a-z-]{5,}\b", text.lower()):
        if word not in STOP and word not in terms:
            terms[word] = 0.6
    return terms


def grep_files(rev: str, term: str) -> dict[str, list[int]]:
    proc = subprocess.run(
        ["git", "grep", "-n", "-I", "-F", "--max-count=40", "-e", term, rev, "--", ".", *SKIP],
        cwd=REPO, capture_output=True, text=True,
    )
    hits: dict[str, list[int]] = defaultdict(list)
    for line in proc.stdout.splitlines()[:4000]:
        parts = line.split(":", 3)
        if len(parts) >= 3 and parts[2].isdigit():
            hits[parts[1]].append(int(parts[2]))
    return hits


def all_paths(rev: str) -> list[str]:
    return git("ls-tree", "-r", "--name-only", rev).splitlines()


def lite_files(task: dict, levers: dict) -> list[dict]:
    rev = task["parent"]
    text = task["title"] + "\n" + (task["body"] or "")
    terms = tokens_of(text)
    paths = all_paths(rev)
    scores: dict[str, float] = defaultdict(float)
    lines: dict[str, set[int]] = defaultdict(set)
    reasons: dict[str, list[str]] = defaultdict(list)
    # Paths the issue names, matched by suffix.
    for term, weight in terms.items():
        if "/" in term or re.search(r"\.(rs|toml|md|sh|py)$", term):
            for p in paths:
                if p.endswith(term) or (("/" in term) and term.rstrip("/") in p):
                    scores[p] += 8.0
                    reasons[p].append(f"the issue names `{term}`")
    # Strong terms: grep the repo, weighted by rarity.
    strong = sorted((t for t, w in terms.items() if w >= 2.5), key=lambda t: -terms[t])[:40]
    for term in strong:
        hits = grep_files(rev, term)
        if not hits or len(hits) > 120:
            continue
        rarity = 1.0 / math.log(2 + len(hits))
        for path, found in hits.items():
            if path.startswith("docs/") and not path.endswith(".md"):
                continue
            weight = terms[term] * rarity * (1.0 if not path.startswith("docs/") else 0.4)
            scores[path] += weight
            lines[path].update(found[:12])
            if len(reasons[path]) < 6:
                reasons[path].append(f"has `{term}`")
    # Crate names the issue mentions lift that crate's files a little.
    crates = {p.split("/")[1] for p in paths if p.startswith("crates/") and p.count("/") >= 2}
    named = {c for c in crates if re.search(rf"\b{re.escape(c)}\b", text)}
    for path in list(scores):
        if path.startswith("crates/") and path.split("/")[1] in named:
            scores[path] *= 1.5
    ranked = sorted(scores, key=lambda p: -scores[p])
    picked = []
    for path in ranked:
        if path.endswith(".md") and len(picked) < 2:
            continue
        picked.append(path)
        if len(picked) >= int(levers["briefing_files"]):
            break
    return [
        {"path": p, "score": round(scores[p], 2), "lines": sorted(lines[p]), "why": reasons[p][:6]}
        for p in picked
    ]


def oracle_files(task: dict) -> list[dict]:
    rev = task["parent"]
    diff = git("diff", "-U0", rev, task["fix"])
    lines: dict[str, list[int]] = defaultdict(list)
    current = None
    for line in diff.splitlines():
        if line.startswith("+++ "):
            current = line[6:] if line.startswith("+++ b/") else None
        m = re.match(r"@@ -(\d+)", line)
        if m and current:
            lines[current].append(int(m.group(1)))
    out = []
    for path in task["source_files"] + task["test_files"]:
        if path.endswith("Cargo.lock"):
            continue
        why = ["the fix changed it (oracle)"]
        if path in task["test_files"]:
            why.append("tests for the change go here")
        out.append({"path": path, "score": None, "lines": lines.get(path, []), "why": why})
    return out


SECRETS = Path(os.environ.get("AB_OPENROUTER_ENV", "/Users/christopherdavid/work/.secrets/openrouter.env"))


def finder_tool() -> Path | None:
    """#11210's finder (`scripts/filefind/filefind.py`), taken from
    origin/main into the bench's work dir."""
    dest = WORK / "filefind"
    dest.mkdir(parents=True, exist_ok=True)
    # cards.py too: filefind imports it (#11249); refreshed when main changes.
    for name in ("filefind.py", "model.json", "cards.py"):
        text = show("origin/main", f"scripts/filefind/{name}")
        if text is None:
            return None
        path = dest / name
        if not path.exists() or path.read_text() != text:
            path.write_text(text)
    return dest / "filefind.py"


def finder_env() -> dict:
    """The environment with the embeddings key loaded from the secrets
    file (never printed)."""
    env = dict(os.environ)
    if "OPENROUTER_API_KEY" not in env and SECRETS.exists():
        for line in SECRETS.read_text().splitlines():
            line = line.strip().removeprefix("export ")
            if line.startswith("OPENROUTER_API_KEY="):
                env["OPENROUTER_API_KEY"] = line.split("=", 1)[1].strip().strip("'\"")
    return env


def finder_files(task: dict, levers: dict, no_cards: bool = False) -> tuple[list[dict], str]:
    """The #11210 context finder at the base commit; `lite` if it fails.
    `no_cards` leaves out its card decision (the Haiku briefer's input)."""
    tool = finder_tool()
    if tool is not None:
        proc = subprocess.run(
            [sys.executable, str(tool), "query", "--repo", str(REPO), "--rev", task["parent"],
             "--issue", str(task["issue"]), "--k", str(int(levers["briefing_files"])), "--json"]
            + (["--no-cards"] if no_cards else []),
            cwd=REPO, capture_output=True, text=True, timeout=600, env=finder_env(),
        )
        try:
            found = json.loads(proc.stdout)["files"]
        except (json.JSONDecodeError, KeyError):
            found = None
        if found:
            hits = {f["path"]: f["lines"] for f in lite_files(task, dict(levers, briefing_files=60))}
            return [
                {"path": f["path"], "score": f.get("confidence"), "lines": hits.get(f["path"], []),
                 "why": [f"finder confidence {f.get('confidence')}"] + list(f.get("reasons") or [])[:4]}
                for f in found
            ], "filefind (#11210)"
    return lite_files(task, levers), "lite (finder unavailable)"


def outline(rows: list[str], cap: int) -> list[int]:
    """Line numbers of a Rust file's items (fn, struct, enum, impl, mod,
    trait, const), for a file the finder listed without a text match."""
    pat = re.compile(r"^\s*(pub(\([^)]*\))?\s+)?(async\s+)?(fn|struct|enum|impl|mod|trait|const|static|type)\b|^\s*#\[cfg\(test\)\]")
    return [i for i, row in enumerate(rows, 1) if pat.match(row)][:cap]


def excerpt(rev: str, path: str, hit_lines: list[int], levers: dict) -> str:
    text = show(rev, path)
    if text is None:
        return "(new file)"
    rows = text.splitlines()
    cap = int(levers["excerpt_lines"])
    if not hit_lines and len(rows) > cap and levers["excerpt"] != "whole":
        items = outline(rows, min(cap, 60))
        if items:
            body = [f"--- outline: {len(items)} items of {len(rows)} lines ---"]
            body += [f"{i:>5}  {rows[i - 1]}" for i in items]
            return "\n".join(body)
    if levers["excerpt"] == "whole" or len(rows) <= cap or not hit_lines:
        chosen = range(1, min(len(rows), cap if hit_lines else min(cap, 60)) + 1)
        if levers["excerpt"] == "whole" or len(rows) <= cap:
            chosen = range(1, min(len(rows), cap) + 1)
        spans = [(chosen.start, chosen.stop - 1)]
    else:
        spans = []
        for ln in sorted(hit_lines):
            lo, hi = max(1, ln - 12), min(len(rows), ln + 18)
            if spans and lo <= spans[-1][1] + 3:
                spans[-1] = (spans[-1][0], max(spans[-1][1], hi))
            else:
                spans.append((lo, hi))
        budget, kept = cap, []
        for lo, hi in spans:
            if budget <= 0:
                break
            hi = min(hi, lo + budget - 1)
            kept.append((lo, hi))
            budget -= hi - lo + 1
        spans = kept
    out = []
    for lo, hi in spans:
        out.append(f"--- lines {lo}-{hi} of {len(rows)} ---")
        for i in range(lo, hi + 1):
            out.append(f"{i:>5}  {rows[i - 1]}")
    return "\n".join(out)


def history(rev: str, files: list[str], n: int) -> list[dict]:
    seen, out = set(), []
    for path in files[:3]:
        log = git("log", rev, "-n", "8", "--no-merges", "--format=%H%x09%s", "--", path).splitlines()
        for row in log:
            sha, subject = row.split("\t", 1)
            if sha in seen:
                continue
            seen.add(sha)
            stat = git("show", "--numstat", "--format=", sha)
            changed = sum(
                int(a) + int(d) for a, d, *_ in (r.split("\t") for r in stat.splitlines() if r.count("\t") >= 2)
                if a.isdigit() and d.isdigit()
            )
            if changed > 160 or changed == 0:
                continue
            diff = git("show", "--format=", "-U2", sha, "--", path)
            out.append({
                "commit": sha[:10], "subject": subject, "file": path,
                "diff": "\n".join(diff.splitlines()[:70]),
            })
            break
        if len(out) >= n:
            break
    return out


def crate_rule(path: str, rev: str) -> str | None:
    if not path.startswith("crates/"):
        return None
    crate = path.split("/")[1]
    agents = show(rev, "AGENTS.md") or ""
    m = re.search(rf"(?ms)^- `crates/{re.escape(crate)}`.*?(?=^- `|^## )", agents)
    return " ".join(m.group(0).split())[:900] if m else None


def seeded(files: list[dict], seeds: list[dict], levers: dict) -> list[dict]:
    """A re-briefing's files (#11311): the first attempt's evidence first
    (files it had to open, files its checks blamed), then the finder's."""
    out, seen = [], set()
    for s_ in seeds:
        if s_["path"] in seen:
            continue
        seen.add(s_["path"])
        out.append({"path": s_["path"], "score": None, "lines": sorted(set(s_.get("lines") or [])),
                    "why": list(s_.get("why") or ["the first attempt needed it"])[:4]})
    for f in files:
        if f["path"] not in seen:
            seen.add(f["path"])
            out.append(f)
    return out[: int(levers["briefing_files"]) + min(4, len(seeds))]


def kind_packages(rev: str, files: list[dict], levers: dict) -> tuple[list[dict], list[dict]]:
    """(packages, checks) for a kinds briefing (#11310): the packages of the
    finder's top files grouped by kind, at most `packages` of them, with
    each package's checks from `kinds.py`."""
    import kinds
    tree = kinds.Tree(REPO, rev)
    top = [f["path"] for f in files][:6]
    pkgs = [p for p in kinds.group(top, tree) if p["kind"] != "other"][: int(levers["packages"])]
    for f in files:
        for p in pkgs:
            if f["path"] not in p["files"] and any(
                    q["id"] == p["id"] and q["kind"] == p["kind"] for q in kinds.group([f["path"]], tree)):
                p["files"].append(f["path"])
    checks = kinds.plan([f for p in pkgs for f in p["files"]], tree, rev)
    return pkgs, checks


def _shrinkable(f: dict) -> bool:
    """An excerpt the budget trim can still cut: over 400 characters and over
    the 10 lines a cut keeps. An excerpt of 10 or fewer long lines cannot
    shrink, and choosing it looped forever (ablate-levers' Blarge on #10295,
    s2b's B0 on #10095). Every trim that ended before this rule chose only
    shrinkable excerpts, so those briefings are unchanged."""
    return len(f["excerpt"]) > 400 and len(f["excerpt"].splitlines()) > 10


def budget_by_package(briefing: dict, budget_chars: int) -> str:
    """Trim excerpts so each package keeps a fair share of the briefing:
    the package most over its share loses lines from its longest excerpt."""
    md = render(briefing)
    pkgs = briefing.get("packages") or []
    owner = {f: p["id"] for p in pkgs for f in p["files"]}
    while len(md) > budget_chars and any(_shrinkable(f) for f in briefing["files"]):
        by: dict[str, int] = {}
        for f in briefing["files"]:
            by[owner.get(f["path"], "")] = by.get(owner.get(f["path"], ""), 0) + len(f["excerpt"])
        share = max(1, budget_chars // max(1, len(by)))
        worst = max(by, key=lambda k: by[k] - share)
        cands = [f for f in briefing["files"] if owner.get(f["path"], "") == worst and _shrinkable(f)] \
            or [f for f in briefing["files"] if _shrinkable(f)]
        if not cands:
            break
        longest = max(cands, key=lambda f: len(f["excerpt"]))
        rows = longest["excerpt"].splitlines()
        longest["excerpt"] = "\n".join(rows[: max(10, len(rows) * 2 // 3)])
        md = render(briefing)
    return md


TESTED_KINDS = ("rust", "rust-nested", "python", "shell")


def haiku_files(task: dict, levers: dict, root: Path | None) -> tuple[list[dict], str, dict]:
    """The Haiku briefer (haiku_briefer.py): filefind's candidates without its
    card decision (none for `haiku-solo`), then a constrained Haiku agent
    reading the checkout at the base. Falls back to filefind's own list."""
    import haiku_briefer as hb

    solo = levers["finder"] == "haiku-solo"
    t0 = time.time()
    cands, used = ([], "none") if solo else finder_files(
        task, dict(levers, briefing_files=int(levers.get("briefer_candidates", hb.DEFAULTS["briefer_candidates"]))),
        no_cards=True)
    meta = {"finder_secs": round(time.time() - t0, 2), "candidates": [c["path"] for c in cands]}
    if root is None:
        meta["fallback"] = "no checkout at the base commit"
    else:
        r = hb.ask(task, Path(root), levers, cands or None)
        meta.update({k: r[k] for k in r if k != "answer"})
        files = hb.to_files(r["answer"], Path(root), int(levers["briefing_files"])) if r["ok"] else []
        if files:
            meta["plan"] = hb.plan_lines(r["answer"])
            meta["answer"] = r["answer"]
            return files, f"haiku briefer ({hb.MODEL}{', solo' if solo else ''})", meta
        meta["fallback"] = r["error"] or "Haiku named no file that exists"
    if solo:
        cands, used = finder_files(task, levers)
    return cands[: int(levers["briefing_files"])], f"{used}; Haiku fell back", meta


def door_files(task: dict, levers: dict) -> tuple[list[dict], str, dict]:
    """The Briefer's file judgments asked of a decision door (#11389):
    filefind's candidates without its card decision, then decide.rs's Noul
    per file at the door the lever names (`clef-arm`, `hosted-arm`,
    `jev-arm`). Kept files (p >= 0.35, at least 3) in probability order.
    Falls back to filefind's own list when the door does not answer."""
    import briefer_arms as arms

    arm = levers["finder"].removesuffix("-arm")
    cands, used = finder_files(
        task, dict(levers, briefing_files=int(levers.get("briefer_candidates", 12))), no_cards=True)
    row = arms.run_door(arm, task, cands)
    meta = {k: row[k] for k in ("arm", "secs", "cost_usd", "unpriced", "errors", "kind")}
    meta["candidates"] = [c["path"] for c in cands]
    if row["ok"] and row["keep"]:
        p = {j["path"]: j["p"] for j in row["judgments"]}
        by = {c["path"]: c for c in cands}
        files = [dict(by[path], score=p[path], why=[f"{arm} door: p={p[path]:.2f} the fix needs it"] + by[path]["why"][:2])
                 for path in sorted(row["keep"], key=lambda x: -p[x])]
        return files[: int(levers["briefing_files"])], f"{used}; {arm} door judgments", meta
    meta["fallback"] = "; ".join(row["errors"][:2]) or "the door kept no file"
    return cands[: int(levers["briefing_files"])], f"{used}; {arm} door fell back", meta


def build(task: dict, levers: dict, seeds: list[dict] | None = None, wrong: str | None = None,
          root: Path | None = None) -> dict:
    """The briefing. `seeds` and `wrong` make a re-briefing (#11311): the
    first attempt's evidence files first, and what went wrong. `root`: a
    checkout at the base commit (the Haiku briefer, `finder=haiku`, reads it)."""
    started = time.time()
    rev = task["parent"]
    finder = levers["finder"]
    briefer = None
    if finder == "oracle":
        files, finder_used = oracle_files(task), "oracle"
    elif finder in ("haiku", "haiku-solo"):
        files, finder_used, briefer = haiku_files(task, levers, root)
    elif finder in ("clef-arm", "hosted-arm", "jev-arm"):
        files, finder_used, briefer = door_files(task, levers)
    elif finder == "finder":
        files, finder_used = finder_files(task, levers)
    else:
        files, finder_used = lite_files(task, levers), "lite"
    if seeds:
        files = seeded(files, seeds, levers)
    for f in files:
        f["excerpt"] = excerpt(rev, f["path"], f["lines"], levers)
    kinds_mode = levers.get("checks") == "kinds"
    pkgs: list[dict] = []
    if kinds_mode:
        pkgs, checks = kind_packages(rev, files, levers)
        packages = [p["id"] for p in pkgs if p["kind"] == "rust"]
        # Files grouped by package (package order, then rank), the rest last.
        rank = {f["path"]: i for i, f in enumerate(files)}
        pos = {f: i for i, p in enumerate(pkgs) for f in p["files"]}
        files = sorted(files, key=lambda f: (pos.get(f["path"], len(pkgs)), rank[f["path"]]))
    else:
        packages = []
        for f in [f for f in files if f["path"].endswith(".rs")][:3]:
            if f["path"].startswith("crates/"):
                pkg = package_of(rev, f["path"])
                if pkg and pkg not in packages:
                    packages.append(pkg)
        packages = packages[:2]
        checks = []
        for pkg in packages:
            checks += [
                {"id": f"check:{pkg}", "argv": ["cargo", "check", "-p", pkg, "--tests", "--message-format", "short"],
                 "what": f"compile {pkg} and its tests"},
                {"id": f"test:{pkg}", "argv": ["cargo", "test", "-p", pkg], "filter": True,
                 "what": f"run {pkg}'s tests (pass a test-name filter to run fewer)"},
                {"id": f"fmt:{pkg}", "argv": ["cargo", "fmt", "-p", pkg], "what": f"format {pkg}"},
            ]
    rules = list(CORE_RULES)
    if kinds_mode:
        rules[0] = "Product code is Rust; scripts stay in the language they are in. Do not add TypeScript."
        rules[1] = ("The check for a change is the briefing's checks for the packages you edited (`cargo test -p` "
                    "and `cargo fmt` for a crate; the script's tests; the link check for a document). Do not run "
                    "clippy, release gates, or other packages' tests.")
    for f in files[:3]:
        rule = crate_rule(f["path"], rev)
        if rule and rule not in rules:
            rules.append(rule)
    acc = None
    if levers.get("acceptance") == "typed":
        import acceptance
        acc = acceptance.extract({"issue": task["issue"], "title": task["title"], "body": task["body"]}, checks)
    plan = []
    if levers["plan"] != "none":
        body = task["body"] or ""
        asks = [
            re.sub(r"^\s*(?:[-*]|\d+\.)\s*", "", line).strip()
            for line in body.splitlines()
            if re.match(r"^\s*(?:[-*]|\d+\.)\s+\S", line)
        ][:8]
        if acc is not None:
            asks = [i["text"] for i in acc["items"]]
        top = [f["path"] for f in files if not is_test_file(f["path"])][:3]
        tests = [f["path"] for f in files if is_test_file(f["path"])][:2]
        plan.append(f"Goal: {task['title']}")
        for ask in asks:
            plan.append(f"Required: {ask[:220]}")
        # Haiku's acceptance points only when typed acceptance (#11310) is off.
        plan += [line for line in (briefer or {}).get("plan", [])
                 if acc is None or not line.startswith("Acceptance: ")]
        if top:
            plan.append("Change the behavior where it lives, most likely in " + ", ".join(f"`{p}`" for p in top) + ".")
        tested = not kinds_mode or any(p["kind"] in TESTED_KINDS for p in pkgs)
        if tested:
            plan.append(
                "Add or update a test that pins the new behavior"
                + (f", next to the existing ones in {', '.join(f'`{t}`' for t in tests)}" if tests else ", beside the code's existing tests")
                + "."
            )
        if kinds_mode and any(p["kind"] == "docs" for p in pkgs):
            plan.append("Keep every relative link and repository path in the documents you edit valid.")
        if kinds_mode and any(not c.get("available", True) for c in checks):
            plan.append("Some files here have no checker on this computer ("
                        + ", ".join(sorted({c["kind"] for c in checks if not c.get("available", True)}))
                        + "): read your change back carefully.")
        if checks:
            plan.append("Run " + ", then ".join(f"`{c['id']}`" for c in checks if not c["id"].startswith("fmt")
                                              and c.get("available", True)) + ", then `fmt`; stop when they pass.")
    hist = history(rev, [f["path"] for f in files if not f["path"].endswith(".md")], int(levers["history"]))
    briefing = {
        "issue": task["issue"], "title": task["title"], "body": task["body"], "base": rev,
        "finder": finder_used, "plan": plan, "files": files, "history": hist,
        "checks": checks, "rules": rules, "levers": {k: levers[k] for k in levers},
    }
    if kinds_mode:
        briefing["packages"] = [{"kind": p["kind"], "id": p["id"], "files": p["files"]} for p in pkgs]
    if acc is not None:
        briefing["acceptance"] = acc
    if wrong:
        briefing["wrong"] = wrong
    if seeds:
        briefing["seeds"] = [s_["path"] for s_ in seeds]
    if briefer is not None:
        briefing["briefer"] = {k: v for k, v in briefer.items() if k != "plan"}
    budget = int(levers["briefing_tokens"]) * 4
    if kinds_mode:
        md = budget_by_package(briefing, budget)
    else:
        md = render(briefing)
        while len(md) > budget and any(_shrinkable(f) for f in briefing["files"]):
            longest = max((f for f in briefing["files"] if _shrinkable(f)), key=lambda f: len(f["excerpt"]))
            longest["excerpt"] = "\n".join(longest["excerpt"].splitlines()[: max(10, len(longest["excerpt"].splitlines()) * 2 // 3)])
            md = render(briefing)
    briefing["build_secs"] = round(time.time() - started, 2)
    briefing["chars"] = len(md)
    return briefing


def render(b: dict) -> str:
    out = [f"# Briefing: #{b['issue']} {b['title']}", "", "## The issue", "", (b["body"] or "").strip(), ""]
    if b.get("wrong"):
        out += ["## What went wrong in the first attempt", "", b["wrong"].strip(), ""]
    if b["plan"]:
        out += ["## Change plan", ""] + [f"{i}. {s}" for i, s in enumerate(b["plan"], 1)] + [""]
    if b.get("acceptance"):
        out += ["## Acceptance", "", "What must be true when you are done, from the issue, and the check that "
                "shows each (`verify` reports every item):", ""]
        for item in b["acceptance"]["items"]:
            shown = ", ".join(f"`{c}`" for c in item["check_ids"]) or "no check here: make sure by reading"
            out.append(f"- [{item['kind']}] {item['text']} ({shown})")
        out.append("")
    label = " (ORACLE: the fix's real files; an upper bound, not a finder result)" if b["finder"] == "oracle" else ""
    out += [f"## Files to change{label}", ""]
    pkgs = b.get("packages") or []
    owner = {f: p for p in pkgs for f in p["files"]}
    shown_pkg = None
    for f in b["files"]:
        p = owner.get(f["path"])
        if len(pkgs) > 1 and p is not None and p is not shown_pkg:
            out += [f"### Package `{p['id']}` ({p['kind']})", ""]
            shown_pkg = p
        out += [f"### `{f['path']}`", "", "Why: " + "; ".join(f["why"] or ["listed by the finder"]), "", "```", f["excerpt"], "```", ""]
    if b["history"]:
        out += ["## Similar past changes", ""]
        for h in b["history"]:
            out += [f"### {h['commit']} {h['subject']}", "", "```diff", h["diff"], "```", ""]
    if b["checks"]:
        out += ["## Checks", ""]
        out += [f"- `{c['id']}`: `{' '.join(c['argv'])}{' [FILTER]' if c.get('filter') else ''}` ({c['what']})"
                + ("" if c.get("available", True) else " (not runnable on this computer)") for c in b["checks"]]
        out.append("")
    out += ["## Repo rules", ""] + [f"- {r}" for r in b["rules"]] + [""]
    return "\n".join(out)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("issue", type=int)
    ap.add_argument("--finder", default=None)
    ap.add_argument("--out", default=None)
    ap.add_argument("levers", nargs="*", help="lever=value")
    args = ap.parse_args()
    levers = dict(LEVERS)
    for kv in args.levers:
        k, v = kv.split("=", 1)
        levers[k] = type(LEVERS[k])(v) if not isinstance(LEVERS[k], bool) else v.lower() in ("1", "true", "yes")
    if args.finder:
        levers["finder"] = args.finder
    task = load_json(TASKS / str(args.issue) / "task.json")
    root = None
    if levers["finder"].startswith("haiku"):
        import ab  # the bench's shallow worktree at the base (the fix is not visible)
        root = ab.make_worktree(task, WORK / "wt" / f"brief-{args.issue}")
    try:
        b = build(task, levers, root=root)
    finally:
        if root is not None:
            ab.drop_worktree(task, root)
    out = Path(args.out) if args.out else TASKS / str(args.issue) / f"briefing-{levers['finder']}"
    out.mkdir(parents=True, exist_ok=True)
    dump_json(out / "briefing.json", b)
    (out / "briefing.md").write_text(render(b))
    fix = set(task["source_files"] + task["test_files"])
    got = {f["path"] for f in b["files"]}
    print(json.dumps({"out": str(out), "secs": b["build_secs"], "chars": b["chars"], "files": sorted(got),
                      "recall": round(len(fix & got) / max(1, len(fix)), 2)}))


if __name__ == "__main__":
    main()
