#!/usr/bin/env python3
"""Build a round's results tables for the out-of-sample study (#9683).

Reads only outcome lines and whitelisted summary.json fields (see study.py),
applies the pre-registration's win rules, and prints Markdown ready to paste
into docs/terminal-bench/2026-09-26-out-of-sample-study-results.md, or JSON.

    report.py --round r2 --host coderos-4080          # collect over ssh
    report.py --round r2                              # on the execution host
    report.py --collected r2.json                     # from a saved collection
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import study  # noqa: E402

KIND_TEXT = {
    "pass": "Pass", "fail": "Fail", "grade_unknown": "Grade unknown",
    "provider_fault": "Provider fault", "not_supported": "Not supported",
    "interrupted": "Interrupted", "crashed": "Crashed", "running": "Running",
}


def fetch(args) -> dict:
    if args.collected:
        return json.loads(Path(args.collected).read_text())
    if args.host:
        src = (Path(study.__file__)).read_text()
        cmd = ["ssh", args.host, "python3", "-", "--round", args.round,
               "--study-dir", args.study_dir, "--runs-dir", args.runs_dir]
        out = subprocess.run(cmd, input=src, capture_output=True, text=True, check=True)
        return json.loads(out.stdout)
    return study.collect(args.study_dir, args.round, args.runs_dir)


def clock(seconds) -> str:
    if seconds is None:
        return "—"
    s = int(round(seconds))
    h, m, s = s // 3600, s % 3600 // 60, s % 60
    return f"{h}:{m:02d}:{s:02d}" if h else f"{m}:{s:02d}"


def money(usd, digits=4) -> str:
    return "unknown" if usd is None else f"${usd:.{digits}f}"


def build(collected: dict, prereg=study.PREREG, replays=study.REPLAYS, lexicon=study.LEXICON) -> dict:
    held, fails = study.pools(prereg)
    ref = study.fable_reference(replays)
    table = study.prereg_reference(prereg)
    studied = study.studied_by_earlier_harnesses(lexicon)
    burned = study.listed(collected.get("burned"), "BURNED")
    ungradeable = study.listed(collected.get("notes"), "UNGRADEABLE")
    runs = study.assemble(collected)

    def pool_of(task):
        return "held-out" if task in held else "fable-fails" if task in fails else "outside"

    for r in runs:
        r["pool"] = pool_of(r["task"])
        r.update(study.win_flags(r, ref.get(r["task"])) if r["pool"] == "held-out"
                 else {"cost_win": False, "time_win": False})
        labels = []
        if r["task"] in burned:
            labels.append("in-sample (burned)")
        if r["task"] in ungradeable:
            labels.append("ungradeable")
        if r["task"] in studied:
            labels.append("studied by earlier harnesses")
        r["labels"] = labels

    reference = {}
    for t in held + fails:
        rr = ref.get(t) or {}
        reference[t] = {
            "attempts": rr.get("attempts", 0), "passes": rr.get("passes", 0),
            "cheapest_usd": rr["cheapest"]["cost_usd"] if rr.get("cheapest") else None,
            "cheapest_id": rr["cheapest"]["id"] if rr.get("cheapest") else None,
            "fastest_seconds": rr["fastest"]["seconds"] if rr.get("fastest") else None,
            "fastest_id": rr["fastest"]["id"] if rr.get("fastest") else None,
        }
    mismatches = []
    for t, row in table.items():
        c, f = reference[t]["cheapest_usd"], reference[t]["fastest_seconds"]
        if c is None or abs(round(c, 2) - row["cheapest_usd"]) > 0.005:
            mismatches.append({"task": t, "field": "cheapest", "prereg": row["cheapest_usd"], "replays": c})
        fm = None if f is None else round(f / 60, 1)
        if fm != row["fastest_min"]:
            mismatches.append({"task": t, "field": "fastest_min", "prereg": row["fastest_min"], "replays": fm})

    verdicts = []
    tasks_seen = sorted({r["task"] for r in runs if r["pool"] == "held-out"}, key=held.index)
    for t in tasks_seen:
        mine = [r for r in runs if r["task"] == t]
        on = [r for r in mine if r["arm"] == "on" and r["kind"] in study.RESULT_KINDS][:3]
        off = [r for r in mine if r["arm"] == "off" and r["kind"] in study.RESULT_KINDS][:3]
        on_w, off_w = sum(r["cost_win"] for r in on), sum(r["cost_win"] for r in off)
        on_t = sum(r["time_win"] for r in on)
        screen = on[0]["kind"] if on else None
        if t in burned:
            verdict = "In-sample (burned): not counted"
        elif screen is None:
            verdict = "No graded screen yet"
        elif screen != "pass":
            verdict = "Screen failed"
        elif on_w >= 2:
            verdict = "Confirmed out-of-sample win"
        elif on_w + (3 - len(on)) < 2:
            verdict = "Not confirmed"
        else:
            verdict = "Awaiting confirmation"
        off_note = None
        if screen == "pass":
            if off_w >= 2:
                off_note = "knowledge-off also wins: credit the loop"
            elif off_w + (3 - len(off)) < 2:
                off_note = "knowledge-off does not win"
            else:
                off_note = "knowledge-off pending"
        verdicts.append({"task": t, "screen": screen, "on_results": len(on), "on_cost_wins": on_w,
                         "on_time_wins": on_t, "off_results": len(off), "off_cost_wins": off_w,
                         "verdict": verdict, "knowledge_off": off_note,
                         "labels": sorted({l for r in mine for l in r["labels"]})})

    fails_rows = []
    for t in sorted({r["task"] for r in runs if r["pool"] == "fable-fails"}, key=fails.index):
        mine = [r for r in runs if r["task"] == t]
        passes = sum(r["kind"] == "pass" for r in mine)
        fails_rows.append({"task": t, "passes": passes,
                           "graded": sum(r["kind"] in study.RESULT_KINDS for r in mine),
                           "verdict": "Beats Fable outright (confirmed by a second pass)" if passes >= 2
                           else "Pass: needs a second pass to confirm" if passes == 1 else "No pass"})

    touched = {r["task"] for r in runs}
    not_run = {"held-out": [t for t in held if t not in touched and t not in burned],
               "fable-fails": [t for t in fails if t not in touched]}
    public = []
    for r in runs:
        public.append({k: r[k] for k in ("task", "pool", "arm", "kind", "reward", "steps", "seconds", "usd",
                                         "cost_basis", "ending", "detail", "entries", "cost_win",
                                         "time_win", "labels", "log", "record")})
    return {
        "schema": "openagents.oos-study.report.v1",
        "round": collected.get("round"), "meta": collected.get("meta"),
        "collected_at": collected.get("collected_at"),
        "burned": burned, "ungradeable": ungradeable,
        "reference": reference, "reference_mismatches": mismatches,
        "runs": public, "held_out_verdicts": verdicts, "fable_fails": fails_rows, "not_run": not_run,
        "totals": {
            "held_out_graded": sum(r["pool"] == "held-out" and r["kind"] in study.RESULT_KINDS for r in runs),
            "held_out_passes": sum(r["pool"] == "held-out" and r["kind"] == "pass" for r in runs),
            "cost_wins": sum(r["cost_win"] for r in runs),
            "time_wins": sum(r["time_win"] for r in runs),
            "confirmed_wins": sum(v["verdict"] == "Confirmed out-of-sample win" for v in verdicts),
            "fable_fails_passes": sum(f["passes"] for f in fails_rows),
            "faults": sum(r["kind"] in study.FAULT_KINDS for r in runs),
            "running": sum(r["kind"] == "running" for r in runs),
        },
    }


def task_cell(r) -> str:
    marks = ""
    if "in-sample (burned)" in r["labels"]:
        marks += " **(in-sample: burned)**"
    if "studied by earlier harnesses" in r["labels"]:
        marks += " †"
    return f"`{r['task']}`{marks}"


def yes(b) -> str:
    return "**yes**" if b else "no"


def markdown(rep: dict) -> str:
    L = []
    rnd = rep["round"]
    L.append(f"### Round {rnd[1:] if rnd and rnd.startswith('r') else rnd}: tables (generated)")
    L.append("")
    if rep["meta"]:
        L.append(f"Round record: `{rep['meta']}`. Collected {rep['collected_at']}.")
        L.append("")
    tot = rep["totals"]
    L.append(f"**So far:** {tot['held_out_passes']} passes in {tot['held_out_graded']} graded held-out runs; "
             f"{tot['cost_wins']} cost wins, {tot['time_wins']} time wins, "
             f"{tot['confirmed_wins']} confirmed out-of-sample wins; "
             f"{tot['fable_fails_passes']} passes on Fable-fails tasks; "
             f"{tot['faults']} faults (not results); {tot['running']} still running.")
    L.append("")
    ref = rep["reference"]
    held = [r for r in rep["runs"] if r["pool"] == "held-out"]
    graded = [r for r in held if r["kind"] in study.RESULT_KINDS]
    L.append("#### Held-out tasks: graded runs")
    L.append("")
    L.append("| Task | Arm | Result | Steps | Time | Cost | Cost basis | How it ended | Entries | "
             "Fable low cheapest win | Fable low fastest win | Cost win | Time win |")
    L.append("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for r in graded:
        f = ref[r["task"]]
        L.append(f"| {task_cell(r)} | {r['arm']} | {KIND_TEXT[r['kind']]} | {r['steps'] if r['steps'] is not None else '—'} "
                 f"| {clock(r['seconds'])} | {money(r['usd'])} | {r['cost_basis'] or '—'} | {r['ending'] or '—'} "
                 f"| {r['entries'] if r['entries'] is not None else '—'} | {money(f['cheapest_usd'], 2)} "
                 f"| {clock(f['fastest_seconds'])} | {yes(r['cost_win'])} | {yes(r['time_win'])} |")
    if not graded:
        L.append("| (none yet) | | | | | | | | | | | | |")
    L.append("")
    L.append("#### Held-out tasks: verdicts")
    L.append("")
    L.append("| Task | Screen | Knowledge-on cost wins | Knowledge-off cost wins | Verdict | Knowledge-off arm |")
    L.append("| --- | --- | --- | --- | --- | --- |")
    for v in rep["held_out_verdicts"]:
        cell = task_cell({"task": v["task"], "labels": v["labels"]})
        L.append(f"| {cell} | {KIND_TEXT.get(v['screen'], '—') if v['screen'] else '—'} "
                 f"| {v['on_cost_wins']} of {v['on_results']} (of 3) | {v['off_cost_wins']} of {v['off_results']} (of 3) "
                 f"| {v['verdict']} | {v['knowledge_off'] or '—'} |")
    L.append("")
    ff = [r for r in rep["runs"] if r["pool"] == "fable-fails" and r["kind"] in study.RESULT_KINDS]
    L.append("#### Fable-fails pool (any pass beats Fable outright once a second pass confirms it)")
    L.append("")
    L.append("| Task | Arm | Result | Steps | Time | Cost | Cost basis | How it ended | Entries |")
    L.append("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for r in ff:
        L.append(f"| {task_cell(r)} | {r['arm']} | {KIND_TEXT[r['kind']]} | {r['steps']} | {clock(r['seconds'])} "
                 f"| {money(r['usd'])} | {r['cost_basis']} | {r['ending'] or '—'} | {r['entries'] if r['entries'] is not None else '—'} |")
    if not ff:
        L.append("| (none yet) | | | | | | | | |")
    passed = [f for f in rep["fable_fails"] if f["passes"]]
    if passed:
        L.append("")
        for f in passed:
            L.append(f"- `{f['task']}`: {f['passes']} pass(es). {f['verdict']}.")
    L.append("")
    faults = [r for r in rep["runs"] if r["kind"] in study.FAULT_KINDS]
    L.append("#### Faults (not results)")
    L.append("")
    L.append("| Task | Pool | Arm | Fault | Steps | Time | Cost | Detail |")
    L.append("| --- | --- | --- | --- | --- | --- | --- | --- |")
    order = {"provider_fault": 0, "grade_unknown": 1, "not_supported": 2, "crashed": 3, "interrupted": 4}
    for r in sorted(faults, key=lambda r: order[r["kind"]]):
        L.append(f"| {task_cell(r)} | {r['pool']} | {r['arm']} | {KIND_TEXT[r['kind']]} "
                 f"| {r['steps'] if r['steps'] is not None else '—'} | {clock(r['seconds'])} | "
                 f"{money(r['usd']) if r['usd'] is not None else '—'} | {r['detail'] or r['ending'] or '—'}"
                 f"{' (task marked ungradeable)' if 'ungradeable' in r['labels'] else ''} |")
    if not faults:
        L.append("| (none) | | | | | | | |")
    L.append("")
    running = [r for r in rep["runs"] if r["kind"] == "running"]
    if running:
        L.append("**Running (no outcome line yet):** " + ", ".join(f"`{r['task']}` ({r['arm']})" for r in running) + ".")
        L.append("")
    nr = rep["not_run"]
    if nr["held-out"] or nr["fable-fails"]:
        L.append(f"**Not yet run this round:** {len(nr['held-out'])} held-out"
                 + (": " + ", ".join(f"`{t}`" for t in nr["held-out"]) if nr["held-out"] else "")
                 + f"; {len(nr['fable-fails'])} Fable-fails"
                 + (": " + ", ".join(f"`{t}`" for t in nr["fable-fails"]) if nr["fable-fails"] else "") + ".")
        L.append("")
    notes = []
    for t, why in rep["burned"].items():
        notes.append(f"`{t}` is burned ({why}); its runs are in-sample.")
    for t, why in rep["ungradeable"].items():
        notes.append(f"`{t}` is marked ungradeable ({why}).")
    notes.append("† studied by earlier harnesses (retained winning traces or `tuned-lexicon.json`); "
                 "a reader can drop these.")
    notes.append("Fable 5.1 low reference: its winning runs (reward ≥ 1) in `fable-5.1-replays.json`; "
                 "time is the trial's wall clock. Cost wins compare total cost (model, Jev, embeddings); "
                 "an unknown cost is never a cost win.")
    if rep["reference_mismatches"]:
        mm = "; ".join(f"`{m['task']}` {m['field']}: table {m['prereg'] if m['prereg'] is not None else '—'}, "
                       f"replays {m['replays'] if m['replays'] is not None else '—'}"
                       for m in rep["reference_mismatches"])
        notes.append("The replays differ from the pre-registration's table here (these tables use the replays): "
                     + mm + ".")
    L.append("Notes:")
    L.append("")
    L.extend(f"- {n}" for n in notes)
    return "\n".join(L) + "\n"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--round", default=None, help="round name, e.g. r2")
    ap.add_argument("--host", help="collect over ssh from this host")
    ap.add_argument("--study-dir", default="~/study-oos")
    ap.add_argument("--runs-dir", default="~/.openagents/microcoder/runs")
    ap.add_argument("--collected", help="read a saved collection (study.py output) instead")
    ap.add_argument("--save-collected", help="also write the collection to this file")
    ap.add_argument("--format", choices=("md", "json", "both"), default="both",
                    help="both (default) prints Markdown, then the JSON after a line '---JSON---'")
    ap.add_argument("--json-out", help="write the JSON report to this file")
    a = ap.parse_args()
    if not a.collected and not a.round:
        ap.error("--round is required unless --collected is given")
    collected = fetch(a)
    if a.save_collected:
        Path(a.save_collected).write_text(json.dumps(collected))
    rep = build(collected)
    if a.json_out:
        Path(a.json_out).write_text(json.dumps(rep, indent=1) + "\n")
    if a.format in ("md", "both"):
        sys.stdout.write(markdown(rep))
    if a.format == "both":
        sys.stdout.write("\n---JSON---\n")
    if a.format in ("json", "both"):
        sys.stdout.write(json.dumps(rep, indent=1) + "\n")


if __name__ == "__main__":
    main()
