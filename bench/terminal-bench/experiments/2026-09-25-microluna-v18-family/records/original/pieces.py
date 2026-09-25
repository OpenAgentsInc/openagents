#!/usr/bin/env python3
"""Per-trial loop facts for the v18 family: baseline, finish rule, scores, bounds."""
import glob
import json
import os

HOME = os.path.expanduser("~")
tally = json.load(open(f"{HOME}/.cache/openagents/v18-family/tally.json"))
cards = f"{HOME}/.cache/openagents/v18-family/cards"
out = []
for t in tally:
    job = f"{HOME}/.openagents/terminal-bench/jobs/{t['job']}"
    row = {"task": t["task"], "label": t["label"], "trial": t["trial"]}
    loop = glob.glob(f"{job}/*/agent/episode/artifacts/microluna-1.json")
    if not loop:
        loop = glob.glob(f"{job}/*/agent/live/artifacts/microluna-1.json")
    row["loop_record"] = bool(loop)
    if loop:
        d = json.load(open(loop[0]))
        row["stopped"] = d.get("stopped")
        row["stopped_by"] = d.get("stopped_by")
        moves = d.get("moves", [])
        base = next((m for m in moves if m.get("kind") == "lean.baseline"), None)
        if base:
            row["baseline_commands"] = base.get("commands")
            row["baseline_refused"] = [r.get("command") if isinstance(r, dict) else r for r in base.get("refused") or []]
            row["baseline_none"] = base.get("none")
            row["baseline_runs"] = [
                {k: r.get(k) for k in ("command", "exit", "exit_code", "ms", "timed_out") if k in r}
                for r in base.get("runs") or []
            ]
        lean = [m for m in moves if m.get("kind") == "lean"]
        row["scores"] = [(m.get("after_session"), (m.get("score") or {}).get("passed"), (m.get("score") or {}).get("total")) for m in lean]
        sub = next((m for m in moves if m.get("kind") == "lean.submitted"), None)
        if sub:
            s = sub.get("score") or {}
            row["submitted_score"] = [s.get("passed"), s.get("total")]
            row["submitted_session"] = sub.get("selected_session")
        row["sessions"] = [
            {k: s.get(k) for k in ("status", "cause", "finish_refusals", "unverified", "turns")}
            for s in d.get("sessions", [])
        ]
    else:
        row["sessions"] = []
    ep = glob.glob(f"{job}/*/agent/episode/manifest.json")
    if ep:
        m = json.load(open(ep[0]))
        row["episode_outcome"] = m.get("outcome") or m.get("status") or m.get("result")
    card = glob.glob(f"{cards}/*{t['trial']}.card.json")
    if card:
        rows = {r["id"]: r.get("text") for r in json.load(open(card[0]))["rows"]}
        row["not_found_turns"] = rows.get("waste.not_found_turns")
        row["not_found"] = {k.split(".", 2)[2]: v for k, v in rows.items() if k.startswith("waste.not_found.")}
        row["self_score_agrees"] = rows.get("claims.self_score_agrees")
        row["frozen_score"] = rows.get("checks.frozen_score")
        row["untouched_score"] = rows.get("checks.untouched_score")
        row["review_s"] = [v for k, v in rows.items() if k.startswith("phase.session_") and k.endswith(".s")]
    out.append(row)
json.dump(out, open(f"{HOME}/.cache/openagents/v18-family/pieces.json", "w"), indent=1)
for r in out:
    print(r["task"][:24].ljust(24), r["label"],
          "base:", (r.get("baseline_commands") or r.get("baseline_none", "")[:40]),
          "| scores:", r.get("scores"), "sub:", r.get("submitted_score"),
          "| sessions:", [(s["status"], s["cause"], s["finish_refusals"], s["unverified"]) for s in r["sessions"]],
          "| stopped:", r.get("stopped_by") or r.get("stopped"),
          "| nf:", r.get("not_found_turns"), r.get("not_found"))
