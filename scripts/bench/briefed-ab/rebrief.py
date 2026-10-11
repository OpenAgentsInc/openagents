"""A second briefing from the first attempt's evidence (#11311).

When the briefed agent's first attempt fails (no change, `verify` failing
again and again, or the replayed checks failing on its diff), the run used
to fall back to bare Claude Code and throw away what the attempt had
learned: the files the agent had to open that the briefing did not list
(misses, #11210) and every `verify` verdict. This module turns that
evidence into a re-briefing:

- **Seeds**: files the checks blamed (compile errors, failing tests,
  failing checks' output, verify's untouched-but-implicated files, the
  replayed checks' output), then the files the agent had to open. The
  briefing lists them first, with excerpts at the lines named.
- **What went wrong**: the first attempt's change (files and a short
  diff), its last `verify` verdict, the replayed checks that failed with
  their output, and the misses.

The driver (`scripts/work/work_issue.py`) and the bench (`ab.py`, arm
`B0R`) both call `evidence`, so production and the bench re-brief the
same way. At most one re-brief per run.
"""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

MAX_SEEDS = 6
DIFF_LINES = 120
# Paths a compiler or test prints (`path:line:col: error`, `--> path:line:col`).
PATH_LINE = re.compile(r"(?:^|\s|-->\s*)((?:crates|scripts|docs|apps|bins|os|packages|plugins)/[\w./@+-]+\.\w+):(\d+)")


def digest(text: str) -> str:
    return "sha256:" + hashlib.sha256(text.encode()).hexdigest()[:16]


def rows_of(path: Path) -> list[dict]:
    out = []
    if path.exists():
        for line in path.read_text().splitlines():
            try:
                out.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return out


def named(text: str) -> list[tuple[str, int]]:
    """(file, line) pairs a check's output names."""
    import kinds
    out = [(m.group(1), int(m.group(2))) for m in PATH_LINE.finditer(text or "")]
    out += [(i["file"], i["line"]) for i in kinds.implicated(text or "")]
    seen, keep = set(), []
    for f, n in out:
        f = f.lstrip("./")
        if (f, n) not in seen:
            seen.add((f, n))
            keep.append((f, n))
    return keep


def evidence(attempt_dir: Path, replay: list[dict] | None, diff: str, exists, worktree: str = "clean") -> dict:
    """`{seeds, wrong, sections, misses, last_verify}` from one attempt's
    folder (summary.json, verify.jsonl), its replayed checks and its diff.
    `exists(path)` says whether a path exists at the base commit."""
    summary = {}
    if (attempt_dir / "summary.json").exists():
        try:
            summary = json.loads((attempt_dir / "summary.json").read_text())
        except ValueError:
            summary = {}
    misses = [m.get("file") for m in summary.get("misses") or [] if m.get("file")]
    verdicts = [r for r in rows_of(attempt_dir / "verify.jsonl") if r.get("status")]
    last = verdicts[-1] if verdicts else None
    seeds: dict[str, dict] = {}

    def seed(path: str, line: int | None, why: str) -> None:
        path = (path or "").lstrip("./")
        if not path or not exists(path):
            return
        s = seeds.setdefault(path, {"path": path, "lines": [], "why": []})
        if line:
            s["lines"].append(int(line))
        if why not in s["why"] and len(s["why"]) < 3:
            s["why"].append(why)

    failed_replay = [c for c in replay or [] if c.get("ok") is False]
    for c in failed_replay:
        for f, n in named(c.get("tail") or ""):
            seed(f, n, f"the replayed check `{c.get('name') or c.get('id')}` names it")
    for row in reversed(verdicts[-3:]):
        for e in row.get("compile_errors") or []:
            seed(e.get("file"), e.get("line"), f"compile error: {str(e.get('msg'))[:120]}")
        for t in row.get("failing_tests") or []:
            seed(t.get("file"), t.get("line"), f"failing test {t.get('name')} asserts here")
        for c in row.get("failing_checks") or []:
            for f, n in named(c.get("tail") or ""):
                seed(f, n, f"the failing check {c.get('id')} names it")
        for u in row.get("untouched_but_implicated") or []:
            seed(u.get("file"), None, str(u.get("reason"))[:140])
    for f in misses:
        seed(f, None, "the first attempt had to open it; the briefing did not list it")
    seed_list = list(seeds.values())[:MAX_SEEDS]

    sections, wrong = [], []
    changed = sorted({line[6:] for line in diff.splitlines() if line.startswith("+++ b/")})
    if not diff.strip():
        wrong.append("- It made no change.")
        sections.append("no change")
    else:
        wrong.append("- It changed " + ", ".join(f"`{f}`" for f in changed[:10]) + ".")
        sections.append("its change")
    if last:
        bits = [f"- Its last `verify` said `{last.get('status')}`."]
        for e in (last.get("compile_errors") or [])[:5]:
            bits.append(f"  - compile error `{e.get('file')}:{e.get('line')}`: {str(e.get('msg'))[:160]}")
        for t in (last.get("failing_tests") or [])[:5]:
            bits.append(f"  - failing test `{t.get('name')}` at `{t.get('file')}:{t.get('line')}`: "
                        f"{str(t.get('assert'))[:200]}")
        for c in (last.get("failing_checks") or [])[:4]:
            bits.append(f"  - failing check `{c.get('id')}`:\n\n```\n{str(c.get('tail'))[-900:]}\n```")
        wrong += bits
        sections.append("last verify")
    elif diff.strip():
        wrong.append("- It never called `verify`.")
        sections.append("no verify")
    if failed_replay:
        wrong.append("- The checks replayed on its change failed:")
        for c in failed_replay[:4]:
            wrong.append(f"  - `{c.get('name') or c.get('id')}`:\n\n```\n{str(c.get('tail'))[-1200:]}\n```")
        sections.append("replayed checks")
    if misses:
        wrong.append("- Files it had to open that the briefing did not list (now listed first): "
                     + ", ".join(f"`{m}`" for m in misses[:8]) + ".")
        sections.append("misses")
    if diff.strip():
        rows = diff.splitlines()
        cut = "\n".join(rows[:DIFF_LINES]) + ("\n... (cut)" if len(rows) > DIFF_LINES else "")
        if worktree == "keep":
            wrong.append("- Your working copy still has that change; fix it rather than starting over.")
        else:
            wrong.append("- That change was discarded; this working copy starts clean. For reference:")
        wrong.append(f"\n```diff\n{cut}\n```")
    wrong.append("\nStart from what failed. Do not repeat the same change.")
    return {"seeds": seed_list, "wrong": "\n".join(wrong), "sections": sections, "misses": misses,
            "last_verify": (last or {}).get("status")}
