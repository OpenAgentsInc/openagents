# Capacity-aware project supervision

Coder's project supervisor takes a scoped backlog, admits ready work under host
bounds, and refills capacity when a task finishes. The target is accepted work
per unit of time, not the largest number of active agents.

Implementation is tracked in [#9514](https://github.com/OpenAgentsInc/openagents/issues/9514),
under [Project 16](https://github.com/orgs/OpenAgentsInc/projects/16). The
[Devin runbook](devin-delegation-runbook.md) remains the tested operator procedure.
This document specifies the scheduling contract and records the implementation
boundary; it does not claim that the whole project is already automated.

## Why a session count is insufficient

An executor slot, a compiler slot, and a quiet measurement host are different
resources. Eight agents waiting for remote inference might use less local CPU
than two agents compiling Rust. An agent classified as a coding task can still
start a compiler. A task label alone cannot guarantee an uncontested measurement.

The host therefore declares several capacities:

| Resource | Admission rule |
| --- | --- |
| Executor slots | Count active managed delegates plus declared external reservations. |
| CPU units | Reserve a stated build allowance; pass bounded build/test settings to supported adapters. |
| Memory | Account for declared memory reservations before admission. |
| Quiet host | Drain other managed work before admitting a measurement; admit no new work until it ends. |
| Integration | Serialize review acceptance and changes to the integration branch. |
| Pending review | Stop producing more patches when the review backlog reaches its bound. |

CPU and memory reservations are accounting rules, not kernel isolation. A hard
CPU or memory guarantee needs an enforcing adapter, container, or dedicated
worker. Unsupported guarantees must be refused. A quiet-host reservation cannot
stop an unrelated process or the operator's other agent. Record external work
and verify actual host conditions before treating timing as conformance evidence.

The initial workstation has 18 CPU cores and 128 GiB of memory. The retained live
evidence establishes six concurrent local Devins on bounded tasks. Neither fact
establishes an optimal setting or a provider account entitlement. Keep the
operational session limit at six until a relevant workload justifies a change.
For the initial Project 16 takeover, reserve capacity for the separately owned
#9476 and exclude that issue from dispatch.

## What the project must say

Each runnable task needs a stable ID, pinned issue version, repository/base
identity, complete dependency observations, owned write paths, read paths,
acceptance requirements, and resource requirements. A large issue can contain
several independently useful implementation tasks; completing one task does not
close the containing issue.

Include upstream blockers in the project's visible items and link them through
GitHub's native dependency relationships. Readiness labels help people scan the
board, but dispatch uses the dependency observations and accepted task state.
An absent dependency is unknown. A closed issue also needs its recorded
implementation or disposition checked before it can satisfy a task's acceptance.

Project membership is a catalog, not execution authority. A scoped operator
configuration identifies the repository, project, permitted task definitions,
external owners, exclusions, capabilities, and effect limits. Issue text is
untrusted task context. It cannot change those settings or authorize publication.

## Deterministic scheduling and measured decisions

The harness owns dependency ordering, path conflicts, resource sums, exclusions,
attempt ownership, deadlines, retry limits, and authorization. Shared reads can
overlap. A write conflicts with an overlapping read or write, including ancestor
paths. Unknown footprints require conservative scheduling. Worktrees preserve
separate edits; they do not resolve incompatible interface changes.

The decision engine can propose a resource class, identify likely semantic
dependencies, or estimate review complexity from task descriptions and retained
outcomes. Those proposals cannot override host limits. Evaluate any proposed
policy against labeled historical outcomes and a deterministic baseline before
it changes admission. Start with host-owned resource declarations; collecting
evidence does not require inventing a confidence threshold.

Refill after each completion instead of waiting for every task in a wave.
Preserve stable task identities and return results in a documented order even
when execution finishes out of order. Add fairness so a continuous stream of
small jobs cannot starve a quiet-host measurement or a larger ready task.

## Durable lifecycle

Keep the queue outside delegated checkouts. Use cross-process locking, durable
atomic replacement, unique attempt identities, and checked transitions. Persist
a claim before dispatch. A second supervisor must observe the claim rather than
start another copy. Record the task/input/base identity on both the claim and
the result so stale completion cannot settle a newer task.

Separate execution completion, pending review, verified acceptance, and
integration. Preserve scratch worktrees and their commits. A final `done` line,
exit `0`, or model judgment cannot supply independent verification. Inspect the
actual changed files and base, review the patch, and run its required checks.
Merge, push, and issue closure are separate authorized effects.

On coordinator loss, keep unresolved attempts blocked until their process and
artifact state is reconciled. Expiring a lease does not prove that a writer
stopped. Do not restart ambiguous writing work automatically. Provider quota
failures require backoff; repeated contract failures require rescoping or repair.
Preserve unknown external-executor cost as unknown, and refuse a requested hard
monetary guarantee when the adapter cannot enforce it.

## Verification without a compiler traffic jam

Separate implementation, focused checks, integration checks, and measurement.
Delegates use isolated target directories and bounded focused checks. Completed
patches wait as artifacts, not as paid sessions occupying executor capacity.
The integration worker runs the repository's required manual gate on the
combined revision. Record the revision, toolchain, features, relevant environment,
command, and result; do not repeat an unchanged check without a reason, and do
not reuse evidence after its inputs change.

Quiet-host Gym or model measurements run after managed coding and build work
drain. Their queue wait is a scheduling result, not model latency. Distinguish
external load, cache warmth, and measurement exclusions in the evidence. Never
raise a conformance threshold to hide host contention.

## Benchmarks and release gates

Compare fixed waves and completion-driven refill at widths 1, 2, 4, 6, 8, and 10
on the same pinned workload. Include skewed task durations, dependency chains,
shared reads, conflicting writes, exclusive measurements, review backpressure,
and an externally owned task. Report admitted and accepted work, makespan,
queue wait, resource peaks, refusals, retries, review/gate time, and known or
unknown cost.

A deterministic simulation proves scheduling behavior under its stated inputs.
It does not measure Devin throughput. A local fake executor proves dispatch and
cleanup behavior without inference. Bounded live runs establish the actual
adapter and host behavior. Keep these three forms of evidence separate. Raise
the live concurrency bound only when accepted throughput improves without
increasing failures or invalidating measurements.

The outer scheduler depends on #9504 authority, #9507 intake, #9508 conflict
admission, #9509 independent verification, and #9510 durable program execution.
Implement and test their independent foundations first. Keep unavailable service
profiles and account features explicitly blocked instead of claiming that local
queue operation completes the full Decision Router consumer.

## Lessons retained from earlier work

The [delegation after-action report](../audits/2026-09-20-delegation-after-action-report.md)
identified fragmented verification, excessive unfinished work, reactive retries,
and supervisor integration as bottlenecks. The later
[workstation runbook verification](verification/2026-09-20-devin-runbook.md)
established real writing delegation and repeated six-task smoke runs, while
retaining manual review and integration. Together these support a small bounded
implementation queue with prompt acceptance, explicit recovery, and measured
refill. They do not establish that every delay came from CPU contention.

## Run the supervisor

The implemented entry point is `coder-project`. It uses the existing Coder
runtime and approved `devin-local` executor through an explicit one-task program.
The scheduler is `coder-scheduler`; neither crate implements another agent.

1. Complete the local executor setup in the [Devin runbook](devin-delegation-runbook.md).
   Build with a separate `CARGO_TARGET_DIR` for this checkout:

   ```sh
   cargo build -p coder-project -p coder-scheduler -p capability --bins
   export BIN="$CARGO_TARGET_DIR/debug"
   export CODER_PROGRAMS=burn-down,project-task
   export CODER_PROGRAM_EFFECTS=reads,writes,delegation,network,subprocesses,spend
   "$BIN/capability-trust" approve github-project --in "$REPO"
   ```

2. Create a protected supervisor directory outside the checkout and outside every
   executor write grant. Keep it separate from the writable Devin state:

   ```sh
   umask 077
   export QUEUE="$HOME/.openagents/project16-supervisor"
   mkdir -p "$QUEUE"
   chmod 700 "$QUEUE"
   cp "$REPO/docs/coder/examples/project-supervisor.json" "$QUEUE/template.json"
   ```

3. Edit the template's absolute repository path, open issue IDs, task prompts, expected outputs,
   owned paths, resource reservations, and task dependencies. Keep #9476 excluded
   while it has a separate owner. Reserve external capacity and list external
   write ownership in `external_owners`, using objects with `owner` and `writes`.
   Record a closed prerequisite in `accepted_closed_issues` only after reviewing
   its landed implementation or recorded disposition. The example deliberately
   leaves that set empty, so prerequisites require review.

4. Fetch and integrate the current default branch. Pin the prepared tasks to a
   fresh project observation and the local committed base:

   ```sh
   "$BIN/coder-project" pin-config "$QUEUE/template.json" "$QUEUE/config.json"
   "$BIN/coder-project" project "$QUEUE/config.json" "$QUEUE/state" --watch
   ```

`pin-config` fills base, issue-version, body-digest, and input-digest fields. It
creates a new output file and refuses a local base that does not contain the
observed default branch. It does not generate tasks or widen their authority.
The watcher reloads configuration and polls the project. Each result triggers
another admission round immediately. Configuration changes never rewrite an
in-flight or pending-review identity; changed pins stop the supervisor.

Omit `--watch` to drain currently admissible work and exit when no delegates
remain. `dispatch_limit` and `admission_minutes` stop new admissions; active tasks
retain their individual deadlines and drain. The admission duration is not a
whole-run time or monetary budget. Ctrl-C also stops new admissions and drains.
A process crash leaves active attempts `unknown` on recovery, holding their
resources and paths. Reconciliation requires host inspection; no ambiguous write
is automatically retried.

Each snapshot lists task blockers, excluded issues, and visible issues without a
prepared task. Project membership alone does not generate execution authority.
Prepared work needs host review once; scheduling and repeated capacity decisions
then run deterministically.

## Review and accept a result

The ledger is held by one supervisor process. Results land in
`state/attempts/<attempt>/`, with the prepared assignment, result record, and ATIF
trace. Execution status, expected-text match, and artifact verification are
separate fields. A timeout retains a writing worktree and never passes acceptance.

For writing work, inspect the exact retained scratch artifact:

```sh
"$BIN/coder-project" inspect "$REPO" "$RETAINED_WORKTREE" "$BASE" \
  crates/coder/src/tracker.rs
```

The inspector requires a clean committed patch, the correct seed tree, and only
owned paths. It does not run the artifact's hooks, tests, or build scripts and
never declares code correctness. Review the patch, run appropriate bounded
checks, integrate accepted changes through the manual gate, and retain the
evidence. A text marker alone is insufficient.

For an approved host checker, use the separate
[artifact verification command](artifact-verification.md) to bind bounded checks
to the inspected commit and artifact digest. A passing report does not accept the
scheduler result or publish the patch.

To accept or reject the result, the supervising operator writes a new JSON file
under `state/control/` with these fields:

```json
{
  "task": "9509-review-artifact",
  "attempt": "COPY_FROM_LEDGER",
  "task_digest": "COPY_FROM_LEDGER",
  "result_digest": "COPY_FROM_LEDGER",
  "accepted": true,
  "evidence": "Exact revision, reviewed artifact, commands, and observed results"
}
```

Use an atomic rename from a temporary file in the protected directory so the
watcher never sees a partially written decision. The controller checks all
identities before changing state and moves consumed decisions to `reviewed/`.
Acceptance releases dependent tasks. Rejection holds the task for deliberate
repair; it does not retry. Publishing and issue closure are separate operations.

When preparing the next queue, omit completed entries from the template and keep
the same state directory. Their accepted task IDs remain available as dependency
evidence. Preserve active, unknown, and pending-review entries unchanged until
reconciled. A changed default branch blocks queued work until it is integrated
and repinned. Do not repin an entire catalog that still contains in-flight work.

## Current implementation boundary

The queue supports one host capacity domain, local Devin, operator-prepared
resource/path declarations, durable single-writer claims, GitHub polling, and
host acceptance. It does not discover other agents automatically, enforce CPU or
memory with kernel quotas, reconnect to an external Devin after coordinator
loss, choose measured semantic admission policies, or integrate and close issues
without supervision. Unknown provider cost remains unknown. These gaps remain
tracked under #9503, #9504, #9508, #9509, #9510, and #9514.

The [2026-09-21 verification record](verification/2026-09-21-project-supervisor.md)
contains the passing manual gate, scheduling comparisons, live queue results,
restart checks, and timed-out implementation jobs.

## Proposed shared context and background work

The [TypeSafe-native roadmap](typesafe-agent-roadmap.md#phase-4-share-context-across-parallel-and-background-work)
extends this controller with immutable evidence snapshots, task-specific
context manifests, and structured findings. The current prepared-task
scheduler and durable claims are foundations; they do not yet provide that
shared working state or a general background-assistance system.

Bind each task's context to the base, issue version, and evidence digests
it read. Share valid read observations across tasks. Recheck affected
references after a writing task completes and before integrating another
artifact. A returned finding should name its sources and verification;
combining findings should not concatenate every delegate transcript.

A first background feature can explain a diff using observations the
foreground task already collected. Give it a lower priority, an explicit
resource/disclosure allowance, changed-input debounce, cancellation, and
revision-bound results. Coalesce obsolete work and reserve foreground
capacity. Read-only tasks still consume resources and may disclose source
to a provider; existing session policy applies.

Extend #9508/#9514 for these scheduling behaviors and #9510 for complete
recovery and whole-run accounting. Do not infer execution authority from
model-selected relevance, duplicate-task similarity, project membership,
or an available scheduler slot.
