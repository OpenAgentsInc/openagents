# Delegation brief

Written 2026-09-20 to hand this work to the next agent. It says where things
stand, what to do next and why, and the constraints that are easy to lose.

Read [`AGENTS.md`](../AGENTS.md), the
[glossary](glossary.md), and
[`.agents/skills/google-developer-style/SKILL.md`](../.agents/skills/google-developer-style/SKILL.md)
before starting. Every piece of prose here follows that style guide.

## The goal

An operator says *clear the backlog*, and Coder runs a program that fans out
delegated sessions to do it, over our own relay, with every decision and
delegation recorded as an ATIF trace that `coderbench` can judge against a
golden.

[#9404](https://github.com/OpenAgentsInc/openagents/issues/9404) is the
roadmap. [#9413](https://github.com/OpenAgentsInc/openagents/issues/9413) is
the burndown that goal exists for.

## What works today

| Piece | State |
| --- | --- |
| ATIF traces | Every session records itself. Decision calls are first-class. |
| Headless turns | `coder -p`, and both modes share one turn so they cannot drift. |
| Delegation | Six parallel Devin sessions, 6 of 6 correct, 4.2x over sequential. |
| Capability probe | Three states. A present executor can still refuse a directory. |
| Program runtime | Runs `delegate-fan-out` from its definition. Zero faults against the golden. |
| Relay transport | Proven against production, 470 ms round trip. A worker exists. |
| CoderBench | Runs an episode, refuses when the machine is wrong, judges the trace. |
| Execution boundary | The host decides whether a turn may run commands. |

## What to do next, in order

### 1. Finish the blockers that are in flight

Five issues were being worked when this was written:
[#9416](https://github.com/OpenAgentsInc/openagents/issues/9416)/[#9417](https://github.com/OpenAgentsInc/openagents/issues/9417)
(subprocess supervision),
[#9418](https://github.com/OpenAgentsInc/openagents/issues/9418) (grader
integrity),
[#9434](https://github.com/OpenAgentsInc/openagents/issues/9434) (the turn
reaches the runtime),
[#9439](https://github.com/OpenAgentsInc/openagents/issues/9439)/[#9440](https://github.com/OpenAgentsInc/openagents/issues/9440)
(stream bounds and gateway lanes), and
[#9441](https://github.com/OpenAgentsInc/openagents/issues/9441) (finding
work). Check which landed before starting anything.

### 2. [#9427](https://github.com/OpenAgentsInc/openagents/issues/9427), held on purpose

Read-only is declared and not enforced. It was held because it rewrites
`crates/coder/src/delegate.rs`, which #9416 was rewriting at the same time.
Start it once #9416 lands, and not before.

### 3. Re-record the golden as observed

`crates/coderbench/goldens/devin-fan-out-six.atif.jsonl` is **staged**: every
call in it is real and a shell script drove them, not Coder. Once #9434 and
#9441 land, run the episode from an operator's sentence, and **replace** the
staged file rather than keeping both. Its sidecar becomes `observed` with
orchestrator `coder`, and the test asserting it is staged fails — that test
is written to fail, and updating it is part of the change.

This is #9404's definition of done.

### 4. The first burndown, and not the one the issue describes

**Do not point a query-driven fan-out at the open backlog.** Run the first
one on issues chosen **by inspection**, with hosted Jev as the door, and the
independence decision in **shadow** — recorded, not acted on.

The reason is in the next section.

## The constraints that are easy to lose

### The independence gate is not safe

[#9414](https://github.com/OpenAgentsInc/openagents/issues/9414) measured it
on a 192-item factorial panel. On plans whose tasks **genuinely collide**, 11
of 12 answers cleared the 0.7 bound on local doors — `kev-8b` at 0.96 and
0.97.

**Raising the bound does not help: the wrong answers sit above the right
ones.** An `independent` accuracy of 0.75 hides this entirely, because most
panel items are genuinely independent, so a door that says "independent" to
everything scores well and refuses nothing.

The mechanism is worse than a bad threshold. `kev-4b` scores 0 of 16 on *"No
task writes a file"* and 14 of 16 on *"At least one task writes a file"* over
the same states, *p* = 0.00003. Those answers do not contradict — they
**agree**, correlating +0.95 where hosted Jev correlates −1.00. The door
answers the **topic** and attaches it to whichever proposition it is handed.
Calibration cannot fix that, because the signal is about the wrong question.

Hosted Jev is the only door measured that this gate can rest on.

### Our backlog collides

Several open issues touch `crates/gym`, and #9391 must land before #9401. A
`select` that returns the top N open issues hands a known-unsafe gate exactly
the input it fails on.

### Prefer an enforced boundary over a decision

#9409 built worktree isolation and verified it both directions. Where both an
enforced boundary and a decision are available, take the boundary. It does
not depend on a model reading a proposition correctly.

### Judge against the measured floors

- Accuracy: **0.056** for a two-door comparison, two sigma of a 0.0197 seed spread.
- Calibration, derived in #9376: ECE **0.0266**, Brier **0.0119**, NLL **0.6428**.
- Confident errors: σ **2.49**, which is why that criterion refuses an unchanged door about half the time ([#9401](https://github.com/OpenAgentsInc/openagents/issues/9401)).

**Five claims were withdrawn in one week for not clearing their own noise**,
three of them because the floor was measured after the claim was published.
Measure the floor first.

### Check headroom before running an experiment

[#9392](https://github.com/OpenAgentsInc/openagents/issues/9392) ran a
complete text-optimization experiment against a partition scoring 0.975 at
baseline. Total headroom was 0.025 against a floor of 0.056, so **no win was
available at any strength, whatever it tried.** Publish the headroom beside
the digest.

### A question that is nearly always the same answer is a trap

[#9395](https://github.com/OpenAgentsInc/openagents/issues/9395) found six of
seven production questions score no better than a constant on real traffic.
Before adding a question, measure what a constant scores. And weigh errors by
what they cost: a missed program answers normally; a spurious one starts six
subprocesses.

### Evidence is not edited to stay tidy

A recording says what happened. Two renames invalidated two goldens today,
and both times the golden was **deleted and re-recorded** rather than having
its prompts rewritten. A recording of a world that no longer exists reads as
evidence and is not.

The same rule is why `Provenance` has three states: `observed`, `staged`,
`authored`. "Recorded" was covering both a recording of the program under
test and a recording of something else doing what it should do.

## Working notes

- **`cargo +1.97.1`.** The default toolchain is older and the workspace needs 1.95 or better.
- **`main` is not rustfmt-clean.** `cargo fmt --all` rewrites about 50 files across five crates. Six agents have now reverted that churn to keep a commit scoped. [#9402](https://github.com/OpenAgentsInc/openagents/issues/9402) fixes it and wants a quiet moment.
- **Hosted Jev:** `set -a; . ~/work/.secrets/typesafe.env; set +a`. `crates/jev`'s `Config` reads the process environment and loads **no** dotenv, so exporting first is required; a missing key and a missing `model` field in the body produce different errors. Never print the key anywhere.
- **Local doors:** `~/work/kev-artifacts/` holds four kev checkpoints and their bases. `kev-serve` takes about 45 seconds to load on CPU and answers in roughly 2 seconds.
- **Devin:** at `~/.local/bin/devin`, **not on a spawned subshell's `PATH`**. It also refuses a workspace it does not trust, including a git worktree under `/private/tmp` — which is where agent worktrees live, so live delegation from one is refused.
- **A flake to watch:** `program_run::the_recorded_run_is_the_path_the_task_expects` failed once under a parallel multi-crate run and passed alone and on three repeats. A flaky test in the runtime an unattended burndown depends on is worth chasing rather than waiting out.

## The audit

`docs/audits/2026-09-19-codebase-audit/` reviewed the workspace at
`1843fa6c18` and reproduced failures through public APIs **while the
applicable test suites passed**. Its 25 findings are filed as #9415–#9433.

A01 is fixed. A02–A04 were in flight when this was written. The rest are
intended as burndown fodder — they are the first real workload for the system
this brief describes.

Read a finding before its issue. The audit's reasoning is better than any
summary of it, including this one.
