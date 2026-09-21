# Audit of project completion failure

Updated September 21, 2026. All timestamps below are UTC.

## Outcome and responsibility

The user reported 46 open issues before leaving and 45 on returning about five
hours later. Only #9473 closed in that complaint window. I spread implementation
across too many issues, repeatedly integrated unfinished delegated artifacts, and
used passing checks and commits as progress signals while acceptance criteria
remained unmet. The work was real; the delivery rate was not acceptable. These
were my execution and prioritization failures.

The first version of this audit listed examples instead of the full history and
did not quantify verification overhead. This revision corrects that omission.
After the stop instruction, I finished #9482, pushed `72587db11f`, and closed it at
12:02:48. The open count was then 44. Recurring supervision was deleted and the
project-wide goal remains paused. This revision is documentation work only.

## Scope and evidence

The narrow comparison window is **06:47–11:47 on September 21**, matching the
original audit's approximately five-hour scope. The interruption was recorded at
11:45:15; 11:47 is the original audit's observation boundary, not a more precise
claim about when the user returned.

For completeness, the inventories cover the **whole recorded task**, beginning
September 20 at 23:00:08, through September 21 at 12:03:30, immediately after the
requested final issue completion. This includes initial specification and runbook
work, takeover work, the five-hour complaint window, and the final correction.
Earlier work must not be credited as progress during the five-hour window.

Sources are Git objects and reflogs, this task's timestamped tool calls/results,
retained manual-gate logs, and Coder result records. The commit inventory contains
**175 retained commit objects across 107 subjects**:
**106** are reachable from published main at `72587db11f`, and
**69** are source, rebased, cherry-picked, or retained versions
outside that ancestry. These are not 175 separate delivered changes.
The inventory includes delegated commits I preserved or integrated, not just
commits whose code I personally wrote. Unrelated concurrent commits by other
agents, including the separately owned #9476 work, are excluded.

Commit purposes below are reconstructed from recorded changes, commit messages,
and the surrounding implementation record. They explain the technical reason for
a change, not a claim that switching to it was the right priority. Equivalent
subjects on different hashes share a rationale; that does not assert byte-identical
patches. Unretained objects or unlogged commands cannot be reconstructed reliably.
The inventory includes observed commit outputs, integration merges, and retained
versions of those changes found in the reflogs; it is not an attribution of every
commit on the machine to this task.

Raw transcripts and execution logs remain private because they can contain
credentials, prompts, paths, or user data. The checked-in evidence contains only
commit metadata, numeric measurements, record names, and log digests:

- [Every commit and rationale as CSV](commits.csv).
- [Every recovered gate phase and duration](gate-phases.csv).
- [Gate and delegation timing records](timing.json).
- [Timestamped command and wait intervals](waits.csv).

## Time accounting

| Measure | Whole task through final stop | Five-hour complaint window |
| --- | ---: | ---: |
| Identified full-gate launches | 46 | 26 launched inside the window |
| Recorded completed gate-phase runtime | 3h 35m 01.7s | 2h 19m 10.7s for those launches |
| Explicit wait-call elapsed time, overlapping intervals merged | 0h 50m 29.4s | 0h 27m 23.9s |
| Command calls plus explicit waits, overlapping intervals merged | 1h 02m 24.7s | 0h 33m 08.8s |
| Recorded executor-result elapsed time, summed across attempts | 8h 19m 25.2s | Not allocated without reliable per-attempt start/end timestamps |
| Initial delegation smoke-run wall times, summed across runs | 0h 18m 20.0s | Earlier work; not credited in this window |

**These columns must not be added together.** Gate phases overlap command waits;
executor attempts ran concurrently with host work and other attempts. Their sum
is process time, not a unique wall-clock delay. The explicit wait total is the
observed time from invoking a poll/sleep/wait tool to receiving its result. The
command-inclusive total also contains builds, reads, mutations, and tool overhead;
it is not all idle time. Neither number measures time spent composing a response
while a process ran in the background. There is no defensible single total for
"all time doing nothing" in these records.

The gate runtime is a **lower bound** on total gate occupancy: four overwritten
attempts have only printed phase records; two logs end without the final phase
completion. Their unfinished phases retain an additional 60 seconds of
heartbeat evidence not included in the completed-phase sum. Phase timings are
rounded by the runner. Launch timestamps identify the enclosing command, which
sometimes performed a commit or focused check before starting the gate. They are
not precise timestamps for each phase. The five-hour phase subtotal counts runs
launched inside the window and does not prorate the run launched at 06:44:40.

Across the full inventory, **33 runs reached a successful PostgreSQL phase,
11 recorded a failing phase, and 2 have incomplete terminal records**. In the
five-hour launch cohort, **21 completed and 5 failed**. A passing gate on a
partial implementation did not complete its issue.

### Where gate time went

| Phase | Recorded time across runs |
| --- | ---: |
| Default workspace tests | 1h 33m 12.3s |
| Feature workspace tests | 1h 25m 20.8s |
| PostgreSQL acceptance | 0h 16m 28.7s |
| Rust 1.95 workspace check | 0h 06m 48.9s |
| Default workspace Clippy | 0h 05m 47.1s |
| Feature workspace Clippy | 0h 03m 39.4s |
| Rust 1.94 Kev check | 0h 01m 42.5s |
| Backup collection tests | 0h 00m 33.3s |
| Dependency policy | 0h 00m 33.2s |
| Workspace formatting | 0h 00m 27.0s |
| Delegation evidence checks | 0h 00m 24.3s |
| Artifact acquisition tests | 0h 00m 04.2s |

Default and feature workspace tests account for most recorded gate runtime.
Repeated full runs on adjacent slices were therefore expensive even when the
changed code was small. The repository requires the manual gate; the mistake was
how I scheduled integrations and reruns, not the existence of verification.

## Every identified manual-gate run

`supervision/` means the retained project-supervision evidence directory;
`delegation/` means the earlier Coder delegation evidence directory. Repeated log
names are separate launches, not duplicate rows. Durations sum recovered completed
phases. “Incomplete” and “printed only” rows are lower bounds, not successful
runs. The phase CSV lists each individual phase, duration, and exit code.

| Run | Launch UTC | Log | Recorded duration | Result and evidence |
| --- | --- | --- | ---: | --- |
| G01 | 2026-09-21 02:51:03 | `delegation/manual-gate.log` | 0h 07m 04.3s | failed |
| G02 | 2026-09-21 03:10:18 | `delegation/manual-gate-final.log` | 0h 02m 12.2s | incomplete records |
| G03 | 2026-09-21 03:17:03 | `delegation/manual-gate-integrated.log` | 0h 04m 04.4s | completed |
| G04 | 2026-09-21 04:29:41 | `supervision/manual-gate.log` | 0h 03m 43.0s | completed |
| G05 | 2026-09-21 05:00:51 | `supervision/verification-manual-gate.log` | 0h 04m 02.8s | completed |
| G06 | 2026-09-21 05:07:26 | `supervision/artifact-integration-gate.log` | 0h 03m 23.0s | completed |
| G07 | 2026-09-21 05:20:36 | `supervision/money-manual-gate.log` | 0h 01m 50.0s | failed; printed only, overwritten |
| G08 | 2026-09-21 05:23:57 | `supervision/money-manual-gate-retry.log` | 0h 04m 36.0s | completed |
| G09 | 2026-09-21 05:32:38 | `supervision/local-client-manual-gate.log` | 0h 07m 58.6s | completed |
| G10 | 2026-09-21 05:44:36 | `supervision/suite-integration-gate.log` | 0h 00m 01.3s | failed |
| G11 | 2026-09-21 05:48:14 | `supervision/suite-integration-gate-retry.log` | 0h 04m 04.2s | completed |
| G12 | 2026-09-21 05:56:24 | `supervision/relay-protocol-manual-gate.log` | 0h 00m 02.1s | failed |
| G13 | 2026-09-21 05:56:44 | `supervision/relay-protocol-manual-gate-retry.log` | 0h 02m 41.9s | incomplete records; unfinished phase ≥60s extra |
| G14 | 2026-09-21 06:02:08 | `supervision/relay-protocol-manual-gate-complete.log` | 0h 02m 13.2s | failed |
| G15 | 2026-09-21 06:11:59 | `supervision/relay-classification-combined-gate.log` | 0h 04m 26.3s | completed |
| G16 | 2026-09-21 06:24:33 | `supervision/decision-profile-manual-gate.log` | 0h 04m 31.6s | completed |
| G17 | 2026-09-21 06:32:11 | `supervision/scorable-bound-manual-gate.log` | 0h 04m 23.2s | completed |
| G18 | 2026-09-21 06:38:35 | `supervision/repo-bounds-manual-gate.log` | 0h 04m 20.6s | completed |
| G19 | 2026-09-21 06:44:40 | `supervision/classification-modes-manual-gate.log` | 0h 03m 29.8s | failed |
| G20 | 2026-09-21 06:51:37 | `supervision/classification-backup-manual-gate.log` | 0h 01m 12.8s | failed |
| G21 | 2026-09-21 06:54:48 | `supervision/classification-backup-final-gate.log` | 0h 03m 19.3s | completed |
| G22 | 2026-09-21 07:10:21 | `supervision/gym-suite-manual-gate.log` | 0h 15m 17.7s | completed |
| G23 | 2026-09-21 07:26:38 | `supervision/repository-evidence-manual-gate.log` | 0h 05m 59.6s | completed |
| G24 | 2026-09-21 07:34:05 | `supervision/classification-summaries-manual-gate.log` | 0h 03m 54.2s | completed |
| G25 | 2026-09-21 07:40:37 | `supervision/context-manifest-manual-gate.log` | 0h 03m 02.2s | failed |
| G26 | 2026-09-21 07:44:20 | `supervision/context-manifest-final-gate.log` | 0h 03m 52.8s | completed |
| G27 | 2026-09-21 07:52:52 | `supervision/membership-manual-gate.log` | 0h 04m 10.8s | completed |
| G28 | 2026-09-21 08:03:20 | `supervision/membership-key-manual-gate.log` | 0h 04m 01.5s | completed |
| G29 | 2026-09-21 08:12:45 | `supervision/batch-membership-manual-gate.log` | 0h 04m 08.1s | completed |
| G30 | 2026-09-21 08:24:45 | `supervision/discovery-limits-manual-gate.log` | 0h 04m 01.5s | completed |
| G31 | 2026-09-21 08:44:15 | `supervision/review-workflow-manual-gate.log` | 0h 00m 02.1s | failed; printed only, overwritten |
| G32 | 2026-09-21 08:44:49 | `supervision/review-workflow-manual-gate.log` | 0h 06m 29.8s | completed |
| G33 | 2026-09-21 09:13:45 | `supervision/classification-callers-manual-gate.log` | 0h 00m 03.3s | failed; printed only, overwritten |
| G34 | 2026-09-21 09:14:15 | `supervision/classification-callers-manual-gate.log` | 0h 09m 08.6s | completed |
| G35 | 2026-09-21 09:32:44 | `supervision/money-manual-gate.log` | 0h 00m 01.8s | failed; printed only, overwritten |
| G36 | 2026-09-21 09:33:14 | `supervision/money-manual-gate.log` | 0h 05m 50.4s | completed |
| G37 | 2026-09-21 09:51:53 | `supervision/admission-manual-gate.log` | 0h 10m 13.2s | completed |
| G38 | 2026-09-21 10:02:29 | `supervision/admission-combined-manual-gate.log` | 0h 09m 53.2s | completed |
| G39 | 2026-09-21 10:20:13 | `supervision/classification-contract-manual-gate.log` | 0h 05m 30.7s | completed |
| G40 | 2026-09-21 10:32:34 | `supervision/mcp-docs-manual-gate.log` | 0h 05m 54.3s | completed |
| G41 | 2026-09-21 10:38:49 | `supervision/mcp-docs-final-manual-gate.log` | 0h 05m 48.3s | completed |
| G42 | 2026-09-21 11:04:26 | `supervision/review-policy-final-manual-gate.log` | 0h 11m 09.4s | completed |
| G43 | 2026-09-21 11:18:15 | `supervision/classification-response-final-manual-gate.log` | 0h 06m 44.0s | completed |
| G44 | 2026-09-21 11:26:42 | `supervision/classification-review-response-final-gate.log` | 0h 04m 33.6s | completed |
| G45 | 2026-09-21 11:32:01 | `supervision/gateway-backend-boundary-final-gate.log` | 0h 04m 47.5s | completed |
| G46 | 2026-09-21 11:54:13 | `supervision/final-classification-gate.log` | 0h 06m 42.5s | completed |

### What caused failed or repeated gates

- G01 failed PostgreSQL gateway acceptance. G02 lacks a terminal phase record;
  G03 verified the integrated runbook tree. The missing completion is not a pass.
- G07 failed the NIP-44 padding property; G08 reran after the property repair.
- G10 failed formatting. G11 was the corrected integration run.
- G12 failed strict Clippy on a large enum variant. G13 has an incomplete ending;
  G14 then failed the Kev oversized-body refusal test with a connection reset.
  G15 verified the combined relay/classification tree.
- G19 exposed the concurrent backup race. G20 then failed a backup static test;
  G21 was the repaired run. This dragged unrelated gate repair into classification.
- G25 failed the local-profile headless socket fixture. G26 reran after its repair.
- G31, G33, and G35 failed strict Clippy before their respective corrected runs.
  A focused lint check before a full-gate launch could have caught these earlier.
- G37/G38, G40/G41, and G43/G44/G45 passed on successive nearby increments.
  Passing was legitimate evidence for each tree, but the implementation should
  have been planned and batched around issue acceptance before full verification.

The records do not establish that every repeated minute was avoidable. Changed
behavior needs verification, and some gates found real defects. They do establish
that I repeatedly paid full-workspace cost while leaving the owning issue open.

## Delegation and other process durations

The following rows report the elapsed value in each retained executor result.
“Answered” is the executor status, **not independent acceptance**. Some answered
artifacts required host repair, and the final worker recovery remained unaccepted.
`artifact_verified` is the recorded result field, not a retrospective verdict.
The timing JSON includes source-record hashes. Smoke runs are listed separately;
none of these process sums should be added to blocking-wait time.

| Result record | Task | Elapsed | Executor status | Recorded artifact verified |
| --- | --- | ---: | --- | --- |
| `9504-authority-completion/result.json` | 9504-authority-completion | 0h 15m 06.4s | no execution status | false |
| `9507-intake-hardening/result.json` | 9507-intake-hardening | 0h 15m 06.4s | no execution status | false |
| `adapter-smoke-1/result.json` | adapter-smoke | 0h 00m 20.5s | answered | false |
| `decision-worker-recovery-retry/attempts/att-7f943dfc34a60df8d8df9466e9126499-000001/result.json` | 9469-worker-recovery | 0h 36m 59.5s | answered | false |
| `decision-worker-recovery/attempts/att-c1409c3d470e00c3d94389a6326853e6-000001/result.json` | 9469-worker-recovery | 0h 00m 00.0s | Not reported | false |
| `project16-admission/attempts/att-613901c119e9a5f896ca5eb85f62d841-000001/result.json` | 9473-candidate-admission | 0h 30m 06.6s | timed out | false |
| `project16-batch/attempts/att-55b8232a9aac424d4089d8b9461186b4-000001/result.json` | 9483-bounded-classification-concurrency | 0h 24m 35.7s | answered | false |
| `project16-callers/attempts/att-5072df152782661c46bbaaf18646a444-000001/result.json` | 9482-classification-callers | 0h 30m 06.5s | timed out | false |
| `project16-decision-worker/attempts/att-892f33c9a98339ca24bda23bd3a5a7f0-000001/result.json` | 9469-decision-relay-worker | 0h 45m 08.0s | timed out | false |
| `project16-membership/attempts/att-03a12763641b1339ba83a8aea524d91d-000001/result.json` | 9490-workspace-membership | 0h 18m 21.8s | answered | false |
| `project16-money-retry/attempts/att-ec32dbc4b95cc2dc6a9b52ebb448b2a9-000001/result.json` | 9491-gateway-monetary-holds | 0h 33m 17.2s | answered | false |
| `project16-money/attempts/att-414585874051a923582568267802abb5-000001/result.json` | Not recorded | Unknown | no execution status | false |
| `project16-production/attempts/att-7816a2a7c31b4e9311dc351bb85e31e7-000001/result.json` | 9482-classification-envelope | 0h 07m 31.1s | answered | false |
| `project16-production/attempts/att-7816a2a7c31b4e9311dc351bb85e31e7-000002/result.json` | 9469-relay-decision-contract | 0h 08m 41.5s | answered | false |
| `project16-production/attempts/att-7816a2a7c31b4e9311dc351bb85e31e7-000003/result.json` | 9486-backend-capability-record | 0h 10m 16.9s | answered | false |
| `project16-production/attempts/att-7816a2a7c31b4e9311dc351bb85e31e7-000004/result.json` | 9469-decision-protocol | 0h 26m 58.6s | answered | false |
| `project16-production/attempts/att-7816a2a7c31b4e9311dc351bb85e31e7-000005/result.json` | 9482-classification-http | 0h 30m 18.3s | timed out | false |
| `project16-production/attempts/att-7816a2a7c31b4e9311dc351bb85e31e7-000006/result.json` | 9509-run-suite-host | 0h 23m 36.3s | answered | false |
| `project16-production/attempts/att-7816a2a7c31b4e9311dc351bb85e31e7-000007/result.json` | 9482-classification-modes | 0h 20m 58.2s | answered | false |
| `project16-production/attempts/att-7816a2a7c31b4e9311dc351bb85e31e7-000008/result.json` | 9509-suite-adapter | 0h 30m 12.3s | timed out | false |
| `project16-production/attempts/att-7816a2a7c31b4e9311dc351bb85e31e7-000009/result.json` | 9482-classification-summaries | 0h 17m 24.0s | answered | false |
| `project16-review-changes/attempts/att-2f051fba9185181347010b33c02a0c45-000001/result.json` | 9509-review-changes | 0h 30m 06.5s | timed out | false |
| `project16-review-policy/attempts/att-93a686e0829b141d5ac3fe9b80fe87e4-000001/result.json` | 9485-gateway-review-policy | 0h 41m 29.4s | answered | false |
| `queue-smoke-1/attempts/att-3dcce94340cc67faa243e2ec05562f97-000001/result.json` | queue-authority | 0h 00m 35.5s | answered | false |
| `queue-smoke-1/attempts/att-3dcce94340cc67faa243e2ec05562f97-000002/result.json` | queue-effects | 0h 00m 26.7s | answered | false |
| `queue-smoke-1/attempts/att-3dcce94340cc67faa243e2ec05562f97-000003/result.json` | queue-tracker | 0h 00m 25.5s | answered | false |
| `queue-smoke-2/attempts/att-a02d72f7e4eeed27bdefbec25724ac45-000001/result.json` | queue-authority | 0h 00m 23.9s | answered | false |
| `queue-smoke-2/attempts/att-a02d72f7e4eeed27bdefbec25724ac45-000002/result.json` | queue-effects | 0h 00m 27.0s | answered | false |
| `queue-smoke-2/attempts/att-a02d72f7e4eeed27bdefbec25724ac45-000003/result.json` | queue-tracker | 0h 00m 25.0s | answered | false |

The raw `bootstrap-1`, `9504-program-authority`, `9507-tracker-intake`, and
`9514-scheduler-core` result envelopes do not carry an `elapsed_ms` field. Their
elapsed executor times are **unknown in this inventory**, not zero. Likewise,
the first `project16-money` attempt lacks an elapsed result. The worker recovery
refused before execution reports zero; that does not imply every setup action
around it took zero time. No provider-spend total can be established from these
records.

| Earlier smoke-run metrics | Wall duration | Exit code |
| --- | ---: | ---: |
| `local-smoke-1.metrics.json` | 0h 01m 07.1s | 0 |
| `local-smoke-2.metrics.json` | 0h 00m 57.4s | 0 |
| `relay-six.metrics.json` | 0h 00m 20.4s | 0 |
| `relay-two.metrics.json` | 0h 00m 22.2s | 0 |
| `six-read-1.metrics.json` | 0h 00m 44.2s | 0 |
| `six-read-2.metrics.json` | 0h 01m 14.5s | 0 |
| `six-write-2.metrics.json` | 0h 13m 27.1s | 0 |
| `six-write.metrics.json` | 0h 00m 06.9s | 2 |

Focused builds, tests, schema checks, CLI calls, and polling also consumed time.
The wait CSV records every identified command/wait invocation interval in the
task through the cutoff, including these short calls. It does not falsely assign
a background process's entire lifetime to a one-second launch call. A command
that returned a session continued running outside that invocation; the gate and
executor tables capture their separately recorded runtime where available.

## Commit-by-commit rationale

Each row names a retained commit object. **Main** means reachable from the final
published revision; **retained** means a source/rebased copy or pending artifact
outside that ancestry. In particular, the worker hashes are not shipped work.
Timestamps are commit timestamps, not measurements of how long coding took.
A cherry-pick often gives several source changes the same integration timestamp.

| Commit | Committed UTC | Change and rationale | State |
| --- | --- | --- | --- |
| [`a52824b5ee`](https://github.com/OpenAgentsInc/openagents/commit/a52824b5eefc82daa2a9f419393a551295ef0298) | 2026-09-20 23:50:46 | **Specify the complete Decision API product and delivery plan.** Translate the requested product parity into a public spec and issue-sized delivery plan. Specification work did not implement the backlog. | Main |
| [`db22fead00`](https://github.com/OpenAgentsInc/openagents/commit/db22fead007a3bffd43842bdd7639b270da4b732) | 2026-09-21 02:13:16 | **Define Coder as the flagship Decision Router consumer.** Describe how Coder should exercise the Decision Router and identify missing dependencies. This set the target, not release evidence. | Main |
| [`6fdb018f11`](https://github.com/OpenAgentsInc/openagents/commit/6fdb018f111a493d30f0b5c7a548c95b883438c8) | 2026-09-21 03:17:04 | **Add a tested runbook for supervised Coder delegation to Devin.** Make local and hosted delegation reproducible, including credentials, relay setup, smoke tests, and supervision. The second hash updates the integrated version; it is not a second runbook delivered. | Retained |
| [`f7e55a727e`](https://github.com/OpenAgentsInc/openagents/commit/f7e55a727e97ff76ff272b9760c680adf5f6fc3e) | 2026-09-21 03:17:04 | **Add a tested runbook for supervised Coder delegation to Devin.** Make local and hosted delegation reproducible, including credentials, relay setup, smoke tests, and supervision. The second hash updates the integrated version; it is not a second runbook delivered. | Retained |
| [`1e84bb108a`](https://github.com/OpenAgentsInc/openagents/commit/1e84bb108a5c03f1d865bbcf12d9572f282e5eb5) | 2026-09-21 03:22:31 | **Add a tested runbook for supervised Coder delegation to Devin.** Make local and hosted delegation reproducible, including credentials, relay setup, smoke tests, and supervision. The second hash updates the integrated version; it is not a second runbook delivered. | Main |
| [`2d0cc96f96`](https://github.com/OpenAgentsInc/openagents/commit/2d0cc96f96304fc6bccbd0523ac1c2c8dad6bf26) | 2026-09-21 03:44:41 | **Refill delegation slots as individual tasks finish.** Remove batch-wide waiting so a free executor slot can take the next task. This improved scheduling mechanics without proving issue completion. | Main |
| [`3fd53030d1`](https://github.com/OpenAgentsInc/openagents/commit/3fd53030d11e53b896a555567cda3d8d78988990) | 2026-09-21 03:55:17 | **Add coder-scheduler: the deterministic backlog-scheduling core.** Move readiness, dependencies, claims, and resource bounds into deterministic Rust scheduling instead of agent judgment. This was enabling infrastructure. | Retained |
| [`4c0cb67e0b`](https://github.com/OpenAgentsInc/openagents/commit/4c0cb67e0bcbb0ff0020a8b062ec0bfcf955c64f) | 2026-09-21 03:56:30 | **Add coder-scheduler: the deterministic backlog-scheduling core.** Move readiness, dependencies, claims, and resource bounds into deterministic Rust scheduling instead of agent judgment. This was enabling infrastructure. | Retained |
| [`b73038c5b5`](https://github.com/OpenAgentsInc/openagents/commit/b73038c5b59999f214aeae32baeac59a1091a9cf) | 2026-09-21 03:57:45 | **Add coder-scheduler: the deterministic backlog-scheduling core.** Move readiness, dependencies, claims, and resource bounds into deterministic Rust scheduling instead of agent judgment. This was enabling infrastructure. | Main |
| [`5de4418e89`](https://github.com/OpenAgentsInc/openagents/commit/5de4418e89d7c2c39ba3316a36951bb4eea61456) | 2026-09-21 04:04:59 | **Preserve the bounded program-authority draft for integration review.** Retain the delegated authorization draft for review and integration. Preservation prevented loss but did not mean acceptance. | Retained |
| [`64986666f6`](https://github.com/OpenAgentsInc/openagents/commit/64986666f6bf5b35d5794fb065b8b8e6e651b693) | 2026-09-21 04:05:00 | **Preserve the bounded tracker-intake draft for integration review.** Retain the delegated tracker-intake draft for review and integration. It still needed host validation. | Retained |
| [`aa94cba091`](https://github.com/OpenAgentsInc/openagents/commit/aa94cba091b8d090c99ade34bf348c3c292a9eb7) | 2026-09-21 04:05:00 | **Preserve the bounded program-authority draft for integration review.** Retain the delegated authorization draft for review and integration. Preservation prevented loss but did not mean acceptance. | Main |
| [`b4cc74a736`](https://github.com/OpenAgentsInc/openagents/commit/b4cc74a736c0f26242fb2e1fc052244c0d6176ce) | 2026-09-21 04:05:00 | **Preserve the bounded tracker-intake draft for integration review.** Retain the delegated tracker-intake draft for review and integration. It still needed host validation. | Main |
| [`a0a71cf1cd`](https://github.com/OpenAgentsInc/openagents/commit/a0a71cf1cd3d387602a2299391db9ac7de7ede7d) | 2026-09-21 04:29:25 | **Integrate scoped project intake and durable Coder delegation.** Connect project intake to durable claims and Coder execution so the backlog could drive dispatch. Integration and recovery consumed time before issue delivery. | Main |
| [`b5a7021751`](https://github.com/OpenAgentsInc/openagents/commit/b5a70217515a33bb91771fea7aaade4b72209551) | 2026-09-21 04:29:25 | **Merge remote-tracking branch 'origin/main' into codex/coder-devin-runbook.** Bring concurrently published main changes into the integration checkout. These merge commits synchronize history; they are not new features. | Main |
| [`00a3e990d0`](https://github.com/OpenAgentsInc/openagents/commit/00a3e990d0a1fbde5e2cf94aeb72ccb4e002933b) | 2026-09-21 04:35:48 | **Record project supervisor benchmarks and live queue evidence.** Publish scheduler simulations and live queue observations so readiness claims had evidence. These measured the harness, not completed product issues. | Main |
| [`ac7d5427af`](https://github.com/OpenAgentsInc/openagents/commit/ac7d5427af0a051523c1976563d4f9b852fe191a) | 2026-09-21 04:37:34 | **Track dynamic program authorization with the shared client.** Record remaining dynamic authorization work rather than presenting the current shared client as complete. Documentation exposed an outstanding boundary. | Main |
| [`a1ec53d254`](https://github.com/OpenAgentsInc/openagents/commit/a1ec53d2545336948c9ea62ff253d099e3711adb) | 2026-09-21 04:42:22 | **Use an open verification issue in the project supervisor example.** Correct a stale example to reference an open verification issue. This was a small documentation repair, not new capability. | Main |
| [`cb95ea3212`](https://github.com/OpenAgentsInc/openagents/commit/cb95ea3212caabad50174d53e60ba2969c6115ef) | 2026-09-21 04:48:51 | **Add the classification envelope's first bounded slice.** Establish the bounded classification request envelope and planning rules. This left runtime behavior and other acceptance items outstanding. | Retained |
| [`94c2187a79`](https://github.com/OpenAgentsInc/openagents/commit/94c2187a7960844daa00ff2ced1e4430a21b4b92) | 2026-09-21 04:49:56 | **Specify the relay decision-job contract (proposed).** Define the proposed relay wire contract before implementation. A protocol proposal alone could not complete the transport issue. | Retained |
| [`dcb62a9703`](https://github.com/OpenAgentsInc/openagents/commit/dcb62a9703f766dd08183b3fba1dcf4919e26a57) | 2026-09-21 04:51:26 | **tenancy: add the backend capability record.** Give discovery and admission a typed backend-capability record. This was a foundation, not live capability proof. | Retained |
| [`839d8cf408`](https://github.com/OpenAgentsInc/openagents/commit/839d8cf4086b8376fdc19a10a7eb087d72a2265b) | 2026-09-21 05:02:52 | **Bind retained artifacts to bounded host verification checks.** Require host checks tied to retained artifacts rather than trusting an executor's success message. Necessary for trustworthy acceptance, but another harness increment. | Main |
| [`7867a09e91`](https://github.com/OpenAgentsInc/openagents/commit/7867a09e919422bb836ce48f9d969ba553ba5c92) | 2026-09-21 05:05:58 | **Add the classification envelope's first bounded slice.** Establish the bounded classification request envelope and planning rules. This left runtime behavior and other acceptance items outstanding. | Main |
| [`86c8efea66`](https://github.com/OpenAgentsInc/openagents/commit/86c8efea663d0609dd57ef4379f5b553a3f331a7) | 2026-09-21 05:05:58 | **Specify the relay decision-job contract (proposed).** Define the proposed relay wire contract before implementation. A protocol proposal alone could not complete the transport issue. | Main |
| [`baece84cfe`](https://github.com/OpenAgentsInc/openagents/commit/baece84cfefed901afcb390c9e36f315a8f7468a) | 2026-09-21 05:05:58 | **tenancy: add the backend capability record.** Give discovery and admission a typed backend-capability record. This was a foundation, not live capability proof. | Main |
| [`81ae574bce`](https://github.com/OpenAgentsInc/openagents/commit/81ae574bcedb572530be1a2291d7f2021186d52d) | 2026-09-21 05:09:02 | **Require explicit backend limits and bind relay cancellation to its caller.** Fail closed on undeclared backend limits and prevent cancellation by a different caller. These corrected admission and ownership gaps in the initial slices. | Main |
| [`e85b0d1e9a`](https://github.com/OpenAgentsInc/openagents/commit/e85b0d1e9ac3260f9380e3da67610e9b1430756b) | 2026-09-21 05:22:01 | **Add durable workspace holds and fixed-point monetary accounting.** Represent workspace spending with durable reservations and fixed-point amounts. Ledger support alone did not finish gateway charging integration. | Main |
| [`823006f7d6`](https://github.com/OpenAgentsInc/openagents/commit/823006f7d61f6fcbeff1775a0d64048231915e99) | 2026-09-21 05:23:57 | **Correct the NIP-44 padding property at power-of-two boundaries.** Correct an erroneous NIP-44 padding property exposed by the gate at a power-of-two boundary. This was gate-unblocking regression repair outside the active feature. | Main |
| [`b1df4df7f9`](https://github.com/OpenAgentsInc/openagents/commit/b1df4df7f9bd84ee6bfdd57a46da66ad489fea4f) | 2026-09-21 05:33:30 | **Support explicit credential-free loopback decision clients.** Allow explicitly configured local loopback decision services without manufacturing bearer credentials. This enabled local dogfooding but not the whole client issue. | Main |
| [`5684a2409b`](https://github.com/OpenAgentsInc/openagents/commit/5684a2409b2ed8a51d5df4c244254ed8ce9bea12) | 2026-09-21 05:34:57 | **Run a pinned suite through the host verification path.** Connect pinned suite execution to bounded host verification. The first path still needed CLI and Gym adapter integration. | Retained |
| [`764832e90a`](https://github.com/OpenAgentsInc/openagents/commit/764832e90a2cb2841e99b738983d372667be4663) | 2026-09-21 05:38:38 | **nostr: add the pure NIP-CJ decision-job protocol layer.** Implement pure decision-job protocol encoding and validation for the relay lane. The worker execution path remained separate. | Retained |
| [`9c9c30f344`](https://github.com/OpenAgentsInc/openagents/commit/9c9c30f34487072b0dcc1a69f219b36922f36c1b) | 2026-09-21 05:42:13 | **Run a pinned suite through the host verification path.** Connect pinned suite execution to bounded host verification. The first path still needed CLI and Gym adapter integration. | Main |
| [`7c75e4f944`](https://github.com/OpenAgentsInc/openagents/commit/7c75e4f9448d6ea27fdb39b4aa362bdc86b42d34) | 2026-09-21 05:44:17 | **Preserve classification draft from timed-out delegation for review.** Save a timed-out classification draft before inspecting it. The timeout was not successful delivery; later commits repaired and tested the draft. | Retained |
| [`e60dd2512f`](https://github.com/OpenAgentsInc/openagents/commit/e60dd2512f70253fa5634b4ddee6ed43094a8b9a) | 2026-09-21 05:52:33 | **Expose bounded suite verification through the project CLI.** Make bounded suite checks accessible through the project CLI. This exposed existing verification work rather than finishing every verification backend. | Main |
| [`8dd1a79e95`](https://github.com/OpenAgentsInc/openagents/commit/8dd1a79e95baffb3158cb39411f66ab277bfefd2) | 2026-09-21 05:54:07 | **nostr: add the pure NIP-CJ decision-job protocol layer.** Implement pure decision-job protocol encoding and validation for the relay lane. The worker execution path remained separate. | Main |
| [`417df7cb8e`](https://github.com/OpenAgentsInc/openagents/commit/417df7cb8e18006a7e575691df4617c4ab05858a) | 2026-09-21 06:05:25 | **Bound classification responses and preserve incomplete accounting.** Cap backend responses and preserve incomplete or unknown usage rather than declaring successful accounting. This repaired the recovered classification draft. | Retained |
| [`48fe380751`](https://github.com/OpenAgentsInc/openagents/commit/48fe38075147752d1a3025fdf486d84721efa11d) | 2026-09-21 06:05:30 | **Bind relay receipts to sealed request and attempt identities.** Tie relay receipt claims to their request and attempt so unrelated evidence could not be substituted. Protocol integrity did not establish a running decision worker. | Main |
| [`1c445e2488`](https://github.com/OpenAgentsInc/openagents/commit/1c445e2488cc11f1d94c85c448416bd3aaeeaf67) | 2026-09-21 06:05:40 | **Bound classification responses and preserve incomplete accounting.** Cap backend responses and preserve incomplete or unknown usage rather than declaring successful accounting. This repaired the recovered classification draft. | Main |
| [`e1275d9b40`](https://github.com/OpenAgentsInc/openagents/commit/e1275d9b405bfbd073a2c1614e39443716ba0419) | 2026-09-21 06:05:40 | **Preserve classification draft from timed-out delegation for review.** Save a timed-out classification draft before inspecting it. The timeout was not successful delivery; later commits repaired and tested the draft. | Main |
| [`17f1ea8495`](https://github.com/OpenAgentsInc/openagents/commit/17f1ea84951979841500ebc9a3abd2a1fd4bb0cc) | 2026-09-21 06:16:36 | **Verify classification outcomes through the native answer contract.** Check classification output against native answer semantics instead of accepting arbitrary backend JSON. This strengthened the facade's contract evidence. | Main |
| [`9dd4ddab67`](https://github.com/OpenAgentsInc/openagents/commit/9dd4ddab67485df8db75fdacc5c52f1d19cd414b) | 2026-09-21 06:16:50 | **Merge remote-tracking branch 'origin/main' into codex/coder-devin-runbook.** Bring concurrently published main changes into the integration checkout. These merge commits synchronize history; they are not new features. | Main |
| [`07d7c1c31b`](https://github.com/OpenAgentsInc/openagents/commit/07d7c1c31ba2996f6cbee43813b84d434c3a3ad8) | 2026-09-21 06:29:33 | **Share explicit decision profiles across Coder callers.** Reuse explicit decision profiles across headless and terminal callers. It reduced divergent client behavior but left broader decision-site adoption open. | Main |
| [`cf8bb31053`](https://github.com/OpenAgentsInc/openagents/commit/cf8bb31053ce8b618cc3439f3f061bde066781c7) | 2026-09-21 06:29:48 | **Merge remote-tracking branch 'origin/main' into codex/coder-devin-runbook.** Bring concurrently published main changes into the integration checkout. These merge commits synchronize history; they are not new features. | Main |
| [`1b12564be3`](https://github.com/OpenAgentsInc/openagents/commit/1b12564be3b226c518f6e8ba58344ad2efb59269) | 2026-09-21 06:36:46 | **Name the program scoreability check without implying calibration.** Remove a calibration implication from a scoreability check's name. A terminology correction avoided claiming evidence the check did not provide. | Main |
| [`9d78ef7544`](https://github.com/OpenAgentsInc/openagents/commit/9d78ef75443d10ba3cb4de3195c83968481eb72c) | 2026-09-21 06:42:04 | **Add binary filtering and rubric ranking to classification.** Add binary Noul filtering and rubric Score ranking to the facade. These completed modes while leaving other classification acceptance items unfinished. | Retained |
| [`aea9528956`](https://github.com/OpenAgentsInc/openagents/commit/aea9528956192548f6abf9d15fe8a5630b3aa995) | 2026-09-21 06:43:05 | **Bound repository context capture and preserve UTF-8 excerpts.** Bound repository excerpts and keep valid UTF-8 at truncation boundaries. This protected context capture but did not complete end-to-end repository evidence use. | Main |
| [`5cc0d33bab`](https://github.com/OpenAgentsInc/openagents/commit/5cc0d33babf8636aa15d88c74cb29541694f4ec9) | 2026-09-21 06:43:07 | **Add binary filtering and rubric ranking to classification.** Add binary Noul filtering and rubric Score ranking to the facade. These completed modes while leaving other classification acceptance items unfinished. | Main |
| [`99ef9ac150`](https://github.com/OpenAgentsInc/openagents/commit/99ef9ac15025046b452d33d19491c58f02000bcf) | 2026-09-21 06:58:28 | **Verify classification ranking ties and invalid rubric answers.** Test ranking ties and malformed rubric answers to pin deterministic selection and refusal behavior. These were meaningful contract regressions for the new modes. | Main |
| [`f5f9645da4`](https://github.com/OpenAgentsInc/openagents/commit/f5f9645da49776f5c6729a279c17b519a22519fd) | 2026-09-21 06:58:28 | **Stage referenced backup blobs before archiving concurrent mutations.** Stage backup blobs before concurrent changes could invalidate archive references. This fixed a failing gate dependency; it was not Decision Router feature progress. | Main |
| [`b0cbef7aca`](https://github.com/OpenAgentsInc/openagents/commit/b0cbef7aca4832d9b4b08cb5ea2ca0acc7a39d76) | 2026-09-21 07:10:21 | **Measure pinned Gym suites through the approved verification adapter.** Execute pinned Gym suites through an approved adapter instead of implying all suite families were available. This advanced verification integration, with explicit limitations. | Main |
| [`b82b0206cf`](https://github.com/OpenAgentsInc/openagents/commit/b82b0206cf130470b2ca4a6a21474c4e57679ce2) | 2026-09-21 07:10:22 | **Merge remote-tracking branch 'origin/main' into codex/gym-suite-adapter.** Reconcile the Gym adapter checkout with concurrently updated main. The merge itself added no independent product capability. | Main |
| [`313c66eee3`](https://github.com/OpenAgentsInc/openagents/commit/313c66eee32e8ec8de0a5644aed111ec1096521b) | 2026-09-21 07:33:14 | **Record bounded repository excerpts with source identities.** Attach source identity to bounded excerpts so later decisions could explain which repository material they used. This was one part of the evidence chain. | Main |
| [`17e4bb1006`](https://github.com/OpenAgentsInc/openagents/commit/17e4bb1006a0d743e281094c4f4cd2a3e929c88f) | 2026-09-21 07:33:26 | **classify: per-unit label counts and declared-cut uncertainty flags.** Expose aggregate label counts and caller-declared uncertainty cuts. This supported consumers without asking an agent to recount results. | Main |
| [`63e9bc20af`](https://github.com/OpenAgentsInc/openagents/commit/63e9bc20af059a57b73e4816e6ebd4d6b4b652f8) | 2026-09-21 07:33:26 | **classify: aggregate tests and HTTP contract documentation.** Test aggregate semantics and document the HTTP shape. These checks supported the preceding implementation rather than a separate completed issue. | Main |
| [`c8c80d02f6`](https://github.com/OpenAgentsInc/openagents/commit/c8c80d02f686919f075b77cfbe6eefff98c51b98) | 2026-09-21 07:33:26 | **classify: compute the binary selection once; fix an `unevaluated` claim.** Compute binary selection once and correct an unsupported unevaluated claim. This cleaned up duplicated logic and inaccurate reporting in the aggregate work. | Main |
| [`b5fc8d2453`](https://github.com/OpenAgentsInc/openagents/commit/b5fc8d24536cdd6695c8bca5beb81da19852bbf8) | 2026-09-21 07:38:07 | **Bind generation traces to captured repository evidence.** Carry captured source evidence into generation traces. This closed a missing connection between capture and attribution, while the wider context workflow remained open. | Retained |
| [`c8cc1709b2`](https://github.com/OpenAgentsInc/openagents/commit/c8cc1709b28a2ea3d4bbadba90ecf353e47e29dd) | 2026-09-21 07:40:37 | **Bind generation traces to captured repository evidence.** Carry captured source evidence into generation traces. This closed a missing connection between capture and attribution, while the wider context workflow remained open. | Main |
| [`a274d37bf5`](https://github.com/OpenAgentsInc/openagents/commit/a274d37bf5bb99a11594a6fd7e5d11cadbc991d7) | 2026-09-21 07:43:34 | **Update verification coverage for the shipped Gym adapter.** Update documentation to describe the actual shipped Gym adapter. This prevented stale coverage claims after implementation. | Retained |
| [`930322b2f7`](https://github.com/OpenAgentsInc/openagents/commit/930322b2f7de84ecfa4a99d0c781904554513f13) | 2026-09-21 07:44:20 | **Use a blocking accepted socket in the local decision fixture.** Fix the local fixture's accepted socket mode so a test did not fail on nonblocking reads. This was test infrastructure repair during a gate. | Main |
| [`db6d78f537`](https://github.com/OpenAgentsInc/openagents/commit/db6d78f53781bb8f50c1d628bee372b688512681) | 2026-09-21 07:44:20 | **Update verification coverage for the shipped Gym adapter.** Update documentation to describe the actual shipped Gym adapter. This prevented stale coverage claims after implementation. | Main |
| [`840c2c07a4`](https://github.com/OpenAgentsInc/openagents/commit/840c2c07a48d697f01982e75bb37fdf053696e2c) | 2026-09-21 07:46:30 | **Document captured context traces and current suite support.** Align context-trace and suite-support documentation with the current code. Another prose correction after adjacent implementation changes. | Retained |
| [`e7efc661a2`](https://github.com/OpenAgentsInc/openagents/commit/e7efc661a2110f1898567c465e7843e57bf52223) | 2026-09-21 07:48:25 | **Document captured context traces and current suite support.** Align context-trace and suite-support documentation with the current code. Another prose correction after adjacent implementation changes. | Main |
| [`c8255721b5`](https://github.com/OpenAgentsInc/openagents/commit/c8255721b5f8d77617ea533a6eb7c7d2ccfecfae) | 2026-09-21 07:50:17 | **tenancy: workspace membership and authorization core.** Add workspace membership and authorization primitives to tenancy. Gateway enforcement and provisioning hardening still remained. | Main |
| [`e852a0a003`](https://github.com/OpenAgentsInc/openagents/commit/e852a0a003b45621f45d41f046302b1de8c6d5f5) | 2026-09-21 07:58:04 | **Harden workspace provisioning and membership persistence.** Repair provisioning and persistence boundaries in the delegated membership implementation. This was host follow-up to incomplete integration quality. | Main |
| [`221c7c027b`](https://github.com/OpenAgentsInc/openagents/commit/221c7c027b750b64a2eb28a26a72c3bf04eb9855) | 2026-09-21 08:06:06 | **gateway: bounded per-item scheduling for POST /v1/classify.** Schedule per-input classification forwards with bounded concurrency. This implemented host fan-out, not efficient model packing. | Retained |
| [`da31a30bb1`](https://github.com/OpenAgentsInc/openagents/commit/da31a30bb198eeb3a2a8ff866a854bb6404b3e1a) | 2026-09-21 08:06:06 | **docs: the classify scheduling contract and its measured fixture.** Document scheduler behavior and synthetic fixture measurements without converting them into production throughput claims. | Retained |
| [`dc7564d17c`](https://github.com/OpenAgentsInc/openagents/commit/dc7564d17c83eb5df358619842c0caf1fdb4981e) | 2026-09-21 08:08:19 | **Authenticate workspace membership against the key tenant.** Bind membership checks to the authenticated key tenant. This prevented cross-tenant authority from being inferred from an unrelated workspace identifier. | Main |
| [`425034b26f`](https://github.com/OpenAgentsInc/openagents/commit/425034b26f5d3a0f09131a21e6958b48f077bb77) | 2026-09-21 08:09:03 | **Bound queued classification inputs and use one execution deadline.** Bound queued inputs and share one execution deadline so queued work could not multiply the requested runtime. This repaired the initial scheduler's resource semantics. | Retained |
| [`1784ebe90e`](https://github.com/OpenAgentsInc/openagents/commit/1784ebe90e5cd16f54883a7a691c243a286857ff) | 2026-09-21 08:09:17 | **gateway: bounded per-item scheduling for POST /v1/classify.** Schedule per-input classification forwards with bounded concurrency. This implemented host fan-out, not efficient model packing. | Main |
| [`47a09d457e`](https://github.com/OpenAgentsInc/openagents/commit/47a09d457e7bc62f00d570f518aef10cf5c7f5ba) | 2026-09-21 08:09:17 | **Bound queued classification inputs and use one execution deadline.** Bound queued inputs and share one execution deadline so queued work could not multiply the requested runtime. This repaired the initial scheduler's resource semantics. | Main |
| [`ac507233a4`](https://github.com/OpenAgentsInc/openagents/commit/ac507233a4b0e2dd1c686768403ca66f727ccaf8) | 2026-09-21 08:09:17 | **docs: the classify scheduling contract and its measured fixture.** Document scheduler behavior and synthetic fixture measurements without converting them into production throughput claims. | Main |
| [`b10e32fa35`](https://github.com/OpenAgentsInc/openagents/commit/b10e32fa35e07c6ca04dd61046cd7f6bee47d21d) | 2026-09-21 08:17:22 | **Enforce optional workspace membership at gateway admission.** Apply optional workspace membership in gateway admission. This connected tenancy primitives to requests but did not complete all workspace product features. | Main |
| [`c72162e3b3`](https://github.com/OpenAgentsInc/openagents/commit/c72162e3b3b569bc6bd0e6342822cf25ad02b81e) | 2026-09-21 08:17:22 | **Clarify scheduling fixture latency coverage.** Clarify what the scheduling latency fixture actually measured. Synthetic timing was not backend-quality or production-speed evidence. | Main |
| [`908b0b1a60`](https://github.com/OpenAgentsInc/openagents/commit/908b0b1a60f1c187ffb913bf998f72aefbe135f1) | 2026-09-21 08:29:15 | **Publish configured classification limits in model discovery.** Expose configured classification bounds before a caller submits work. This advanced discovery while total expanded context limits remained unfinished. | Main |
| [`1a77d4f6ec`](https://github.com/OpenAgentsInc/openagents/commit/1a77d4f6ecd81e54af7299f06bf950aa6a12aa7a) | 2026-09-21 08:31:14 | **Preserve incomplete review workflow after executor deadline.** Preserve the review-workflow draft after its executor deadline. The retained artifact required host repairs and was not an accepted completion. | Retained |
| [`1c023d4272`](https://github.com/OpenAgentsInc/openagents/commit/1c023d42723ef41e052695b901fe738f9c98cf14) | 2026-09-21 08:32:49 | **Preserve incomplete review workflow after executor deadline.** Preserve the review-workflow draft after its executor deadline. The retained artifact required host repairs and was not an accepted completion. | Main |
| [`86654843fe`](https://github.com/OpenAgentsInc/openagents/commit/86654843fedbbeadeb92ee7fe52cc6215334f9db) | 2026-09-21 08:39:40 | **Repair review evidence parsing and preserve unanswered findings.** Repair review evidence parsing and retain unanswered findings rather than treating missing answers as approval. This corrected the recovered workflow. | Main |
| [`8b3231680d`](https://github.com/OpenAgentsInc/openagents/commit/8b3231680d9326dab1a2bd9f932c9a757947d8fa) | 2026-09-21 08:39:40 | **Format recovered review question templates.** Format recovered question templates to meet repository formatting requirements. It changed presentation, not review policy. | Main |
| [`c3d654f853`](https://github.com/OpenAgentsInc/openagents/commit/c3d654f8532cdf1a9140c3a3bd609006e356a46e) | 2026-09-21 08:46:48 | **Run pinned artifact reviews through the Coder program harness.** Run reviews through the same pinned Coder program machinery used by other workflows. This connected the repaired reviewer to the harness. | Main |
| [`777617e0d4`](https://github.com/OpenAgentsInc/openagents/commit/777617e0d48be650e0106e1a8fb48d1a55547f56) | 2026-09-21 08:51:34 | **Preserve incomplete candidate admission after executor deadline.** Preserve the incomplete admission draft after timeout, including its source and integration copies. Recovery was necessary, but exposed delegation's failure to return finished work. | Retained |
| [`dd1d9b1d7c`](https://github.com/OpenAgentsInc/openagents/commit/dd1d9b1d7c5e9d4b48c24965581574bc81c60463) | 2026-09-21 08:53:28 | **Preserve incomplete candidate admission after executor deadline.** Preserve the incomplete admission draft after timeout, including its source and integration copies. Recovery was necessary, but exposed delegation's failure to return finished work. | Retained |
| [`45a6e20097`](https://github.com/OpenAgentsInc/openagents/commit/45a6e200978fa105b98341c4681e5fabcbf70892) | 2026-09-21 09:01:21 | **Format recovered admission commands and manifest fields.** Format recovered admission CLI and manifest code separately from behavioral repair. This addressed mechanical gate requirements. | Retained |
| [`8a924ac8ef`](https://github.com/OpenAgentsInc/openagents/commit/8a924ac8ef022c14f93233754ef312593eb4c1fa) | 2026-09-21 09:02:45 | **Require native evidence replay and serialize candidate activation.** Replay native evidence and serialize activation so admission could not rely on unchecked claims or race another activation. | Retained |
| [`e23730d7cf`](https://github.com/OpenAgentsInc/openagents/commit/e23730d7cf1bf822d5814061f8ee1424c769db8c) | 2026-09-21 09:03:09 | **Preserve incomplete classification callers after executor deadline.** Preserve timed-out classification caller work for independent repair. A saved draft was not CLI/MCP acceptance. | Retained |
| [`ed9c2da582`](https://github.com/OpenAgentsInc/openagents/commit/ed9c2da58236eb9b0611c519176ee9e4a8fa4393) | 2026-09-21 09:03:21 | **Preserve incomplete classification callers after executor deadline.** Preserve timed-out classification caller work for independent repair. A saved draft was not CLI/MCP acceptance. | Retained |
| [`25fbfb264c`](https://github.com/OpenAgentsInc/openagents/commit/25fbfb264cdd8c24e7baec2282440f765b5e47f9) | 2026-09-21 09:04:54 | **Format recovered classification caller implementation.** Format the recovered caller implementation. This was mechanical integration work after delegation. | Retained |
| [`f8ba9b8df5`](https://github.com/OpenAgentsInc/openagents/commit/f8ba9b8df5d96829f5ab098c196cafc2b158ec0d) | 2026-09-21 09:04:54 | **Repair classification fixture score types and distributions.** Correct score fixture types and probability distributions so tests exercised the real native contract rather than invalid examples. | Retained |
| [`5411e05471`](https://github.com/OpenAgentsInc/openagents/commit/5411e0547136e662c011ebd860aec601b86c34d1) | 2026-09-21 09:13:45 | **Verify classification CLI and MCP parity with bounded retries.** Verify CLI/MCP classification parity and retry bounds against gateway behavior. These checks supported caller compatibility for the facade. | Main |
| [`5891ff7254`](https://github.com/OpenAgentsInc/openagents/commit/5891ff7254a187f79c759b91903f69432262d4a4) | 2026-09-21 09:13:45 | **Repair classification fixture score types and distributions.** Correct score fixture types and probability distributions so tests exercised the real native contract rather than invalid examples. | Main |
| [`6526cd2e5f`](https://github.com/OpenAgentsInc/openagents/commit/6526cd2e5fb31ed0d67e05fb67ba46caf76f5c40) | 2026-09-21 09:13:45 | **Verify classification CLI and MCP parity with bounded retries.** Verify CLI/MCP classification parity and retry bounds against gateway behavior. These checks supported caller compatibility for the facade. | Retained |
| [`a7bf2e6673`](https://github.com/OpenAgentsInc/openagents/commit/a7bf2e6673cc7e5d2b5b960c964e7c592729e198) | 2026-09-21 09:13:45 | **Preserve incomplete classification callers after executor deadline.** Preserve timed-out classification caller work for independent repair. A saved draft was not CLI/MCP acceptance. | Main |
| [`d5d9609cfc`](https://github.com/OpenAgentsInc/openagents/commit/d5d9609cfccf6c80f0054851b9178f9f35a00ca6) | 2026-09-21 09:13:45 | **Format recovered classification caller implementation.** Format the recovered caller implementation. This was mechanical integration work after delegation. | Main |
| [`845bf7116b`](https://github.com/OpenAgentsInc/openagents/commit/845bf7116bfd0da0e1eaacdeb830de095eaf6565) | 2026-09-21 09:14:53 | **Keep caller admission fixtures within strict Clippy bounds.** Refactor caller admission fixtures to satisfy strict Clippy. This was a small gate failure repair after integration. | Main |
| [`dc28acd41e`](https://github.com/OpenAgentsInc/openagents/commit/dc28acd41ed674ac6b7b276a8f04dde191c10747) | 2026-09-21 09:24:06 | **Reject repeated locked exposure and incomplete transfer coverage.** Reject reuse of locked evaluation exposure and incomplete transfer evidence. This tightened admission against evidence leakage and coverage gaps. | Retained |
| [`e24151e706`](https://github.com/OpenAgentsInc/openagents/commit/e24151e706fda0a9b245df962bc90d7f75be2b5d) | 2026-09-21 09:26:20 | **Revalidate admission suites and require declared question pins.** Reload and validate suites and require question pins rather than trusting a previously loaded description. This made admission evidence reproducible. | Retained |
| [`5b95e2f635`](https://github.com/OpenAgentsInc/openagents/commit/5b95e2f6352648339561b761f342010b1e6ab9ec) | 2026-09-21 09:28:48 | **Require full coverage for every declared admission seed block.** Require coverage for every declared seed block so a partial run could not masquerade as full admission evidence. | Retained |
| [`dbd6ae3224`](https://github.com/OpenAgentsInc/openagents/commit/dbd6ae322490029440a7d94a30be6c8dc23c15a5) | 2026-09-21 09:29:48 | **Preserve monetary gateway delegation artifact for independent review.** Save the monetary gateway artifact for independent review. Subsequent duplicate-dispatch and release repairs demonstrate that preservation was not acceptance. | Retained |
| [`74bde0cafc`](https://github.com/OpenAgentsInc/openagents/commit/74bde0cafc90e26b9aa227bbc4aa54ff4676d089) | 2026-09-21 09:31:08 | **Refuse duplicate monetary dispatch and preserve original reservations.** Reject duplicate monetary dispatch without overwriting original reservations. This fixed retry behavior that could otherwise spend or account twice. | Retained |
| [`a1801575bb`](https://github.com/OpenAgentsInc/openagents/commit/a1801575bb788524c43c641a60423484b61bb5e7) | 2026-09-21 09:32:37 | **Report the actual monetary release result.** Report the actual hold-release outcome rather than implying release always succeeded. This corrected operator-visible accounting evidence. | Retained |
| [`a1e6148820`](https://github.com/OpenAgentsInc/openagents/commit/a1e614882027976dd5530c66ff4a766d6af264ce) | 2026-09-21 09:32:44 | **Refuse duplicate monetary dispatch and preserve original reservations.** Reject duplicate monetary dispatch without overwriting original reservations. This fixed retry behavior that could otherwise spend or account twice. | Main |
| [`a810d2c9df`](https://github.com/OpenAgentsInc/openagents/commit/a810d2c9df1909c894939ad4c9693dd64bb0d543) | 2026-09-21 09:32:44 | **Preserve monetary gateway delegation artifact for independent review.** Save the monetary gateway artifact for independent review. Subsequent duplicate-dispatch and release repairs demonstrate that preservation was not acceptance. | Main |
| [`cb736e04f2`](https://github.com/OpenAgentsInc/openagents/commit/cb736e04f203757c63dd8ef92bcdabb24a0f17cb) | 2026-09-21 09:32:44 | **Report the actual monetary release result.** Report the actual hold-release outcome rather than implying release always succeeded. This corrected operator-visible accounting evidence. | Main |
| [`cf5e87e814`](https://github.com/OpenAgentsInc/openagents/commit/cf5e87e814f8166052539af0a476d59f6f7d77c2) | 2026-09-21 09:33:14 | **Group monetary binding identity for strict lint checks.** Group related monetary identity fields to satisfy strict lint checks without changing the accounting policy. | Main |
| [`4b31c69d37`](https://github.com/OpenAgentsInc/openagents/commit/4b31c69d370f55ef95bfe140dc9a7af8188f2e7d) | 2026-09-21 09:35:15 | **Test admission improvement outcomes and isolate transfer variance.** Exercise improvement, tie, and failure outcomes and separate transfer variance. This expanded admission tests toward full acceptance rather than another product surface. | Retained |
| [`f460ae2197`](https://github.com/OpenAgentsInc/openagents/commit/f460ae219720ccc29cc05802f59530596faedf53) | 2026-09-21 09:36:23 | **Verify transfer regressions use transfer-specific variance.** Use transfer-specific variance in regression checks. This fixed a statistical-input mismatch in admission evaluation. | Retained |
| [`587fa62fc8`](https://github.com/OpenAgentsInc/openagents/commit/587fa62fc8f1b6d711916427083452c418de2af9) | 2026-09-21 09:37:47 | **Keep unknown admission cost and incomplete timing evidence explicit.** Keep missing cost and timing evidence unknown instead of converting absence into a favorable result. This prevented unsupported admission claims. | Retained |
| [`c2e19040f4`](https://github.com/OpenAgentsInc/openagents/commit/c2e19040f404541d6d5abf8b657cd80818c74f8a) | 2026-09-21 09:39:06 | **Require a retained development commitment for admission CLI.** Require a retained development commitment before admission CLI use. This preserved separation between development selection and locked evaluation. | Retained |
| [`65286c8973`](https://github.com/OpenAgentsInc/openagents/commit/65286c89735e71fdbaf1af3963e4422a9fe611b9) | 2026-09-21 09:42:07 | **Verify admission report chains and retained selections.** Verify retained report chains and selections so admission referenced the actual recorded evidence. | Retained |
| [`0d6764a889`](https://github.com/OpenAgentsInc/openagents/commit/0d6764a889c87c33080b753845680237dae8ff54) | 2026-09-21 09:43:41 | **Require retained reports in the admission evaluator.** Require retained reports in the evaluator itself, not only in an outer caller. This closed an enforcement bypass. | Retained |
| [`cac7ecfc99`](https://github.com/OpenAgentsInc/openagents/commit/cac7ecfc99c3cfac15437fe3a4a13a3c5d3677ef) | 2026-09-21 09:45:14 | **Pass retained locked and transfer reports through admission CLI.** Pass locked and transfer reports through the CLI so the stricter evaluator could be used end to end. | Retained |
| [`e1349a9420`](https://github.com/OpenAgentsInc/openagents/commit/e1349a9420dcce2705798a4d4c0d66d85106667a) | 2026-09-21 09:47:37 | **Exercise complete retained-evidence admission and failure controls.** Exercise a complete retained-evidence path and its failure controls. This moved admission toward an issue-level acceptance proof. | Retained |
| [`6d86197d54`](https://github.com/OpenAgentsInc/openagents/commit/6d86197d54e53fc6c47d199807267f9ff2640cf5) | 2026-09-21 09:48:53 | **Verify admitted activation preserves tenancy and supports rollback.** Verify activation preserves tenant authority and supports rollback. This covered deployment behavior beyond the admission calculation. | Retained |
| [`7348c9faba`](https://github.com/OpenAgentsInc/openagents/commit/7348c9faba774dc6244cffaa4b4b3bad54d55fbb) | 2026-09-21 09:50:36 | **Accept explicit deployment evidence and document candidate admission.** Accept explicit deployment evidence and explain the admission procedure. This connected implementation and the operator's release steps. | Retained |
| [`ae722bfe1d`](https://github.com/OpenAgentsInc/openagents/commit/ae722bfe1dd5e1586ee125ce4e6c96f2952b50fa) | 2026-09-21 09:51:27 | **Assert family regression and artifact drift refuse admission.** Assert family regressions and artifact identity drift refuse admission. These were negative controls for the completed admission path. | Retained |
| [`105f16c149`](https://github.com/OpenAgentsInc/openagents/commit/105f16c149d15cc5bdb88d5dbd849237bbabd4ff) | 2026-09-21 09:51:36 | **Preserve incomplete candidate admission after executor deadline.** Preserve the incomplete admission draft after timeout, including its source and integration copies. Recovery was necessary, but exposed delegation's failure to return finished work. | Main |
| [`09e2aac97e`](https://github.com/OpenAgentsInc/openagents/commit/09e2aac97edb7c98cf8b868909b6f282d4807d4c) | 2026-09-21 09:51:37 | **Verify admitted activation preserves tenancy and supports rollback.** Verify activation preserves tenant authority and supports rollback. This covered deployment behavior beyond the admission calculation. | Main |
| [`15b72c6cd4`](https://github.com/OpenAgentsInc/openagents/commit/15b72c6cd42d08d5fd372ca5bb87a649652d0dca) | 2026-09-21 09:51:37 | **Accept explicit deployment evidence and document candidate admission.** Accept explicit deployment evidence and explain the admission procedure. This connected implementation and the operator's release steps. | Main |
| [`1b149d86de`](https://github.com/OpenAgentsInc/openagents/commit/1b149d86ded85207af770c571cf4f38b87039bee) | 2026-09-21 09:51:37 | **Format recovered admission commands and manifest fields.** Format recovered admission CLI and manifest code separately from behavioral repair. This addressed mechanical gate requirements. | Main |
| [`207dd7a36d`](https://github.com/OpenAgentsInc/openagents/commit/207dd7a36d1b0225e56150608bdbe8d4aa6302e1) | 2026-09-21 09:51:37 | **Revalidate admission suites and require declared question pins.** Reload and validate suites and require question pins rather than trusting a previously loaded description. This made admission evidence reproducible. | Main |
| [`325beda15f`](https://github.com/OpenAgentsInc/openagents/commit/325beda15ffacab9ab5d438d53f4749dcf105a61) | 2026-09-21 09:51:37 | **Pass retained locked and transfer reports through admission CLI.** Pass locked and transfer reports through the CLI so the stricter evaluator could be used end to end. | Main |
| [`6042ccbc18`](https://github.com/OpenAgentsInc/openagents/commit/6042ccbc182ad3ce17281902c05122bbd40b4b22) | 2026-09-21 09:51:37 | **Verify transfer regressions use transfer-specific variance.** Use transfer-specific variance in regression checks. This fixed a statistical-input mismatch in admission evaluation. | Main |
| [`661e949275`](https://github.com/OpenAgentsInc/openagents/commit/661e94927577d928c67858c5a893a67b18c80a4f) | 2026-09-21 09:51:37 | **Require a retained development commitment for admission CLI.** Require a retained development commitment before admission CLI use. This preserved separation between development selection and locked evaluation. | Main |
| [`6745298701`](https://github.com/OpenAgentsInc/openagents/commit/6745298701c3b0925b734c4df7425aae4cebf9fd) | 2026-09-21 09:51:37 | **Verify admission report chains and retained selections.** Verify retained report chains and selections so admission referenced the actual recorded evidence. | Main |
| [`6d9611bc88`](https://github.com/OpenAgentsInc/openagents/commit/6d9611bc88df33f59821cb9e5930aec91b10c811) | 2026-09-21 09:51:37 | **Reject repeated locked exposure and incomplete transfer coverage.** Reject reuse of locked evaluation exposure and incomplete transfer evidence. This tightened admission against evidence leakage and coverage gaps. | Main |
| [`72a92657ca`](https://github.com/OpenAgentsInc/openagents/commit/72a92657ca6a67b478dc810fccf228c0d2932918) | 2026-09-21 09:51:37 | **Exercise complete retained-evidence admission and failure controls.** Exercise a complete retained-evidence path and its failure controls. This moved admission toward an issue-level acceptance proof. | Main |
| [`81637669b8`](https://github.com/OpenAgentsInc/openagents/commit/81637669b8e5447bda061a26146b42eeb906081d) | 2026-09-21 09:51:37 | **Assert family regression and artifact drift refuse admission.** Assert family regressions and artifact identity drift refuse admission. These were negative controls for the completed admission path. | Main |
| [`a2216b5aac`](https://github.com/OpenAgentsInc/openagents/commit/a2216b5aac102228a072338682bf1a91c5f7e5ed) | 2026-09-21 09:51:37 | **Require full coverage for every declared admission seed block.** Require coverage for every declared seed block so a partial run could not masquerade as full admission evidence. | Main |
| [`c4be96c594`](https://github.com/OpenAgentsInc/openagents/commit/c4be96c5943d9feaf2291e51ff4369b62917cdb2) | 2026-09-21 09:51:37 | **Keep unknown admission cost and incomplete timing evidence explicit.** Keep missing cost and timing evidence unknown instead of converting absence into a favorable result. This prevented unsupported admission claims. | Main |
| [`e9fecdfb77`](https://github.com/OpenAgentsInc/openagents/commit/e9fecdfb77078d1d871528638f5d7c09e64d9095) | 2026-09-21 09:51:37 | **Require native evidence replay and serialize candidate activation.** Replay native evidence and serialize activation so admission could not rely on unchecked claims or race another activation. | Main |
| [`f73b57a2cc`](https://github.com/OpenAgentsInc/openagents/commit/f73b57a2cc89799495e779073c70f293ad4d9cdf) | 2026-09-21 09:51:37 | **Require retained reports in the admission evaluator.** Require retained reports in the evaluator itself, not only in an outer caller. This closed an enforcement bypass. | Main |
| [`faa47ee5ae`](https://github.com/OpenAgentsInc/openagents/commit/faa47ee5ae4293f8a76466aa61b20654351b1c63) | 2026-09-21 09:51:37 | **Test admission improvement outcomes and isolate transfer variance.** Exercise improvement, tie, and failure outcomes and separate transfer variance. This expanded admission tests toward full acceptance rather than another product surface. | Main |
| [`e875ab8531`](https://github.com/OpenAgentsInc/openagents/commit/e875ab853148aa0728a27faa818802aa0e29b9f1) | 2026-09-21 09:51:54 | **Declare empty admission scope in gateway base fixtures.** Update gateway fixtures with an explicit empty admission scope after the manifest contract changed. This was integration repair. | Main |
| [`e1f7d15d2e`](https://github.com/OpenAgentsInc/openagents/commit/e1f7d15d2ea799a03608ae300d954b7d00534cda) | 2026-09-21 09:55:56 | **Expose admitted scope and evidence references in authorized discovery.** Expose admitted scope and evidence references through authorized discovery so clients could inspect what had actually been admitted. | Retained |
| [`2ab3002ab2`](https://github.com/OpenAgentsInc/openagents/commit/2ab3002ab29ec256d26370208dfdbdf979b66785) | 2026-09-21 09:56:54 | **Align Decision API specification with candidate admission.** Update the Decision API spec to match the implemented admission contract rather than leave its earlier planned description. | Retained |
| [`83eeec996b`](https://github.com/OpenAgentsInc/openagents/commit/83eeec996b289166f7a6792234ba869538f70e23) | 2026-09-21 09:58:55 | **Require development and locked coverage for every admitted family.** Require both development and locked coverage for every admitted family. This addressed the final family-coverage acceptance gap for #9473. | Retained |
| [`12896b7f1f`](https://github.com/OpenAgentsInc/openagents/commit/12896b7f1fdcccfd9acbc818bcc1283eb158fcee) | 2026-09-21 10:02:29 | **Align Decision API specification with candidate admission.** Update the Decision API spec to match the implemented admission contract rather than leave its earlier planned description. | Main |
| [`7cf7e25eba`](https://github.com/OpenAgentsInc/openagents/commit/7cf7e25eba5550a0c709d23f6b03a3155bc1f6d2) | 2026-09-21 10:02:29 | **Require development and locked coverage for every admitted family.** Require both development and locked coverage for every admitted family. This addressed the final family-coverage acceptance gap for #9473. | Main |
| [`f82ecb04c1`](https://github.com/OpenAgentsInc/openagents/commit/f82ecb04c1056dcabc46967c254b7464d132c1c6) | 2026-09-21 10:02:29 | **Expose admitted scope and evidence references in authorized discovery.** Expose admitted scope and evidence references through authorized discovery so clients could inspect what had actually been admitted. | Main |
| [`5edbec6839`](https://github.com/OpenAgentsInc/openagents/commit/5edbec68396598b1cd6a38972c1cb3eeb61fbcd7) | 2026-09-21 10:07:22 | **Inventory Coder decision sites and their evidence boundaries.** Inventory existing Coder decision sites and distinguish implemented paths from unsupported evidence claims. Useful analysis, but it expanded work in progress while classification remained open. | Retained |
| [`4654a59451`](https://github.com/OpenAgentsInc/openagents/commit/4654a594513a2f895b80dd9cae5988d5a519d8cf) | 2026-09-21 10:13:27 | **Inventory Coder decision sites and their evidence boundaries.** Inventory existing Coder decision sites and distinguish implemented paths from unsupported evidence claims. Useful analysis, but it expanded work in progress while classification remained open. | Main |
| [`7057324796`](https://github.com/OpenAgentsInc/openagents/commit/70573247960c787795fdf9cd4abc299ac2ba7e69) | 2026-09-21 10:20:08 | **Publish the classification request schema and planner fixtures.** Publish a versioned classification request schema and planner corpus. This covered invalid and maximum shapes, but response and cancellation evidence were still missing. | Retained |
| [`3268508946`](https://github.com/OpenAgentsInc/openagents/commit/32685089462749f8c10b62b046bf757adc7da02a) | 2026-09-21 10:20:13 | **Publish the classification request schema and planner fixtures.** Publish a versioned classification request schema and planner corpus. This covered invalid and maximum shapes, but response and cancellation evidence were still missing. | Main |
| [`99b740e1d8`](https://github.com/OpenAgentsInc/openagents/commit/99b740e1d8b3745a263d3bd208d9efc158633dfc) | 2026-09-21 10:21:28 | **Document complete classification refusals as reports.** Explain that complete classification refusal can be a structured report. This corrected documentation without adding runtime behavior. | Retained |
| [`35c180c15b`](https://github.com/OpenAgentsInc/openagents/commit/35c180c15ba89bd74f2e5e1b63e1544ac241dcee) | 2026-09-21 10:25:53 | **Document complete classification refusals as reports.** Explain that complete classification refusal can be a structured report. This corrected documentation without adding runtime behavior. | Main |
| [`0498b3633a`](https://github.com/OpenAgentsInc/openagents/commit/0498b3633aba21a7f0dcb90b7cc13ff55b87eb7f) | 2026-09-21 10:31:37 | **Add the NIP-CJ decision worker lane and its protocol helpers.** Preserve the decision worker lane and protocol helpers on a repair branch. It remained unaccepted and unpublished at the stop instruction. | Retained |
| [`95df77c09f`](https://github.com/OpenAgentsInc/openagents/commit/95df77c09ff467afa7241d09a240cd57c06bcc17) | 2026-09-21 10:32:29 | **Serve bounded versioned documentation through MCP tools.** Expose bounded, versioned documentation via MCP tools. This advanced another issue before completing the active classification contract. | Retained |
| [`acb009b98c`](https://github.com/OpenAgentsInc/openagents/commit/acb009b98c6f4f16f0a1738caf57978a45a73acb) | 2026-09-21 10:32:34 | **Serve bounded versioned documentation through MCP tools.** Expose bounded, versioned documentation via MCP tools. This advanced another issue before completing the active classification contract. | Main |
| [`b07a8d467a`](https://github.com/OpenAgentsInc/openagents/commit/b07a8d467abeeb91dd6e94b58900f231f2f72b5f) | 2026-09-21 10:35:11 | **Validate documentation arguments against each tool schema.** Reject documentation arguments outside each tool's declared schema. This repaired validation omissions after the initial MCP implementation. | Retained |
| [`d2eac19808`](https://github.com/OpenAgentsInc/openagents/commit/d2eac198080638609eeac20088a67da292a8fb3e) | 2026-09-21 10:35:39 | **Reject null documentation arguments outside the tool schemas.** Reject null arguments where the schemas require a different shape. This was another small follow-up validation repair before publication. | Retained |
| [`77d219883a`](https://github.com/OpenAgentsInc/openagents/commit/77d219883a48bb2d9add5082790560b7b515f8ee) | 2026-09-21 10:38:09 | **Clarify documentation and service tool descriptions.** Clarify the distinction between documentation tools and live service tools. This prevented misleading interface descriptions. | Retained |
| [`735349f479`](https://github.com/OpenAgentsInc/openagents/commit/735349f479254490f7d3f5dfb55e5d17e56588bb) | 2026-09-21 10:38:49 | **Validate documentation arguments against each tool schema.** Reject documentation arguments outside each tool's declared schema. This repaired validation omissions after the initial MCP implementation. | Main |
| [`9b7e5b12f5`](https://github.com/OpenAgentsInc/openagents/commit/9b7e5b12f5bedd41ecdd6116ad027dfc1bcd976d) | 2026-09-21 10:38:49 | **Reject null documentation arguments outside the tool schemas.** Reject null arguments where the schemas require a different shape. This was another small follow-up validation repair before publication. | Main |
| [`ea7237b9e8`](https://github.com/OpenAgentsInc/openagents/commit/ea7237b9e870e9996b6d25648324c3ebe68b43d8) | 2026-09-21 10:38:49 | **Clarify documentation and service tool descriptions.** Clarify the distinction between documentation tools and live service tools. This prevented misleading interface descriptions. | Main |
| [`392cd4c34f`](https://github.com/OpenAgentsInc/openagents/commit/392cd4c34f1434aa6146a8fd52af2921a8e51da6) | 2026-09-21 10:46:03 | **gateway: add the opt-in review/fallback policy and audited sub-dispatch.** Add opt-in review and fallback with separately admitted, audited calls. This expanded classification policy scope and required substantial host hardening afterward. | Main |
| [`4a1b137532`](https://github.com/OpenAgentsInc/openagents/commit/4a1b137532aadef09bcc5b9ca953616de20d7e83) | 2026-09-21 10:46:03 | **gateway: test and document the review/fallback execution path.** Test and document review/fallback execution. These were contract fixtures, not held-out policy-quality evidence. | Main |
| [`a32c67bf93`](https://github.com/OpenAgentsInc/openagents/commit/a32c67bf931eb3a59b66f04c5272c549883be4ea) | 2026-09-21 10:46:03 | **gateway: cover a score unit's absent review answer.** Test absent Score review answers so review failure could not silently become a confirmed selection. | Main |
| [`13657f8691`](https://github.com/OpenAgentsInc/openagents/commit/13657f869167c1058c439780aee43cbdbde27e8b) | 2026-09-21 10:51:26 | **Preserve unfinished decision worker tests and documentation after timeout.** Save unfinished worker tests and documentation after timeout. Both source and repair-branch copies remained pending independent acceptance. | Retained |
| [`c01d08fba5`](https://github.com/OpenAgentsInc/openagents/commit/c01d08fba56b9d106f663ac30a99971ad47be8bc) | 2026-09-21 10:52:50 | **Enforce review failure bounds and bind secondary receipts to results.** Enforce review failure bounds and bind secondary receipts to their returned results. This repaired correctness gaps in the delegated review policy. | Main |
| [`309af43ea7`](https://github.com/OpenAgentsInc/openagents/commit/309af43ea7f146dec6480c6938d7bfb8b9339414) | 2026-09-21 10:53:14 | **Add the NIP-CJ decision worker lane and its protocol helpers.** Preserve the decision worker lane and protocol helpers on a repair branch. It remained unaccepted and unpublished at the stop instruction. | Retained |
| [`b0cac8713e`](https://github.com/OpenAgentsInc/openagents/commit/b0cac8713ea0d0ef6ce23386049f3408f712eec0) | 2026-09-21 10:53:14 | **Preserve unfinished decision worker tests and documentation after timeout.** Save unfinished worker tests and documentation after timeout. Both source and repair-branch copies remained pending independent acceptance. | Retained |
| [`cbac127800`](https://github.com/OpenAgentsInc/openagents/commit/cbac12780037c9976f7c83243ec99903a57f9d6d) | 2026-09-21 11:04:17 | **Recheck secondary authorization and bound identity verification.** Reauthorize secondary calls, freeze expected identity, and bound identity reads. These repairs prevented stale authorization and deadline bypass in review/fallback. | Main |
| [`86a16f86bc`](https://github.com/OpenAgentsInc/openagents/commit/86a16f86bce5305b5ffcb3607f08b1e2b30622da) | 2026-09-21 11:17:54 | **Publish classification response schemas and runtime fixtures.** Publish response schemas and real gateway fixture reports, with Jev decoding checks. Another classification acceptance increment that should have been batched with the complete corpus. | Retained |
| [`82112dbaba`](https://github.com/OpenAgentsInc/openagents/commit/82112dbaba1ae5e0e30d6180ea258c8c1ddb26f8) | 2026-09-21 11:18:15 | **Publish classification response schemas and runtime fixtures.** Publish response schemas and real gateway fixture reports, with Jev decoding checks. Another classification acceptance increment that should have been batched with the complete corpus. | Main |
| [`16e0916173`](https://github.com/OpenAgentsInc/openagents/commit/16e0916173585ede84628e5218f557d8898d1756) | 2026-09-21 11:19:45 | **Describe the implemented host review workflow consistently.** Remove stale descriptions of the already implemented review workflow. Source and cherry-picked copies are the same documentation purpose, not separate deliveries. | Retained |
| [`5637447b29`](https://github.com/OpenAgentsInc/openagents/commit/5637447b29f3d3bc70508a990e8f68604fb3774a) | 2026-09-21 11:20:42 | **Describe the implemented host review workflow consistently.** Remove stale descriptions of the already implemented review workflow. Source and cherry-picked copies are the same documentation purpose, not separate deliveries. | Retained |
| [`83f71e19e3`](https://github.com/OpenAgentsInc/openagents/commit/83f71e19e33debebb43cf55cb795302c59062fa0) | 2026-09-21 11:20:42 | **Publish classification response schemas and runtime fixtures.** Publish response schemas and real gateway fixture reports, with Jev decoding checks. Another classification acceptance increment that should have been batched with the complete corpus. | Retained |
| [`aed24eb3e9`](https://github.com/OpenAgentsInc/openagents/commit/aed24eb3e9c0cd4bab55f6732908ee4196f101f4) | 2026-09-21 11:25:10 | **Describe the implemented host review workflow consistently.** Remove stale descriptions of the already implemented review workflow. Source and cherry-picked copies are the same documentation purpose, not separate deliveries. | Main |
| [`d7f97a9e01`](https://github.com/OpenAgentsInc/openagents/commit/d7f97a9e01483bf639f402d35d840c2e422bbbc0) | 2026-09-21 11:26:36 | **Constrain review and fallback response contracts with runtime fixtures.** Extend response schemas to nested review/fallback records and fixtures. Splitting this from native schemas triggered another adjacent full-gate run. | Retained |
| [`94c371cf0b`](https://github.com/OpenAgentsInc/openagents/commit/94c371cf0b1e3ba17908dddfa1db7aa06ad9dc7e) | 2026-09-21 11:26:42 | **Constrain review and fallback response contracts with runtime fixtures.** Extend response schemas to nested review/fallback records and fixtures. Splitting this from native schemas triggered another adjacent full-gate run. | Main |
| [`d754af1ca0`](https://github.com/OpenAgentsInc/openagents/commit/d754af1ca0744be1622858146b71372103137afe) | 2026-09-21 11:31:55 | **Keep backend calls on their configured destination and bound identity reads.** Disable backend redirects and bound identity responses. This closed transport destination and resource gaps discovered during review. | Retained |
| [`be087f67c5`](https://github.com/OpenAgentsInc/openagents/commit/be087f67c5ad6e413d0f652b1cb118531ec0f282) | 2026-09-21 11:32:01 | **Keep backend calls on their configured destination and bound identity reads.** Disable backend redirects and bound identity responses. This closed transport destination and resource gaps discovered during review. | Main |
| [`1a2314c713`](https://github.com/OpenAgentsInc/openagents/commit/1a2314c71309e8deb9061ccc34fb7b12f486758e) | 2026-09-21 11:40:54 | **Preserve returned worker recovery patch for independent review.** Preserve the returned worker recovery patch for later review. It was not verified or published; the task remained unfinished when work stopped. | Retained |
| [`8daaf76fad`](https://github.com/OpenAgentsInc/openagents/commit/8daaf76fad8549b06da4eafafd4f9868b1111db2) | 2026-09-21 11:50:50 | **Audit the failure to complete project issues.** Record the execution failure at the user's request. The first audit was too shallow, which is why this detailed expansion is needed. | Main |
| [`14e5921a51`](https://github.com/OpenAgentsInc/openagents/commit/14e5921a5191bf9de0ec5c3ed4c8b5a076fa09e1) | 2026-09-21 12:01:53 | **Complete classification context bounds and cancellation evidence.** Complete #9482 with total native-context byte bounds, disconnect cleanup, monetary cancellation tests, and runtime fixtures. This was verified, pushed, and the issue closed before stopping. | Retained |
| [`72587db11f`](https://github.com/OpenAgentsInc/openagents/commit/72587db11fafe8f5984c337c2336d8bfdaf36a7f) | 2026-09-21 12:02:10 | **Complete classification context bounds and cancellation evidence.** Complete #9482 with total native-context byte bounds, disconnect cleanup, monetary cancellation tests, and runtime fixtures. This was verified, pushed, and the issue closed before stopping. | Main |

Retained-only hashes may not resolve on GitHub because they were not pushed.
Their full identifiers are in the CSV and remain inspectable in the local object
store. The documentation-only commit publishing this expanded audit is outside
the cutoff; its purpose is to replace the inadequate example list with the
complete retained inventory and measured time accounting.

## Why this became a loop

1. **Too much unfinished work.** I advanced classification, review, membership,
   accounting, discovery, context capture, verification adapters, and worker
   recovery before finishing the oldest issue. A new useful slice continually
   displaced the final acceptance work of an existing slice.
2. **Delegation output was not delivery.** Timeouts produced partial trees; even
   answered sessions needed repairs. I spent supervision time preserving,
   reconstructing, cherry-picking, and testing those trees. Artifact recovery
   should have been the exception, not a normal production lane.
3. **Acceptance was checked too late.** Request schemas, native response schemas,
   review schemas, and cancellation fixtures arrived as separate increments.
   Mapping the whole #9482 checklist before implementation would have exposed
   those missing pieces together.
4. **Review followed expensive integration.** Small lint and contract fixes
   arrived after a full-gate launch or an earlier successful full gate. The
   metadata shows serial follow-up commits, not a stable candidate entering one
   release check.
5. **Infrastructure consumed the mission.** Scheduler and review machinery were
   useful, but I continued extending them instead of proving sustained completed
   issue throughput with the machinery already available.
6. **Progress reporting rewarded activity.** Commits, passing tests, and executor
   answers appeared in updates without enough emphasis on remaining acceptance
   criteria, time spent, and issues closed. The user had to identify the failure.
7. **Timing records were not managed as audit evidence.** Reusing log filenames
   overwrote failed attempts. Some records lacked terminal measurements. This
   makes exact accounting harder and obscures retry cost.

## How a TypeSafe-native Coder could prevent this failure

This recommendation applies the Jev founder's
[original coding-agent document](../../coder/thoughts-on-a-typesafe-coding-agent.md),
the [Coder architecture analysis](../../coder/typesafe-agent-analysis.md), and the
[delivery roadmap](../../coder/typesafe-agent-roadmap.md) to the failures measured
above. The Jev founder's relevant ideas are query-specific context, explicit shared
state, subgoal deduplication, and background consumers of existing observations.
The roadmap supplies the crucial implementation boundary: Rust owns authority,
state transitions, freshness, and resource limits; typed judgments supply narrow
semantic assessments; generation proposes code and explanations.

These are proposed prevention mechanisms, not features added by this audit or a
measured claim that Jev would have completed these issues faster. The analysis
was checked against `9dd4ddab67`; its implementation inventory is historical.
For example, its description of unfinished classification modes predates the
#9482 completion recorded here. The proposal IDs and issue numbers below identify
roadmap ownership, not a fresh assertion that every named issue is still open.

### The missing control was an evidence-backed completion loop

My effective loop was: find a useful slice, implement or recover it, run checks,
publish progress, and find another slice. Coder should instead retain the current
issue's acceptance state and construct each next action around the oldest
unresolved requirement. A successful commit or test changes only the requirements
it supplies evidence for. It cannot erase unrelated missing requirements or
release the issue's work-in-progress slot.

The Jev founder's meta-attention idea makes this practical: each context request asks
what is needed to resolve a particular remaining requirement. It need not replay
every earlier log, delegate transcript, and product plan. But context selection
alone would not have stopped my scope switching. The controller must also enforce
the selected completion unit, review capacity, and stop instruction.

| Observed failure | State Coder should retain | Proposed prevention and exact host action | Roadmap connection |
| --- | --- | --- | --- |
| #9482 gained request schemas, native response schemas, review schemas, and cancellation evidence in separate increments | One versioned acceptance matrix, including missing response and disconnect evidence | Build the next context from unresolved rows. Keep the issue active until each row has current supporting evidence; require an explicit scope transition before starting another host implementation | Phase 1 `CTX-1`/`CTX-2`; #9513, #9505; requirement review under #9503 |
| G37/G38 and G43/G44/G45 checked adjacent partial implementations | Candidate tree identity, pending edits, required checks, prior results, and reason for each rerun | Review the assembled acceptance candidate and run focused checks before admitting a full gate. Coalesce identical pending gate requests; invalidate results when tested inputs change | Phase 2 `OPS-1`; #9509; Phase 5 recovery/accounting |
| Gate failures led to rereading logs and repairing unrelated fixtures | Addressable failure output, exact diagnostic spans, command/environment identity, previous attempted repairs | Retrieve the failure, relevant code, fixture, and rejected approach as a bundle. Generate an anchored repair and check it locally before paying the full gate cost again | Phase 1 `CTX-1`/`CTX-2`; Phase 2 `OPS-1` |
| Timeout or answered status still left unfinished delegated work | Parent requirement IDs, pinned snapshot, returned artifact digest, actual checks, and unresolved findings | Validate the structured result, then independently review it. An executor answer moves to review, never directly to accepted or done; missing artifacts remain incomplete | Phase 4 `SHARE-1`; #9508, #9509, #9514 |
| Available executor slots encouraged more work while host review accumulated | Separate execution, review, integration, and exclusive-measurement capacity, plus the age of pending results | Reserve review capacity at dispatch and stop refill when review is full. Prioritize completing the existing candidate over starting another issue | Phase 4 shared work and resource policy; #9514 |
| Similar recovery or verification actions could be proposed again after a long session or restart | Attempt identity, outcome, input/base digests, invalidation reasons, and the criterion the action serves | Reuse an exactly compatible completed observation or join an existing running operation. Retrieve similar failed attempts for context; do not retry an unknown write or cancel a merely similar task | Phase 4 duplicate suggestions; Phase 5 durable recovery; #9510, #9514 |
| Updates emphasized commits and passing checks while issue throughput stayed low | Verified acceptance transitions, issue age, gate time, recovery count, queue depth, and closed issues | Render progress from recorded events. When a configured no-progress or recovery bound is reached, block new dispatch and expose the unresolved requirement | Phase 0 observability; `UI-1`; #9505, #9506 |
| The user had to say “one issue, then stop” to regain control | A new task-frame revision that supersedes the broad project goal | Narrow dispatch authority immediately, stop refill, cancel expendable background work, and stop after the named completion. Mandatory instructions cannot be filtered out by relevance | `CTX-1`/`CTX-2`; Phase 4 preemption; Phase 5 cancellation |
| Reused log filenames obscured failed-run timing | Immutable run records and source-linked summaries, separate from controller claims | Allocate one log per attempt, preserve terminal or unknown status, and compute duration/overlap mechanically. Missing logs remain visible gaps | `CTX-1`, #9505, #9510; evidence retention and recovery |

### What the task frame and evidence store need to contain

Extend the roadmap's task frame with a small, explicit completion contract:

- The user objective and correction revision, selected issue, accepted scope,
  binding instructions, and stop condition. Distinguish user requirements from
  inferred subgoals. The selected issue must survive context rebuilding.
- Stable acceptance IDs with the original text, required artifact/check types,
  evidence references, unresolved questions, and state. Useful states are
  `missing`, `candidate`, `verified`, `invalidated`, and `unknown`; a delegate may
  submit candidate evidence but cannot set the parent's verified state.
- Current repository/base and candidate-tree digests, allowed read/write
  footprint, attempts already made, failed approaches, and outstanding effects.
- Known elapsed time and usage by decision, generation, executor, review, and
  verification, plus resource reservations and unknown costs. Expected future
  cost is an estimate with its source; it is not recorded spend.

Store observations independently of their summaries. A check record needs the
artifact it tested, command and adapter identity, toolchain, features, relevant
environment/configuration identity, start/end and elapsed time, exit status,
coverage, and output references. A pass with missing prerequisites is partial
evidence. Key a cached result by all inputs that can affect its validity, not
just a commit message or an unchanged issue number. If the system cannot establish
freshness or relevant external-state equivalence, the result is not reusable.

Keep the three roles from the analysis separate: ATIF records what happened;
the evidence store supplies inspectable observations and derived context; the
durable controller decides which effects may run or resume. An evidence-store
entry is not a lease or permission. A model judgment is not a controller
transition. Summary construction uses available observations and explicit model
output, not inaccessible internal reasoning.

### Where Jev judgments help, and where they do not

Start with the analysis's proposed functions, each registered under #9503 with
versioned state, question, policy, model identity, and a consuming action. Do not
add an unbounded agent that asks whether the project is “making progress.” The
analysis explicitly warns against reviving retired progress/risk questions
without evidence. Count elapsed time, retries, missing checks, and accepted
transitions directly in Rust.

| Narrow semantic function | Relevant input | Proposed typed result and consumer | Failure behavior |
| --- | --- | --- | --- |
| Evidence relevance | One unresolved criterion and candidate bundles containing code, diagnostic, test, and previous attempt references | Independent Noul judgments or comparable rubric Scores help the context builder select useful bundles within its budget | Mandatory requirements remain included. Missing answers do not exclude candidates; use deterministic retrieval or expand within bounds |
| Remaining requirement review | One criterion, its claimed implementation, attributable tests, and known coverage gaps | A Choice among supported, contradicted, or insufficient evidence becomes a review finding alongside mechanical checks | Insufficient, refused, or unavailable remains unresolved. Even a supported judgment cannot waive required tests or independently close the issue |
| Summary sufficiency | The next task, retained source evidence, and a proposed summary | A Noul helps decide whether to use that representation or expand its sources | Preserve missing diagnostics and constraints as coverage gaps. Do not silently use a summary that omits a required item |
| Duplicate-task suggestion | A proposed action and retrieved prior attempts, with objectives, inputs, bases, and acceptance IDs | A typed suggestion directs the host to compare a prior attempt | Only exact compatible identity and current evidence permit reuse. Semantic similarity alone cannot suppress requested work or authorize retry |

No production question wording or confidence thresholds are prescribed here.
These functions need their own development cases and held-out evaluations;
existing action, shell-outcome, or program-selection results do not admit a
model for acceptance review. Choice/Score confidence describes the returned
distribution, not a guarantee that an issue is complete. This distinction is
consistent with the current [TypeSafe confidence documentation](https://docs.typesafe.ai/confidence).

Reuse eligible judgments only under their complete input and policy identities.
Run semantic checks at an evidence-changing boundary, not on every timer tick.
If deterministic retrieval supplies the needed evidence adequately, another
judgment has no demonstrated value. Counting the cost of summary checks, retries,
and context rebuilding prevents meta-attention from becoming another source of
unproductive work.

### How the #9482 sequence should have differed

At the request-schema stage (`3268508946`), the task frame would still have shown
missing response/cancellation corpus coverage and total backend context bounds.
The next generator context would have included those acceptance rows, native
answer types, existing gateway tests, and the current limit enforcement. Passing
request-schema tests would support only the request-schema row.

The controller would keep #9482 as the host completion candidate. Before a final
full gate, the host would assemble and review native responses, nested
review/fallback responses, disconnect behavior, and complete expanded-context
bounds together. Focused checks would expose fixture and lint errors before the
workspace run. A semantic requirement review could help identify a missing edge
case; the actual TCP-disconnect test and accounting assertions would still be
required evidence.

That arrangement could have reduced the successive full gates for related
schema and transport changes. It would not justify reusing an old pass after
behavior changed, nor guarantee that one gate would succeed. G19's backup race,
for example, still needed investigation when encountered; better context could
make its repair more directed but cannot make a real defect disappear. The
recorded 2h 19m of five-hour gate runtime is not a measured savings estimate.

For the worker recovery, the returned `artifact_verified: false` result would
remain a review item tied to its parent requirement and pinned base. Dispatching
another issue would depend on remaining review capacity, not the worker's
answered status. The parent would read the patch, failed checks, and unresolved
findings through their references instead of reconstructing the whole session.
Routing to Devin would still leave Devin's internal agent loop outside Coder's
control; the contract governs the handoff and acceptance, not its hidden internals.

### Parallelism should optimize accepted delivery

The Jev founder's shared-state proposal could reduce duplicated repository searches
and context preparation across delegates and reviewers. It would not multiply
integration capacity. Use immutable snapshots and declared write footprints;
recheck changed bases and conflicts on return. Keep one integration lane and an
exclusive lease for measurements that require an uncontested machine. Derive
refill eligibility from dependencies, claims, resource leases, and review slots.

Use historical process and gate records to estimate workload durations, with
uncertainty and failures retained. A model may help classify an ambiguous task's
likely resource needs, but the host must conservatively enforce declared resource
classes. Do not infer that eight tasks are safe because a model predicts they are
“software only.” Nor should a cheaper generator be preferred from token price
alone: include context loading, failed escalation, review, and repair costs, as
the Jev founder's cache example motivates without establishing current prices.

Start background work with one evidence-derived progress or diff view. It should
reuse the same observations, debounce changes, cancel superseded work, and run
below foreground priority. A progress view should expose the outstanding
acceptance rows and aging review queue during a long gate. It should not trigger
another repository exploration, test run, or autonomous product workstream just
to make the agent appear busy.

### Smallest implementation and proof that would be worth doing

Follow the roadmap's order instead of turning this audit into another large
orchestration project:

1. **Phases 0–1:** give one pinned issue an explicit task frame, acceptance IDs,
   immutable command evidence, and a deterministic context manifest. Expose the
   remaining gaps through `UI-1` and headless events. This baseline should already
   preserve corrections and prevent exact duplicate gate dispatch without Jev.
2. **Phase 2:** use `OPS-1` to complete a representative failing-test repair with
   retained diagnostics, anchored edits, focused checks, and the required manual
   gate. Add one evidence-relevance or remaining-requirement function only where
   the deterministic baseline exposes a specific gap.
3. **Phase 4:** give a bounded delegate and an independent reviewer the same
   snapshot and requirement references through `SHARE-1`. Demonstrate useful
   shared reads and enforced review backpressure before raising concurrency.
4. **Phase 5:** test restart, cancellation, unknown effects, and stop-after-one
   behavior against the same task and attempt identities. Resume valid work
   rather than asking a new generator to infer orchestration state from prose.

Use this incident as a development replay, with observations revealed in their
original order so the policy cannot see the eventual fix. Include a stale green
check, an answered but incomplete delegate, a needed diagnostic after the initial
output excerpt, an overwritten/missing log, an uncertain requirement judgment,
and the user correction to finish one issue and stop. Fault-injection fixtures
can establish host invariants without paying for another large delegation run.

Then compare on separate held-out repair tasks, keeping the generator, repository
snapshots, acceptance criteria, required verification, and resource allowance
fixed: deterministic task/evidence state first, then the same system with typed
relevance or requirement review. Measure verified issue completion, false
acceptance, time to a completed artifact, full-gate launches, repair cycles,
context preparation, review backlog, operator corrections, and total known cost.
Include refused, missing, timed-out, and incomplete outcomes in the denominator.

The proposal earns adoption only if it reduces avoidable work or improves
completion without increasing false acceptance or weakening required checks.
The deterministic stop condition, evidence freshness, and resource boundaries
must pass even when every optional semantic call refuses. That is the specific
way the Jev founder's architecture could prevent this incident from recurring:
useful judgments over retained state, inside a controller that makes unfinished
acceptance work and the user's actual objective impossible to lose.

## Rules for future agents

These are recommended operating rules, not newly implemented scheduler features.
They retain the repository's required manual gate and independent review.

1. **Choose the completion unit before coding.** For each issue, write a compact
   matrix: acceptance item, current evidence, missing change, required check, and
   closure condition. Do not silently substitute a smaller slice for the issue.
   If an issue cannot be completed within the available authority, identify that
   before dispatch rather than after several partial integrations.
2. **Limit work in progress.** Use one host integration candidate and at most two
   delegated issues initially. Increase concurrency only after a complete
   dispatch → review → gate → issue-closure cycle succeeds. Eight available
   executor slots do not imply capacity to review eight outputs.
3. **Drain finished work before refilling.** Pending independent review counts
   against capacity. If the review queue is full, stop dispatch. A timeout with
   partial edits goes to one explicit recovery decision, not another automatic
   sequence of retries and source branches.
4. **Review and run focused checks before the full gate.** Inspect the diff,
   formatting, changed-crate Clippy, and acceptance regressions first. Assemble
   all acceptance changes for that issue on a stable integration tree, then run
   the required full gate. Never remove a required check to improve the metric.
5. **Rerun for a concrete reason.** Record the exact changed files or failed
   assertion that invalidates prior evidence. Batch adjacent implementation and
   schema corrections before the next full gate. A passed gate is reusable only
   for the tree and coverage it actually tested; it is not permission to skip
   verification of later behavioral changes.
6. **Make no-progress visible early.** After 30 minutes without completing an
   acceptance item, report the exact remaining blocker and stop opening new
   work. After two repair cycles on the same delegated artifact, choose one
   bounded host repair or abandon that attempt with its evidence preserved.
   These are escalation triggers, not an excuse to close an incomplete issue.
7. **Use resource classes, not a universal parallelism number.** Independent
   coding can run concurrently in isolated worktrees. Integrations and manual
   gates share one lane. Quiet-host conformance measurements require an exclusive
   resource lease. Release those leases on terminal completion and retain
   unknown claims when termination cannot be established.
8. **Require executor deliverables that can actually be reviewed.** A pinned
   base, committed patch, owned paths, acceptance mapping, focused-check results,
   and explicit remaining failures are part of the task. An answer string or a
   done marker is not completion. Check this before accepting another assignment.
9. **Record every run immutably.** Give each gate and process a unique run ID and
   log path; record start/end UTC, monotonic elapsed time, base/tree digest,
   commands, resource lease, exit status, and reason for rerun. Preserve failed
   attempts. Compute both summed process time and union wall time; never add
   concurrent intervals as if they were sequential waiting.
10. **Report delivery, not motion.** Each progress update should identify the
    current issue, acceptance items finished/remaining, elapsed gate and executor
    time, review backlog, and issues closed. Do not use commit count as a proxy
    for output. If the user says finish one issue and stop, disable refill and
    recurring supervision immediately and honor that limit.

A suitable handoff instruction is:

> Finish one selected issue through its full acceptance matrix before starting
> another host implementation. Keep one integration lane, bounded review backlog,
> and immutable timing records. Review and run focused checks before the required
> manual gate. Stop refilling on review congestion or repeated recovery. Report
> remaining acceptance gaps and elapsed verification time; close only with
> evidence, then stop or take the next explicitly authorized issue.

No new harness, issue, delegation, automation, or product change is introduced by
this audit expansion. Project-wide work remains stopped.
