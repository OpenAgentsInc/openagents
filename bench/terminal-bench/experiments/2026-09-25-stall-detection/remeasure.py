#!/usr/bin/env python3
"""Remeasures the #9627 stall detector with the code's suspect call deciding.

This follow-up reads only the retained rows (features, recorded Jev answers,
and calls, which replay.py reproduces from the retained inputs with no call)
and the retained labels. It asks no model. It reports, for both partitions:

- precision and recall with Wilson intervals for the code-only rule
  (`suspect`, the `code` mode of `executor.microluna.lean.detect`) beside
  the Jev-confirmed cascade;
- every call either rule makes in a passing attempt, labeled or not, since
  the rule acts whatever the label;
- the re-brief-then-stop replay at session ends; and
- the in-session replay: inside one session, a first stall re-briefs and a
  stall at the next checkpoint after a re-brief ends the session.

The evaluation labels were read in the first measurement, so the
evaluation numbers here are not a fresh held-out result.

    python3 bench/terminal-bench/experiments/2026-09-25-stall-detection/remeasure.py
"""

import json
import os
from collections import defaultdict

import measure

HERE = os.path.dirname(os.path.abspath(__file__))
RULES = ("code_suspect", "cascade")


def load_all():
    rows = measure.load(os.path.join(HERE, "records/rows.jsonl"))
    labels = []
    for part in ("calibration", "evaluation"):
        labels += measure.load(os.path.join(HERE, f"records/labels-{part}.jsonl"))
    params = json.load(open(os.path.join(HERE, "selection.json")))["chosen"]["params"]
    return rows, labels, params


def every(rows, labels):
    """Every checkpoint with its label, labeled or not."""
    by = {measure.key(r): r for r in rows}
    return [(by[measure.key(l)], l) for l in labels if measure.key(l) in by]


def calls_in_passes(pairs, params):
    out = {}
    passing = [p for p in pairs if p[1]["passed"]]
    for rule in RULES:
        out[rule] = sum(1 for row, _ in passing if measure.predictors(row, params)[rule])
    out["checkpoints"] = len(passing)
    out["trials"] = len({l["trial"] for _, l in passing})
    return out


def session_turns(pairs):
    """Each (trial, session)'s length: a session end's turn, or, for an
    attempt's last session, its checkpoints' turn plus the turns left."""
    turns = {}
    for _, label in pairs:
        k = (label["trial"], label["session"])
        if label["at"] == "session_end":
            turns[k] = label["turn"]
    for _, label in pairs:
        k = (label["trial"], label["session"])
        if k not in turns:
            turns[k] = label["turn"] + label["hindsight"]["later_turns"]
    return turns


def between(pairs, params, rule):
    """The lean hook at session ends: a first stall re-briefs the next
    session, and a stall right after a re-brief stops the work sessions."""
    by_trial = defaultdict(list)
    for row, label in pairs:
        if label["at"] == "session_end":
            by_trial[label["trial"]].append((row, label))
    out = {"session_ends": 0, "rebriefs": 0, "stops": 0, "stops_in_passing_trials": 0,
           "stops_before_later_progress": 0, "turns_after_stops": 0, "stopped": []}
    for trial, items in sorted(by_trial.items()):
        items.sort(key=lambda p: p[1]["session"])
        rebriefed = False
        for row, label in items:
            out["session_ends"] += 1
            stalled = measure.predictors(row, params)[rule]
            if stalled and rebriefed:
                h = label["hindsight"]
                out["stops"] += 1
                out["stops_in_passing_trials"] += int(label["passed"] is True)
                out["stops_before_later_progress"] += int(h["progress_after"])
                out["turns_after_stops"] += h["later_turns"]
                out["stopped"].append({"trial": trial, "after_session": label["session"],
                                       "later_turns": h["later_turns"],
                                       "later_progress": h["progress_after"],
                                       "passed": label["passed"]})
                break
            if stalled:
                out["rebriefs"] += 1
            rebriefed = stalled
    return out


def inside(pairs, params, rule):
    """The in-session action: at the every-8-turns checkpoints of one
    session, a first stall injects a re-brief, and a stall at the next
    checkpoint after it ends the session."""
    turns = session_turns(pairs)
    # Whether a score gain came after each session's end: a session end's
    # own label, or none for an attempt's last session.
    after_end = {(l["trial"], l["session"]): l["hindsight"]["progress_after"]
                 for _, l in pairs if l["at"] == "session_end"}
    by_session = defaultdict(list)
    for row, label in pairs:
        if label["at"] == "in_session":
            by_session[(label["trial"], label["session"])].append((row, label))
    out = {"sessions_with_checkpoints": len(by_session), "rebriefs": 0, "ends": 0,
           "ends_in_passing_trials": 0, "ends_before_later_progress": 0,
           "ends_cutting_a_gain": 0, "ends_maybe_cutting_a_gain": 0,
           "session_turns_cut": 0, "ended": []}
    for (trial, session), items in sorted(by_session.items()):
        items.sort(key=lambda p: p[1]["turn"])
        rebriefed = False
        for row, label in items:
            stalled = measure.predictors(row, params)[rule]
            if stalled and rebriefed:
                h = label["hindsight"]
                cut = turns[(trial, session)] - label["turn"]
                later = after_end.get((trial, session), False)
                # A gain after the checkpoint and none after the session's
                # end fell in the turns the end cuts. With gains after
                # both, whether one fell in the cut turns is unknown.
                cutting = h["progress_after"] and not later
                maybe = h["progress_after"] and later
                out["ends"] += 1
                out["ends_in_passing_trials"] += int(label["passed"] is True)
                out["ends_before_later_progress"] += int(h["progress_after"])
                out["ends_cutting_a_gain"] += int(cutting)
                out["ends_maybe_cutting_a_gain"] += int(maybe)
                out["session_turns_cut"] += cut
                out["ended"].append({"trial": trial, "session": session, "turn": label["turn"],
                                     "session_turns_cut": cut,
                                     "later_progress": h["progress_after"],
                                     "gain_in_cut_turns": True if cutting else (None if maybe else False),
                                     "passed": label["passed"]})
                break
            if stalled:
                out["rebriefs"] += 1
            rebriefed = stalled
    return out


def main():
    rows, labels, params = load_all()
    result = {"params": params, "rules": {"code_suspect": "suspect: the code-only mode",
                                          "cascade": "suspect, then Jev: the jev mode"},
              "note": "The evaluation labels were read in the first measurement; "
                      "these evaluation numbers are not a fresh held-out result.",
              "partitions": {}}
    for part in ("calibration", "evaluation"):
        part_labels = [l for l in labels if l["partition"] == part]
        labeled = measure.join(rows, part_labels)
        allpairs = every(rows, part_labels)
        section = {"labeled_checkpoints": len(labeled), "checkpoints": len(allpairs)}
        for at in ("all", "session_end", "in_session"):
            sub = labeled if at == "all" else [p for p in labeled if p[1]["at"] == at]
            section[at] = {rule: measure.summarize(*measure.confusion(sub, rule, params))
                           for rule in RULES}
        section["calls_in_passing_trials"] = calls_in_passes(allpairs, params)
        section["replay_between_sessions"] = {rule: between(allpairs, params, rule) for rule in RULES}
        section["replay_in_session"] = {rule: inside(allpairs, params, rule) for rule in RULES}
        section["bootstrap_code_minus_cascade"] = measure.bootstrap(
            labeled, params, names=("code_suspect", "cascade"))
        result["partitions"][part] = section
    text = json.dumps(result, indent=2)
    with open(os.path.join(HERE, "records/remeasure-code-only.json"), "w") as f:
        f.write(text + "\n")
    print(text)


if __name__ == "__main__":
    main()
