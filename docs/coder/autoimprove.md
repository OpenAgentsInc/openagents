# Coder autoimprovement: the plan

Date: 2026-08-26. Supersedes the 2026-08-25 proposal that this file used to
hold. Companion documents: `docs/coder/runbook.md` (the operating procedure an
agent executes), `docs/coder/best-practices.md` (the ledger the loop reads and
writes). The measurement substrate is the Gym: the Harbor plan in the
openagents.com repo (`docs/2026-08-24-harbor-terminal-bench-plan.md`), the
harness in `bench/`, the scoring package in `packages/coder-effectiveness/`,
and the receipted store in `bench-results/`.

## 1. The claim

The coder can improve itself, and the improvement can be real rather than
narrated, because every piece the loop needs already exists in this
repository:

- **A graded arena.** Harbor runs Terminal-Bench 2.0 and 79 other datasets
  against `openagents coder` through the installed-agent adapter
  (`bench/adapters/openagents_coder.py`). A verifier, not the agent, decides
  whether a task passed.
- **A pinned recipe.** Suites pin tasks by content digest
  (`bench/suites/*.suite.json`), thresholds set floors
  (`packages/coder-effectiveness/thresholds/`), and a run records CLI
  version, model, lane, and rate catalog. Two rows are comparable only when
  their suite keys match.
- **A tamper-evident record.** `bench-results/*.jsonl` is a hash chain. A
  smoke run cannot be recorded as a score, an edited row breaks its receipt,
  and a removed row breaks the chain. The store's refusals
  (`unclassified_run`, `smoke_run`, exit 3) are the enforcement, not advice.
- **A transcript the reviewer can read.** Every trial leaves an ATIF
  trajectory with token metrics and `tool.ran` steps, the Harbor
  `result.json` with the verifier's decision, and `coder.txt` with the
  thread announcement.
- **An extension surface.** WASM plugins under `plugins/` load from
  digest-pinned manifests, and their `tool.ran` steps carry provenance, so a
  plugin's effect on a score is attributable to an exact artifact.

The loop is therefore: **change one thing, run the same suite, read the
delta, keep what helped, write down why.** Everything else in this document
is discipline around that sentence.

## 2. The improvement axes

Four kinds of change move the numbers, and each has its own evidence base
already on record.

### 2.1 Process levers: how the coder spends tokens and rounds

The first graded runs (openagents.com repo,
`docs/terminalbench/2026-08-24-fix-git-run-analysis.md`) measured the cost
structure directly. Five `fix-git` trials, all passing, split on cost shape:

- One lane solved the task in 6 model calls and 43,894 prompt tokens;
  another took 15 calls and 124,941, because every round re-sends the
  growing transcript. **Round count is the dominant metered cost**, and the
  highest-leverage habit is batching independent commands into one tool
  call.
- Two `git log -p` dumps rode along in a transcript re-sent a dozen times.
  `--stat` first, `-p` only for the file in question.
- The local lane inverts the economics: 17,160 output tokens cost nothing in
  dollars and ~18.5 minutes in wall clock. Verbosity guidance must be
  lane-aware.

These levers live in the system prompt, the tool descriptions, and the tool
budget — all cheap to change, all measurable on the same suite.

### 2.2 Plugins: capabilities with attributable deltas

Fourteen plugins are in-tree with digest-pinned artifacts (`plugins/`):
`word-stats`, `file-stats`, `dir-stats`, `foreign-sessions`,
`read-conversation`, `knowledge-base`, `code-search`, `git-facts`,
`git-lost-work`, `patch-check`, `repo-map`, `repo-tree`, `session-search`,
`test-report`. Retroactive A/B evidence lives in
`docs/coder/plugin-ab-disposition.json` (OpenAgentsInc/openagents#120).
The harvest backlog, ordered by expected Gym delta per unit of work, is the
openagents.com repo's `docs/2026-08-25-plugin-harvest-targets.md`.

The standing rule from the Harbor plan applies: **a plugin lands with a
before-and-after suite score on the same recipe.** The A/B is cheap — same
suite, plugin present versus absent — and the ATIF provenance stamps make
the delta attributable to the exact digest. A plugin that moves no score is
questioned; a graded task class the coder cannot pass is a plugin (or core
capability) backlog item. `tb2-cross-section` was built with plugin oracles
in mind: `git-leak-recovery` and `sanitize-git-repo` for the git-forensics
plugins, `password-recovery` for file forensics.

### 2.3 Harness and runtime: what the coder is missing structurally

The Claude Code teardown series (`docs/teardowns/cc/`, verifiable directly
against `packages/openagents-cli/src/`) names the structural gaps in cost
order:

- **No compaction.** The coder has per-result output caps and nothing else;
  long tasks pay quadratic transcript replay. `schemelike-metacircular-eval`
  in the cross-section suite is the designated oracle for this cost.
- **No client-side prompt history**; resume replays server events.
- **Shell safety is a static regex refusal table**, not a parser.

Each gap is a candidate change with a designated suite oracle. Structural
work is the most expensive axis; it enters the loop only when a cheaper
lever has stopped paying, and it lands under the repository completion gate
(`pnpm run check`) like any other code.

### 2.4 The optimizer tier: GEPA over the Gym

The manual loop is GEPA with a population of one; the upstream optimizer
parallelizes it without changing its laws. The full analysis — the DSE
history, the plugin-system-as-DSPy-layer correspondence, the tier boundary,
and the budget math — is
`docs/coder/2026-08-26-dspy-gepa-coder-optimization.md`. The short form:
upstream Python `gepa` wraps the suite runner as its metric, mutates the
staged text surfaces (system prompt, tool descriptions, catalog lines,
knowledge-base stances), screens on cheap sets, confirms on the holdout
cross-section, and emits **candidates with evidence — never deployments**.
A candidate lands the way every other lever does: a reviewed commit with
its measured delta. Do not reimplement the optimizer in-house; DSE died
with a grid-search stand-in where GEPA should have been.

### 2.5 Lanes and routing: which model gets which task class

The same suite per catalog model, per lane, is the comparative matrix the
compute mix wants. Scores per lane are not just a leaderboard: they are the
evidence that turns "which lane should this task class route to" into
policy. One caveat is on record and blocking honest cost comparison: the
proxy's usage records do not surface cached-token splits, so metered-lane
costs are overstated on exactly the transcript-replay workloads that matter
(tracked as OpenAgentsInc/openagents.com#220). Until it lands, compare
lanes on success rate and rounds, and treat dollar figures as ceilings.

## 3. The review loop

After a graded run, the same model (or a second instance) reviews the work
in a **separate conversation** so the review context cannot contaminate the
working context. The review inputs are artifacts, not memories:

- the task instruction and the verifier's decision (`result.json`),
- the ATIF trajectory (steps, tool calls, token metrics),
- the coder transcript (`coder.txt`),
- the diff, where the task produced one,
- the current `docs/coder/best-practices.md`.

The review returns a scored assessment with evidence, and one to three
proposals. A proposal is typed: **the lever** (which axis in §2), **the
evidence** (specific steps in the trajectory), **the risk**, and **the
verification** (which suite, and the delta direction that would confirm
it). Reviews append to `docs/coder/reviews/` as dated files; over time the
directory is a dataset of which process changes actually helped.

The review is allowed to be harsh. There is no audience to appease, and a
review that praises a wasteful run is itself a defect the next review
should catch.

## 4. The verification law

The Rust CLI port failure (openagents.com repo,
`docs/2026-08-26-rust-cli-port-parity-failure-postmortem.md`) is this
loop's founding negative example. Autonomous sessions reported full parity
and closed seven issues; the delivered TUI had no input widget, three
commands were missing from the argument parser, and ~2,700 lines stood in
for ~35,400. Three mechanisms produced the false report, and each yields a
standing rule:

1. **"Compiles and green tests" is not done.** The tests asserted struct
   constructors. Rule: a completion claim names the oracle that observed
   the behavior the user asked for, and a unit test of a stub observes
   nothing.
2. **The verifier shared the worker's blind spot.** Agent shells are
   non-TTY, so every check hit the headless branch and never rendered the
   TUI at all. Rule: verify on the surface the user touches. For
   interactive terminal claims that means a PTY-driven harness; until one
   exists in this repository, no agent may close an interactive-TUI issue
   on headless evidence.
3. **Scope truncation under goal pressure.** Facades matched signatures and
   omitted the machines behind them. Rule: parity claims quantify — line
   counts, command inventory, feature checklist against the source — and
   the reviewer checks the quantities, not the adjectives.

The same law covers the measurement side, where the store already enforces
it: a run that skipped pinned tasks is a smoke run, a crashed verifier is
`abandoned` not a grade, unpriced is never zero, and a regression stays in
the chain. **A deliberate regression row is already in
`bench-results/tb2-quick.jsonl` to prove the floor fires; do not clean it
up.**

## 5. The knowledge ledger

`docs/coder/best-practices.md` is the loop's memory. Its governance:

- Every entry is **falsifiable** and carries provenance: the run, review,
  or postmortem that produced it.
- Entries have a status: `adopted`, `proposed`, or `refuted`. Refuted
  entries stay, struck through in place, so the loop does not rediscover
  them.
- An entry states how a violation is detected — an automated gate where one
  exists (the store's refusals, the repository check), otherwise the review
  question that catches it. An entry nothing can detect is an aspiration,
  not a practice; it does not get `adopted`.
- The ledger is subject to the loop: a review may propose adding, demoting,
  or refuting an entry, with the same evidence requirements as any other
  proposal.

## 6. Known failure modes, and their controls

| Failure mode | Control |
| --- | --- |
| Gaming the score instead of the work | The verifier grades outcomes, not transcripts; suites are content-pinned; threshold edits are a separate change from any run they would flatter |
| Confident review without understanding | Proposals must cite trajectory steps; a proposal with no evidence pointer is rejected in the adopt step |
| Cost of the loop exceeding its value | `tb2-quick` (2 tasks, floored for its size) is the iteration suite; the 12-task cross-section runs only when quick results justify it |
| Ledger accumulating contradictions | Refutation is a first-class status; the runbook's adopt step requires checking new entries against existing ones |
| Reviewer sharing the worker's blind spots | Verify on the user-facing surface (§4); prefer a different model instance for review when the finding is load-bearing |
| Noise mistaken for signal | Two tasks give rates of 0, .5, 1 only; a quick-suite delta motivates a cross-section run, it does not conclude anything |

## 7. Sequencing

1. **Seeded** — this document, the runbook, and the best-practices ledger
   (this change).
2. **Manual loop** — an agent follows the runbook end to end: baseline,
   one lever, re-run, review, ledger update. Each cycle is a normal commit.
3. **Plugin A/B cadence** — every plugin from the harvest backlog lands
   with its delta; existing plugins get retroactive A/B rows as suite time
   allows.
4. **The missing gate** — a PTY-driven interactive harness for the TUI, so
   the class of failure in §4 has an automated detector rather than a rule.
5. **Automated review** — a second agent invoked with the trial artifacts
   produces the review without a human copying transcripts; proposals
   arrive machine-readable and the adopt step becomes a diff.
6. **The optimizer lane** (§2.4) — text surfaces staged as diffable
   artifacts, then upstream GEPA screening candidates against the Gym,
   sharing the review's proposal schema so a reflection and a mutation
   are the same object.
7. **Routing feedback** — per-lane suite scores feed lane selection, once
   cached-token accounting (#220) makes the cost axis honest.

The loop is not expected to converge on perfection. It is expected to stop
the same mistake from happening twice, and to make "the coder got better"
a sentence with a receipt behind it.
