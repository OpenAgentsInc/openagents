# The consumer conformance suite and the deterministic baseline

`crates/coder/tests/conformance.rs` is the reproducible demonstration
that Coder is a Decision Router consumer: thirteen scenarios, each a
complete `Runtime` run against a scratch machine — a temporary Git
repository carrying a copy of the registry (`programs/`, `questions/`,
`sources/`), one pinned caller-owned work list, and a stub decision door
that records every request it is asked. Nothing outside the repository
runs, no credential is read, and the whole suite finishes in about two
seconds:

```
cargo test -p coder --test conformance
```

Every scenario asserts the same contract from a different side: a
program step, a decision attempt, an identity, a typed outcome, and a
cost — known or unknown — reconcile with each other and with what the
door actually saw.

## What each scenario demonstrates

| Scenario | Acceptance item it proves |
| --- | --- |
| `a_selection_is_only_ever_a_program_the_host_offered` | Wrong program selection: an answer naming a program the host never offered fails answer-space validation at the SDK and becomes `door_unavailable`, never a selection. A `none` answer is an answer, not an error — the turn goes on as it always did. |
| `a_selected_program_runs_only_under_the_operators_grant` | Explicit execution authority: a missing program grant refuses before the first step; a read-only grant refuses a program that writes. A model's choice is a proposal, not permission. |
| `a_typed_refusal_is_the_steps_typed_outcome` | Refusal: a door declining maps to the step's typed `Refused`, not a generic error, and the run stops on it. |
| `quota_exhaustion_is_bounded_then_terminal` | Quota exhaustion: a `429` door is retried within the SDK's bound — three dispatches carry `x-typesafe-retry-count` `None`, `1`, `2` — then the refusal is terminal, not silent. |
| `an_answer_from_an_unadmitted_artifact_is_the_refusal` | Artifact mismatch: an answer served under a model the set's policy does not admit is `unbound_model`, and no step runs on it. |
| `a_cancelled_run_marks_the_callers_end_not_the_works` | Cancellation: a cancelled run records `Cancelled` as the caller's end; work already dispatched keeps its own outcome. |
| `a_crash_leaves_unknown_and_the_restart_is_a_decision` | Restart: recovery marks unfinished records `Unknown` rather than inferring success, and reconciliation distinguishes a replayable read-only step (`Replayable`), an ambiguous delegation (`NeedsDecision`), and a replay that exceeds the resuming grant (`OutsideAuthority`). |
| `partial_work_reports_its_coverage_against_the_program` | Partial work: a door that fails after useful work leaves the run's steps, delegations, and coverage consistent with what actually finished. |
| `a_reworded_question_set_is_drift_the_inventory_reports` | Stale inputs: the site inventory reports the registry's standing `UnboundSet` for the retained `openagents.independence.v1`, and one reworded instruction inside `questions` surfaces as `DigestDrift` on the site that pinned the old digest. |
| `work_that_cannot_be_verified_never_passes` | Missing verification: a task that stated no expected answer can be `answered` but is `Unverifiable`, and unverifiable work never counts as passed. |
| `every_attempt_reconciles_with_identities_outcomes_and_costs` | Reconciliation: each decision attempt carries its question-set id and digest, the model identity, a typed outcome, and usage; counted tokens and unmetered dispatches sum against the run's ledger. |
| `the_local_profile_sends_no_credential_anywhere` | Local profile isolation: `CODER_DECISION_PROFILE=local` sends no `Authorization` header to the loopback door and refuses `CODER_DECISION_KEY` outright — local-only mode cannot silently fall back to a hosted credential. |
| `the_workflow_reports_against_its_deterministic_baseline` | The baseline comparison below, and the retained-worktree commitment: every writing delegation's checkout outlives its step for review. |

## The baseline comparison

The workflow's last test runs `burn-down` twice over one pinned
five-item work list — one write pair colliding on a path, one read pair,
one item the list itself blocks. Arm one is the complete workflow:
lookup, semantic independence, mechanical admission, bounded fan-out,
and per-requirement acceptance. Arm two is the same program with the
two `decide` steps removed — the deterministic baseline a consumer must
beat or at least match on coverage before its model spend means
anything. Both arms run the same lookup, the same mechanical collision
handling, and the same stub executor, so the difference the report
records is what the decision layer bought:

| Measure | Decision workflow | Deterministic baseline |
| --- | --- | --- |
| Steps | `select`, `independence`, `admit`, `fan_out`, `accept` | `select`, `admit`, `fan_out` |
| Delegations | 3 | 3 |
| Answered | 3 | 3 |
| Correct of graded | 3 of 3 | 3 of 3 |
| Tally | 3 passed | 3 passed |
| Decision dispatches | 2 | 0 |
| Decision tokens | 256 | 0 |
| Retained worktrees | 1 | 1 |

On this pinned case the semantic gate buys coverage certainty — the
independence question and the acceptance question both answered — at 256
counted tokens and two dispatches, and changes neither what the
mechanics selected nor what the grading found. That is the honest
shape of the comparison: the work list's collision and its blocked item
are mechanical facts no judgment can waive, so the decision layer's
value on this case is the typed record it leaves, not a different
outcome. Error direction stays asymmetric by construction — a `none`
answer costs one ordinary turn and is recoverable by rephrasing, while a
spurious selection is a proposal the operator's grant refuses before any
step runs.

The test emits this report as JSON when `CODER_CONFORMANCE_OUT` names a
path; the run this document records is retained at
[`2026-09-22-consumer-conformance.report.json`](2026-09-22-consumer-conformance.report.json).
Wall-clock fields are one machine's timing and regenerate per run.

## What this suite does not demonstrate, and where that evidence lives

- **Terminal and headless parity.** Both front ends run the same
  `coder::turn::run`; `crates/coder/tests/headless.rs` proves the
  headless binary leaves the trace the terminal leaves, by contract of
  the shared path rather than a second implementation.
- **The hosted HTTP profile.** The dated evaluations in this directory —
  [`2026-09-21-independence-v2-eval`](2026-09-21-independence-v2-eval.md),
  [`2026-09-21-evidence-select`](2026-09-21-evidence-select.md),
  [`2026-09-22-review-finding-baseline`](2026-09-22-review-finding-baseline.md) —
  ran hosted `jev-1.13.0` through the same `POST /v1/systemone` contract
  the suite's stub door answers, with raw exchanges retained. Hosted
  runs need a credential and are not part of the offline suite.
- **The relay profile.** [`relay-transport.md`](relay-transport.md) is
  the measured proof that the two ends of the relay door meet, with
  per-transport latency and refusal causes; the `relay_lifecycle`,
  `worker_lifecycle`, `relay_binding`, `relay_delegation`, and
  `relay_job` suites cover the handoff's lifecycle.
- **Delegation under an enforced boundary.** Scenarios that spawn
  delegates skip when `coder-boundary` reports no enforcement backend —
  on macOS `sandbox-exec`, on Linux `bwrap`. A skip is reported as a
  skip; it is never counted as proof.

## Privacy and retention

Every scenario runs against a stub or loopback door in a temporary
directory. No environment credential is read — the local-profile
scenario asserts the header's absence — and no request leaves the
machine. The trace records digests of questions and states alongside
the answers, so the evidence is attributable without retaining the
prompts themselves. What a run does retain is the commitment the
release makes: the ATIF trace, the durable runstate records, and the
writing delegation's worktree, held for the operator's review rather
than cleaned away.
