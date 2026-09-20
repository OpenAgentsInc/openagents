#!/usr/bin/env python3
"""Builds `coder-turns-v1.json` from recorded coding-agent sessions.

`support-v2` is 196 support-desk messages written in this repository by the
person who reads the results. Every number the Gym has published rests on
it. `crates/coder/src/classify.rs` is the only production decision surface
here, and it had never been scored on anything: its question set reads a
state of a coding turn, and its thresholds were chosen by reading.

This builder produces the suite that scores that surface on states the
agent actually meets.

Where the states come from
--------------------------

Not from `docs/transcripts/`. That directory is the retained archive of the
*video* series — 293 machine transcriptions of episodes — and holds no
agent turns at all. `crates/coder` persists no session either: the
transcript lives in `Agent` for the life of the process and is dropped when
the terminal exits. So there is no recording of `coder` itself to harvest,
and this suite does not claim one.

What exists is the local Claude Code session record for this repository:
JSON Lines, one file per session, holding every human turn, every assistant
reply, every tool call and every tool result, in order. That is a coding
agent working in this repository for the same person, and the states are
reconstructed from it into the exact shapes `classify::state_of` and
`shell::state_of` build, under the same caps the production code applies.

Only sessions whose working directory is this repository are read. Sessions
against the private sibling repositories on this machine are excluded by
`AGENTS.md`, which forbids carrying their prompts here.

Where the labels come from
--------------------------

Two kinds of evidence, never mixed silently. Every item carries
`label_source` and the rule that produced it:

`outcome`
    The label is what happened next in the same session, read by a rule
    this file applies mechanically. Nobody's opinion enters it.

`author`
    The label is a reading of the state, written in
    `coder-turns-v1-judgments.json` and committed beside this builder.

`action`, `needs_code` and `risk` are outcome labels: the turn is followed
by a segment of the session that says whether the agent asked a clarifying
question, whether it opened the repository, and whether it wrote to it.
`shell_outcome` and `damage` are outcome labels for the same reason: the
round is followed by what the agent did with the outputs, and by whether
anyone ever undid anything.

`progress` and `useful` have no such successor evidence, so they are the
author's reading and say so. A suite that let them pass as outcomes would
make every downstream number inherit an ambiguity nothing could see.

What the sampling does
----------------------

Turn states are taken whole: every human turn with a complete segment.

Shell rounds are not. The archive holds far more rounds than a door can be
asked in an evening, and 94% of them are clean passes. Every round that is
not a pass is kept, and the passes are sampled to `--shell-pass`. The
suite therefore over-represents failure against the archive, deliberately,
and `sampling` in the manifest records the population counts so the archive
rate is recoverable from the per-class rates.

Rebuilding it
-------------

    cp ~/.claude/projects/<this repo>/*.jsonl /tmp/sessions/
    python3 build_coder_turns_v1.py --sessions /tmp/sessions > coder-turns-v1.json

Copy first. An open session file grows while it is being read, so a build
against the live directory harvests a different population every time, and
the author labels keyed to item ids would land on different items. The
sampling is append-stable for the same reason; the copy is belt and braces.

The sessions are not in this repository and cannot be: they are one
operator's local record. The suite file with its digest is the artifact, and
this builder is the statement of how the labels were derived, not a
reproduction anyone else can run.
"""

import argparse
import hashlib
import json
import os
import re
import sys

# The caps production applies, reproduced exactly.
TRANSCRIPT_TURNS = 12  # `classify::state_of`
HEAD_MAX = 2048  # `shell::HEAD_MAX`
COMMANDS_MAX = 10  # `shell::COMMANDS_MAX`

# A harvest cap with no production counterpart. Coder's own replies are
# terse by instruction; the source agent's are not, and an unbounded
# transcript message would make the state a fact about the other agent.
MESSAGE_MAX = 2048
TASK_MAX = 4096

# The workspace members `Repo::members` lists for this repository, which is
# the `repo_members` field of every turn state.
REPO_MEMBERS = [
    "coder",
    "coder-terminal",
    "gym",
    "jev",
    "kev",
    "lev",
    "nostr",
    "nostr-relay",
]

# A state matching any of these is dropped rather than redacted. The
# archive is an operator's console as well as a coding session: it reaches
# cloud projects, service accounts and secret files. Redaction that has to
# be right every time is the wrong shape for that; exclusion only has to be
# broad.
SECRET = re.compile(
    r"""(?ix)
    \.secrets/ | print-access-token | authorization:\s*bearer
    | (api[_-]?key|secret|token|password|passwd|mnemonic|private[_-]?key)\s*[:=]
    | \bsk-[A-Za-z0-9]{16,} | \bghp_[A-Za-z0-9]{20,} | \boa_agent_[A-Za-z0-9]{10,}
    | \bya29\. | \beyJ[A-Za-z0-9_-]{20,} | -----BEGIN\ [A-Z]
    """
)

# A run long enough and mixed enough to be a credential rather than a word,
# a path or a commit. Paths and identifiers carry separators, and a git
# object name is lower-case hex, so both stay out; a base64 or hex-mixed
# secret does not.
OPAQUE = re.compile(r"[A-Za-z0-9+=]{32,}")


def opaque(text):
    """Whether `text` holds a run that looks like a credential."""
    for run in OPAQUE.findall(text):
        if any(c.islower() for c in run) and any(c.isupper() for c in run) and any(c.isdigit() for c in run):
            return True
    return False

# The one identifying path in the archive, which every state would
# otherwise carry.
HOME = "/Users/christopherdavid"

# A path into one of the private sibling repositories on this machine, or
# into Apple's adapter toolkit, which is licensed and not redistributable.
#
# `AGENTS.md` forbids carrying private backend code from the siblings into
# this repository, and a recorded session does it by accident: an agent runs
# `sed -n 80,260p` in another checkout and 180 lines of somebody else's
# deploy script land in the output. A state whose *commands* reach out that
# way is dropped.
#
# A state whose prose merely names a sibling is kept. "look deeply thru
# ~/work/psionic" is the operator describing the work, it is already in
# public issues, and it is the most common shape of turn this workload has.
# The rule is about reproduced content, not about mentions.
FOREIGN = re.compile(
    r"~/work/(?!openagents(?![\w.-])|kev-artifacts(?![\w.-])|\.)[\w.][\w.-]*"
    r"|adapter_training_toolkit"
)


def reaches_out(state):
    """Whether this state carries content read from outside this repository."""
    for command in state.get("commands", []):
        if FOREIGN.search(command.get("command", "")) or FOREIGN.search(
            command.get("output", "")
        ):
            return True
    for message in state.get("transcript", []):
        text = message.get("text", "")
        if text.startswith('{"v":1') or text.startswith("ran shell commands:"):
            if FOREIGN.search(text):
                return True
    return False

# Tools that read or change files in a repository.
FILE_TOOLS = {"Read", "Edit", "Write", "NotebookEdit", "MultiEdit", "Grep", "Glob"}
# Tools that change them.
WRITE_TOOLS = {"Edit", "Write", "NotebookEdit", "MultiEdit"}
# Tools that hand the work to another agent, which still opens the
# repository — through somebody else's hands.
DELEGATE_TOOLS = {"Agent", "Task"}
# A shell command that writes, as `risk` level 2 describes one.
WRITES = re.compile(
    r"""(?x)
    (^|[;&|]\s*)(rm|mv|cp|mkdir|touch|chmod|tee|install|ln)\s
    | \bsed\s+-i | \bgit\s+(commit|push|add|checkout|restore|reset|rebase|merge|stash|apply)\b
    | \bcargo\s+(fmt|fix|add|remove)\b | \bgh\s+(issue|pr)\s+(create|close|comment|edit)\b
    | >>?\s*[^|&\s>]
    """
)
# What a failed command looks like in a recorded tool result.
FAILED = re.compile(
    r"(?i)exit code:?\s*[1-9]|command not found|no such file or directory|"
    r"^error:|\berror\[E\d+\]|permission denied|fatal:"
)

SCHEMA = "openagents.gym.suite.v1"
GATE = "probability-v1"
# The question text lives in `crates/gym/questions/`, once per family, and
# the items carry none of it. The id is outside the digest, like the gate's:
# rewording a question produces a new set to compare against these items
# rather than a new suite that cannot be compared with them at all.
QUESTIONS = "coder-turns-v1"

# The rule that produced each family's label, carried by every item.
RULES = {
    "action": (
        "outcome: the agent asked a question and ran nothing before the next "
        "human turn is `clarify`; anything else it did is `respond`"
    ),
    "needs_code": (
        "outcome: the agent opened, changed or delegated work on repository "
        "files before the next human turn"
    ),
    "risk": (
        "outcome: 2 if the agent wrote a file or ran a writing command, 1 if "
        "it only read, 0 if it ran nothing"
    ),
    "progress": "author: a reading of where the turn sits in the work",
    "shell_outcome": (
        "outcome: every command exited 0 is `pass`; a failure the agent "
        "followed with more commands is `retry`; a failure it stopped on is "
        "`stop`"
    ),
    "damage": (
        "outcome: `yes` only where the session afterwards undoes, restores or "
        "reports harm from this round"
    ),
    "useful": "author: a reading of whether the outputs move the task",
}

# The three-way round robin, applied within each family. The states are
# sorted by their labels first, so every partition gets the same label mix
# rather than whichever cases the session happened to open with.
CYCLE = ["calibration", "calibration", "development", "development", "locked"]


def text_of(message):
    """Every text block of one message, joined."""
    content = message.get("content")
    if isinstance(content, str):
        return content
    blocks = [b.get("text", "") for b in (content or []) if b.get("type") == "text"]
    return "\n".join(b for b in blocks if b.strip()).strip()


def tool_uses(message):
    """The tool calls one assistant message made, in order."""
    content = message.get("content")
    if not isinstance(content, list):
        return []
    return [b for b in content if b.get("type") == "tool_use"]


def result_text(block):
    """A tool result's text, whatever shape it arrived in."""
    content = block.get("content")
    if isinstance(content, list):
        content = "".join(b.get("text", "") for b in content if b.get("type") == "text")
    return content or ""


def clip(text, limit):
    """`text` cut to `limit` bytes on a character boundary."""
    raw = text.encode("utf-8")
    if len(raw) <= limit:
        return text
    return raw[:limit].decode("utf-8", "ignore")


def scrub(text):
    """The one identifying path removed. Nothing else is rewritten."""
    return text.replace(HOME, "~")


def load(path):
    """The user and assistant rows of one session, in order."""
    rows = []
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if not line:
                continue
            row = json.loads(line)
            if row.get("type") in ("user", "assistant"):
                rows.append(row)
    return rows


def is_human(row):
    return row.get("type") == "user" and row.get("origin", {}).get("kind") == "human"


def results_of(rows):
    """Every tool result in the session, by the call it answers."""
    index = {}
    for row in rows:
        content = row["message"].get("content")
        if row.get("type") == "user" and isinstance(content, list):
            for block in content:
                if block.get("type") == "tool_result":
                    index[block.get("tool_use_id")] = block
    return index


def status_of(block):
    """The status line production would have written for this result."""
    text = result_text(block)
    found = re.search(r"(?i)exit code:?\s*(\d+)", text[:4000])
    if found:
        return "exit " + found.group(1)
    if block.get("is_error") or FAILED.search(text[:4000]):
        return "exit 1"
    return "exit 0"


def failed(block):
    return status_of(block) != "exit 0"


def render(rows, results):
    """The rows before a turn, as the messages coder's transcript holds.

    Coder has one tool. An assistant message that ran shell commands is a
    plan in its transcript, and the outputs come back as a user message. A
    call to a tool coder does not have — an editor, a subagent — has no
    faithful rendering, so it is left out of the transcript and counted only
    where the labels read it.
    """
    messages = []
    for row in rows:
        if is_human(row):
            text = text_of(row["message"])
            if text:
                messages.append({"role": "user", "text": clip(text, MESSAGE_MAX)})
            continue
        if row.get("type") == "assistant":
            text = text_of(row["message"])
            commands = [
                {
                    "command": use["input"].get("command", ""),
                    "why": use["input"].get("description", ""),
                }
                for use in tool_uses(row["message"])
                if use.get("name") == "Bash"
            ][:COMMANDS_MAX]
            if text:
                messages.append({"role": "assistant", "text": clip(text, MESSAGE_MAX)})
            if commands:
                plan = json.dumps({"v": 1, "commands": commands}, ensure_ascii=False)
                messages.append({"role": "assistant", "text": clip(plan, MESSAGE_MAX)})
                outputs = "ran shell commands:\n"
                for use in tool_uses(row["message"]):
                    if use.get("name") != "Bash":
                        continue
                    block = results.get(use["id"])
                    if block is None:
                        continue
                    outputs += "\n$ {}\n{}\n{}\n".format(
                        use["input"].get("command", ""),
                        status_of(block),
                        clip(result_text(block), HEAD_MAX),
                    )
                messages.append({"role": "user", "text": clip(outputs, MESSAGE_MAX)})
    return messages


def turn_states(rows, session):
    """One state per human turn, with the labels its segment supports."""
    results = results_of(rows)
    humans = [i for i, row in enumerate(rows) if is_human(row)]
    out = []
    for nth, index in enumerate(humans):
        end = humans[nth + 1] if nth + 1 < len(humans) else len(rows)
        segment = rows[index + 1 : end]
        if not segment:
            # A turn the agent never answered: queued, interrupted, or the
            # live session's last word. There is no successor evidence, so
            # there is no outcome label.
            continue
        task = text_of(rows[index]["message"])
        if not task.strip():
            continue

        uses = [use for row in segment if row.get("type") == "assistant" for use in tool_uses(row["message"])]
        names = [use.get("name") for use in uses]
        said = [text_of(row["message"]) for row in segment if row.get("type") == "assistant"]
        said = [text for text in said if text.strip()]

        opened = any(name in FILE_TOOLS or name == "Bash" for name in names)
        delegated = any(name in DELEGATE_TOOLS for name in names)
        wrote = any(name in WRITE_TOOLS for name in names) or any(
            use.get("name") == "Bash" and WRITES.search(use["input"].get("command", ""))
            for use in uses
        )

        if not uses and said and said[-1].rstrip().endswith("?"):
            action = "clarify"
        elif uses or said:
            action = "respond"
        else:
            continue

        state = {
            "task": clip(task, TASK_MAX),
            "transcript": render(rows[: index + 1], results)[-TRANSCRIPT_TURNS:],
            "repo_members": REPO_MEMBERS,
        }
        out.append(
            {
                "kind": "turn",
                "source": "{}#{}".format(session, nth),
                "state": state,
                "labels": {
                    "action": action,
                    "needs_code": "yes" if opened or delegated else "no",
                    "risk": str(2 if wrote else 1 if uses else 0),
                },
            }
        )
    return out


def shell_states(rows, session):
    """One state per round of shell commands, with what followed it."""
    results = results_of(rows)
    humans = [i for i, row in enumerate(rows) if is_human(row)]
    out = []
    for nth, index in enumerate(humans):
        end = humans[nth + 1] if nth + 1 < len(humans) else len(rows)
        segment = rows[index + 1 : end]
        task = text_of(rows[index]["message"])
        rounds = []
        for row in segment:
            if row.get("type") != "assistant":
                continue
            uses = [use for use in tool_uses(row["message"]) if use.get("name") == "Bash"]
            if uses:
                rounds.append(uses[:COMMANDS_MAX])
        for count, uses in enumerate(rounds):
            commands = []
            broke = False
            for use in uses:
                block = results.get(use["id"])
                if block is None:
                    continue
                commands.append(
                    {
                        "command": use["input"].get("command", ""),
                        "why": use["input"].get("description", ""),
                        "status": status_of(block),
                        "output": clip(result_text(block), HEAD_MAX),
                    }
                )
                broke = broke or failed(block)
            if not commands:
                continue
            if not broke:
                outcome = "pass"
            elif count + 1 < len(rounds):
                outcome = "retry"
            else:
                outcome = "stop"
            # `damage` is `yes` only where the session says so afterwards.
            # Nothing in this archive does, and the builder refuses to
            # invent one: a label with no instance is reported as absent.
            out.append(
                {
                    "kind": "shell",
                    "source": "{}#{}.{}".format(session, nth, count),
                    "state": {"task": clip(task, TASK_MAX), "commands": commands},
                    "labels": {"shell_outcome": outcome, "damage": "no"},
                }
            )
    return out


def harvest(sessions):
    """Every usable state in every session file, in file order."""
    states = []
    for name in sorted(os.listdir(sessions)):
        if not name.endswith(".jsonl"):
            continue
        rows = load(os.path.join(sessions, name))
        if not rows:
            continue
        session = name[:8]
        states.extend(turn_states(rows, session))
        states.extend(shell_states(rows, session))
    kept, seen, dropped, outside = [], set(), 0, 0
    for state in states:
        blob = json.dumps(state["state"], sort_keys=True, ensure_ascii=False)
        if SECRET.search(blob) or opaque(blob):
            dropped += 1
            continue
        state["state"] = json.loads(scrub(blob))
        if reaches_out(state["state"]):
            outside += 1
            continue
        digest = hashlib.sha256(
            json.dumps(state["state"], sort_keys=True, ensure_ascii=False).encode()
        ).hexdigest()
        if digest in seen:
            dropped += 1
            continue
        seen.add(digest)
        kept.append(state)
    return kept, dropped, outside


def sample(states, keep, seed):
    """Every turn, every interesting round, and a sample of the clean ones.

    The sample is a hash of each round's own id against a threshold, not a
    draw of `n` from a list. A session file grows while it is open, and a
    draw of `n` reshuffles every time the population changes, so a label
    written against one build would land on a different item in the next.
    A threshold over ids adds candidates without moving the ones already
    chosen.
    """
    turns = [state for state in states if state["kind"] == "turn"]
    shells = [state for state in states if state["kind"] == "shell"]
    broken = [state for state in shells if state["labels"]["shell_outcome"] != "pass"]
    clean = [state for state in shells if state["labels"]["shell_outcome"] == "pass"]
    chosen = sorted(
        (state for state in clean if keeps(state["source"], keep, seed)),
        key=lambda state: state["source"],
    )
    population = {
        "turns": len(turns),
        "shell_rounds": len(shells),
        "shell_pass": len(clean),
        "shell_pass_kept": len(chosen),
        "shell_retry": sum(1 for s in broken if s["labels"]["shell_outcome"] == "retry"),
        "shell_stop": sum(1 for s in broken if s["labels"]["shell_outcome"] == "stop"),
    }
    return turns + broken + chosen, population


def keeps(source, keep, seed):
    """Whether this round is in the sample, decided by its own id alone."""
    digest = hashlib.sha256("{}:{}".format(seed, source).encode()).hexdigest()
    return int(digest[:8], 16) % 1000 < keep


def partitioned(states):
    """A partition per state, so one state's items never straddle two."""
    order = sorted(
        states,
        key=lambda state: (
            state["kind"],
            tuple(sorted(state["labels"].items())),
            state["source"],
        ),
    )
    seen = {}
    for state in order:
        key = state["kind"]
        state["partition"] = CYCLE[seen.get(key, 0) % len(CYCLE)]
        seen[key] = seen.get(key, 0) + 1
    return states


def questions(path):
    """The question set's bodies, which name each family's question type."""
    with open(path, encoding="utf-8") as handle:
        return json.load(handle)["questions"]


def build(sessions, wire, judgments, passes, seed):
    states, dropped, outside = harvest(sessions)
    states, population = sample(states, passes, seed)
    states = partitioned(states)
    kinds = {"choice": "choice", "noul": "noul", "score": "score"}
    items = []
    for state in sorted(states, key=lambda state: (state["kind"], state["source"])):
        for family, truth in state["labels"].items():
            items.append(
                {
                    "id": "{}/{}".format(family, state["source"]),
                    "family": family,
                    "kind": kinds[wire[family]["type"]],
                    "state": state["state"],
                    "truth": truth,
                    "partition": state["partition"],
                    "label_source": "outcome",
                    "label_rule": RULES[family],
                }
            )
        for family in ("progress", "useful"):
            key = "{}/{}".format(family, state["source"])
            if key not in judgments:
                continue
            items.append(
                {
                    "id": key,
                    "family": family,
                    "kind": kinds[wire[family]["type"]],
                    "state": state["state"],
                    "truth": judgments[key],
                    "partition": state["partition"],
                    "label_source": "author",
                    "label_rule": RULES[family],
                }
            )
    suite = {
        "schema": SCHEMA,
        "name": "coder-turns-v1",
        "created": "2026-09-19",
        "description": (
            "Real turns and shell rounds from coding-agent sessions on this "
            "repository, asked the questions crates/coder/src/classify.rs "
            "sends, in the state shapes classify::state_of and "
            "shell::state_of build. Every item names its label_source: "
            "`outcome` labels are what happened next in the same session, "
            "read by the rule in label_rule; `author` labels are a reading "
            "of the state. The two are never pooled without saying so."
        ),
        "tier": "scored",
        "gate": GATE,
        "questions": QUESTIONS,
        "sampling": population
        | {
            "dropped": dropped,
            "reached_outside": outside,
            "pass_per_mille": passes,
            "seed": seed,
        },
        "items": items,
    }
    blob = json.dumps(items, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    suite["digest"] = hashlib.sha256(blob.encode()).hexdigest()
    return suite


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--sessions",
        default=os.path.expanduser(
            "~/.claude/projects/-Users-christopherdavid-work-openagents"
        ),
        help="the directory of session JSON Lines files to harvest",
    )
    parser.add_argument(
        "--wire",
        default=os.path.join(here, "..", "questions", "coder-turns-v1.json"),
        help="the question set, which names each family's question type",
    )
    parser.add_argument(
        "--judgments",
        default=os.path.join(here, "coder-turns-v1-judgments.json"),
        help="the author's labels, by item id",
    )
    parser.add_argument(
        "--shell-pass",
        type=int,
        default=62,
        help="how many of every thousand clean rounds to keep, by id hash",
    )
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument(
        "--states",
        action="store_true",
        help="print the harvested states instead of the suite, for labelling",
    )
    args = parser.parse_args()

    wire = questions(args.wire)
    judgments = {}
    if os.path.exists(args.judgments):
        with open(args.judgments, encoding="utf-8") as handle:
            judgments = json.load(handle)["labels"]
    if args.states:
        states, dropped, outside = harvest(args.sessions)
        states, population = sample(states, args.shell_pass, args.seed)
        json.dump(
            {
                "dropped": dropped,
                "reached_outside": outside,
                "population": population,
                "states": states,
            },
            sys.stdout,
            indent=1,
            ensure_ascii=False,
        )
        sys.stdout.write("\n")
        return
    suite = build(args.sessions, wire, judgments, args.shell_pass, args.seed)
    json.dump(suite, sys.stdout, indent=1, ensure_ascii=False)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
