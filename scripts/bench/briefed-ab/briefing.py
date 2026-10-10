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
    if not (dest / "filefind.py").exists():
        dest.mkdir(parents=True, exist_ok=True)
        for name in ("filefind.py", "model.json"):
            text = show("origin/main", f"scripts/filefind/{name}")
            if text is None:
                return None
            (dest / name).write_text(text)
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


def finder_files(task: dict, levers: dict) -> tuple[list[dict], str]:
    """The #11210 context finder at the base commit; `lite` if it fails."""
    tool = finder_tool()
    if tool is not None:
        proc = subprocess.run(
            [sys.executable, str(tool), "query", "--repo", str(REPO), "--rev", task["parent"],
             "--issue", str(task["issue"]), "--k", str(int(levers["briefing_files"])), "--json"],
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


def build(task: dict, levers: dict) -> dict:
    started = time.time()
    rev = task["parent"]
    finder = levers["finder"]
    if finder == "oracle":
        files, finder_used = oracle_files(task), "oracle"
    elif finder == "finder":
        files, finder_used = finder_files(task, levers)
    else:
        files, finder_used = lite_files(task, levers), "lite"
    for f in files:
        f["excerpt"] = excerpt(rev, f["path"], f["lines"], levers)
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
    for f in files[:3]:
        rule = crate_rule(f["path"], rev)
        if rule and rule not in rules:
            rules.append(rule)
    plan = []
    if levers["plan"] != "none":
        body = task["body"] or ""
        asks = [
            re.sub(r"^\s*(?:[-*]|\d+\.)\s*", "", line).strip()
            for line in body.splitlines()
            if re.match(r"^\s*(?:[-*]|\d+\.)\s+\S", line)
        ][:8]
        top = [f["path"] for f in files if not is_test_file(f["path"])][:3]
        tests = [f["path"] for f in files if is_test_file(f["path"])][:2]
        plan.append(f"Goal: {task['title']}")
        for ask in asks:
            plan.append(f"Required: {ask[:220]}")
        if top:
            plan.append("Change the behavior where it lives, most likely in " + ", ".join(f"`{p}`" for p in top) + ".")
        plan.append(
            "Add or update a test that pins the new behavior"
            + (f", next to the existing ones in {', '.join(f'`{t}`' for t in tests)}" if tests else ", beside the code's existing tests")
            + "."
        )
        if checks:
            plan.append("Run " + ", then ".join(f"`{c['id']}`" for c in checks if not c["id"].startswith("fmt")) + ", then `fmt`; stop when they pass.")
    hist = history(rev, [f["path"] for f in files if not f["path"].endswith(".md")], int(levers["history"]))
    briefing = {
        "issue": task["issue"], "title": task["title"], "body": task["body"], "base": rev,
        "finder": finder_used, "plan": plan, "files": files, "history": hist,
        "checks": checks, "rules": rules, "levers": {k: levers[k] for k in levers},
    }
    md = render(briefing)
    budget = int(levers["briefing_tokens"]) * 4
    while len(md) > budget and any(len(f["excerpt"]) > 400 for f in briefing["files"]):
        longest = max(briefing["files"], key=lambda f: len(f["excerpt"]))
        longest["excerpt"] = "\n".join(longest["excerpt"].splitlines()[: max(10, len(longest["excerpt"].splitlines()) * 2 // 3)])
        md = render(briefing)
    briefing["build_secs"] = round(time.time() - started, 2)
    briefing["chars"] = len(md)
    return briefing


def render(b: dict) -> str:
    out = [f"# Briefing: #{b['issue']} {b['title']}", "", "## The issue", "", (b["body"] or "").strip(), ""]
    if b["plan"]:
        out += ["## Change plan", ""] + [f"{i}. {s}" for i, s in enumerate(b["plan"], 1)] + [""]
    label = " (ORACLE: the fix's real files; an upper bound, not a finder result)" if b["finder"] == "oracle" else ""
    out += [f"## Files to change{label}", ""]
    for f in b["files"]:
        out += [f"### `{f['path']}`", "", "Why: " + "; ".join(f["why"] or ["listed by the finder"]), "", "```", f["excerpt"], "```", ""]
    if b["history"]:
        out += ["## Similar past changes", ""]
        for h in b["history"]:
            out += [f"### {h['commit']} {h['subject']}", "", "```diff", h["diff"], "```", ""]
    if b["checks"]:
        out += ["## Checks", ""]
        out += [f"- `{c['id']}`: `{' '.join(c['argv'])}{' [FILTER]' if c.get('filter') else ''}` ({c['what']})" for c in b["checks"]]
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
    b = build(task, levers)
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
