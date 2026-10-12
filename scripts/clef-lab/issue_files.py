#!/usr/bin/env python3
"""Which files does an issue's fix need? Ask a local Clef, batched.

A standalone lab for the ideas in docs/inference/clef-batching.md, run
against the solver's own first steps (docs/coder/solver.md, the Briefer's
`issue_run::decide`):

1. The context finder ranks candidates
   (`scripts/filefind/filefind.py query --json`), as the Briefer does.
2. The top K candidates are packed into one System One request: one
   bounded representation per file (path, finder rank and reasons, then an
   outline of its declarations, never the whole file), and one `noul` per
   file (f1..fK). The solver today sends one request per file instead.
3. With `--mode both` (the default) the same representations are also
   asked one file per request, so the two shapes can be compared on the
   same model and the same text.

The model defaults to Clef 27B on this Mac's Ollama: batching helped 27B
and hurt Clef-Flash in the relevance bench (F1 0.81 -> 0.88 against
0.65 -> 0.44; docs/inference/clef-jev-relevance-bench.md). Point
`--base-url` and `--model` at a Psionic server (`clef-flash` on 18098) to
compare.

Everything the run sent and got back is written to the run folder, with a
Markdown report, for review. Nothing is committed or posted anywhere.

  python3 scripts/clef-lab/issue_files.py
  python3 scripts/clef-lab/issue_files.py --k 8 --tier 2 --mode batch
  python3 scripts/clef-lab/issue_files.py --finder-json RUN/finder.json   # skip the finder
"""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import secrets
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ISSUE = 11432
GITHUB = "OpenAgentsInc/openagents"

# The solver's own question and rule (crates/coder-new/src/issue_run/decide.rs).
QUESTION = "Does the fix for this issue need to change this file?"
KEEP_AT = 0.35
MIN_FILES = 3

# Ollama refuses a body over 64 KiB of text and schema; stay under it.
OLLAMA_BODY_CAP = 60 * 1024
# A rough byte-per-token figure for this repository (decision-throughput.md).
BYTES_PER_TOKEN = 3.3
TOKEN_BUDGET = 16_384

DECL = re.compile(
    r"^\s*(pub(\([^)]*\))?\s+)?(async\s+)?(unsafe\s+)?"
    r"(fn|struct|enum|impl|mod|trait|const|static|type|macro_rules!)\b"
    r"|^\s*#\[cfg\(test\)\]"
    r"|^\s*(export\s+)?(async\s+)?(function|class|interface|type|const)\s+\w+"
    r"|^\s*(def|class)\s+\w+"
    r"|^#{1,4}\s+\S"
    r"|^\[[^\]]+\]\s*$"
)


def run(argv: list[str], cwd: Path, timeout: int = 600) -> str:
    out = subprocess.run(argv, cwd=cwd, capture_output=True, text=True, timeout=timeout)
    if out.returncode != 0:
        tail = (out.stderr or out.stdout).strip().splitlines()[-3:]
        raise SystemExit(f"{argv[0]} failed: {' | '.join(tail)}")
    return out.stdout


def issue(repo: Path, number: int, github: str) -> dict:
    raw = run(["gh", "issue", "view", str(number), "--repo", github,
               "--json", "number,title,body,state"], repo)
    return json.loads(raw)


def finder(repo: Path, rev: str, number: int) -> dict:
    script = repo / "scripts/filefind/filefind.py"
    raw = run([sys.executable, str(script), "query", "--repo", str(repo), "--rev", rev,
               "--issue", str(number), "--k", "40", "--json"], repo)
    return json.loads(raw)


def show(repo: Path, rev: str, path: str) -> str | None:
    out = subprocess.run(["git", "show", f"{rev}:{path}"], cwd=repo,
                         capture_output=True, text=True, errors="replace")
    return out.stdout if out.returncode == 0 else None


def outline(text: str, cap_items: int) -> list[str]:
    """Declaration lines, `LINE  source`, the briefing's outline widened to
    Markdown headings, TOML tables and a few other languages."""
    rows = text.splitlines()
    items = [f"{i:>5}  {row.rstrip()[:160]}" for i, row in enumerate(rows, 1) if DECL.match(row)]
    return [f"--- outline: {min(len(items), cap_items)} of {len(items)} items, {len(rows)} lines ---"] + items[:cap_items]


def head(text: str, lines: int) -> list[str]:
    rows = text.splitlines()
    return [f"--- first {min(lines, len(rows))} of {len(rows)} lines ---"] + [
        f"{i:>5}  {row.rstrip()[:160]}" for i, row in enumerate(rows[:lines], 1)
    ]


def represent(repo: Path, rev: str, file: dict, fid: str, tier: int, chars: int) -> dict:
    """One file, bounded: tier 1 path and finder score, 2 + reasons,
    3 + outline (or the head when the file has no declarations)."""
    entry: dict = {"id": fid, "path": file["path"]}
    if tier >= 1:
        entry["finder"] = {"rank": file["rank"], "confidence": round(file["confidence"], 3)}
    if tier >= 2:
        entry["reasons"] = file.get("reasons", [])[:6]
    if tier >= 3:
        text = show(repo, rev, file["path"])
        if text is None:
            entry["detail"] = ["(the file does not exist at this commit)"]
        else:
            lines = outline(text, 80)
            if len(lines) <= 3:
                lines = head(text, 40)
            kept, used = [], 0
            for line in lines:
                if used + len(line) + 1 > chars:
                    kept.append("…")
                    break
                kept.append(line)
                used += len(line) + 1
            entry["detail"] = kept
    return entry


def request_body(model: str, issue_doc: dict, entries: list[dict], nonce: str) -> dict:
    body = (issue_doc.get("body") or "")[:4000]
    state = {
        # A leading key keeps a fresh nonce early in the rendered state, so
        # no prompt cache answers a repeat (render() sorts keys).
        "_run": nonce,
        "issue": {"number": issue_doc["number"], "title": issue_doc["title"], "body": body},
        "candidates": entries,
    }
    questions = {
        e["id"]: {
            "type": "noul",
            "instructions": f"{QUESTION} The file is {e['id']}: {e['path']}",
        }
        for e in entries
    }
    return {"model": model, "state": state, "questions": questions}


def ask(base_url: str, body: dict, timeout: int) -> tuple[dict, float]:
    data = json.dumps(body, ensure_ascii=False).encode()
    req = urllib.request.Request(base_url.rstrip("/") + "/v1/systemone", data=data,
                                 headers={"content-type": "application/json"})
    started = time.monotonic()
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            answer = json.loads(resp.read())
    except urllib.error.HTTPError as error:
        raise SystemExit(f"{error.code} from {base_url}: {error.read().decode(errors='replace')[:400]}")
    return answer, time.monotonic() - started


def fit(repo: Path, rev: str, files: list[dict], tier: int, model: str, issue_doc: dict,
        cap_bytes: int) -> tuple[list[dict], int]:
    """The richest per-file detail whose batched body stays under the cap."""
    chars = 4000
    while True:
        entries = [represent(repo, rev, f, f"f{i}", tier, chars) for i, f in enumerate(files, 1)]
        size = len(json.dumps(request_body(model, issue_doc, entries, "x" * 16), ensure_ascii=False).encode())
        if size <= cap_bytes or chars <= 200 or tier < 3:
            return entries, chars
        chars = int(chars * 0.8)


def logit(p: float) -> float:
    p = min(max(p, 1e-6), 1 - 1e-6)
    return math.log(p / (1 - p))


def solver_keep(files: list[dict], p: dict[str, float]) -> list[str]:
    """`choose()` from decide.rs: the finder's order, minus files under
    KEEP_AT, refilled to MIN_FILES."""
    kept = [f["path"] for f in files if p.get(f["path"], 1.0) >= KEEP_AT]
    dropped = [f["path"] for f in files if p.get(f["path"], 1.0) < KEEP_AT]
    while len(kept) < min(MIN_FILES, len(files)) and dropped:
        kept.append(dropped.pop(0))
    return kept


def main() -> None:
    here = Path(__file__).resolve().parents[2]
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--issue", type=int, default=ISSUE)
    ap.add_argument("--github", default=GITHUB)
    ap.add_argument("--repo", type=Path, default=here, help="the checkout the finder and git read")
    ap.add_argument("--rev", default="origin/main")
    ap.add_argument("--base-url", default="http://127.0.0.1:11434", help="a System One server")
    ap.add_argument("--model", default="clef", help="clef (27B) or clef-flash")
    ap.add_argument("--k", type=int, default=12, help="candidates to ask about (the solver asks 6-12)")
    ap.add_argument("--tier", type=int, default=3, choices=[1, 2, 3],
                    help="per-file detail: 1 path+score, 2 +reasons, 3 +outline")
    ap.add_argument("--mode", default="both", choices=["batch", "single", "both"])
    ap.add_argument("--cap-bytes", type=int, default=OLLAMA_BODY_CAP,
                    help="largest batched body; Ollama refuses past 64 KiB")
    ap.add_argument("--finder-json", type=Path, help="reuse a saved finder answer")
    ap.add_argument("--out", type=Path, help="run folder (default ~/.openagents/clef-lab/ISSUE-TIME)")
    ap.add_argument("--timeout", type=int, default=900)
    args = ap.parse_args()

    repo = args.repo.resolve()
    out = args.out or Path.home() / ".openagents/clef-lab" / f"{args.issue}-{time.strftime('%Y%m%d-%H%M%S')}"
    out.mkdir(parents=True, exist_ok=True)
    say = lambda *a: print(*a, flush=True)  # noqa: E731

    say(f"issue   {args.github}#{args.issue}")
    issue_doc = issue(repo, args.issue, args.github)
    say(f"        {issue_doc['title']} ({issue_doc['state'].lower()})")
    rev = run(["git", "rev-parse", "--short", args.rev], repo).strip()

    started = time.monotonic()
    found = json.loads(args.finder_json.read_text()) if args.finder_json else finder(repo, args.rev, args.issue)
    (out / "finder.json").write_text(json.dumps(found, indent=2))
    files = found["files"][: args.k]
    say(f"finder  {len(found['files'])} ranked at {rev} in {time.monotonic() - started:.1f}s; asking about the top {len(files)}")

    entries, chars = fit(repo, args.rev, files, args.tier, args.model, issue_doc, args.cap_bytes)
    p_batch: dict[str, float] = {}
    p_single: dict[str, float] = {}
    runs: dict = {}

    if args.mode in ("batch", "both"):
        body = request_body(args.model, issue_doc, entries, secrets.token_hex(8))
        size = len(json.dumps(body, ensure_ascii=False).encode())
        say(f"batch   1 request, {len(entries)} questions, {size / 1024:.1f} KiB "
            f"(~{size / BYTES_PER_TOKEN:,.0f} tokens est.), detail ≤ {chars} chars a file …")
        if size / BYTES_PER_TOKEN > TOKEN_BUDGET:
            say(f"        warning: past the {TOKEN_BUDGET}-token trained length; lower --k or --tier")
        answer, secs = ask(args.base_url, body, args.timeout)
        (out / "batch-request.json").write_text(json.dumps(body, indent=2, ensure_ascii=False))
        (out / "batch-response.json").write_text(json.dumps(answer, indent=2, ensure_ascii=False))
        for e in entries:
            p_batch[e["path"]] = answer["answers"][e["id"]]["noul"]
        runs["batch"] = {"seconds": round(secs, 2), "bytes": size,
                         "input_tokens": answer.get("usage", {}).get("input_tokens"),
                         "model": answer.get("model")}
        say(f"        {secs:.1f}s, {runs['batch']['input_tokens']} input tokens, "
            f"{len(entries) / secs:.2f} decisions/s")

    if args.mode in ("single", "both"):
        say(f"single  {len(entries)} requests, one question each, the same detail …")
        total, tokens, singles = 0.0, 0, []
        for e in entries:
            one = dict(e, id="f1")
            body = request_body(args.model, issue_doc, [one], secrets.token_hex(8))
            answer, secs = ask(args.base_url, body, args.timeout)
            p_single[e["path"]] = answer["answers"]["f1"]["noul"]
            total += secs
            tokens += answer.get("usage", {}).get("input_tokens") or 0
            singles.append({"path": e["path"], "seconds": round(secs, 2), "answer": answer})
            say(f"        {p_single[e['path']]:.3f}  {secs:5.1f}s  {e['path']}")
        (out / "single-responses.json").write_text(json.dumps(singles, indent=2, ensure_ascii=False))
        runs["single"] = {"seconds": round(total, 2), "input_tokens": tokens}
        say(f"        {total:.1f}s in all, {tokens} input tokens, {len(entries) / total:.2f} decisions/s")

    # The table, in the finder's order.
    cols = ["#", "finder"]
    if p_batch:
        cols += ["batch", "fused"]
    if p_single:
        cols += ["single"]
    lines = ["| " + " | ".join(cols + ["file", "finder's reasons"]) + " |",
             "|" + "---|" * (len(cols) + 2)]
    for f in files:
        path = f["path"]
        row = [str(f["rank"]), f"{f['confidence']:.3f}"]
        if p_batch:
            # The frozen fusion that won in file-finding-bench.md (#11217):
            # finder logit + Clef logit, no learned weight.
            fused = 1 / (1 + math.exp(-(logit(f["confidence"]) + logit(p_batch[path]))))
            row += [f"{p_batch[path]:.3f}", f"{fused:.3f}"]
        if p_single:
            row += [f"{p_single[path]:.3f}"]
        reasons = "; ".join(f.get("reasons", [])[:2]).replace("|", "\\|")
        lines.append("| " + " | ".join(row + [f"`{path}`", reasons]) + " |")
    table = "\n".join(lines)

    picks = []
    for name, p in (("batch", p_batch), ("single", p_single)):
        if not p:
            continue
        keep = solver_keep(files, p)
        top = sorted(files, key=lambda f: -p[f["path"]])[:6]
        picks.append(f"**{name}**: the solver's rule (≥ {KEEP_AT}, at least {MIN_FILES}) keeps "
                     f"{len(keep)}: " + ", ".join(f"`{k}`" for k in keep))
        picks.append(f"**{name}**: top 6 by Clef: " + ", ".join(f"`{f['path']}` {p[f['path']]:.2f}" for f in top))
    if p_batch:
        fused_top = sorted(files, key=lambda f: -(logit(f["confidence"]) + logit(p_batch[f["path"]])))[:6]
        picks.append("**fused** (finder + batch): top 6: " + ", ".join(f"`{f['path']}`" for f in fused_top))
    if p_batch and p_single:
        flips = [f["path"] for f in files if (p_batch[f["path"]] >= KEEP_AT) != (p_single[f["path"]] >= KEEP_AT)]
        picks.append(f"batch and single disagree at {KEEP_AT} on {len(flips)} of {len(files)}: "
                     + (", ".join(f"`{x}`" for x in flips) or "none"))

    report = "\n\n".join([
        f"# {args.github}#{args.issue}: {issue_doc['title']}",
        f"Model `{args.model}` at {args.base_url}, finder at `{rev}`, top {len(files)} candidates, "
        f"tier {args.tier} detail (≤ {chars} chars a file).",
        "Timing: " + json.dumps(runs),
        table,
        "\n".join(f"- {x}" for x in picks),
    ]) + "\n"
    (out / "report.md").write_text(report)
    (out / "run.json").write_text(json.dumps({
        "issue": args.issue, "github": args.github, "rev": rev, "model": args.model,
        "base_url": args.base_url, "k": len(files), "tier": args.tier, "detail_chars": chars,
        "runs": runs, "p_batch": p_batch, "p_single": p_single,
    }, indent=2))
    say("")
    say(table)
    say("")
    for x in picks:
        say(f"- {x}")
    say(f"\nrun folder  {out}")


if __name__ == "__main__":
    main()
