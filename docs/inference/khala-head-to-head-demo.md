# Khala Head-to-Head Demo Evidence Pack

Status: scaffold evidence for Agent Demo M8 / issue #6016. This document and
the paired manifest/reducer define how the head-to-head will be measured before
the live M3/M5/M7 receipts exist. Fixture output is useful for integration and
review, but it is not product proof.

## Goal

Publish the OpenAgents version of the Fugu-Ultra-vs-frontier prompt:

> build a really high quality single html file crossy road game with three.js

The published pack must compare `openagents/khala` against a frontier baseline
and report:

- tokens, dollars, and wall-clock;
- verifier verdict and verified rate;
- cost per accepted outcome;
- accepted outcomes per kWh when measured telemetry exists;
- in-world vs gateway split;
- artifact playback refs, Verse playback refs, and settlement refs.

External Fugu/Opus numbers stay in the manifest as reported claims until we can
cite or reproduce them. They are never mixed into OpenAgents measurements.

## Current Scaffold

- Fixture manifest:
  `docs/inference/fixtures/khala-head-to-head-dry-run.v1.json`
- Reducer/validator:
  `scripts/khala-demo/reduce-head-to-head.mjs`
- Test:
  `scripts/khala-demo/reduce-head-to-head.test.mjs`

Run the dry-run reducer:

```sh
bun scripts/khala-demo/reduce-head-to-head.mjs \
  docs/inference/fixtures/khala-head-to-head-dry-run.v1.json
```

Run the focused tests:

```sh
bun test scripts/khala-demo/reduce-head-to-head.test.mjs
```

## Manifest Contract

The manifest is a public-safe JSON object with schema
`openagents.khala_head_to_head_evidence.v1`.

Required top-level fields:

- `manifestRef`: stable public-safe ref for this evidence packet.
- `evidenceMode`: `fixture_scaffold` or `live`.
- `generatedAt`: ISO timestamp.
- `scope`: issue, roadmap, prompt, and benchmark refs.
- `runs`: at least one `khala` run and one `frontier_baseline` run for the live
  pack.
- `externalReportedClaims`: reported public numbers, separated from measured
  runs.
- `publication`: publication status, output refs, and blocker refs.

Each run records:

- `runId`, `lane`, `label`, `model`, `provider`, `coordinator`, `evidenceMode`.
- `usage.promptTokens`, `usage.completionTokens`, `usage.totalTokens`.
- `costUsd`, `costMsat`, `priceMsat`, `wallClockMs`.
- `acceptedOutcome`: verifier class, accepted flag, receipt/verifier refs,
  evidence refs, and blocker refs.
- `artifact`: single-file HTML artifact ref and playable-in-world ref.
- `settlement`: settled flag, receipt refs, and blocker refs.
- `verse`: playback ref, source refs, and in-world/gateway work units.
- `energy`: measured kWh, measurement ref, and blocker refs.
- `sourceRefs` and `blockerRefs`.

The reducer rejects local paths, `.secrets` refs, private-key blobs, and obvious
API-token strings anywhere in manifest string values.

## Metric Rules

- `tokens`: use `usage.totalTokens`, or derive prompt plus completion tokens.
- `dollars`: use measured `costUsd`.
- `wallClockMs`: use measured run wall-clock.
- `costPerAcceptedOutcomeUsd`: `costUsd` only when the run was accepted;
  otherwise `not_applicable`.
- `verifiedRate`: accepted runs divided by runs with a verifier verdict.
- `inWorldVsGatewaySplit`: derive from `verse.inWorldWorkUnits` and
  `verse.gatewayWorkUnits`; if both are zero, mark `not_measured`.
- `acceptedOutcomesPerKwh`: accepted outcome count divided by measured kWh only
  when measured telemetry exists; otherwise `not_measured`.

## Publication Skeleton

The final publication pack must include:

1. Setup: prompt, exact model ids, coordinator mode, dates, and environment.
2. Methodology: how runs were started, stopped, verified, and priced.
3. Raw inputs: manifest refs, receipt refs, artifact refs, and public-safe logs.
4. Scoreboard: the reducer output, with reported external claims separated.
5. Accepted-outcome verdict: verifier evidence and failure reasons if any.
6. Payment/settlement: worker and validator settlement refs, or explicit
   non-settlement caveats.
7. Verse playback: playback refs and in-world vs gateway split.
8. Artifact playback: the generated single HTML file playable in the
   three-effect world.
9. Honest losses: failures, retry loops, poor quality, missing telemetry, or
   higher cost must stay in the pack.
10. Product-promise boundary: no world-first, AO/kWh, or broad product claim is
    upgraded without the DE-10 evidence pack and owner sign-off.

## Closure Gate For #6016

Issue #6016 cannot honestly close until the reducer's `closureAudit.canClose`
is `true` for a live manifest. The current fixture intentionally returns
`false`.

Minimum live evidence:

- a live `openagents/khala` run, not only `khala-mini` or fixture data;
- a live frontier baseline run;
- a verifier receipt for the accepted outcome;
- settlement refs for worker and validator payment;
- Verse playback refs and artifact-in-world playback refs;
- measured energy telemetry for AO/kWh;
- publication refs for the final comparison;
- no public-safety blockers in the manifest.
