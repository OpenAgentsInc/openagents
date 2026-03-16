# 2026-03-16 Psionic Apple Acceptance Harness Status Audit

## Scope

This audit records the exact live architecture-explainer acceptance-harness
result that now defines the truthful claim boundary for the Rust-only Apple
adapter lane.

It exists because export success, runtime smoke, and authority publication are
not the same thing as benchmark-useful adapter success, and the operator/docs
surface needed to say that explicitly.

## Command And Receipt

The live acceptance run used the canonical repo entrypoint:

```bash
OPENAGENTS_APPLE_ACCEPTANCE_REPORT_PATH=/tmp/openagents-apple-acceptance-report.json \
  scripts/release/check-psionic-apple-architecture-explainer-acceptance.sh
```

That receipt currently reports:

- `acceptance_passed = false`
- top-level reason code: `StandardGateRejected`

Stage results:

- `overfit_non_zero`
  - run id:
    `psionic-architecture-explainer-first-real-run-overfit-non-zero-1773694818159`
  - disposition: `RejectedAuthorityUnavailable`
  - runtime smoke: passed
  - export completed: true
  - benchmark gate accepted: true
  - useful adapter accepted: true
  - aggregate score: `520` bps
  - aggregate pass rate: `1428` bps
  - improved case count: `1`

- `standard`
  - run id:
    `psionic-architecture-explainer-first-real-run-standard-1773694877205`
  - disposition: `ExportedButNotUseful`
  - runtime smoke: passed
  - export completed: true
  - benchmark gate accepted: false
  - useful adapter accepted: false
  - aggregate score: `571` bps
  - aggregate pass rate: `1428` bps
  - improved case count: `1`
  - rejection reasons:
    - `adapter_score_below_minimum`
    - `adapter_pass_rate_below_minimum`
    - `score_delta_below_minimum`
    - `pass_rate_delta_below_minimum`
    - `improved_case_count_below_minimum`

## Why The Operator Surface Changed

Before this pass, the operator surface still made it too easy to read:

- export completed
- runtime smoke passed
- accepted outcome published

as if that meant:

- the adapter had cleared the benchmark-useful gate

That implication is false for the current Apple lane.

The fix in `autopilotctl` is intentionally narrow:

- the lifecycle field is now named `authority_accept`
- the published object field is now named `authority_outcome`
- the text output now prints an explicit note that export, runtime smoke, and
  authority acceptance do not by themselves prove benchmark-useful adapter
  quality

This keeps the CLI aligned with the actual Apple-lane boundary:

- operator status tells you what the app and authority did
- the acceptance harness tells you whether the adapter is useful enough to
  claim benchmark-improving success

## Current Truthful Claim Boundary

What we can say today:

- the Rust-only Psionic Apple lane trains adapter-only runs in-repo
- it exports Apple-valid `.fmadapter` packages
- those packages load through the real Apple bridge and pass runtime smoke
- the same lane clears the weak overfit non-zero gate

What we still cannot say today:

- the standard architecture-explainer gate is green
- export validity alone proves adapter usefulness
- authority publication alone proves adapter usefulness

## Consequence For The Remaining Epic

This audit closes the wording/documentation gap, not the benchmark-improvement
gap.

`#3901` is therefore about making the repo truthful.

The broader master task `#3891` is only honestly complete when the standard
stage clears its declared benchmark bar and the acceptance harness turns fully
green.
