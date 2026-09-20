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
| CoderBench | Runs an episode, refuses when the machine is wrong, judges the trace three-valued. |
| Execution boundary | The host decides whether a turn may run commands. |
| Doors | Two gateway lanes, bounded streams, and model ids in exactly one file. |
| Subprocesses | One supervisor owns a job, its process tree, and what it prints. |
| Finding work | A `query` step resolves a named source, bounded and ordered, and records what it dropped. |
| The whole path | `coder -p "Delegate six…"` selects the program, fans out six real delegations, and lands no missing decisions or checks. |

## What to do next, in order

### 0. Know what a grade now means

[#9418](https://github.com/OpenAgentsInc/openagents/issues/9418) made the
grade three-valued, reusing `gym::gate::Verdict` rather than inventing a
second vocabulary. Two consequences will surprise you:

- **`coderbench diff` can no longer return a pass.** A trace carries neither
  the run's exit code nor the workspace, so a diff reports what it cannot
  see. Only `coderbench run` observes both. A diff of the staged golden
  exits `4` naming exactly what is missing, and that is correct behaviour
  rather than a regression.
- **Missing evidence is `unverifiable`, not a pass.** A delegation counts
  only if the trace says it completed, says it was correct, and holds an
  answer. `correct: None` is a fault.

`coderbench` now depends on `gym` for that type, which pulls `jev`, `tokio`,
and `reqwest` into its build graph. One vocabulary was judged worth the
weight; moving `Verdict` somewhere lighter is a reasonable future change.

### 1. Nothing is in flight

Every blocker listed on #9404 has landed. The path runs end to end from a
sentence. What remains is below, in order.

### 2. [#9427](https://github.com/OpenAgentsInc/openagents/issues/9427), held on purpose

Read-only is declared and not enforced. It was held because it rewrites
`crates/coder/src/delegate.rs`, which #9416 was rewriting at the same time.
**#9416 has landed and that file is free — this is the next thing to start.**

Note `shell::run` gained a `Permit` parameter from #9415 and the bounded
form is now `run_within(proposal, permit, wall)`.

### 3. Make a delegation's answer checkable, then re-record the golden

The path already runs: four `coderbench run devin-fan-out-six` episodes
driven through `coder -p` selected `delegate-fan-out` at 0.96, ran six real
Devin delegations that all answered correctly, and landed **no
`DecisionMissing` and no `CheckMissing`** — the line #9409 left open.

**The grade is `unverifiable`, and correctly so.** #9418 requires a checked
answer per delegation, and a task read out of a sentence carries no expected
answer. The six answers were right and nothing in the run establishes it.

Two candidates, neither picked:

- The manifest checks recorded outputs, which keeps the expectation in the
  task where a reader can argue with it.
- The `accept` decision becomes the evidence, which is closer to how a real
  burndown would work and inherits whatever that door's judgment is worth.

Pick one before re-recording, because the golden should be observed **and**
graded rather than observed and unverifiable.

### 3b. Re-record the golden as observed

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

### The supervisor is not a sandbox, and says so

#9416 gives a job its own process group and terminates the tree on a
deadline or a cancellation — the audit's probe went from
`descendant_wrote_after_timeout=true` to `false`. Three limits are written
down rather than implied, and they matter before anything runs unattended:

- **It is not a sandbox.** It bounds what a job costs, not what it may reach.
- A descendant that calls `setsid` **escapes the group**. The output drains
  are bounded for exactly that case.
- The group is signalled microseconds after its leader is reaped, which is a
  pid-recycle window. `waitid(WNOWAIT)` would close it and is not on Tokio's
  wait path.

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

### The selection question loses to a constant, and shipped anyway

The first measurement of program selection is the clearest instance of the
trap below, and worth reading before adding any question.

On 32 real turns the constant `none` scores **0.969**. Hosted Jev scores
**0.938** — below the constant. Headroom was 0.031 against a 0.056 floor, so
**no win was available at any strength**, which is #9392's lesson arriving a
second time.

It shipped because the **error structure**, not the accuracy, is what
matters here: 0 false negatives in 9, 3 false positives in 35, and all four
mistakes chose `answer-question` rather than `delegate-fan-out`. **None ran
anything**, because a program cannot fan out over work the request never
named. The cheap error is the one that happens, and it is structural rather
than threshold-dependent.

Keep that distinction. An accuracy figure would have refused this feature or
approved it for the wrong reason.

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
- **`cargo clippy --workspace` fails** on `crates/kev` and `crates/lev` under 1.97.1 (`manual_is_multiple_of` and similar). Pre-existing, and part of what [#9429](https://github.com/OpenAgentsInc/openagents/issues/9429) is for. Lint one crate at a time until it lands.
- **`main` is not rustfmt-clean.** `cargo fmt --all` rewrites about 50 files across five crates. Six agents have now reverted that churn to keep a commit scoped. [#9402](https://github.com/OpenAgentsInc/openagents/issues/9402) fixes it and wants a quiet moment.
- **Credentials:** hosted Jev is `set -a; . ~/work/.secrets/typesafe.env; set +a`, and a local door key is at `~/work/.secrets/coder-local-door.env`. Both are machine-local and gitignored; never print either. `crates/jev`'s `Config` reads the process environment and loads **no** dotenv, so exporting first is required, and a missing key and a missing `model` field in the body produce different errors.
- **Local doors:** `~/work/kev-artifacts/` holds four kev checkpoints and their bases. `kev-serve` takes about 45 seconds to load on CPU and answers in roughly 2 seconds.
- **Devin:** at `~/.local/bin/devin`, **not on a spawned subshell's `PATH`**. It also refuses a workspace it does not trust, including a git worktree under `/private/tmp` — which is where agent worktrees live, so live delegation from one is refused.
- **One flake is explained, one is not.** `delegate::tests::the_fan_out_is_concurrent_under_its_bound` was asserting a fixed 1.5-second ceiling, so under a busy suite it failed **about the machine rather than about concurrency**; #9416 changed it to compare wall clock against summed delegation time. The other is now **reproducible and is a real bug**: `the_first_program_runs_from_its_definition` **loses a delegation about one run in four** under parallel `cargo test`, and it reproduces at `1f78b260e2` without any of today's later changes. A fan-out that silently drops one of six under load is exactly the defect an unattended burndown would hide, and #9418 already found one of that shape — two concurrent `drive::output` calls reading the same nanosecond, sharing a temp filename, and deleting each other's file, which reads as a command that answered with nothing. **Chase this before the first unattended run.**

## The audit

`docs/audits/2026-09-19-codebase-audit/` reviewed the workspace at
`1843fa6c18` and reproduced failures through public APIs **while the
applicable test suites passed**. Its 25 findings are filed as #9415–#9433.

A01 to A04 are fixed. A09's UTF-8 half is fixed for the direct door as a
side effect of bounding it — the old loop ran `String::from_utf8_lossy` per
byte chunk, so a character split across a chunk boundary became replacement
characters in the answer **and in every trace of it**. The rest of A09–A11
is still #9423. The retained harness now reports the A01 to A03 probes closed, and the
record says plainly that a harness line observes a result while the tests
establish when a bound applies. The rest are
intended as burndown fodder — they are the first real workload for the system
this brief describes.

Read a finding before its issue. The audit's reasoning is better than any
summary of it, including this one.
