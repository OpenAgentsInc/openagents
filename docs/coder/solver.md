# `coder solver`: four constrained agents take an issue to a pull request

`coder solver N` runs issue N to a pull request in four hands, each a
Claude Agent SDK session (`crates/claude_agent_sdk`) on this computer's
Claude Code login, each holding only the tools its part of the work needs.
It is the structured sibling of `coder issue-run` (`issue-run.md`), which
runs one briefed agent, and it is the mode `microclaude` promised: Shift+Tab
in the Coder terminal steps `live` → `microclaude` → `solver`.

A solver also runs in a **swarm**: the live model hands N issues to N
solvers at once, and they coordinate over the agent channel so two solvers
do not edit the same file or hold the same build lease. See
[Subagents talk to each other](subagent-channel.md).

## The four hands

| Hand | Model | Tools | Does |
|---|---|---|---|
| **Intaker** | Haiku | `environment`, `issue`, `worktree` (in-process) | Checks that `git`, `gh`, and the Claude Code login are present and that the checkout is clean, reads the issue, makes the run's fresh worktree at the issue's base, and sums up what it found. It asks the person for the repository and the issue when they were not given. |
| **Briefer** | Haiku | Read, Grep, Glob (held to the worktree) | Runs the existing code-brief algorithm (the context finder's stages, scorer and System One questions, then the briefing generator; `issue_run::decide`), then reads the listed files and writes its **analysis**: the files to change, the shape of the change, what tests to add, and the checks that prove it. The analysis and the file list join the briefing. |
| **Coder** | Opus | Read, Edit, Write, Grep, Glob (held to the worktree) | Codes only. No shell, no checks, no issue reading. Gets the briefing, the analysis, and, on a second round, the Tester's notes. |
| **Tester** | Haiku | Read, Grep, Glob, `run_check` (the briefing's checks) | Called only when a required check fails. Reads the failures the harness hands it and the code behind them, and ends with one verdict line: `VERDICT: coder: <what to fix>` or `VERDICT: briefer: <what context is missing>`. |

After the Coder, the harness runs every required check itself: the touched
crates' `check:` and `test:` entries, a browser crate's `wasm:` half, and
Markdown links. A test that already fails at the base commit is excused
(the failed tests rerun in a sibling `baseline` worktree). When everything
passes the run opens the pull request with no Tester session; when a check
reports a computer fault (a full disk, a dead linker) the run stops without
a hand-back, because no edit can fix it (`issue_run::verdict`).

## Handoffs

```
Intaker → Briefer → Coder → Tester → PR
              ↑        ↑       │
              │        └───────┘  VERDICT: coder   (≤ --rounds)
              └────────────────┘  VERDICT: briefer (≤ --rounds)
```

`--rounds R` (default 2) bounds the hand-backs. When the rounds are spent,
or the harness's own checks fail, the run ends as `failed` and keeps its
worktree; no pull request is opened. When they pass and the issue is open,
the run commits to `coder/issue-N`, pushes, and opens the pull request
(`issue_run::setup::open_pr`). `--no-pr` stops before that step.

## Run it

```sh
coder solver 11341                       # terminal screen
coder solver 11341 --plain               # text, each item as it finishes
coder solver 11341 --plain --json        # one JSON line per event (NDJSON)
coder solver 11341 --repo ~/code/openagents --github OpenAgentsInc/openagents
coder solver 11341 --coder-model claude-opus-5-5 --rounds 1 --no-pr
coder solver --replay ~/.openagents/coder-new/solver-runs/<id>/events.jsonl
```

Without a terminal (a pipe), `--plain` is implied. The run's folder,
`~/.openagents/coder-new/solver-runs/<id>/`, holds `run.json`,
`events.jsonl` (the recording `--replay` plays), `briefing.md`,
`analysis.md`, one `HAND-ROUND-events.jsonl` per hand (`intaker-events.jsonl`,
`briefer-0-events.jsonl`, `coder-1-events.jsonl`, ...), `summary.json`, and
the worktree the Coder changed. `summary.json` carries `issue-run`'s summary
plus a `solver` object that names each hand's time, tokens, cost and
verdict, the rounds, and the pull request URL when one was opened.

`coder solver conform --run DIR [--run DIR ...]` judges recorded runs
against `programs/solve-issue.json`: it maps each hand onto the program's
step (`intaker` to `intake`, `briefer` to `brief`, `coder` to `code`, a
Tester turn to `check`, the last Tester turn of a landed run to `land`, and
a landed run ends in `done`), unrolls the program's code and check pair per
coding round, and prints one JSON line per run with `recorded`, `program`,
and `conforms`. It exits with an error when any run does not conform. The
program replaces the hand-coded loop only when every recorded run conforms.

In the terminal, the `solver` mode's conversation takes an issue number or
`owner/repo#N`; anything else gets the Intaker's question back.

## Why four hands

The briefed-agent bench (#11211) won on a briefing plus a small tool set.
Splitting the work keeps that constraint per step: the Coder cannot wander
the repository or run the shell, the Briefer cannot edit, and the Tester
cannot change code, so a failing check comes back as a note, not an
unreviewed fix. Each hand's model matches its job: Haiku for intake and
analysis, Opus for the change, Sonnet for reading failures.
