# The Gym section of the OpenAgents CLI

Date: 2026-08-27. Status: **spec** — nothing here is implemented yet; this
document is the contract the implementation is built against and reviewed
against. Companions: [`plan.md`](plan.md) (the benchmark it serves),
[`process.md`](process.md) (the procedure it automates).

## 1. What the Gym is and why the CLI needs it

The Gym is OpenAgents' graded-work arena: Harbor (and later other harnesses)
run benchmark tasks against agents, a verifier decides, and receipted results
land in the public projections. Today the loop lives in three places that do
not know about each other:

- `bench/run-suite.sh` and `bench/post_gym_run.py` in this repository — the
  registered-run lifecycle (`POST /api/v1/gym/runs/start` → live trial
  updates → `PATCH` to `graded`/`abandoned`).
- The server's Gym surface (`apps/openagents.com/workers/api/src/inference/gym/`)
  — run progress, ladder, leaderboard, Harbor dispatch receipts, full-trace
  archives, Terminal-Bench comparison reports.
- Whatever a person remembers to do with `trace upload` afterwards.

A Gym section in the CLI makes the whole cycle first-class: **discover a
suite, run it, watch it, score it, record it, compare it — and manage the
trace corpus and datasets that feed task generation — without leaving the
terminal or knowing which internal route each step hits.** It is also the
substrate CoderBench runs on, side by side with Terminal-Bench, so numbers
from the established benchmark and our own land in one place under one set of
receipts.

Design law, inherited from the CLI's existing conventions:

- The CLI names an operation; it does not encode a URL per flag. Where a
  command exists, use it; where none does yet, the Gym commands wrap the
  server routes through the same typed clients (`trace_client.rs` style), never
  raw URL strings at call sites.
- Plain output is the product; `--json` is the interface. Every command emits
  a versioned schema (`openagents.gym.<thing>.v1`) alongside human text, same
  as `trace list` does.
- A score is a claim. The CLI refuses to dress smoke runs, ungraded runs, and
  dry runs as recorded results — the store's refusals are the enforcement, and
  the CLI's job is to surface them verbatim, not to soften them.
- Refuse honestly. An operation whose precondition is missing (no Harbor, no
  token, no Docker/Rosetta) fails with the reason and the fix, never a
  placeholder success.

## 2. Where it sits in the CLI

A new top-level subcommand, alongside `forge`, `issue`, `box`, `computer`,
`trace`:

```
openagents gym <subcommand>
```

Wiring follows the existing pattern exactly: a `GymArgs` struct with a
`GymAction` enum in `cli.rs`, a `run_gym` dispatcher, and per-area client
modules (`src/gym/`) in the shape of `trace_client.rs` — typed requests,
typed errors through `ApiError`, `diag` logging of every request.

Scope note: `gym` talks to the **Gym API and local harness**. It is not a
replacement for `harbor run` — Harbor stays the execution engine, invoked
through `bench/run-suite.sh`'s lifecycle (start → live update → finalize).
The CLI drives that lifecycle and reads its outputs; it does not reimplement
the runner.

## 3. The command surface

Subcommands group into five areas: **suites, runs, results, corpus, datasets**,
plus `env` for environment plumbing. Commands are listed with their contracts;
flags marked `(t2)` are Terminal-Bench-path, `(cb)` CoderBench-path — most
commands are shared and take `--suite` either way.

### 3.1 `gym suite` — what can be run

```
openagents gym suite list                    # installed suites + their pins
openagents gym suite show <suite-id>         # manifest: tasks, digests, tier
openagents gym suite check <suite-file>      # rebuild manifest, diff against pins
```

- `list` reads `bench/suites/*.suite.json` (local manifests) and reports id,
  tier (`smoke`/`score`), task count, digest. Server-side registered suites
  appear too, when the API exposes them.
- `check` is the CLI face of `pnpm run effectiveness:suites -- --check`:
  exit 0 when the manifest matches its pins, exit 1 with the diff when a pin
  drifted. A suite whose pins drifted cannot be run with `gym run` — drift is
  an error, not a warning, because a run against moved pins records a lie.
- Both Terminal-Bench (`tb2-quick`, `tb2-cross-section`) and CoderBench
  (`coderbench-<domain>-v1`) suites appear here identically. Nothing in the
  surface knows which benchmark a suite came from; the manifest does.

### 3.2 `gym run` — execute

```
openagents gym run <suite-id> --model <harbor-model> [options]
openagents gym run status <run-id>           # live progress of one run
openagents gym run list [--mine]             # recent runs, by state
openagents gym run cancel <run-id>
```

`gym run` is the friendly front door to `bench/run-suite.sh`:

- Resolves the suite, verifies pins (`suite check` semantics first), verifies
  prereqs (Harbor installed; Docker up and amd64 emulation working on Apple
  Silicon — probe it, name the Rosetta fix in the refusal), then shells the
  runner with `OPENAGENTS_TOKEN` from the stored credential, not the
  environment.
- Registers the run (`POST /api/v1/gym/runs/start`) when a token is present,
  streams trial states live (`GET /api/public/gym/run-progress` or polling the
  job dir when running against a dev server), and finalizes through the same
  path `post_gym_run.py --run-id` uses. The finalize rule is enforced client
  side too: **a run whose verifier never graded a single trial is patched
  `abandoned`, never `graded`** — a crashed grader is not a grade.
- `--lane proxy|local` (default inferred from the model string, exactly as
  the shell runner does), `--n-concurrent`, `--jobs-dir`, `--timeout-multiplier`,
  `--dry-run` (prints the exact commands; registers nothing).
- Output: the run id, the per-task states as they land
  (`accepted`/`rejected`/`ungraded`), and the summary line. `--json` emits
  `openagents.gym.run_status.v1`.
- `run status` on a run registered server-side reads the run-progress
  projection; on a local-only run (no token at start time) it reads the job
  directory the way `coder-effectiveness` does.

### 3.3 `gym results` — score, record, compare

```
openagents gym results score <job-dir> --suite <id> [--append]
openagents gym results show <suite-id>
openagents gym results compare <suite-id> [--last N]
openagents gym results trend <suite-id>
```

- `score` wraps `pnpm run effectiveness:report` + the append step. Refusals
  pass through verbatim (`unclassified_run`, `smoke_run`, exit 3): the CLI
  prints the store's reason and does not editorialize. `--append` requires
  the suite to be at `score` tier in its manifest — a smoke-tier suite
  records nowhere, and the refusal says so.
- `show`/`compare`/`trend` read `bench-results/<suite>.jsonl` through
  `@openagentsinc/coder-effectiveness`'s compare/verify tooling. Verify-first:
  the chain is checked before the trend is drawn; a broken chain stops the
  command with the offending row named.
- The comparison that motivates this whole section lives here:
  `gym results compare tb2-cross-section coderbench-agent-building-v1` puts
  the established benchmark and our own side by side — per-model accepted
  rates, cost per accepted outcome, trend arrows — so a lever's tb2 delta and
  CoderBench delta are read together, which is exactly the paired-evidence
  discipline `plan.md` §8 asks for.

### 3.4 `gym corpus` — trace import at scale (CoderBench's feeding line)

The corpus pipeline in `process.md` Stages A–C, as commands. This is the
"trace import" work named in the brief.

```
openagents gym corpus inventory [--path <dir>]... [--out <file>]
openagents gym corpus qualify <inventory-file>     # apply §7 filters, report
openagents gym corpus import <inventory-file>      # convert → redact → tripwire → upload
openagents gym corpus status                       # what's imported, what's pending
openagents gym corpus verify <digest>...           # corpus ↔ cloud consistency
```

- `inventory` walks the three stores (`~/.openagents/exports`,
  `~/.codex/sessions`, `~/.claude/projects`) using the existing
  `trace::discover` machinery, converts what it can through the packaged
  converters, and emits `openagents.gym.corpus_inventory.v1` — one row per
  session with source, digest, bytes, steps, timestamps, repo hint, domain
  tag. Deterministic: two runs diff clean apart from mtimes, and that is a
  tested property, not a hope.
- `qualify` applies the `plan.md` §7 filters and writes the exclusion
  report. Nothing uploads without qualification; no filter is silent.
- `import` is the batch pipeline: convert (for non-native sources) →
  `trace redact` semantics → tripwire → `trace upload` at the chosen
  visibility (default `ledger`; the command refuses `dark` outright — an
  unpublishable trace has no corpus value — and refuses `glass` as a batch
  flag; per-trace only). Batch size ≤100, rate-limit aware (server allows
  ~120/hour/owner; the command paces itself and reports ETAs rather than
  hammering), idempotent by digest — a re-run of a failed batch costs
  nothing. Emits `corpus.jsonl` rows: digest → uuid → source → domain.
- The **redaction + tripwire step is not bypassable by any flag**. This is a
  standing invariant of the corpus (process.md failure mode 1) and belongs in
  the command's contract, not just the docs.
- `verify` re-hashes local rows against the server's stored digests and
  reports drift. The corpus ledger and the cloud store disagreeing is failure
  mode 6; this command is how it gets caught.

Native OpenAgents sessions already in ATIF skip conversion; Codex/Claude JSONL
goes through the converters. Conversion failure = recorded exclusion with
reason (never forced), same as the inventory.

### 3.5 `gym dataset` — membership management

Datasets are named, versioned groups of traces/tasks — the bridge between a
flat corpus and a pinned suite.

```
openagents gym dataset list
openagents gym dataset create <id> [--description ...]
openagents gym dataset add <id> <trace-ref|task-ref>... [--tag ...]
openagents gym dataset remove <id> <trace-ref|task-ref>...
openagents gym dataset show <id>
openagents gym dataset pin <id> [--out bench/suites/<id>.suite.json]
openagents gym dataset diff <id> <suite-file>
```

- A **trace-ref** is a corpus digest or a `/trace/{uuid}`; a **task-ref** is a
  `bench/tasks/coderbench/<task-id>` pin. Membership is append-recorded with
  who/when/why, so a dataset's provenance reads like the results store does.
- `add` validates the reference resolves (digest exists in the corpus ledger;
  task pin builds) before writing it — a dataset is not a wishlist.
- `pin` compiles a dataset into a suite manifest
  (`openagents.effectiveness_suite.v1`) with content digests — the only path
  from "a pile of traces" to "a thing `gym run` accepts." `diff` shows when a
  suite and its source dataset have drifted apart.
- Removing a member from a dataset never rewrites a suite that already ran;
  it changes what the *next* pin produces. Recorded rows are immutable, as
  everywhere else in this repository.

### 3.6 `gym env` — compute plumbing (the Computer connection)

Benchmarks need boxes. The env subcommands wire Gym runs to compute through
the pieces that already exist — `openagents box` VMs, the Computer daemon,
and cloud providers Harbor already knows (Daytona, Modal, e2b, EC2, …):

```
openagents gym env probe                    # local: docker, amd64 emulation, harbor, disk
openagents gym env doctor                   # everything a scored run needs, with fixes
openagents gym env list                     # available execution targets
openagents gym env use <target>             # default target for this machine
openagents gym env box create [--count N]   # spin up Box VMs for a run (wraps box create/fanout)
openagents gym env box release <box-id>...  # stop + release after a run
```

- `probe` composes the existing `computer probe` (toolchains, coding agents,
  worktrees) with Gym-specific checks: Docker present, amd64 images runnable
  (the qemu/Rosetta verifier segfault detection from `bench/README.md` made
  mechanical — run the canary, read the result, don't ask the human to
  remember), Harbor importable, disk headroom.
- `doctor` is probe plus the fix: every failed check prints the exact remedy
  (enable Rosetta in Docker Desktop settings; `pip install harbor`; `openagents
  auth login`). Exit 0 means a scored run can happen on this machine now.
- `env box create` provisions execution targets through the existing Box
  client (`box create`, `box fanout` for N) labeled for the Gym run, so a
  proxy-lane suite can run its trials on cloud boxes instead of the local
  Docker daemon; `env box release` wraps `box stop`. Boxes carry a
  `gym:<run-id>` label so `env list` can show which boxes belong to which
  run and orphan nothing.
- Harbor's own provider flag (`--env daytona` and peers) passes through
  `gym run --env <provider>` for lanes that want provider-native sandboxes
  instead of Box VMs; the CLI does not pick a favorite.

### 3.7 What is deliberately absent

- **No `gym grade`/`gym judge`.** Grading is the verifier's job inside the
  harness. The CLI reports verdicts; it never renders one.
- **No composite scoreboard command.** Per-suite rows only; cross-suite
  aggregates live in `results compare`'s output, derived, and a headline
  single number waits for `plan.md` open question 5.
- **No trace upload without the corpus pipeline** for benchmark purposes —
  `trace upload` stays the one-off tool; `gym corpus import` is the batch tool
  with the ledger and the pacing. They share the client, not the workflow.

## 4. Schemas

Every `--json` output names its schema, following the CLI convention:

| Command | Schema |
| --- | --- |
| `suite list/show` | `openagents.gym.suite_manifest_view.v1` |
| `run status` | `openagents.gym.run_status.v1` |
| `results score` | passthrough of `openagents.coder_effectiveness.report.v1` |
| `corpus inventory` | `openagents.gym.corpus_inventory.v1` |
| `corpus import` (per row) | `openagents.gym.corpus_import_record.v1` |
| `dataset show` | `openagents.gym.dataset_view.v1` |
| `env probe/doctor` | `openagents.gym.env_report.v1` |

The suite manifests themselves stay `openagents.effectiveness_suite.v1` —
that schema is already the pinned contract; the Gym layer reads it and does
not fork it.

## 5. Errors worth refusing well

- **Suite pin drift** → refuse `run`, print the `suite check` diff.
- **No token for a registering run** → run locally, say so, mark the row
  unregistered; do not silently skip the lifecycle.
- **Verifier never ran on any trial** → finalize `abandoned`; the summary
  line says why in plain words.
- **Tripwire catch after redaction** → halt the batch, report the digest and
  finding codes, never continue past it.
- **`--visibility dark` / batch `--visibility glass`** → refuse with the
  ladder explained.
- **Dataset referencing a missing digest** → refuse `add`, name the nearest
  existing digest when one is within edit distance (typo help, not guessing).

## 6. Build order

This maps to `plan.md`'s milestones; the CLI work lands alongside them, not
before:

1. **`gym suite list/show/check` + `gym run` (driving `run-suite.sh`)** —
   the existing tb2 path, first-class. Immediately useful with zero corpus.
2. **`gym results score/compare` + `env probe/doctor`** — the scoring loop
   and the prereq checks, still all-Terminal-Bench. At this point
   "run tb2 and coderbench through one CLI" is real for everything recorded
   so far.
3. **`gym corpus inventory/qualify/import/status`** — M0/M1 of CoderBench.
   The inventory determinism test and the import pacing are the two
   non-negotiables here.
4. **`gym dataset`** — M2–M4: labels accumulate, datasets get pinned into the
   first CoderBench suite.
5. **`gym env box`** — cloud-box execution lanes, after the local path is
   boring.
6. **`gym results trend`, `corpus verify`, run `cancel`** — the polish that
   only matters once the earlier stages are load-bearing.

Each step leaves the previous ones working; nothing lands as a flag-gated
half-command. The test suite follows the house pattern (`tests/trace_upload_test.rs`
style: stub server, assert what goes on the wire, assert honest refusals) plus
a golden test pinning the command surface so surface changes are deliberate,
never accidental.
