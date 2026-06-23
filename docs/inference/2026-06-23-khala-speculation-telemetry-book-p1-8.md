# Khala speculation acceptance telemetry + dynamic-disablement policy (book P1-8, #6091)

Status: buildable-now telemetry + decision-policy + receipt-disclosure machinery
merged; the REAL speculative decode (a draft model / a real serving engine) is
compute/owner-gated and stays inert. Not deployed by this change.

## The principle (book Ch.5 "Speculative Decoding", in our own words)

Speculative decoding speeds up DECODE by letting a cheap drafter guess the next
few tokens and letting the expensive target model VERIFY all the guesses in a
single parallel forward pass. Accepted drafts give several tokens for the cost of
one verification step; a rejection still makes forward progress of one true token
(the target's own prediction at the rejection point). Verifying K tokens costs
roughly one forward pass because transformers process all positions in parallel,
which is exactly what they are good at.

The catch is the load-bearing one for Khala:

- **Speculation is NOT a universal win.** The verification pass spends spare
  compute. It only pays off when there IS spare compute to spend — i.e. at LOW
  batch, where decode is the bottleneck and the machine is not already saturated.
- **At HIGH batch (or under compute pressure) speculation is a LOSS.** The extra
  verification work competes with real throughput. So it must be MEASURED
  (acceptance rate) and DYNAMICALLY DISABLED when the batch/pressure signal says
  it will not profit.
- **It is a serving-product fact, not an invisible trick.** A request served WITH
  speculation is a different serving product than plain autoregressive decode, so
  the mode is DISCLOSED in the receipt — tying to the shard-WAN
  speculative/direct-return/async receipt-mode disclosure
  (`2026-06-19-decentralized-serving-shard-wan.md`): for shard-WAN, speculation
  is a WAN latency-hiding strategy recorded in the Psionic receipt mode, not just
  a local speed trick.

Code generation is a strong FIT: generated code repeats syntax and reuses prompt
context, so cheap **n-gram / lookahead** drafting (no separate draft model) hits a
high acceptance rate. This is why the Worker-side policy only ever selects
draft-free modes.

## Drafting modes (and the EAGLE/Psionic boundary)

The typed `KhalaSpeculationMode` union:

- `n_gram` — reuse repeated n-grams already seen in the current generation/context
  as the draft. No draft model. **Fit for code repetition.**
- `lookahead` — maintain an n-gram table over the KV cache and propose matching
  continuations. No draft model. Same code-repetition fit; used for long-context
  codebase questions (a large context to mine).
- `eagle` — **EAGLE-style learned hidden-state drafting. FLAGGED AS A LATER
  Psionic / learned-serving lane.** EAGLE predicts the target model's next hidden
  state (richer than predicting tokens directly) using a trained draft head, so it
  needs target-model hidden-state DATA + TRAINING. It is a named-but-unbuilt mode
  here: the policy never SELECTS it (the Worker has no draft model / learned head),
  and `decideSpeculation` returns `disabled_mode_unavailable` for it. It exists in
  the vocabulary so a future Psionic learned-serving lane has a stable receipt
  mode to disclose.
- `none` — we KNOW no speculation ran (plain autoregressive decode).
- `not_measured` — the honest sentinel: a managed lane may speculate behind its
  API without disclosing the mode/acceptance.

n-gram and lookahead are the two Worker-runnable draft-free modes today
(`isDraftFreeMode`); `eagle` is the only learned mode (`isLearnedMode`).

## Honesty contract (mirrors the telemetry `not_measured` discipline)

- The acceptance rate is a real number ONLY when an actual draft/verify pass
  produced an accepted + proposed count pair; the rate is DERIVED from those
  counts and clamped to `[0, 1]`. Absent counts (or zero proposals) =>
  `not_measured`, never a fabricated rate and never a defaulted `0`.
- `none` (we know no speculation ran) is distinct from `not_measured` (we do not
  know whether/how a managed lane speculated). A `none` cell's acceptance rate is
  the sentinel, NOT a measured 0 — a 0 would falsely imply a drafter ran and
  accepted nothing.
- The recorded `draftTokensAccepted` is clamped `<= draftTokensProposed`, so a
  malformed disclosure can never record a rate above 1.

## The dynamic-disablement policy (`decideSpeculation`)

A bounded, typed decision over an explicit compute-pressure signal — NOT ad-hoc
per-request string matching. Inputs: the requested mode, a
`KhalaSpeculationPressureSignal { batchSize, computePressure }`, and an optional
threshold config (`KhalaSpeculationPolicyConfig`). Decision order:

1. No drafting requested (`none`/`not_measured`) => off (`disabled_not_requested`).
2. A learned/unavailable mode (`eagle`) => off (`disabled_mode_unavailable`) — the
   Worker has no draft model; that is the Psionic lane.
3. The pressure signal is unknown (`not_measured` batch or pressure) => off
   (`disabled_pressure_unknown`) — we cannot confirm the low-batch sweet spot, so
   be conservative.
4. Batch above `maxProfitableBatchSize` => off (`disabled_high_batch`) — the
   verification work competes with throughput.
5. Compute pressure above `maxProfitableComputePressure` => off
   (`disabled_high_pressure`) — no spare compute to verify drafts.
6. Otherwise (a draft-free mode at low batch + low pressure) => ON
   (`enabled_low_batch`).

The default policy (`DEFAULT_SPECULATION_POLICY`) is conservative and documented:
`maxProfitableBatchSize: 4`, `maxProfitableComputePressure: 0.6` — the book's
"low-batch sweet spot" turned into bounded numbers, tunable from observed
acceptance/throughput telemetry. The decision records a typed
`KhalaSpeculationDecisionReason` so a receipt reader knows WHY speculation was on
or off.

## Receipt-mode disclosure

The speculation metadata is a first-class field on the canonical
`openagents.khala.telemetry.v1` record (`KhalaTelemetryRecord.speculation`),
alongside the P1-7 quantization disclosure. It records the mode, whether it was
active, the acceptance rate, and the draft-token counts behind the rate. The live
hot path has no draft model, so it discloses `not_measured` (a managed provider
may speculate without telling us) — honest, never a fabricated active mode.

## What is buildable-now vs gated

**Buildable now (merged, exercised by tests):**

- The typed speculation telemetry metadata + honest builder
  (`khala-speculation.ts`), wired onto the telemetry record.
- The bounded `decideSpeculation` dynamic-disablement policy + thresholds.
- The fixture decode-trace lane (`benchmark/speculation-lane.ts`): a deterministic
  per-cell speculation outcome that REQUESTS a draft-free mode for code workloads,
  runs the policy against the cell's batch (concurrency) + derived pressure, and
  produces honest draft counts only when ENABLED. This is the only place the
  acceptance-rate plumbing is exercised end-to-end without spend.
- The report's draft-acceptance aggregate per (workload × model × temperature ×
  route) (`benchmark/report.ts`), with a null rate (honest absence) where
  speculation did not run.

**Compute / owner / flag-gated (built shape, inert — NOT armed here):**

- The REAL speculative decode (an actual n-gram/lookahead drafter or a learned
  draft head) needs a real serving engine; there is none in the Worker, so the
  live path discloses `not_measured`/`none`. A future serving engine threads real
  draft counts into the same metadata fields.
- EAGLE / learned hidden-state drafting is a Psionic learned-serving lane that
  needs target-model hidden-state data + training. Named, never selected.
- A real, billable benchmark sweep stays behind the owner-armed real lane seam
  (`makeRealLaneSeam`, default off — refuses to run unarmed).

## Where

- `apps/openagents.com/workers/api/src/inference/khala-speculation.ts` — the typed
  metadata, canonical shapes, builder, and the `decideSpeculation` policy.
- `apps/openagents.com/workers/api/src/inference/khala-telemetry.ts` — the
  `speculation` field on the record + input, built honestly.
- `apps/openagents.com/workers/api/src/inference/benchmark/speculation-lane.ts` —
  the fixture decode-trace derivation.
- `apps/openagents.com/workers/api/src/inference/benchmark/lane-seam.ts` /
  `runner.ts` / `report.ts` — the fixture sample field, the runner thread, and the
  per-(workload × model × temperature × route) acceptance aggregate.
- Tests: `khala-speculation.test.ts` (policy + builder),
  `khala-speculation-telemetry.test.ts` (telemetry/fixture/report integration +
  the flag-gated-off discipline).

## Verification bar (green)

The inference test suites (807 tests, incl. the new speculation suites),
`typecheck`, `check:architecture`, `check:effect-topology`, and
`check:public-projection-freshness`. Tests cover: acceptance-rate telemetry
populating from a fixture decode trace (honest sentinel when no speculation ran);
`decideSpeculation` enabling at low batch and disabling at high batch / pressure /
unknown-signal / learned-mode / not-requested; the speculation mode disclosed in
the record; the report acceptance aggregate keyed by the four axes; and the real
speculative-decoding engine staying flag-gated off (the fixture seam never spends,
the un-armed real seam refuses to run).
