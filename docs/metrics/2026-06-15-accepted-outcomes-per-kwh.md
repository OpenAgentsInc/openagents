# Accepted Outcomes Per Kilowatt-Hour (AO/kWh)

Date: 2026-06-15. Status: **definition frozen; yellow modeled seed instrumentation live in source.**
Promise: `metrics.accepted_outcomes_per_kwh.v1` (state `yellow` in source registry
`2026-06-15.5`). This is the
canonical definition of the metric Episode 232 introduced and Episode 237 names
as "the metric we're going to be defining and measuring primarily."

## 1. Definition

> **AO/kWh** = (number of **accepted outcomes** produced) ÷ (**kilowatt-hours**
> of energy consumed producing them), over a bounded window.

It answers one question: *what is the most cost-efficient way to convert an
electron into accepted agent work?* It deliberately fuses the two things
OpenAgents cares about — **verified, accepted work** (not raw tokens, not raw
FLOPs, not "activity") and **energy efficiency** (the real physical cost floor).

## 2. The numerator: accepted outcomes (not activity)

The numerator is the **accepted outcome**, the atomic unit of the OpenAgents
economy (see `docs/autopilot-coder/2026-06-14-the-load-bearing-wall-verification-accepted-work-essay.md`).
An accepted outcome is a unit of work that was:

1. **scoped before it ran** ("done" defined in advance — a rubric / verification
   class),
2. **executed** (wherever execution was cheapest),
3. **verified** against the definition (e.g. `exact_trace_replay` for the
   Tassadar executor lane), and
4. **accepted + settled** with a public, dereferenceable receipt.

It is explicitly **not** tokens generated, requests served, jobs claimed, or
nodes online. Unverified or unaccepted work does **not** count. This keeps the
metric honest: it measures value the buyer would actually pay for, gated by the
same verification + acceptance ladder tracked in
`payments.accepted_outcome_economics.v1`.

## 3. The denominator: kilowatt-hours

Energy consumed to produce those accepted outcomes — at the device/operator
level. This is the tie-in to the energy-orchestration thesis (Episode 232) and
the flexible-load work (`energy.flexible_load_proof.v1`): squeezing maximum
accepted work out of each kWh, and dispatching work to where/when energy is
cheapest or otherwise would be curtailed.

Energy may be **measured** (operator meter, device power telemetry) or
**explicitly modeled** (device TDP × wall-clock under a labeled assumption) — but
every published figure must carry its evidence-state label. A modeled kWh is not
a measured kWh and must never be presented as one.

## 4. Why this unit (and not the alternatives)

- **vs tokens/sec or FLOPs/W:** those measure capability/throughput, not
  commerce. Capability has been getting cheaper for two years; the binding
  constraint was never "can a system do the work" but "can a stranger pay for it
  without trusting the producer." AO/kWh prices the thing that actually clears.
- **vs $/outcome:** dollars float on subsidy and provider pricing; kWh is the
  physical floor. AO/kWh is the subsidy-resistant version of the efficiency
  question.
- **producer-indifferent:** an accepted outcome doesn't care whether a human, a
  machine, or a swarm produced it — only that "done" was defined, met, and
  proven. So the metric stays stable as the producer mix shifts from mostly
  human to mostly machine.

## 5. Current measurement status (honest)

**Yellow, modeled seed only.** As of 2026-06-20:

- Numerator: `/api/public/metrics/accepted-outcomes-per-kwh` now publishes a
  receipt-backed accepted-outcome counter with one seed accepted outcome: the
  first settled labor job (#4777), backed by
  `docs/labor/2026-06-14-first-negotiated-labor-job-evidence-bundle.md` and the
  public work-request status route.
- Denominator: the seed datapoint is **modeled**, not measured: 100 W provider
  power assumption × acceptance→result wall-clock window. Pylon still does not
  report measured kWh, and there is no operator energy-ingestion path.
- Green gate: the endpoint now exposes the measured-telemetry threshold directly:
  `requiredMeasuredDatapointCount = 2`, `currentMeasuredDatapointCount = 0`,
  `measuredDatapointShortfall = 2`, and
  `measuredTelemetryGateSatisfied = false`.

So today AO/kWh has a **caveated modeled seed datapoint**, not measured production
telemetry. That is enough to move the promise out of `planned` and into `yellow`,
but not enough to call AO/kWh measured, comparable, green, or production-routing
evidence.

Apple FM local-session kWh estimates (#5074) follow the same denominator rule:
they may be retained as modeled or unavailable session-energy evidence, but they
are not AO/kWh unless joined to a verified accepted-outcome receipt. A local
Apple FM tool/chat smoke is raw session activity; by itself it must not be
counted as an accepted outcome.

## 5b. Demand provenance (internal vs external) — `proof.demand_provenance.v1`

AO/kWh is a revenue-bearing public number, so every datapoint now carries a
typed **demand-provenance** label under the rule
**`no_external_dollar_no_demand_claim`**:

- `internal` — first-party / operator-staged demand (ablation, sweep,
  conformance, credit-ledger settlement). This is *plumbing proof*, not market
  proof.
- `external` — a third party paid real dollars for the accepted work.

The projection at `/api/public/metrics/accepted-outcomes-per-kwh` serves a
reconciling split (`internalAcceptedOutcomeCount` +
`externalAcceptedOutcomeCount` == accepted-outcome total) plus
`externalDemandClaimAllowed`, which stays `false` until at least one external
(real-dollar) accepted outcome exists.

The current seed (`#4777`) is labeled **internal**: it was operator-staged and
settled on the internal credit ledger (1 sat), not driven by an external paying
customer. The metric's `unsafeCopy` copy-gate forbids presenting it as external
market demand. This makes AO/kWh the first revenue-bearing projection serving a
real internal/external demand split — see
`apps/openagents.com/workers/api/src/accepted-outcomes-per-kwh.ts` and its test.

## 6. Path to a real number (green gate)

`metrics.accepted_outcomes_per_kwh.v1` goes green only with:

1. a frozen definition (this doc — **done**),
2. an accepted-outcome counter tied to verified-work receipts,
3. measured or explicitly-modeled energy (kWh) per device/window, and
4. at least two **published measured AO/kWh datapoints** from real telemetry
   with evidence-state labels, caveats, and transition receipts.

Until all four exist with at least two measured per-device telemetry datapoints, no AO/kWh
figure may be presented as measured, broadly representative, ranked, or
production-routing evidence (see the promise's `unsafeCopy`).

## 7. References

- `docs/transcripts/232.md` — metric introduced ("accepted outcomes per kilowatt
  hour … most cost-efficient way of converting electron to accepted agent work").
- `docs/transcripts/237.md` — named the primary measure at launch.
- `docs/autopilot-coder/2026-06-14-the-load-bearing-wall-verification-accepted-work-essay.md`
  — why the accepted outcome (not the skill) is the atomic unit.
- `promise:payments.accepted_outcome_economics.v1` — the accepted-outcome state
  machine (numerator integrity).
- `promise:energy.flexible_load_proof.v1` — energy economics (denominator).
- `docs/tassadar/2026-06-15-executor-trace-contributor-completion-design.md` — the
  path that produces accepted outcomes at volume (#5051).
