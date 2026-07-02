# Khala Code QA Status Surface

Status: implementation note for ROADMAP_QA Q1.5 / issue #8016.

Every `bun run qa:nightly` run writes a public-safe owner summary beside the
existing matrix report:

- `qa-status-surface.json`
- `qa-status-surface.md`

The JSON schema is `openagents.khala_code.qa_status_surface.v1`. It is emitted
inside the same dated owned-runner artifact directory as
`qa-nightly-report.json`, and the nightly report cross-links both status
surface paths.

The status surface is the one-page health view for the current nightly. It
contains:

- matrix health: `healthy` when every step passed, otherwise `blocked`
- coverage counts for every tracked frontier dimension
- refs to the union ledger, frontier report, and explorer steering input
- zero-for-seven-days coverage count
- flake, nightly-failure, and zero-coverage issue filing status
- step-duration trends computed from prior public `qa-nightly-report.json`
  artifacts under the owned-runner artifact root
- the Q2 latency budget catalog from `qaMetrics`, including budget IDs,
  metric names, units, thresholds, percentiles, sample counts, and current
  evaluation status
- live-tier status

The live-tier field is intentionally honest. Q1 nightlies are fixture/no-spend
owned-runner jobs, so the status surface currently reports `not_in_matrix` with
roadmap refs to the Q5 live-tier cadence. Q5.5 is the task that rolls armed
live-tier evidence into this surface.

Perf trends currently use `nightly_step_duration_ms`. The full Q2.2 latency
budget catalog is present under `latencyBudgets`; rows with no real-run samples
remain `inconclusive` instead of fabricating measurements. Q2.5 owns per-budget
trend reporting and regression auto-issues once Q2.3 produces real samples.

## Public-Safety Contract

The status surface is redaction-checked before it is written. It may contain
only schema names, run IDs, step IDs, counts, durations, issue-status metadata,
and repo-relative artifact refs. It must not contain local absolute paths,
credential-shaped values, raw command logs, account identifiers, or live
provider payloads.

Raw logs remain under the owned-runner artifact root and must be
redaction-reviewed before external publication.
