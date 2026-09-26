#!/usr/bin/env python3
"""Write Terminal-Bench 4 knowledge quests and the quest board page.

Two commands:

  quests TASK...   write a NIP-XP quest spec per task to knowledge/quests/,
                   from Fable 5.1 low's winning runs in the public replays.
                   An existing file is never rewritten: a quest version is
                   frozen once published.
  board            write docs/terminal-bench/quest-board.md from the quest
                   specs, the public replays, the referee in
                   knowledge/quests/referee.json, and Microcoder's run
                   records.

`board --runs DIR` reads Microcoder's run records for the quest tasks only
(never any other task's) and saves what it read to
knowledge/quests/microcoder-runs.json; without `--runs` it reads that
snapshot. `board --ledger FILE` marks quests that have a counted award in a
`microcoder xp ledger --json` output.

Standard library only; Python 3.9.
"""

import argparse
import datetime as dt
import glob
import json
import math
import os
import re
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
REPLAYS = os.path.join(ROOT, "bench/terminal-bench/reference/fable-5.1-replays.json")
REPLAYS_REL = "bench/terminal-bench/reference/fable-5.1-replays.json"
QUESTS = os.path.join(ROOT, "knowledge/quests")
REFEREE = os.path.join(QUESTS, "referee.json")
SNAPSHOT = os.path.join(QUESTS, "microcoder-runs.json")
BOARD = os.path.join(ROOT, "docs/terminal-bench/quest-board.md")

# The season every quest in this first set belongs to: 90 days from
# 2026-09-26 00:00 UTC.
SEASON = {"id": "tb4-s1", "opens_at": 1790380800, "closes_at": 1798156800}
AWARD = {"author": 6, "runner": 4}
MIN_PASS_RATE = 0.66


def when(text):
    # fromisoformat in 3.9 wants six fractional digits.
    text = re.sub(r"\.(\d+)", lambda m: "." + m.group(1).ljust(6, "0")[:6], text)
    return dt.datetime.fromisoformat(text.replace("Z", "+00:00"))


def fable_wins(task):
    """Fable 5.1 low's trials and winning trials on `task`."""
    with open(REPLAYS) as f:
        trials = json.load(f)["trials"]
    runs = [t for t in trials if t["task"] == task and t["effort"] == "low"]
    wins = [t for t in runs if (t.get("reward") or 0) >= 1]
    for t in wins:
        t["seconds"] = None
        if t.get("started_at") and t.get("finished_at"):
            t["seconds"] = (when(t["finished_at"]) - when(t["started_at"])).total_seconds()
    return runs, wins


def reference(task):
    runs, wins = fable_wins(task)
    priced = [t for t in wins if t.get("cost_usd") is not None]
    timed = [t for t in wins if t["seconds"] is not None]
    if not priced:
        sys.exit(f"{task}: Fable 5.1 low has no priced winning run; no quest")
    cheap = min(priced, key=lambda t: t["cost_usd"])
    fast = min(timed, key=lambda t: t["seconds"]) if timed else None
    return runs, wins, cheap, fast


def quest_id(task):
    return f"tb4.{task}.beat-fable-low"


def quest_path(task, version=1):
    return os.path.join(QUESTS, f"{quest_id(task)}@{version}.json")


def minutes(seconds):
    s = int(seconds)
    return f"{s // 60}:{s % 60:02d}"


def cmd_quests(args):
    os.makedirs(QUESTS, exist_ok=True)
    for task in args.tasks:
        path = quest_path(task)
        if os.path.exists(path):
            print(f"{path}: exists, left as is (a published version is frozen)")
            continue
        runs, wins, cheap, fast = reference(task)
        bar = math.floor(cheap["cost_usd"] * 100) / 100
        label = (
            f"Fable 5.1 low on {task}: passed {len(wins)} of {len(runs)}; "
            f"cheapest winning run ${cheap['cost_usd']:.2f}"
        )
        if fast:
            label += f"; fastest winning run {minutes(fast['seconds'])}"
        source = f"{REPLAYS_REL}: cheapest trial {cheap['id']}"
        if fast:
            source += f", fastest trial {fast['id']}"
        spec = {
            "id": quest_id(task),
            "version": 1,
            "season": SEASON,
            "title": f"Beat Fable 5.1 low's cheapest winning run on {task}",
            "objective": (
                f"Publish a knowledge entry that makes paired Microcoder runs pass {task} "
                f"for less than Fable 5.1 low's cheapest winning run (${cheap['cost_usd']:.2f}), "
                "measured by a runner other than the entry's author, on a task the entry "
                "wasn't written from."
            ),
            "acceptance": {
                "rule": "kb-transfer",
                "task": task,
                "min_pass_rate": MIN_PASS_RATE,
                "max_usd_per_run": bar,
            },
            "reference": {
                "label": label,
                "usd": round(cheap["cost_usd"], 4),
                "seconds": int(fast["seconds"]) if fast else None,
                "source": source,
            },
            "award": AWARD,
            "completions": "first",
        }
        with open(path, "w") as f:
            json.dump(spec, f, indent=2)
            f.write("\n")
        print(f"{path}: written, bar ${bar:.2f}")


def load_quests():
    quests = []
    for path in sorted(glob.glob(os.path.join(QUESTS, "*@*.json"))):
        with open(path) as f:
            quests.append(json.load(f))
    return quests


def read_runs(runs_dir, tasks):
    """Outcome lines of Microcoder's runs on `tasks`, and nothing else."""
    out = {}
    for task in tasks:
        rows = []
        for d in sorted(glob.glob(os.path.join(runs_dir, task + "-[0-9]*"))):
            name = os.path.basename(d)
            if not re.fullmatch(re.escape(task) + r"-\d+", name):
                continue
            ended, reward = None, None
            try:
                with open(os.path.join(d, "events.jsonl")) as f:
                    for line in f:
                        try:
                            e = json.loads(line)
                        except ValueError:
                            continue
                        if e.get("event") == "ended":
                            ended = e.get("outcome") or {}
                        elif e.get("event") == "verified":
                            reward = e.get("reward")
            except OSError:
                continue
            row = {"run": name, "reward": reward}
            if ended:
                model = ended.get("model_usd")
                extra = (ended.get("jev_usd") or 0) + (ended.get("embedding_usd") or 0)
                row["usd"] = None if model is None else round(model + extra, 4)
                row["seconds"] = ended.get("seconds")
                row["ending"] = (ended.get("ending") or {}).get("reason")
            rows.append(row)
        out[task] = rows
    return out


def npub_to_hex(npub):
    charset = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"
    data = [charset.index(c) for c in npub[npub.rindex("1") + 1:]][:-6]
    acc, bits, out = 0, 0, []
    for v in data:
        acc = (acc << 5) | v
        bits += 5
        while bits >= 8:
            bits -= 8
            out.append((acc >> bits) & 0xFF)
    return bytes(out).hex()


def cmd_board(args):
    quests = load_quests()
    tasks = [q["acceptance"]["task"] for q in quests]
    if args.runs:
        runs = read_runs(os.path.expanduser(args.runs), tasks)
        with open(SNAPSHOT, "w") as f:
            json.dump({"read_at": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
                       "tasks": runs}, f, indent=2)
            f.write("\n")
    with open(SNAPSHOT) as f:
        snapshot = json.load(f)
    referee = None
    if os.path.exists(REFEREE):
        with open(REFEREE) as f:
            referee = json.load(f)
    counted = set()
    if args.ledger:
        with open(args.ledger) as f:
            ledger = json.load(f)
        # A counted award credits its awardees; each credit names the
        # referee and the quest version's address.
        for credit in ledger.get("credits", []):
            referee_key = credit["referee"]
            if referee_key.startswith("npub1"):
                referee_key = npub_to_hex(referee_key)
            counted.add(f"30193:{referee_key}:{credit['quest']}")

    lines = [
        "# Terminal-Bench 4 quest board",
        "",
        "Generated by `scripts/tb4-quest-board.py board`; don't edit by hand.",
        f"Microcoder records read {snapshot['read_at']}.",
        "",
        "Each row is a frozen [NIP-XP](../../nips/openagents/NIP-XP.md) quest: pass",
        "the task with Microcoder for less per run than Fable 5.1 low's cheapest",
        "winning run, with a knowledge entry that wasn't written from the task,",
        "measured by someone other than the entry's author. The",
        "[contributor guide](../coder/guides/contribute-knowledge.md) explains how",
        "to take one on, and the [XP guide](../coder/guides/xp.md) how awards work.",
        "",
        "**While the [out-of-sample study](2026-09-26-out-of-sample-study.md) runs,",
        "quests name only the 14 tasks Microcoder has already run.** Quests on",
        "other tasks open when the study closes. The three tasks Microcoder",
        "already beats in-sample (`embedding-drift-monitor`, `gsea-proteomics`,",
        "`fin-saccr-rwa`) have no quest.",
        "",
        "Until [#9687](https://github.com/OpenAgentsInc/openagents/issues/9687)",
        "lands, a runner can't publish evidence about another author's entry, so",
        "no quest can be completed yet.",
        "",
    ]
    if referee:
        lines += [
            f"Referee: `{referee['npub']}`.",
            f"Season `{SEASON['id']}`: 2026-09-26 to 2026-12-25 UTC. Award:",
            f"{sum(AWARD.values())} XP per quest version, {AWARD['author']} to the entry's author and",
            f"{AWARD['runner']} to the runner, once. A quest's runs with the entry must pass",
            f"at least {MIN_PASS_RATE} of the time and cost less per run than its bar: Fable's",
            "cheapest winning cost, rounded down to the cent.",
            "",
        ]
    lines += [
        "| Task | Fable 5.1 low passes | Its cheapest winning run | Its fastest winning run | Bar per run | Microcoder so far | Status | Quest |",
        "| --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for q in quests:
        task = q["acceptance"]["task"]
        runs, wins, cheap, fast = reference(task)
        rows = snapshot["tasks"].get(task, [])
        graded = [r for r in rows if r.get("reward") is not None]
        passes = [r for r in graded if r["reward"] >= 1]
        if passes:
            best = min(passes, key=lambda r: (r.get("usd") is None, r.get("usd") or 0))
            ours = (f"{len(passes)} of {len(graded)} graded passed; cheapest "
                    f"${best['usd']:.4f}, {minutes(best['seconds'])}")
        elif graded:
            ours = f"0 of {len(graded)} graded passed"
        else:
            ours = f"no graded run ({len(rows)} started)"
        address = f"{q['id']}@{q['version']}"
        coordinate = f"30193:{npub_to_hex(referee['npub'])}:{address}" if referee else address
        bar = q["acceptance"]["max_usd_per_run"]
        status = "Completed" if coordinate in counted else "Open"
        if status == "Open" and passes and bar is not None and best["usd"] < bar:
            status = "Open (a Microcoder pass is under the bar; needs out-of-sample evidence)"
        lines.append(
            f"| `{task}` | {len(wins)}/{len(runs)} | ${cheap['cost_usd']:.2f} | "
            f"{minutes(fast['seconds']) if fast else '—'} | under ${bar:.2f} | {ours} | "
            f"{status} | `{address}` |"
        )
    if referee:
        lines += [
            "",
            "A quest's coordinate is `30193:<referee hex>:<address>`; the referee's",
            f"hex key is `{npub_to_hex(referee['npub'])}`.",
        ]
    lines += [
        "",
        "Fable's numbers are its low-effort trials with reward 1 in",
        f"[`{REPLAYS_REL}`](../../{REPLAYS_REL}): cost as the public replay",
        "reports it, time from the trial's start to its finish. Microcoder's",
        "costs are list-price equivalents: model, Jev, and embeddings. \"Microcoder",
        "so far\" counts every recorded run on the task, at any commit, cap, and",
        "knowledge setting; a run that never reached grading isn't graded.",
        "",
        "## Regenerate",
        "",
        "On the execution host, from the repository root:",
        "",
        "```sh",
        "microcoder xp ledger --relay wss://relay.openagents.com \\",
        "  --referee <referee npub> --json > /tmp/ledger.json",
        "python3 scripts/tb4-quest-board.py board --runs ~/.openagents/microcoder/runs \\",
        "  --ledger /tmp/ledger.json",
        "```",
        "",
        "Anywhere else, `python3 scripts/tb4-quest-board.py board` rebuilds the",
        "page from the saved snapshot, `knowledge/quests/microcoder-runs.json`.",
        "`python3 scripts/tb4-quest-board.py quests <task>...` writes a new quest",
        "spec, which is published with `microcoder xp quest <file> --relay URL`.",
        "",
    ]
    with open(BOARD, "w") as f:
        f.write("\n".join(lines))
    print(f"{BOARD}: {len(quests)} quests")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)
    q = sub.add_parser("quests", help="write quest specs for tasks")
    q.add_argument("tasks", nargs="+")
    b = sub.add_parser("board", help="write the quest board page")
    b.add_argument("--runs", help="Microcoder run records to read (quest tasks only)")
    b.add_argument("--ledger", help="`microcoder xp ledger --json` output")
    args = parser.parse_args()
    {"quests": cmd_quests, "board": cmd_board}[args.command](args)


if __name__ == "__main__":
    main()
