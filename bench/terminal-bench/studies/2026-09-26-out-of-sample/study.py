"""Shared pieces of the out-of-sample study tooling (#9683).

Two jobs live here:

* **Collection.** `collect()` reads a round directory on the execution host
  and returns only what the pre-registration lets an operator read about a
  held-out run: the outcome lines, each log's record path (matched from the
  log's last lines, nothing else is kept), and a whitelist of summary.json
  fields (reward, steps, time, cost, how the run ended, cost basis, entries
  used). For a record whose cost is unknown and that carries no
  `usd_upper` (made before calls recorded their bound), it also takes the
  numeric fields of each unpriced model call from `events.jsonl` (step,
  prompt size, tokens, known dollars, milliseconds), and nothing else from
  that file. Transcripts, event text, verifier output, test results, and
  model reasoning are never read into the result. Run as a script, this module
  prints that JSON, so `report.py --host` can pipe it over ssh.

* **Rules.** Pools (parsed from the pre-registration and checked against its
  digests), the Fable 5.1 low reference, outcome-line parsing, run
  classification, and the win rules.

Standard library only; Python 3.9+.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3] if len(HERE.parents) > 3 else HERE
PREREG = REPO / "docs/terminal-bench/2026-09-26-out-of-sample-study.md"
TB21_PREREG = REPO / "docs/terminal-bench/2026-09-26-tb21-oos-study.md"
TB21_DEV_SET = REPO / "bench/terminal-bench/reference/tb21-dev-set.json"
TB21_DIGEST = "310f2588dd8b8079a1992a51aa9dc441149336527fce4be4f9c22009b72fbdb3"
REPLAYS = REPO / "bench/terminal-bench/reference/fable-5.1-replays.json"
LEXICON = REPO / "bench/terminal-bench/reference/tuned-lexicon.json"

HELD_OUT_DIGEST = "46824ed5bd0776b540480f68309443245b843d9c68abd339a47fee1fd6136f31"
FABLE_FAILS_DIGEST = "625fc34c5d27cb87806d0cab539aaf5183aa7a7b1e2696e44c7928a4f390f93b"

# Named in the pre-registration's disclosure: retained winning traces.
STUDIED_WITH_TRACES = {"react-lead-form", "nextjs-performance", "photonic-waveguide-routing"}

LOG_NAME = re.compile(r"^(?P<task>.+)\.(?P<arm>on|off)\.(?P<ms>\d{10,})\.log$")
RECORD_LINE = re.compile(r"Record: (\S+)")

# ---------------------------------------------------------------- collection

SUMMARY_TOP = ("task", "model", "effort", "provider", "kb", "cost_basis", "reward",
               "reward_unknown_because", "knowledge_assisted")
SUMMARY_OUTCOME = ("steps", "seconds", "usd", "known_usd", "usd_upper", "model_usd", "jev_usd",
                   "embedding_usd", "cost_unknown", "knowledge_assisted")

# The numeric fields of an unpriced model call that the bound needs.
EVENT_CALL_FIELDS = ("prompt_tokens", "completion_tokens", "known_usd", "milliseconds")
GENERATED_PREFIX = '{"event":"generated"'


def _number(v):
    return v if isinstance(v, (int, float)) and not isinstance(v, bool) else None


def _unpriced_calls(events: Path) -> list | None:
    """Each model step whose cost is unknown, from events.jsonl: numbers only.

    Only `generated` lines are parsed, and from each only `step`,
    `prompt_chars`, and the numeric token, dollar, and timing fields of a
    call whose `usd` is null are kept. Nothing textual leaves this function.
    """
    calls = []
    try:
        with events.open(encoding="utf-8", errors="replace") as f:
            for line in f:
                if not line.startswith(GENERATED_PREFIX):
                    continue
                try:
                    e = json.loads(line)
                except ValueError:
                    continue
                g = e.get("generated")
                if not isinstance(g, dict) or g.get("usd") is not None:
                    continue
                call = {"step": _number(e.get("step")), "prompt_chars": _number(e.get("prompt_chars"))}
                call.update({k: _number(g.get(k)) for k in EVENT_CALL_FIELDS})
                calls.append(call)
    except OSError:
        return None
    return calls


def _summary_fields(path: Path) -> dict | None:
    """The whitelisted fields of one summary.json, or None."""
    try:
        s = json.loads(path.read_text())
    except (OSError, ValueError):
        return None
    out = {k: s.get(k) for k in SUMMARY_TOP if k in s}
    o = s.get("outcome") or {}
    out.update({k: o.get(k) for k in SUMMARY_OUTCOME if k in o})
    ending = o.get("ending") or {}
    if isinstance(ending, dict):
        out["ending"] = ending.get("reason")
        detail = ending.get("detail")
        if isinstance(detail, str):
            # How the run ended: the provider's error, trimmed. This is the
            # same text the outcome line carries.
            out["ending_detail"] = detail[:300]
    else:
        out["ending"] = str(ending)
    entries = []
    for e in o.get("knowledge") or []:
        if isinstance(e, dict):
            entries.append({"id": e.get("id"), "digest": e.get("digest")})
    out["entries"] = entries
    return out


def _record_from_tail(log: Path, nbytes: int = 4096) -> str | None:
    """The record path named in a log's last lines. Only the path is kept."""
    try:
        with log.open("rb") as f:
            f.seek(0, os.SEEK_END)
            size = f.tell()
            f.seek(max(0, size - nbytes))
            tail = f.read().decode("utf-8", "replace").splitlines()[-8:]
    except OSError:
        return None
    found = None
    for line in tail:
        m = RECORD_LINE.search(line)
        if m:
            found = m.group(1)
    return found


def _first_line(path: Path) -> str | None:
    try:
        return path.read_text().strip() or None
    except OSError:
        return None


def collect(study_dir: str, round_name: str, runs_dir: str | None = None) -> dict:
    """Everything the tooling may know about a round, as plain JSON."""
    study = Path(os.path.expanduser(study_dir))
    runs = Path(os.path.expanduser(runs_dir or "~/.openagents/microcoder/runs"))
    rdir = study / round_name
    logs = []
    tasks = set()
    for p in sorted(rdir.glob("*.log")) if rdir.is_dir() else []:
        m = LOG_NAME.match(p.name)
        if not m:
            continue
        tasks.add(m.group("task"))
        logs.append({"name": p.name, "task": m.group("task"), "arm": m.group("arm"),
                     "ms": int(m.group("ms")), "record": _record_from_tail(p)})
    # Record directories for the tasks this round touched, by name only.
    record_dirs = []
    if runs.is_dir():
        for d in runs.iterdir():
            m = re.match(r"^(.+)-(\d{10,})$", d.name)
            if m and m.group(1) in tasks:
                record_dirs.append(d.name)
    # Attach a record to every log: from its tail, else the record directory
    # for the same task that started within ten seconds after the log.
    for log in logs:
        if not log["record"]:
            best = None
            for name in record_dirs:
                task, ms = name.rsplit("-", 1)
                delta = int(ms) - log["ms"]
                if task == log["task"] and 0 <= delta <= 10_000 and (best is None or delta < best[0]):
                    best = (delta, name)
            if best:
                log["record"] = str(runs / best[1])
                log["record_matched_by"] = "start time"
        else:
            log["record_matched_by"] = "log tail"
    summaries = {}
    for log in logs:
        rec = log.get("record")
        if rec:
            rp = Path(rec)
            if not rp.is_absolute():
                rp = runs / rp
            if rp.name not in summaries:
                fields = _summary_fields(rp / "summary.json")
                if fields is not None:
                    if fields.get("cost_unknown") and "usd_upper" not in fields:
                        fields["unpriced_calls"] = _unpriced_calls(rp / "events.jsonl")
                    summaries[rp.name] = fields
            log["record"] = rp.name
    outcomes = []
    try:
        outcomes = [l for l in (rdir / "outcomes.txt").read_text().splitlines() if l.strip()]
    except OSError:
        pass
    queue = None
    for q in (study / f"queue-{round_name}.txt",):
        if q.exists():
            queue = q.read_text()
    return {
        "schema": "openagents.oos-study.collected.v1",
        "collected_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "study_dir": str(study),
        "round": round_name,
        "meta": _first_line(study / f"{round_name}.meta"),
        "burned": _first_line(study / "burned.txt"),
        "notes": _first_line(study / "notes.txt"),
        "queue": queue,
        "outcomes": outcomes,
        "logs": logs,
        "summaries": summaries,
    }


# --------------------------------------------------------------- rules

def pools(prereg: Path = PREREG) -> tuple[list[str], list[str]]:
    """The held-out and Fable-fails pools, checked against the digests."""
    doc = prereg.read_text()
    held = re.findall(r"^\| `([^`]+)` \|", doc, re.M)
    i = doc.index("A second pool")
    seg = doc[i:doc.index("A pass there", i)]
    fails = re.findall(r"`([^`]+)`", seg.split("):", 1)[1])
    for name, lst, want in (("held-out", held, HELD_OUT_DIGEST),
                            ("Fable-fails", fails, FABLE_FAILS_DIGEST)):
        got = hashlib.sha256("\n".join(sorted(lst)).encode()).hexdigest()
        if got != want:
            raise SystemExit(f"{name} pool digest {got} != pre-registered {want}")
    return held, fails


def prereg_reference(prereg: Path = PREREG) -> dict:
    """The pre-registration table's cheapest ($) and fastest (min) per task."""
    out = {}
    for line in prereg.read_text().splitlines():
        m = re.match(r"^\| `([^`]+)` \| [^|]+\| \$([\d.]+) \| ([^|]+) \|", line)
        if m:
            fast = m.group(3).strip()
            out[m.group(1)] = {"cheapest_usd": float(m.group(2)),
                               "fastest_min": None if fast in ("—", "-", "") else float(fast)}
    return out


def _ts(s: str) -> float | None:
    if not s:
        return None
    s = s.replace("Z", "+00:00")
    s = re.sub(r"\.(\d+)", lambda m: "." + (m.group(1) + "000000")[:6], s)
    try:
        return datetime.fromisoformat(s).timestamp()
    except ValueError:
        return None


def fable_reference(replays: Path = REPLAYS) -> dict:
    """Per task: Fable 5.1 low's cheapest and fastest winning run.

    A winning run has reward >= 1. Time is the trial's wall clock,
    finished_at - started_at, as `gym runs` computes it.
    """
    doc = json.loads(Path(replays).read_text())
    ref: dict = {}
    for t in doc["trials"]:
        if t.get("effort") != "low" or t.get("model", "Fable 5.1") != "Fable 5.1":
            continue
        r = ref.setdefault(t["task"], {"attempts": 0, "passes": 0, "cheapest": None, "fastest": None})
        r["attempts"] += 1
        if not (isinstance(t.get("reward"), (int, float)) and t["reward"] >= 1):
            continue
        r["passes"] += 1
        a, b = _ts(t.get("started_at")), _ts(t.get("finished_at"))
        secs = (b - a) if a is not None and b is not None else None
        cost = t.get("cost_usd")
        run = {"id": t["id"], "cost_usd": cost, "seconds": secs}
        if cost is not None and (r["cheapest"] is None or cost < r["cheapest"]["cost_usd"]):
            r["cheapest"] = run
        if secs is not None and (r["fastest"] is None or secs < r["fastest"]["seconds"]):
            r["fastest"] = run
    return ref


def studied_by_earlier_harnesses(lexicon: Path = LEXICON) -> set:
    try:
        tasks = set(json.loads(Path(lexicon).read_text()).get("tasks", {}))
    except (OSError, ValueError):
        tasks = set()
    return tasks | STUDIED_WITH_TRACES


def listed(text: str | None, tag: str) -> dict:
    """`TAG task: reason` lines from burned.txt / notes.txt."""
    out = {}
    for line in (text or "").splitlines():
        m = re.match(rf"^\s*{tag}\s+(\S+?):\s*(.*)$", line)
        if m:
            out[m.group(1)] = m.group(2).strip()
    return out


def queue_minutes(queue: str | None) -> dict:
    out = {}
    for line in (queue or "").splitlines():
        parts = line.split()
        if len(parts) >= 2 and not line.startswith("#"):
            out[parts[0]] = int(parts[1])
    return out


OUTCOME = re.compile(r"^(?P<ts>\S+) (?P<task>\S+) (?P<arm>on|off) (?:rc|exit)=(?P<rc>-?\d+) ?(?P<rest>.*)$")


def _clock(s: str) -> float:
    parts = [int(x) for x in s.split(":")]
    secs = 0
    for p in parts:
        secs = secs * 60 + p
    return float(secs)


def parse_outcome(line: str) -> dict | None:
    m = OUTCOME.match(line)
    if not m:
        return None
    rest = m.group("rest")
    out = {"ts": m.group("ts"), "task": m.group("task"), "arm": m.group("arm"),
           "rc": int(m.group("rc")), "line": rest, "log": None}
    lg = re.search(r"\blog=(\S+\.log)\s*$", rest)
    if lg:
        out["log"] = lg.group(1)
    if " · reward " in rest:
        r = re.search(r" · reward (\S+)(?: \(([^)]*)\))?", rest)
        out["reward"] = None if r.group(1) == "unknown" else float(r.group(1))
        out["reward_unknown_because"] = r.group(2)
        s = re.search(r" · (\d+) steps", rest)
        out["steps"] = int(s.group(1)) if s else None
        c = re.search(r" · (\d+(?::\d+){1,2}) · ", rest)
        out["seconds"] = _clock(c.group(1)) if c else None
        d = re.search(r" · \$([\d.]+)", rest)
        out["usd"] = float(d.group(1)) if d else None
        out["billed"] = " billed" in rest
        e = re.search(r"ended by (BadReplies|[^·]+?)(?: ·|\(|$)", rest)
        out["ending"] = e.group(1).strip() if e else None
        k = re.search(r"knowledge-assisted \((\d+) entr", rest)
        out["entries"] = int(k.group(1)) if k else None
    return out


ENDINGS = {
    "step_limit": "Step limit", "StepLimit": "Step limit",
    "tests_held": "Luna's own tests held", "TestsHeld": "Luna's own tests held",
    "model_finished": "The model finished", "finished": "The model finished",
    "the model finishing": "The model finished",
    "time_limit": "Time limit", "TimeLimit": "Time limit",
    "budget": "Cost limit", "cost_limit": "Cost limit", "BudgetLimit": "Cost limit",
    "bad_replies": "Bad replies", "BadReplies": "Bad replies",
}

PROVIDER_FAULT = re.compile(r"provider returned HTTP|HTTP (?:429|5\d\d)|usage_limit|rate.?limit", re.I)


def classify(run: dict) -> dict:
    """Fill in kind, cost, time, ending for one run (log + outcome + summary)."""
    o, s = run.get("outcome"), run.get("summary") or {}
    run.update(kind=None, reward=None, steps=None, seconds=None, usd=None, cost_basis=None,
               ending=None, detail=None, entries=None, known_usd=None, usd_upper=None, upper_source=None)
    if o is None:
        run["kind"] = "running"
        run["ending"] = "No outcome line yet"
        return run
    rest = o["line"]
    if "needs several services" in rest:
        run.update(kind="not_supported", ending="Compose task refused by this build")
        return run
    if "reward" not in o:
        if "Interrupted" in rest:
            run.update(kind="interrupted", ending="Interrupted")
        else:
            run.update(kind="crashed", ending=f"No result line (rc={o['rc']})")
        return run
    reward = s.get("reward", o.get("reward")) if s else o.get("reward")
    unknown_because = s.get("reward_unknown_because") or o.get("reward_unknown_because")
    steps = s.get("steps", o.get("steps"))
    seconds = s.get("seconds", o.get("seconds"))
    usd = s.get("usd") if s.get("usd") is not None else o.get("usd")
    known_usd, usd_upper, upper_source = None, None, None
    if s.get("cost_unknown"):
        usd = None  # unknown cost stays unknown
        known_usd = s.get("known_usd")
        usd_upper, upper_source = cost_bound(s)
    basis = s.get("cost_basis") or ("billed" if o.get("billed") else "list_price")
    ending_raw = s.get("ending") or o.get("ending")
    detail = s.get("ending_detail")
    if detail is None and ending_raw in ("BadReplies", "bad_replies"):
        m = re.search(r'BadReplies\("(.*)', rest)
        detail = m.group(1)[:300] if m else None
    ending = ENDINGS.get(ending_raw, (ending_raw or "").replace("_", " ").capitalize() or None)
    entries = len(s["entries"]) if "entries" in s else o.get("entries")
    run.update(reward=reward, steps=steps, seconds=seconds, usd=usd, cost_basis=basis,
               ending=ending, entries=entries, known_usd=known_usd, usd_upper=usd_upper,
               upper_source=upper_source)
    if ending_raw in ("bad_replies", "BadReplies") and PROVIDER_FAULT.search(detail or rest):
        code = re.search(r"HTTP (\d{3})", detail or rest)
        typ = re.search(r'"type\\?":\\?"([a-z_]+)', detail or rest)
        run.update(kind="provider_fault",
                   detail=" ".join(x for x in ("HTTP " + code.group(1) if code else None,
                                               typ.group(1) if typ else None) if x) or "provider error")
    elif reward is None:
        run.update(kind="grade_unknown", detail=unknown_because or "reward unknown")
    elif reward >= 1:
        run["kind"] = "pass"
    else:
        run["kind"] = "fail"
    return run


# ------------------------------------------------------- cost bounds
#
# The same bound crates/microluna/src/price.rs puts on a Codex call that
# failed after it was sent (`upper_bound`), rebuilt for records made before
# calls carried it. List prices in dollars per million tokens (input,
# output); every input token is counted uncached.

LIST_PRICES = {"gpt-6-astra": (10.00, 50.00), "gpt-6-sol": (2.00, 10.00), "gpt-6-luna": (0.10, 0.50)}
MAX_OUTPUT_TOKENS = 128_000        # OpenAI's model pages, retrieved 2026-09-26
LONG_CONTEXT_INPUT_TOKENS = 272_000  # above it: 2x input, 1.5x output
BYTES_PER_TOKEN = 1                # byte-level BPE: a token covers at least one byte
REQUEST_OVERHEAD_TOKENS = 4_096    # framing, rendered tool syntax, provider preamble
# A step's request beyond its prompt (system instructions, the next_action
# tool, model, effort, cache key): microcoder's STEP_REQUEST_FIXED_BYTES,
# which a test holds the real figure under. `prompt_chars` in events.jsonl
# is the prompt's length in bytes (Rust `String::len`).
STEP_REQUEST_FIXED_BYTES = 16_384
RETRIES = 3                        # microluna::oneshot::RETRIES

UNPRICED_MODEL = re.compile(r"^step (\d+) model$")
# A Jev call the API refused with an error status (such as 402, out of
# credit), as a pinned binary recorded it: it cost nothing, like a refused
# model request.
JEV_REFUSED = re.compile(r"^the Jev call failed, so whether it was billed is unknown "
                         r"\((?:GET|POST) \S+: [1-5]\d\d ")
FAILED_ATTEMPTS = re.compile(r"^(an attempt|(\d+) attempts) failed after the request was sent")


def _model_slug(model: str | None) -> str | None:
    if not model:
        return None
    m = model.rsplit("/", 1)[-1]
    return m[:-4] if m.endswith("-pro") else m


def upper_bound(model: str | None, request_bytes: float) -> float | None:
    """The most one attempt of `request_bytes` bytes could cost at list price."""
    rates = LIST_PRICES.get(_model_slug(model) or "")
    if rates is None:
        return None
    tokens_in = -(-int(request_bytes) // BYTES_PER_TOKEN) + REQUEST_OVERHEAD_TOKENS
    rin, rout = rates
    if tokens_in > LONG_CONTEXT_INPUT_TOKENS:
        rin, rout = rin * 2, rout * 1.5
    return (tokens_in * rin + MAX_OUTPUT_TOKENS * rout) / 1_000_000


def _unknown_parts(entry) -> tuple[str | None, str]:
    """(where, why) of one cost_unknown entry, old string or new object form."""
    if isinstance(entry, dict):
        return entry.get("at"), entry.get("reason") or ""
    if isinstance(entry, str) and ": " in entry:
        at, why = entry.split(": ", 1)
        return at, why
    return None, ""


def reconstruct_upper(s: dict) -> float | None:
    """The most a run could have cost, rebuilt from its record, or None.

    Only a list-price run whose every unknown call is a model step that
    failed after the request was sent, or a Jev call the API refused with an
    error status ($0), can be bounded: its known dollars
    plus, for each failed attempt, the request's bytes (the step's
    `prompt_chars` plus STEP_REQUEST_FIXED_BYTES) as input tokens and the
    output cap, at list price. The number of failed attempts comes from the
    recorded reason; when it can't be read, the most a step can have
    (RETRIES + 1) is used. Anything else (an unpriced model, a Jev or
    embeddings call, a billed provider, a step missing from events.jsonl)
    leaves the cost unbounded.
    """
    if s.get("cost_basis") != "list_price" or not isinstance(s.get("known_usd"), (int, float)):
        return None
    calls = s.get("unpriced_calls")
    if not isinstance(calls, list):
        return None
    by_step = {c.get("step"): c for c in calls if isinstance(c, dict)}
    total = float(s["known_usd"])
    for entry in s.get("cost_unknown") or []:
        at, why = _unknown_parts(entry)
        if JEV_REFUSED.match(why):
            continue
        m = UNPRICED_MODEL.match(at or "")
        if not m:
            return None
        call = by_step.get(int(m.group(1)))
        if not call or not isinstance(call.get("prompt_chars"), (int, float)):
            return None
        f = FAILED_ATTEMPTS.match(why)
        if f is None and "no known list price" in why:
            return None
        attempts = (1 if f.group(1) == "an attempt" else int(f.group(2))) if f else RETRIES + 1
        bound = upper_bound(s.get("model"), call["prompt_chars"] + STEP_REQUEST_FIXED_BYTES)
        if bound is None:
            return None
        total += attempts * bound
    return total


def cost_bound(s: dict) -> tuple[float | None, str | None]:
    """A run's upper bound on cost and where it came from.

    ("recorded") when summary.json carries `usd_upper`, ("reconstructed")
    when it is rebuilt from events.jsonl's numbers, or (None, None).
    """
    if isinstance(s.get("usd_upper"), (int, float)):
        return float(s["usd_upper"]), "recorded"
    if "usd_upper" in s:
        return None, None
    up = reconstruct_upper(s)
    return (up, "reconstructed") if up is not None else (None, None)


RESULT_KINDS = ("pass", "fail")
FAULT_KINDS = ("provider_fault", "grade_unknown", "not_supported", "crashed", "interrupted")


def assemble(collected: dict) -> list[dict]:
    """One entry per run: its log, outcome line, and summary, classified."""
    logs = sorted(collected["logs"], key=lambda l: l["ms"])
    summaries = collected["summaries"]
    runs = [{"task": l["task"], "arm": l["arm"], "ms": l["ms"], "log": l["name"],
             "record": l.get("record"), "summary": summaries.get(l.get("record") or ""),
             "outcome": None} for l in logs]
    by_log = {r["log"]: r for r in runs}
    orphans = []
    for line in collected["outcomes"]:
        o = parse_outcome(line)
        if o is None:
            continue
        target = by_log.get(o["log"]) if o["log"] else None
        if target is None or target["outcome"] is not None:
            cands = [r for r in runs if r["task"] == o["task"] and r["arm"] == o["arm"] and r["outcome"] is None]
            target = None
            if "reward" in o:
                for r in cands:
                    s = r["summary"]
                    if s and s.get("steps") == o.get("steps") and (
                            o.get("seconds") is None or s.get("seconds") is None
                            or abs(s["seconds"] - o["seconds"]) < 2):
                        target = r
                        break
            if target is None:
                ts = _ts(o["ts"])
                early = [r for r in cands if ts is None or r["ms"] / 1000 <= ts + 1]
                # Prefer a run without a record when the line has no result,
                # and one with a record when it has.
                pool = early or cands
                pref = [r for r in pool if bool(r["summary"]) == ("reward" in o)]
                target = (pref or pool or [None])[0]
        if target is None:
            orphans.append({"task": o["task"], "arm": o["arm"], "ms": None, "log": None,
                            "record": None, "summary": None, "outcome": o})
        else:
            target["outcome"] = o
    for r in runs + orphans:
        classify(r)
    return runs + orphans


def cost_win_basis(run: dict, bar: float | None) -> str | None:
    """Why a run is a cost win against `bar`, or None when it isn't.

    A pass whose known cost is under the bar with no unknown part is
    "exact". A pass whose cost is partly unknown is a win only when its upper
    bound is under the bar: "upper bound" when Microcoder recorded it,
    "upper bound, reconstructed" when rebuilt from events.jsonl's numbers.
    An unknown cost with no bound is never a cost win.
    """
    if run["kind"] != "pass" or bar is None:
        return None
    if run["usd"] is not None:
        return "exact" if run["usd"] < bar else None
    if run.get("usd_upper") is not None and run["usd_upper"] < bar:
        return "upper bound" if run.get("upper_source") == "recorded" else "upper bound, reconstructed"
    return None


def win_flags(run: dict, ref: dict | None) -> dict:
    """Cost and time wins against Fable 5.1 low's cheapest and fastest
    winning runs (see cost_win_basis)."""
    cheapest = ref["cheapest"]["cost_usd"] if ref and ref.get("cheapest") else None
    fastest = ref["fastest"]["seconds"] if ref and ref.get("fastest") else None
    basis = cost_win_basis(run, cheapest)
    cost_win = basis is not None
    time_win = bool(cost_win and run["seconds"] is not None and fastest is not None
                    and run["seconds"] < fastest)
    return {"cost_win": cost_win, "time_win": time_win, "cost_win_basis": basis}


def tb21_pool(prereg: Path = TB21_PREREG) -> list[str]:
    """The TB2.1 study's 65 tasks, checked against the pre-registered digest."""
    tasks = re.findall(r"^\| `([^`]+)` \|", Path(prereg).read_text(), re.M)
    got = hashlib.sha256("\n".join(sorted(tasks)).encode()).hexdigest()
    if got != TB21_DIGEST:
        raise SystemExit(f"TB2.1 pool digest {got} != pre-registered {TB21_DIGEST}")
    return tasks


def tb21_reference(dev_set: Path = TB21_DEV_SET) -> dict:
    """Per task: Fable 5 xhigh's cost per trial (the bar) and mean agent time."""
    tasks = json.loads(Path(dev_set).read_text()).get("tasks", {})
    out = {}
    for t, row in tasks.items():
        f = row.get("fable_5_xhigh") or {}
        out[t] = {"usd_per_trial": f.get("usd_per_trial"), "mean_agent_sec": f.get("mean_agent_sec"),
                  "successes": f.get("successes"), "trials": f.get("trials")}
    return out


def screen_passed(runs: list[dict], task: str) -> bool:
    """The task's screen: its first knowledge-on run with a result passed."""
    on = [r for r in runs if r["task"] == task and r["arm"] == "on" and r["kind"] in RESULT_KINDS]
    return bool(on) and on[0]["kind"] == "pass"


def main() -> None:
    ap = argparse.ArgumentParser(description="Print a round's collected, whitelisted study data as JSON.")
    ap.add_argument("--study-dir", default="~/study-oos")
    ap.add_argument("--runs-dir", default="~/.openagents/microcoder/runs")
    ap.add_argument("--round", required=True)
    a = ap.parse_args()
    json.dump(collect(a.study_dir, a.round, a.runs_dir), sys.stdout)


if __name__ == "__main__":
    main()
