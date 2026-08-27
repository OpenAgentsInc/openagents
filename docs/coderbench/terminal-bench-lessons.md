# What to learn from Terminal-Bench (and Harbor) for CoderBench

Date: 2026-08-27. Sources read for this document: the local Harbor clone at
`~/work/projects/repos/harbor` (87 benchmark adapters, 48 installed agents, 35
environment providers), Harbor's ATIF RFC 0001 and v1.7 changelog,
`bench/README.md`, `bench-results/README.md`,
`docs/coder/autoimprove.md`, and the first recorded tb2 results.

Terminal-Bench is the external yardstick this repository already runs through
`bench/`. CoderBench should steal its discipline, not its task distribution.
CoderBench is a **general** coding-agent benchmark — general in the work it
measures and in where its tasks may come from — while tb2 is general by
authorship: anyone can write a task for it. Those are different generalities,
and the difference runs through every line below. What follows is split into
what to copy wholesale, what to adapt, and what to reject — with the reason
in each case, so a future reader can re-litigate any line.

## 1. Copy wholesale: the machinery that already proved itself

### 1.1 Verifier decides, never the agent

The single most important Terminal-Bench property: the agent's completion
claim is not the grade; an independent verifier in the environment decides.
This repository already paid for learning it twice — the rust-port parity
postmortem (seven issues closed on stub-passing tests) produced best practice
V1 ("a completion claim names its oracle"). CoderBench tasks carry their real
gate command (`pnpm run check`, `cargo test -p ... --test coder_interactive_pty`)
as the oracle of record, exactly as tb2 tasks carry `run-tests.sh`.

Corollary to keep: **oracles are deterministic.** Same commit + same binary →
same verdict, three consecutive runs running, before the suite promotes to
score tier. A flaky verifier makes every downstream trend line noise.

### 1.2 Content-digest pinning over name pinning

Harbor's registry pins each task by dataset, git url, commit, path — not by
task name that can silently drift. This repository's suites already do this
(`openagents.effectiveness_suite.v1` pins). CoderBench extends it one step:
each task also pins its **provenance traces** by digest, so "task #7 came from
these four sessions" is checkable, not folklore. Provenance digests make the
corpus ↔ suite relationship auditable from either side.

### 1.3 The installed-agent protocol as the only interface

Harbor's installed-agents (`src/harbor/agents/installed/`, 48 of them) all
meet one contract: given a container and a task, produce trajectories and let
the verifier speak. The OpenAgents adapter (`bench/adapters/openagents_coder.py`)
installs the real native binary and runs it headless inside the trial
environment. CoderBench adds zero new agent plumbing — same adapter, same
lane flags (`--model`, `--agent-import-path`), which is precisely why its rows
will be comparable with tb2 rows.

Lesson worth keeping explicit: benchmarks that let each contestant bring its
own harness measure harnesses, not agents. The harness is fixed here; Coder
versions vary through the binary, recorded per row.

### 1.4 Parallelism economics decide where you can iterate

Harbor's provider list (Daytona, Modal, e2b, EC2, GKE…) exists because
thousands of parallel environments are the difference between a suite you run
nightly and one you run quarterly. tb2 economics on one machine forced the
two-task quick lane; the cross-section lane needed a cost ceiling removed
(#125) because proxy money could not fund full measurement. CoderBench should
plan capacity from day one: smoke tier locally, score tier on cloud providers,
and treat wall-clock-per-trial as a first-class budget line like token spend,
not an afterthought.

### 1.5 Trajectory capture as a peer output of grading

Every Harbor trial leaves an ATIF trajectory, a `result.json`, and logs;
ATIF v1.7 formalized trajectory_id / subagent embedding / continuation refs
precisely so runs compose into datasets. This is the format the corpus speaks
already. Keep the invariant: **no graded run without its trajectory** — the
store refuses rows without receipts, and CoderBench refuses tasks whose
generating session cannot show its own trace.

## 2. Adapt with changes

### 2.1 Task provenance: synthetic-and-curious vs. owned-and-evidenced

Terminal-Bench tasks are authored puzzles: someone writes a broken environment
on purpose. Fine for breadth, weak for realism — nothing in them was ever
actually wrong in production. CoderBench inherits the owned-closed-issues
pattern instead: start states are real historical commits, instructions are
rebuilt from the original issue/session, outcomes were shipped under review —
and this holds for every domain, not just this repository. Adaptation
required: historical bias. Every derived task was solvable once by
construction, which risks teaching "reproduce history" rather than "solve the
problem." Mitigations:

- Rebuild instructions from intent (issue text + user turns), not from the
  diff; multiple valid solutions must pass the oracle.
- Prefer outcome checks over diff-matching oracles wherever possible (tb2
  does this too: tests, not patches).
- Track "novel-solution rate": when an agent passes via a different route
  than history did, that is the benchmark working, not misgrading.

### 2.2 Difficulty scaling: curriculum, not gauntlet

Terminal-Bench hard-mode variants (many hold-out difficulty splits across its
ecosystem) assume a broad model population being ranked. CoderBench's primary
consumer is this project's improvement loop. So: keep tiers small and honest
(smoke = environments build; score = oracle deterministic), set floors from
the first baseline rather than aspirational targets, and grow the pool only as
labeling throughput allows. A 12-task suite that gets run weekly beats a
200-task suite that gets run twice. Add tasks when the loop demonstrably needs
finer resolution between configurations, not to look substantial — and add
domains when the corpus earns them, not to look general.

### 2.3 Registry vs. owned tracker

Harbor resolves suites against a giant vendored `registry.json` (13 MB, every
public dataset). Useful for cross-benchmark sweeps; wrong ownership model for
tasks distilled from private sessions. CoderBench registers in this repo's
tracker instead: tasks live in `bench/tasks/coderbench/`, suites reference
them by digest, and the forge issues carry the derivation receipts. If a
CoderBench task later proves generally interesting, packaging it upstream into
a registry contribution is a separate decision with separate review — keep
the pipes unidirectional until then.

### 2.4 Scoring beyond pass/fail once pass/fail works

Harbor metrics extend past binary scores (cost, tokens, partial credits in
some adapters). The autoimprove loop learned round-count dominates metered
cost (#119's evidence base). CoderBench rows should record alongside accepted%
the already-computed effectiveness fields — token totals, duration, rounds —
so optimization levers can be judged on quality-per-cost and not just pass
rate. But: no composite "score" number until binary acceptance is boringly
stable; composites hide regressions, and the deliberate-regression row in
`tb2-quick.jsonl` shows a single clean number failing loudly is worth more
than three blended ones hedging.

## 3. Reject, deliberately

### 3.1 Public-dataset breadth as a goal

87 adapters is Terminal-Bench's moat and explicitly not CoderBench's path.
CoderBench is general in provenance — any real coding domain qualifies — but
narrow in method: every task must trace back to a real session with a real
outcome. Breadth arrives the only honest way it can, by the corpus growing
across the repositories this machine actually touches, never by bolting on
third-party synthetic datasets through Harbor "since it's free." A wide
benchmark of borrowed tasks would be strictly worse than tb2 at being tb2; a
deep benchmark of earned tasks is something tb2 cannot be at all.

### 3.2 Contests and leaderboards

Terminal-Bench exists partly to rank models publicly. CoderBench ranks
configurations of one project against its own history. Publishing aggregate
numbers is fine (traces product, receipted store); inviting comparison traffic
imports incentives (teaching to the test, task leakage into training sets of
competitors) that a self-improvement tool should not carry. If external
interest ever materializes, spin a held-out variant then; don't pre-build it.

### 3.3 Environment-per-task maximalism

Terminal-Bench builds bespoke containers per task. For synthesized puzzles
that's the only option; for CoderBench many tasks share one environment shape:
"this repo at some commit + pnpm + cargo + gates runnable." Amortize: one
parameterized image family, tasks differing only in pinned commit and oracle
command. Cheaper, faster to verify, and closer to the real working conditions
(the sessions we're modeling used exactly this machine-shaped setup).

### 3.4 Ignoring harness-waste signals

Trajectories exist in Terminal-Bench largely as artifacts for humans. The
waste-cluster issues (#152–#158) showed they're also a mirror: re-executed
suites, unread outputs, duplicated pipelines. CoderBench's corpus review step
should mine its own uploaded traces for these patterns per cycle — a lever
proposed from harness waste gets validated against the same lanes as any
other lever, but the mining itself stays cheap because the corpus now exists
in queryable form. Terminal-Bench leaves this value on the table; we shouldn't.

## 4. Standing debt Terminal-Bench taught us to watch

- **Own the redaction boundary.** tb2 tasks are designed public; our sessions
  accidentally private. Everything funnels through the ATIF redactor +
  tripwire before upload, always dry-run reviewed first, and no batch flag
  ever bypasses it.
- **Keep suite keys sacred.** Rows comparable only within identical suite
  digests. CoderBench ≠ tb2 ≠ cross-section, forever, even when tasks resemble
  each other.
- **Refuse fabricated rows more aggressively than missing ones.** The store's
  philosophy — empty file beats fake row — transfers directly to the corpus:
  an honest 60-trace corpus beats a padded 600.
