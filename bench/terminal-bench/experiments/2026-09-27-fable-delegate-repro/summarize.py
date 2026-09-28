"""Per-attempt numbers and per-pass tallies for the #9776 reproduction.

Run on the host that holds the Harbor jobs, with this directory's
``tasks.json`` and ``candidates/`` beside it. It reuses the #9746 row
builder, ``../2026-09-27-fable-delegate/summarize.py`` (or a copy named
``summarize_9746.py`` beside it)::

    python3 summarize.py attempts.jsonl > attempts.json

``attempts.jsonl`` lists one attempt a line, in run order:
``{"task": ..., "pass": 1, "attempt": "p1", "kind": "result"|"fault", "note": ...}``.
The job name is ``tb4--coder-one-delegate-fable-low-kb-jev2--<task>--9776-<attempt>``.

Each row adds, to the #9746 row (timing, delegate, cost), the task's bar,
the verdict, Jev's request with every probability and fate
(``artifacts/briefing-jev.json``), the candidates with digests and whether
each was written from this task, and the verifier's summary line and
failed tests. A beat is reward 1, total cost (with the knowledge search's
embedding charge) below the cheapest win, and whole-trial time below the
fastest win; unknown cost can't beat. Faults don't count as results.
"""

import contextlib
import importlib.util
import io
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
S9746 = HERE / "summarize_9746.py"
if not S9746.exists():
    S9746 = HERE.parent / "2026-09-27-fable-delegate" / "summarize.py"
spec = importlib.util.spec_from_file_location("s9746", S9746)
s9746 = importlib.util.module_from_spec(spec)
_argv, sys.argv = sys.argv, [sys.argv[0]]  # the #9746 script runs its main at import
with contextlib.redirect_stdout(io.StringIO()):
    spec.loader.exec_module(s9746)
sys.argv = _argv

ARM = "tb4--coder-one-delegate-fable-low-kb-jev2"
TASKS = {t["task"]: t for t in json.loads((HERE / "tasks.json").read_text())["tasks"]}


def written_from(text: str) -> list[str]:
    """The ``written_from`` list of an entry's front matter."""
    m = re.search(r"written_from:\s*\n((?:\s+- .*\n)+)", text)
    return [line.strip()[2:].strip() for line in m.group(1).splitlines()] if m else []


def own(task: str, sources: list[str]) -> bool:
    return any(s == task or re.fullmatch(re.escape(task) + r"(-\d+|__\w+)?", s) for s in sources)


def candidates(task: str) -> tuple[list[dict], float]:
    doc = json.loads((HERE / "candidates" / f"{task}.json").read_text())
    usd = float(re.search(r"\$([0-9.]+) for embeddings", doc["search_output"]).group(1))
    return [
        {"rank": e["rank"], "id": e["id"], "version": e["version"], "sha256": e["sha256"],
         "score": e["score"], "chars": e["chars"], "written_from_this_task": own(task, written_from(e["text"]))}
        for e in doc["entries"]
    ], usd


def verifier(trial: Path) -> dict:
    out = trial / "verifier/test-stdout.txt"
    if not out.exists():
        return {"summary": None, "failed": []}
    lines = out.read_text(errors="replace").splitlines()
    summary = next((l.strip("= ") for l in reversed(lines) if re.search(r"\b(passed|failed|error)", l)), None)
    return {"summary": summary, "failed": [l for l in lines if l.startswith(("FAILED", "ERROR"))][:20]}


def attempt(spec: dict) -> dict:
    task, a = spec["task"], spec["attempt"]
    job = f"{ARM}--{task}--9776-{a}"
    bar = TASKS[task]
    cands, search_usd = candidates(task)
    row = {"task": task, "pass": spec["pass"], "attempt": a, "kind": spec["kind"], "note": spec.get("note"),
           "bar": {"cost_usd": bar["bar_cost_usd"], "seconds": bar["bar_seconds"],
                   "cheapest_win": bar["cheapest_win"]["id"], "fastest_win": bar["fastest_win"]["id"],
                   "delegate_deadline_sec": bar["delegate_deadline_sec"]},
           "knowledge_bearing": any(c["written_from_this_task"] for c in cands)}
    try:
        base = s9746.judge(s9746.row(job), bar["bar_cost_usd"], bar["bar_seconds"], search_usd)
    except Exception as err:  # a fault that left no episode
        row.update({"job": job, "error": f"{type(err).__name__}: {err}", "beat_the_bar": False})
        return row
    row.update(base)
    if spec["kind"] != "result":
        row["beat_the_bar"] = False
    trial = next(p for p in (s9746.JOBS / job).iterdir() if p.is_dir() and "__" in p.name)
    ep = trial / "agent/episode"
    jev_path = ep / "artifacts/briefing-jev.json"
    jev = json.loads(jev_path.read_text()) if jev_path.exists() else None
    by_id = {c["id"]: c for c in (jev or {}).get("candidates", [])}
    row["candidates"] = [{**c, "jev_p": (by_id.get(c["id"]) or {}).get("p"),
                          "fate": (by_id.get(c["id"]) or {}).get("fate")} for c in cands]
    row["jev"] = None if jev is None else {
        k: jev.get(k) for k in ("question_set", "outcome", "error", "how", "milliseconds", "input_tokens",
                                "request_key", "thresholds")
    } | {"kept": [c["id"] for c in jev.get("kept", [])],
         "requirements": [{"p": q.get("p"), "flagged": q.get("flagged"), "text": q.get("text")}
                          for q in jev.get("requirements", [])]}
    row["verifier"] = verifier(trial)
    row["deadline_hit"] = row["delegate"]["status"] not in ("answered",)
    return row


def tally(rows: list[dict]) -> dict:
    out = {}
    for p in sorted({r["pass"] for r in rows}):
        res = [r for r in rows if r["pass"] == p and r["kind"] == "result"]
        kb = [r for r in res if r["knowledge_bearing"]]
        nk = [r for r in res if not r["knowledge_bearing"]]
        count = lambda rs, f: sum(1 for r in rs if f(r))  # noqa: E731
        out[f"pass{p}"] = {
            "results": len(res),
            "faults": sum(1 for r in rows if r["pass"] == p and r["kind"] == "fault"),
            "passes": count(res, lambda r: r.get("reward") == 1),
            "beats": count(res, lambda r: r.get("beat_the_bar")),
            "knowledge_bearing": {"n": len(kb), "passes": count(kb, lambda r: r.get("reward") == 1),
                                  "beats": count(kb, lambda r: r.get("beat_the_bar"))},
            "no_knowledge": {"n": len(nk), "passes": count(nk, lambda r: r.get("reward") == 1),
                             "beats": count(nk, lambda r: r.get("beat_the_bar"))},
            "known_cost_usd": round(sum(r["cost"]["total_with_search_usd"] or 0 for r in res if r.get("cost")), 4),
            "unknown_cost_attempts": count(res, lambda r: r.get("cost") and r["cost"]["total_with_search_usd"] is None),
            "estimated_lower_bound_usd_for_unknown": round(sum(
                r["delegate"]["estimate_usd"]["total_lower_bound"] for r in res
                if r.get("cost") and r["cost"]["total_with_search_usd"] is None), 4),
        }
    return out


def main() -> None:
    specs = [json.loads(l) for l in Path(sys.argv[1]).read_text().splitlines() if l.strip()]
    rows = [attempt(s) for s in specs]
    json.dump({"attempts": rows, "tallies": tally(rows)}, sys.stdout, indent=1, default=str)
    print()


if __name__ == "__main__":
    main()
