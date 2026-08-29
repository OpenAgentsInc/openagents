---
name: superdelegate
description: How to hand work to another agent, run Autopilot, and work through an issue backlog with safe parallelism. Loaded into every session.
auto: true
---

# Handing work out

## Which lane

| The work                                 | Use                 |
| ---------------------------------------- | ------------------- |
| One command, one answer                  | `shell`             |
| The same thing to N independent parts    | `delegate`          |
| Run Autopilot / keep going from context  | Autopilot below     |
| A backlog of issues, in parallel         | the method below    |

Run a single command yourself. Starting an agent to run `pwd` costs minutes and
real money and hands back an answer nobody watched being produced.

`delegate` starts child coding agents that run the same prompt in parallel, each
with its own file and shell tools. Children start with no context from this
conversation and cannot ask questions, so the prompt carries everything. Every
child gets the same prompt and is told its own number separately — write for
whichever child is reading rather than naming one.

The `delegate` tool uses the child lane configured for this Coder session. The
standalone `openagents delegate` command accepts `--lane`; supported values are
`openagents`, `gemini`, `opencode/<model>`, `devin`, `claude`, and `codex`.
Choose the lane before you start a fan-out.

`devin` runs the children on the Devin CLI, and `devin:<mode>` picks a
permission mode other than the default `dangerous`. A Devin child brings its own
credentials and model rather than spending this session's grant. Prefer it over
running `devin` yourself through `shell`: a fleet child renders, stops with
`ctrl+x`, and does not block the turn, while a shell child is one opaque call
that freezes the session until it ends.

## Autopilot

When the person says **run Autopilot**, start it. Do not reimplement the loop
in this session.

```
openagents coder --autopilot
openagents coder --autopilot "work the open issues"
openagents coder --autopilot --dry-run
openagents --autopilot
```

The CLI takes stock of this workspace, recent local Coder sessions, and open
issues, then keeps iterating until a stop condition. `--dry-run` prints that
plan without calling a model. Hosted lanes need a token; `--lane local` can
run unsigned when Ollama answers.

Autopilot is one unattended loop that picks the next unit itself. `delegate`
fans the same prompt out to N children. Do not wrap Autopilot in `delegate`.
Do not start Autopilot for one named issue you can finish here. The
`openagents-cli` skill has the rest.

## Burning through a backlog

When the task is "work the issues" rather than one named thing, this is the
method. It is opinionated on purpose: the decisions below are the ones that go
wrong when they are made ad hoc.

### 1. Read the board before touching it

List open issues in every repository the account can reach —
`OpenAgentsInc/openagents` and `OpenAgentsInc/openagents.com` at least. Use the
`openagents` tool, read the plain output, and use `--json` only to take a field.

### 2. Workable means unblocked

`openagents issue view <n>` reports `Blocked` and `Blocked by`. **A blocked issue
is not workable**, however ready it looks, and starting one wastes a child and
produces a change that cannot land. Name the blocked ones as blocked, say what
they wait on, and leave them.

Prefer `agent-ready`. An issue whose shape is still a question is not ready for
a child that cannot ask one.

### 3. One child per surface

Two children editing the same files is a merge conflict nobody asked for. Group
the workable issues by the surface they touch — a package, a directory, a
module — and keep **one child in flight per surface**. Issues on different
surfaces run together freely.

If two issues must touch the same surface, run them in sequence, or give each
child its own git worktree so their edits cannot meet.

### 4. Route by the kind of thinking

- **Devin** for straightforward engineering: a named fix, a test to write, a
  migration, a rename, a documented change with a clear shape.
- **OpenAgents** for design, architecture, and work where the shape of the
  answer is still the question.

If you cannot tell which, it is the second kind.

### 5. Find the width, do not pick it

**Never choose a number and hope.** Start at about four and climb:

1. Run a round of four.
2. Before widening, look: system load and free memory
   (`uptime`, `vm_stat` or `free`), `openagents auth status` for the account,
   and whether any child came back with a rate limit, a quota refusal, or a
   provider error.
3. If all three are clean, raise the width — four, six, eight — and go again.
4. At the **first** sign of a limit, stop raising and hold at the last width
   that was clean. Do not push past it to confirm; the confirmation costs a
   round of failed children.

The width is a measurement, not a setting. A machine with sixteen idle cores and
a healthy account should be running far more than four; one that is swapping
should be running fewer. Report the width you reached and what stopped it.

### 6. Report as you go

After each round, say which issues were selected, which were skipped and why,
what each child returned, and where the width stands. A backlog run that reports
only at the end is one nobody can steer.

## Working and completing issues

When assigned or requested to fix or work on an issue, deliver the solution
completely: verify the fix with tests, push directly to `main` (or the working branch),
and close the issue without a separate confirmation step.

Take care not to step on other people's concurrent work: check git status and remotes
before pushing. An agent reporting that it finished is not evidence that it did:
read the diff, run the test, and verify the output before closing.

### The unit-of-work flow (do this every time, unprompted)

The owner's standing instruction, from the 2026-07-20 mandate in `AGENTS.md`
and unlock 64 of `docs/coder/2026-08-28-local-session-audit.md`: every unit
of implement work runs **fresh worktree → land to `main` → clean up**, and
nobody should have to say so. When the person says "do the issue", this is
the flow:

1. **Claim** — comment on the issue that you are taking it (one line: actor,
   unit, done-when). The tracker is the ledger; this is what stops a sibling
   tab from implementing the same thing an hour later.
2. **Isolate** — `worktree start` (the managed tool, not raw
   `git worktree add`): it fetches `main` when a remote answers, creates a
   detached tree under `~/.openagents/worktrees`, points this session's file
   and shell tools at it, and sets `CARGO_TARGET_DIR` outside the disposable
   tree so the build cache survives. Never implement in the canonical
   checkout — it is frequently dirty with another agent's live work, and a
   mixed reset there has cost real landings.
3. **Implement and verify there** — package-and-name the test from the edit
   (`cargo test -p <pkg> <name>`), never a fishing `--workspace` run; the
   pre-push hook runs the full gate at push time.
4. **Land** — commit with a message that states the change and its evidence
   (a number, not an adjective), push to the forge remote (`openagents`)
   `main`, and bring the canonical checkout fast-forwarded if it is safe.
5. **Close and clean** — close the issue with the landing SHA in the close
   comment, then `worktree finish` with `landed=true` so the tree and branch
   are removed. A retry or regression test for the same unit is a
   continuation: reuse the worktree rather than opening a new one.

If the session dies before landing, the worktree stays and names itself —
`worktree finish` with `landed=false` says so honestly. What it must never
do is leave WIP in the shared checkout.
