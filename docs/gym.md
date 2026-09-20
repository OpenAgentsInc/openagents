# The Gym

A map of the Gym: the benchmark harness and self-improvement loop for the
Coder agent — what it is, where each generation of it lives, what it has
measured, and how to pull it into this repository. Surveyed 2026-09-19
across `~/work/openagents` (this repository and its history) and
`~/work/coder` (`OpenAgentsInc/coder`).

## What the Gym is

The Gym is the measurement and control plane for coding-agent runs. It
does two jobs:

- **Benchmark.** Run the agent against a fixed suite of containerized
  tasks — Terminal-Bench 2 at a pinned commit, verifier files and all —
  and record every attempt as a trajectory with tool calls, token counts,
  wall time, and an independent verifier's verdict.
- **Improve.** Hand the traces to the agent's own editing loop, let it
  propose one source change, and measure the candidate against the frozen
  control on fresh trials. A candidate is retained only when a fixed
  acceptance rule says it won, twice.

The name is from episode 243 of the transcript archive
(`docs/transcripts/243.md`): "the gym" was the benchmark harness to build
next — head-to-head comparisons of models and agents on the same prompts,
reporting tokens, money, wall clock, and cost per accepted outcome, with
the gym training and exercising the same agent it measures. The
transcription heard "gem" for half the episode; the record is corrected.
Episode 245 adds the second half of the idea: an optimizer (GEPA/DSPy)
tunes the program's parameters while deterministic code owns the control
flow.

## The three shapes

The Gym has existed three times. Only the second is alive.

| Shape | Where | State |
| --- | --- | --- |
| `openagents gym` CLI plus the Phoenix record store | This repository's previous shape, and the `openagents.com` app | Deleted here by `dabc08102f` on 2026-09-18; readable in history |
| The Terminal Gym | `~/work/coder`: `crates/coder-bench`, `bins/coder-terminal`, `bins/coder-serve`, `ops/` | Active; the rest of this document describes it |
| This repository | `crates/gym`, `crates/atif` | Underway. The pull-in plan is below |

## The first Gym, in this repository's history

The old `openagents` monorepo built the Gym across the last week of August
2026, and the whole implementation is still in this repository's object
store. The head before the reshape is `dabc08102f~1`.

- `crates/openagents-cli/src/gym/` — the `openagents gym` command family:
  `suite list/show/check`, `run`, `results score/show/compare/trend`,
  `corpus inventory/qualify/import/status/verify`,
  `dataset list/create/add/remove/show/pin/diff/distill`, and
  `env probe/doctor/list/use/box create/box release/pull`.
- `bench/` — the Harbor adapter (`adapters/openagents_coder.py`),
  `run-suite.sh`, the suite manifests (`tb2-cross-section`, `tb2-quick`,
  `coder-effectiveness-v1`, `swebench-verified-subset`, the `plugin-ab-*`
  trios, `smoke`), the rate catalog, and `bench/optimize/`, the GEPA lane
  that evaluates staged text surfaces against a development suite.
- The frozen `openagents.gym.*.v1` view schemas — `run_status`,
  `results_trend`, `corpus_inventory`, `corpus_import_record`,
  `dataset_view`, `env_report`, `suite_manifest_view` — and the
  `openagents.bench_result.v3` receipt-chained result row.
- `docs/coderbench/` — the design docs: `gym-cli-spec.md`, `roadmap.md`,
  `plan.md`, `process.md`, `delegation.md`, `auth.md`, and
  `terminal-bench-lessons.md`.

The server side lived in the `openagents.com` repository: a Phoenix
application that stored one row per run and per trial, exposed
`/api/v1/gym/runs` routes, published lifecycle events over PubSub, and
drew operator-only LiveViews at `/gym`. None of that is in this
repository.

The design rules the first Gym established still hold: content-pinned
suites make a run reproducible, a positive verifier reward is the only
accepted outcome, `ungraded` trials stay in the denominator, unknown cost
is `null` rather than zero, and the results store is append-only with a
receipt chain so a worse result stays visible.

## The Gym in the coder repository

On 2026-09-01 the monorepo's Gym moved to `~/work/coder` by subtree merge
(`6abb7897ad` imported `bench/` with history; `e05a631cb7` made it a
library). It grew there into the Terminal Gym — the same measurement core
plus an in-terminal runner, a self-improvement loop, and a public
dashboard.

### The measurement core: `crates/coder-bench`

About 11,800 lines of Rust plus the suite manifests. It is the direct
descendant of `crates/openagents-cli/src/gym` — this repository's own
history, carried through the subtree merge — and it has no service
dependency: it reads and writes local files.

| Module | What it owns |
| --- | --- |
| `suite.rs`, `bench/suites/` | Manifests pinned by content digest: task identity, dataset, commit, and path; drift checks; eleven committed suites. |
| `results.rs`, `results/` | The append-only `bench-results/<suite>.jsonl` store, the receipt chain, the scorer, `show`/`compare`/`trend`. |
| `gate.rs`, `rates.json` | Per-suite pass floors and the token rate catalog; unknown cost stays unknown. |
| `schemas.rs`, `views.rs` | The frozen `openagents.gym.*.v1` documents and their line renderers. |
| `convert.rs`, `trace.rs`, `corpus.rs`, `dataset.rs`, `distill.rs` | Claude and Codex session conversion to ATIF, trace discovery and redaction, the corpus ledger, and versioned datasets. |
| `tasks.rs`, `paths.rs` | CoderBench task manifests and the store layout under `~/.openagents` and the workspace. |

`bins/coder-cli` mounts it as `coder-cli bench`. ATIF itself — the
trajectory format every trace is written in — is
`coder_contract::atif`, about 1,400 lines.

### The runner: `bins/coder-terminal/src/gym_*.rs`

About 7,700 lines of implementation and 3,300 of tests, behind an opt-in
feature flag. The command grammar in `gym_controls.rs`:

| Command | What it does |
| --- | --- |
| `coder-terminal gym run <task>` | Runs one attempt: fresh container, the model loop, the independent verifier, the retained trace. `--suite`, `--lane`, `--plugin`, `--no-plugins`, `--backend`, `--report`. |
| `gym tasks` | Lists the tasks by suite with an unavailable task's reason. |
| `gym prepare <task>` | Saves a task's image as a named Box snapshot so attempts fork instead of pulling. |
| `gym infrastructure` | Prints the compute endpoint and account limits as JSON. |
| `gym learn` | Gives an editing turn to the agent over a candidate checkout; bounded by minutes and tool calls. |
| `gym upload` | Uploads a retained trace to the service. |

The pieces that matter:

- `gym_agent.rs` — the task agent: a model loop over the service socket
  with one shell tool confined to the task's container. The instructions,
  tool declarations, and their digest are part of the measured surface;
  the self-improvement loop is allowed to edit exactly this file plus its
  test file.
- `gym_process.rs`, `gym_exec.rs` — the execution environment. Two
  backends: `docker` runs the upstream image locally; `box` (the default)
  creates a cloud machine through the service's `/api/box/v1`, forked
  from a prepared snapshot when the account holds one.
- `gym_trace.rs` — the ATIF document: verbatim task instruction, task
  file digests, image ID, lane, plugin configuration, phase timings
  (`setup`, `agent`, `verifier`), every tool call joined to its result,
  and `final_metrics` including the `waste` table. Uploads to
  `POST /v1/traces`; the receipt is part of the evidence.
- `gym_tasks.rs` — the pinned task table: the twelve `tb2-cross-section`
  tasks from `laude-institute/terminal-bench-2` at commit `69671fba`,
  images `alexgshaw/<task>:20251031`, each with its verifier files,
  upstream limits, and memory.
- `gym_learn.rs`, `gym_learning_progress.rs` — the editing turn: the
  agent reads compacted development evidence, writes
  `gym-hypothesis.json`, makes one bounded change, and appends a
  regression test.

### The controllers: `ops/gym-*.py`

About 5,900 lines of Python standard library — the loop machinery, not
the measured surface:

| Script | Role |
| --- | --- |
| `gym-improve.py` | The self-improvement loop: freeze control, development attempts, native edit, regression checks, interleaved screening, confirmation, promote or revert. |
| `gym-ab.py` | One A/B comparison at fixed width; emits `coder.gym-ab-evidence.v1`. |
| `gym-matrix.py` | Queue-driven runner: `coder.gym-matrix-queue.v1` files expand to cells, a pool of boxes stays saturated, infrastructure failures requeue once and never count as task failures. |
| `gym-review.py` | Reads experiment directories and evidence files into one reviewable record; refreshes the lab notebook's marker block. |
| `gym-baseline.py`, `gym-evidence.py`, `gym-progress-*.py`, `gym-search-*.py`, `gym-plugin-compare.py` | Baselines, evidence assembly, progress audits, and the repository-search experiments. |

`docs/gym-self-improvement.md` is the operator runbook and
`docs/evaluation/2026-09-05-gym-self-improvement-lab-notes.md` the living
lab notebook. `docs/optimization-loop.md` holds the results table every
round appends to.

### The service side

- `bins/coder-serve/src/gym.rs`, `crates/coder-contract/src/gym.rs`,
  `crates/coder-ui-core/gym.rs` — the public `/gym` dashboard: a
  read-only projection of the `bench_rows` and `traces` tables under the
  `gym-snapshot.v1` contract (`docs/contracts/gym-snapshot.v1.md`, fixture
  in `bins/coder-serve/fixtures/contracts/`). Scoreboard, optimization
  ladder, task catalog, corpus provenance, plugin manifest; unknown
  renders as `—`, and the receipt chain is verified on load. Live and
  public since 2026-09-01.
- `POST /v1/traces` — trace intake. `record_bench_row` over MCP and
  `coder-cli bench push` publish result rows.
- `bins/coder-serve/src/analytics/gym.rs` — the analytics read.

### The rules the loop enforces

These are the parts worth keeping regardless of implementation:

- **Fixed inputs.** Same lane, origin, task-file digests, image ID,
  container platform, and plugin configuration on both sides; a fresh
  container per attempt; the verbatim upstream instruction in the trace.
- **Development and held-out sets.** `tb2-quick` (`regex-log`,
  `openssl-selfsigned-cert`) is the development set; the other ten
  cross-section tasks are held out and decide promotion. The reason is
  recorded in `docs/gym-tasks.md`: every verdict on two tasks alone has
  flipped, because a candidate tuned on two tasks is tuned to those two.
- **The acceptance rule.** The candidate passes every trial; a per-task
  regression above 10% in mean calls, prompt tokens, or wall time
  rejects; a win needs at least a 10% overall improvement on one of
  those, in that order; a screening win must repeat on fresh trials of
  the frozen binaries before promotion.
- **Honest evidence.** Infrastructure failures are `infra_invalid` and
  never enter a comparison. Missing grades, changed identities, and
  inconsistent token totals invalidate a round. Unknown numbers are
  `null`, never zero. Every benchmark trace stays
  `training_eligible: false` — the loop changes source code, not model
  weights.
- **Provenance.** Executable hashes bound to verified clean source,
  patches and hypotheses preserved whether or not they pass, and a
  receipt-chained ledger that cannot be quietly rewritten.

### What it has measured

The honest summary of the recorded evidence: the harness works, and so
far it has mostly said no. Two complete source-edit A/B batches rejected
their candidates on per-task call regressions; the tool-archive
challenger (coder issue #254) passed twelve of twelve screening attempts
but reverted on a 22.2% call increase the token reduction could not
override. The held-out baseline is a 7-of-12 cross-section result. The
default plugin suite (`docs/plugin-suite.md`) remains empty because no
plugin has passed the strict test. The lab notebook and
`bench/evidence/` hold the per-round records.

## Pulling the Gym into this repository

The goal: the `coder` binary in `crates/coder` gets a `gym` surface that
runs the same honest loop — pinned Terminal-Bench tasks, an ATIF trace
per attempt, an independent verifier, and a screening rule — against the
agent this repository is actually building. That is the piece the rebuild
plan is missing twice over: it answers its own open question ("the
labeled set that tunes the thresholds") and it gives the Jev-routed shell
loop a measured fitness function.

The dependency split decides the order. The measurement core is
service-free and ports as-is; the runner is coupled to the coder
service's socket, bearer, Box API, and trace intake, none of which exist
here — the backend is a relay and the model door is `Generate`. So the
core comes over first and the runner is rebuilt on this repository's
agent rather than ported verbatim.

Ordered, each independently shippable:

1. **`crates/gym` — the measurement core.** Port `crates/coder-bench`:
   suite manifests and digests, the receipt-chained JSONL store, the
   scorer and gates, the rate catalog, the frozen `openagents.gym.*.v1`
   schemas, and the corpus/dataset modules. This code began in this
   repository as `crates/openagents-cli/src/gym` and moved out by subtree
   merge; pulling it back restores the lineage, and the subtree history
   in `~/work/coder` means `git log --full-history` still works. Reuse
   the committed suite manifests, starting with `tb2-quick` and
   `tb2-cross-section`. The ATIF document type (`coder_contract::atif`)
   comes with it or is reimplemented — it is a schema, not service code.

   **Landed, in part.** `crates/atif` is the reimplemented format, and
   Coder Terminal already writes a trace per conversation to local disk
   (`docs/coder/traces.md`, issue #9400). That gives the port its first
   standing consumer — our own agent's episodes, with every `classify`
   and `shell_judge` call recorded as a first-class decision call — and
   a crate `crates/gym` can read traces from. Harvesting those traces
   into a suite is deliberately separate: #9379 dropped 151 states whose
   commands reached into private sibling checkouts, and that filtering
   question needs its own answer.
2. **The task table and the Docker backend.** Port `gym_tasks.rs`'s
   pinned data (the twelve tasks, verifier files, limits, image names)
   and a container environment in the shape of `gym_process.rs`: fresh
   `linux/amd64` container per attempt, the agent's shell confined to it,
   upstream time and memory limits, the verifier running after the agent
   ends. Local Docker first. The Box backend stays behind — it is the
   service's `/api/box/v1` and an account model this repository does not
   have; if remote execution is wanted later, the relay-native shape is a
   NIP-90-style job a worker claims, the same pattern the fulfillment
   worker already uses.
3. **`coder gym run` — the attempt.** Drive `crates/coder`'s own loop
   inside the container: the `Generate` door produces plans, the shell
   executes them bounded, Jev judges each round, the verifier grades.
   The attempt writes the ATIF trace locally — instruction verbatim,
   task digests, image ID, door and model identifiers, phase timings,
   tool calls joined to results, `final_metrics` with the waste table.
   `gym tasks` and a `--report` writer round out the first surface.
4. **`coder gym ab` — the comparison.** Port the acceptance rule and the
   interleaved-trial scheduler from `ops/gym-ab.py`: fixed trials per
   task per side, alternating first mover, `infra_invalid` requeue, the
   `coder.gym-ab-evidence.v1` evidence file. The controllers in `ops/`
   are Python; this workspace is Rust-only, so the loop lands as Rust —
   a `gym` subcommand or a small bin target — not as scripts.
5. **`coder gym improve` — the loop.** Once A/B exists, the
   self-improvement cycle follows the runbook's shape: freeze the
   control, development attempts, a bounded native editing turn
   (`crates/coder` editing a candidate worktree instead of
   `coder-runner acp`), the regression test that fails on the old source,
   screening, confirmation, promote or revert — every artifact retained
   under `~/.openagents/gym/`.
6. **Publication — later, and a decision.** The `/gym` dashboard and
   `POST /v1/traces` belong to the coder service and stay there. This
   repository's honest-options list is: keep traces local, publish the
   `gym-snapshot.v1` document somewhere static, or — the direction that
   fits the relay backend — define the Gym's records as Nostr event
   kinds with relay-side policy, the way the rest of this stack already
   works. Decide when the runner exists; do not block the port on it.

What does not come over: the Box backend and snapshot catalog, the bearer
and account plumbing, `gym learn`'s ACP transport, the `/gym` page, and
the earn/cloud couplings. The GEPA lane in `bench/optimize/` is recorded
history; revisit it only if a text-surface optimizer is wanted on top of
a working loop.

## Cautions

- The transcripts are history, not spec. Episode 243's "the gym trains
  Khala and uses Khala" is direction; the shipped Gym changes source, not
  weights, and every trace stays `training_eligible: false`.
- `~/work/coder` is reference material under this repository's contract:
  carry the design and the lineage, reimplement here, and say so in the
  commit message. The measurement core is the exception in kind — it is
  this repository's own code returning — but the same commit-message
  rule applies.
- The development set is two tasks. Nothing screened on `tb2-quick`
  supports a claim about unseen tasks; the held-out ten exist for that
  reason and stay held out.
- The Gym measures the agent against fixed upstream tasks. A kept
  candidate is evidence about this agent's source, not a claim about a
  model, a lane, or a marketplace.
