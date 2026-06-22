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
- Runner (drives both lanes, emits the manifest):
  `scripts/khala-demo/run-head-to-head.mjs`
- Reducer/validator:
  `scripts/khala-demo/reduce-head-to-head.mjs`
- Publication renderer:
  `scripts/khala-demo/render-publication.mjs`
- Test:
  `scripts/khala-demo/run-head-to-head.test.mjs`,
  `scripts/khala-demo/reduce-head-to-head.test.mjs`, and
  `scripts/khala-demo/render-publication.test.mjs`

Run the runner against the built-in stub transport (no live gateway needed)
and pipe it through the reducer:

```sh
bun scripts/khala-demo/run-head-to-head.mjs --stub \
  | bun scripts/khala-demo/reduce-head-to-head.mjs /dev/stdin
```

Or write the stub manifest to a file first:

```sh
bun scripts/khala-demo/run-head-to-head.mjs --out /tmp/khala-h2h.json
bun scripts/khala-demo/reduce-head-to-head.mjs /tmp/khala-h2h.json
```

Run the dry-run reducer:

```sh
bun scripts/khala-demo/reduce-head-to-head.mjs \
  docs/inference/fixtures/khala-head-to-head-dry-run.v1.json
```

Run the focused tests:

```sh
bun test scripts/khala-demo/reduce-head-to-head.test.mjs \
  scripts/khala-demo/render-publication.test.mjs
```

Render the publication draft:

```sh
bun scripts/khala-demo/render-publication.mjs \
  docs/inference/fixtures/khala-head-to-head-dry-run.v1.json
```

## Runner

`scripts/khala-demo/run-head-to-head.mjs` drives the head-to-head and feeds the
harness above. It sends the crossy-road prompt to two OpenAI-compatible lanes,
collects what the responses actually report, and emits a manifest in the exact
`openagents.khala_head_to_head_evidence.v1` shape the reducer consumes.

- **Khala lane:** `POST {KHALA_BASE_URL}/chat/completions` with model
  `openagents/khala-code` (override with `--khala-model`). It reads the
  non-breaking `openagents` response block (`receipt`, `route`, `workers`,
  `verification`, `cost_msat`, `price_msat`, `settled`, and any optional
  settlement/Verse/in-world/energy fields).
- **Frontier baseline lane:** the same OpenAI-compatible call against a
  separate base URL/token/model.

Configuration (CLI flag or env var):

| Flag | Env | Default |
|---|---|---|
| `--khala-base-url` | `KHALA_BASE_URL` | — (stub if unset) |
| `--khala-token` | `KHALA_AGENT_TOKEN` | — |
| `--khala-model` | `KHALA_MODEL` | `openagents/khala-code` |
| `--frontier-base-url` | `FRONTIER_BASE_URL` | — (stub if unset) |
| `--frontier-token` | `FRONTIER_TOKEN` | — |
| `--frontier-model` | `FRONTIER_MODEL` | `frontier-baseline` |
| `--msat-per-usd` | `KHALA_MSAT_PER_USD` | — (USD stays unmeasured) |
| `--stub` | `KHALA_RUNNER_STUB=1` | force the built-in stub |
| `--out <path>` | — | write manifest to file (else stdout) |

**Stub vs live.** With no live base URLs (or `--stub`) the runner uses a
deterministic, public-safe stub transport and emits an `evidenceMode:
fixture_scaffold` manifest. Only when **both** lanes have a real base URL and
the stub is not forced does it emit `evidenceMode: live`. The live gateway is
owner-gated, so the runner is "flip-ready": set the env vars and drop `--stub`.

**Honesty contract (enforced by the runner, not just the reducer).** The runner
never fabricates metrics:

- Tokens/cost/wall-clock come only from the response and the measured clock.
- `costUsd` is `0` (with a `cost_usd_not_measured` blocker) unless the response
  carries `cost_usd`, or `cost_msat` plus a `--msat-per-usd` conversion rate.
- A missing verifier verdict is `verificationClass: none`, not accepted.
- Settlement is `settled: false` unless the response explicitly says settled and
  supplies worker/validator receipt refs.
- Verse playback, in-world artifact, and energy kWh stay `null` unless the
  response provides them; energy is never estimated.
- Because the owner-gated live gateway does not yet return settlement, Verse,
  in-world, or energy evidence, a current live run still keeps
  `closureAudit.canClose: false` — exactly like the stub and fixture.

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

The renderer produces this structure from the manifest so live and fixture packs
share one visible scoreboard and blocker format.

## Live Promotion Audit

The reducer emits `livePromotionAudit` alongside `closureAudit`. This is the
machine-readable #6016-E gate for replacing fixture refs with live evidence.
Each check has a stable `id`, `passed`, `blockerRef`, and `detail` field.

Current checks:

- `live_manifest`: top-level manifest mode is `live`.
- `no_fixture_refs_in_live_manifest`: a live manifest contains no `fixture.*`,
  `fixture:*`, `fixture-*`, or `fixture_*` refs anywhere.
- `khala_live_run`: the Khala lane is present and live.
- `openagents_khala_model`: the Khala lane uses `openagents/khala`.
- `khala_accepted_outcome`: the Khala run has an accepted verifier outcome.
- `m7_live_conductor`: the Khala coordinator mode is `live_conductor`.
- `settlement_receipts`: the Khala run has settlement receipts.
- `verse_playback`: the Khala run has a Verse playback ref.
- `artifact_playable_in_world`: the artifact has a playable-in-world ref.
- `energy_telemetry`: AO/kWh is measured, not estimated.
- `frontier_live_run`: the baseline lane is present and live.
- `publication_published`: the publication ref exists and status is
  `published`.
- `public_safety`: the manifest passed public-safe string validation.

`closureAudit.canClose` is derived from this promotion audit. If any check is
blocked, #6016 stays open.

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
