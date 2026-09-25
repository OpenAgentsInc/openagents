# Candidate retention and cohort budget repairs

Issues [#9649](https://github.com/OpenAgentsInc/openagents/issues/9649) and
[#9650](https://github.com/OpenAgentsInc/openagents/issues/9650) repair the
measurement infrastructure exposed by the [v18 family](2026-09-25-microluna-v18-family.md).
The repairs add per-session artifact collection and a persistent cohort budget
ledger. They do not improve or replace the v18 results: all 18 submissions still
fail, and candidate oracle coverage remains complete for only six trials.
No new model cohort runs as part of this work, and no policy is promoted.

## Capture a candidate that a verifier can restore

A workspace copy alone cannot reproduce a task whose verifier consumes a database
dump or another service's output. The new `candidate-checkpoint-v1` protocol
pauses the sequential executor between sessions while the Harbor adapter captures
the task's declared artifacts:

1. Coder One writes a request naming the dispatch and session. It waits for a
   matching, complete acknowledgement with a receipt digest before continuing.
2. The host runs the task's declared main-service collection hooks and downloads
   its artifacts, including `/logs/artifacts` and custom destinations.
3. If the task has sidecar artifacts or hooks, the host pauses the main container,
   runs those hooks, downloads those artifacts, and resumes the main container.
   This also stops background processes in main during sidecar collection.
4. The host seals the artifact inventory, task checksum, hook command hashes and
   exit codes, observed absences, and collection status in `receipt.json`.
   The executor records the acknowledgement beside its selection decision.
5. After the trial finishes, `tbench candidates` checks these identities and uses
   Harbor's separate-verifier replay. No verifier grade goes back to the executor.

The adapter takes hook commands and paths from the pinned task, never from the
checkpoint request. It does not invoke the official verifier during generation.
The task checksum identifies verifier bytes without exposing them to the executor.
A hook failure, copy error, timeout, missing acknowledgement, or incomplete
inventory remains a recorded failure. Cancellation resumes the main container;
a failed resume cannot produce a complete receipt. An executor that cannot
complete collection stops before its next session.

Host collection is independent of the old whole-workspace snapshot. A workspace
that exceeds the 20,000-file or 256 MiB snapshot limit can still leave a complete,
smaller declared-artifact capture. The capture inventory itself is bounded to
20,000 entries and 256 MiB and refuses symlinks and special entries. A task that
exceeds those limits remains unknown; this is not unlimited filesystem retention.

### Missing is different from uncollected

A failed copy does not establish that a file was absent. The collector separately
checks whether its source exists in the named service. A confirmed absence is
recorded explicitly; a transport or permission failure remains incomplete.
Partial download directories are removed for confirmed absences.

Harbor's manifest accepts `empty`, but that alone cannot distinguish a missing
path from an empty directory. The checkpoint receipt retains this distinction.
The checkpoint verifier environment removes the recorded absent paths from its
fresh image before Harbor restores the collected files. This avoids accidentally
reintroducing an original file or directory that the candidate deleted. Both
custom destinations and sidecar exports retain Harbor's ordinary restore layout.

### Coverage and admission

Before launching an expensive experiment, inspect each task:

```sh
cd bench/terminal-bench
uv run tbench candidate-preflight /absolute/path/to/task
```

The JSON reports the task checksum, declared artifacts, collection hooks, a
120-second collection deadline, and explicit unsupported reasons. Supported
contracts are single-step tasks with a separate verifier, absolute artifact
sources, and no more than 90 seconds of declared collection-hook timeouts.
The adapter also requires the warm Docker environment and a rebuilt Coder One
binary advertising `candidate-checkpoint-v1`.

Sequential Microluna policies with `lean.retain_candidates` enable checkpoint
collection by default. Parallel or protected lanes are refused: their independent
candidate workspaces need a separate collection protocol. Tasks with no declared
artifacts, shared verifiers, or unsupported collection contracts are refused before
inference. Preflight checks the declared contract; runtime checks still catch
missing services, failed hooks, oversized artifacts, and copy failures.

For an intentional reproduction with an old binary, pass
`--agent-kwarg candidate_capture=false`. This preserves legacy retention and its
known coverage limits. Do not silently disable capture in a new confirmation.
Collection runs task hooks between sessions and consumes wall time, so it is a
harness change that must be frozen in the next protocol, not added to an active one.

Each trial keeps `agent/candidate-preflight.json` and
`agent/candidate-checkpoints/lean-N-session-M/`. The latter contains the artifact
manifest, bytes, and receipt. `tbench retain` includes these in the evidence bundle
and applies its usual credential scan and size-limit reporting. Files excluded by
a publication size limit remain explicit missing evidence; the original job
remains the replay source.

### Oracle verdicts

Discovery now records an error per unavailable candidate and continues to later
sessions. An orphan checkpoint without its executor acknowledgement or an executor session
trace without a candidate record also makes coverage incomplete. A verified passing candidate proves that at least one
candidate passed even if another is missing. A negative oracle verdict requires
complete binary grades for every recorded candidate. Otherwise the verdict is
`null`, never zero.

Use the existing command after execution finishes:

```sh
uv run tbench candidates /absolute/path/to/trial \
  --output /absolute/path/to/new-grades --jobs 2
```

The v2 batch report keeps discovery errors, invalid regrades, identity digests,
per-candidate results, and per-trial coverage. Deduplication remains optional and
never turns a reused grade into another independent verifier execution.

## Reserve budget before launching

`tbench cohort` replaces ad hoc launch accounting for fixed sequential Microluna
cohorts. Its spec freezes the ordered task slots, policy, artifact digest, cohort
ceiling, per-attempt reservation, concurrency, retry limit, and cost-rule version.
Its journal stays outside the source checkout. It requires a clean committed
checkout and checks source, artifact, policy, and task identities before launches.
Each slot must resolve to the exact local task the selected profile would run;
profiles with hidden Harbor retries or multiple attempts are refused.

Example spec, with paths and digest replaced by the actual frozen inputs:

```json
{
  "schema": "openagents.tbench.cohort.v1",
  "id": "new-confirmation",
  "budget_usd": "3.00",
  "reservation_usd": "0.12",
  "concurrency": 2,
  "retries": 1,
  "cost_rule": {"version": "microluna-open-request-v1", "luna_bound_usd": "0.09"},
  "artifact_path": "/absolute/path/to/coder-one",
  "artifact_sha256": "REPLACE_WITH_SHA256",
  "policy_path": "/absolute/path/to/frozen-policy.json",
  "schedule": [
    {"id": "task-a-1", "profile": "tb4", "agent": "coder-one-microluna-v18",
     "task_path": "/absolute/path/to/the/tb4/checkout/tasks/TASK_ID"}
  ]
}
```

The policy's Microluna budget must match `luna_bound_usd`. The reservation must
exceed that bound to allow headroom for Jev. Select a reservation appropriate to
the frozen policy; the example is not a newly approved experiment.

```sh
uv run tbench cohort plan --spec /absolute/path/to/spec.json \
  --output /absolute/path/to/cohort-ledger
uv run tbench cohort run --spec /absolute/path/to/spec.json \
  --output /absolute/path/to/cohort-ledger
uv run tbench cohort report --spec /absolute/path/to/spec.json \
  --output /absolute/path/to/cohort-ledger
```

`plan` checks identity and coverage without inference. `run` executes the frozen
schedule through the existing `tbench run` adapter. `report` regenerates the report
from the journal's original identity and records an inspection epoch. While a driver owns the exclusive lock, read its
atomically written `report.json`; the reconstruction command requires the driver
to be idle. Neither
reporting nor restarting resets previous attempts, retries, or reservations.

To check spend while a cohort runs, read the journal without writing to it:

```sh
uv run tbench cohort status --output /absolute/path/to/cohort-ledger
```

`status` needs no spec and takes no lock. It prints the counted spend, the
amount held by reservations, a lower and an upper bound, the number of attempts
whose full cost is unknown, and the remaining budget. The lower bound adds only
the costs the usage records prove. The upper bound adds every hold to the counted
spend; it is the most the cohort charges against its ceiling under the cost rule,
not a limit on the provider's bill. `report.json` carries the same two bounds.

### One rule for execution and reporting

| Evidence | Recorded cost | Counted cost and action |
| --- | --- | --- |
| Complete, consistent usage | Exact retained amount, including zero | Count that amount. |
| Proven refusal or failure before agent execution, with no usage | Known zero | Count zero; a bounded infrastructure retry may run. |
| One timed-out Microluna dispatch, known Jev cost, no unpriced generation | Full cost unknown; retain the lower bound | Count `max(lower bound, Luna bound + Jev)`. |
| Missing usage, unknown Jev, another unknown operation, or multiple open dispatches | Unknown; preserve any known lower bound | Keep at least the entire reservation and stop new launches. |

The same versioned `price` function produces both the live settlement and report.
The open-request rule reproduces the frozen v18 convention; it is a conservative
experimental accounting rule, not a provider billing receipt or proof of an
absolute charge bound. Reservations enforce the declared accounting ceiling.
If a completed attempt exceeds its reservation, the report records the breach
and stops further launches. Unbounded unknowns also stop launches. This does not
claim to cancel a provider request retroactively or guarantee its eventual bill.

A hash-chained, fsynced `ledger.jsonl` records reservations before processes start,
launches, settlements, process IDs, launch/inspection epochs, and deviations. An
exclusive lock prevents two drivers from owning the same cohort. Stable slot and
attempt identities make settlement idempotent. A corrupt or truncated ledger is
refused rather than discarded. Source or spec changes are retained as deviations
and refused, including a restart whose identities cannot be verified.

A preparation failure remains an attempt; its bounded retry receives a new job
name. A reservation whose launch or completion cannot be proven retains its hold
and is not automatically retried. Completed results are settled after the owned
or adopted process exits. The driver uses task CPU and memory reservations, a
20 GiB disk floor, and the existing single-trial runner. GPU tasks are refused by
this initial cohort driver. It does not coordinate other schedulers; run a frozen
cohort on its reserved host without overlapping benchmark cohorts.

A report distinguishes `complete` (all planned slots have grades) from
`accounting_complete` (all attempts have known or conservatively counted costs).
An incomplete run exits nonzero. Preserve the journal, report, raw job directories,
and source/artifact pins together; the report is a view, not the launch history.

## Recovery of the existing v18 record

The [recovery audit](../../bench/terminal-bench/experiments/2026-09-25-retention-budget-repairs/v18-recovery-audit.json)
reads only the exact 18 jobs in the retained v18 family state. Its 26 usable
workspace identities match the original batch exactly. It finds no additional
recoverable candidate. All 15 missing snapshots are still unavailable under the
original recorded snapshot-bound error; the audit does not pretend it can now
separate size from unreadable-directory causes in that old message.

| Family task | Usable workspace copies | Unavailable copies | Oracle coverage |
| --- | ---: | ---: | --- |
| Payments pipeline | 7 | 0 | Three trials unknown: seven candidate captures lack their collect-hook artifacts. |
| Live database cutover | 7 | 0 | Three trials unknown: seven candidate captures lack their collect-hook artifacts. |
| Checkpoint consolidation | 0 | 6 | Three trials unknown. |
| Cumulative layout shift | 0 | 9 | Three trials unknown. |
| Telecom entity resolution | 6 | 0 | Three complete negative oracles from the original grades. |
| Photonic waveguide routing | 6 | 0 | Three complete negative oracles from the original grades. |

No final submission supplies an invented intermediate artifact. No new verifier
execution or model call is needed for this audit. Confirmation tasks remain
outcome-only, and the sealed #9584 cohort's official outcomes are not opened.

The [accounting replay](../../bench/terminal-bench/experiments/2026-09-25-retention-budget-repairs/v18-accounting-replay.json)
runs the new rule over all 18 retained usage files. Six receive the open-request
bound. The total is **$0.96359699**, matching the published conservative total,
rather than the old driver's $0.4161044. The five earlier setup starts remain
published in the original family report; no historical receipt is overwritten.

`tests/test_cohort_v18_replay.py` feeds the retained v18 launch logs and outcome
records through the cohort journal in their original order, without a trial or
a model call. It shows what the journal would have done:

1. It counts the five setup-only starts as known zeros, because each failed
   before the agent ran and left no usage record. Payments and checkpoint each
   use their one rerun.
2. It refuses to continue after the harness source changes from `3a25a0ff1f`
   to `0f2d7e6bf4`, and keeps the attempted identity as a deviation.
3. Even with the original source, it refuses the restarted driver's fresh
   payments and checkpoint starts, because those slots had no rerun left. The
   layout slot's relaunch is its one permitted rerun.

Run as its own cohort, the 18 completed attempts store a lower bound of
**$0.68201621** and an upper bound of **$0.96359699** in `report.json`, with six
attempts whose full cost is unknown.

## Validation

The [repair evidence directory](../../bench/terminal-bench/experiments/2026-09-25-retention-budget-repairs/)
retains the audit, accounting replay, synthetic tasks, sealed checkpoint bundles,
and separate-verifier results. Both real Docker fixtures pass: single-service and
main-plus-sidecar, with a non-root checkpoint requester, a custom artifact destination, convention logs, and absent
files and directories. The verifier image deliberately starts with those absent
paths present; replay removes them and restores the captured first state after
the original task changes to a second state and is destroyed.

Unit and process tests cover tampered artifacts, missing acknowledgements,
collection cancellation, failed sidecar hooks, unsupported policies, later usable
candidates after earlier failures, orphan checkpoints, and retention publication.
The cohort tests cover unknown charges, deadline bounds, simultaneous reservations,
reservation breaches, exclusive ownership, source changes, corrupt journals,
idempotent settlement, and restart. A real synthetic subprocess queue retains a
setup failure and its retry, counts $0.08, then refuses another $0.12 reservation
under a $0.18 ceiling. A restart launches nothing again. No fixture uses paid models.

The default macOS gate exposes an existing `RLIMIT_DATA` refusal, separately
tracked in [#9651](https://github.com/OpenAgentsInc/openagents/issues/9651).
It is not fixed by these repairs, and the Mac gate is not reported as passing.
Linux verification uses a separate worktree and Cargo target directory. The
retained verification record lists the exact commands, results, and scope.

Verification results: **341 Python tests pass on Linux**, with three skips;
**both real Docker fixtures pass** on macOS. The pinned Rust 1.97.1 package gate
passes formatting, strict Clippy, and both requested test phases. Each test phase
passes 762 unit tests and two integration tests, with one intentionally ignored
whole-checkout timing test. The separate preflight, gate-tooling, artifact,
delegation, and backup phases also pass. These are scoped checks, not a claim
that the full workspace, external models, or PostgreSQL release gate ran. The
[verification summary](../../bench/terminal-bench/experiments/2026-09-25-retention-budget-repairs/verification/summary.json)
binds the checked source files to their SHA-256 digests and retains earlier failed
checks and their corrections.
