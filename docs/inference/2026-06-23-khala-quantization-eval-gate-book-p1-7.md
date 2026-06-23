# Khala quantization eval gate (book P1-7, #6090)

Status: buildable-now metadata + guard + eval-gate machinery merged; the real
quantized-model serving and the real quant-vs-original sweep are
flag/owner/compute-gated (built but not armed). Not deployed by this change.

## The principle (book Ch.5, in our own words)

Quantization — serving a model at a reduced numeric precision (FP8 / MXFP8 /
NVFP4 / INT4 / AWQ / GPTQ) — can win prefill and decode throughput. It can ALSO
damage output quality. A throughput number is not a quality proof. So two rules:

1. **A reduced-precision model is NOT the same product as the unqualified model
   id.** Serving `openagents/khala-code` from an FP8 lane and letting the receipt
   imply the customer got the unqualified model is a silent product
   substitution. The precision/backend must be disclosed.
2. **A throughput/cost win that lowers the accepted-outcome rate is a LOSS**
   unless cost-per-accepted-outcome improves. Quality is measured on EXECUTED
   product evals (the P0-4 Khala-code executed verifier), never assumed from a
   tokens/second figure.

This maps to `khala.md` §6/§7 (verification + receipt disclosure): the receipt
must carry the engine, version, and quantization, and a "same model" claim is
only honest if precision/backend are disclosed and the lane has passed a real
eval comparison vs the original precision.

## Adopted policy

- **Weights-only / FP8 / MXFP8 BEFORE aggressive KV-cache or attention
  quantization.** The quantization SCOPE is a first-class, recorded field
  (`weights_only` / `weights_and_activations` / `kv_cache` / `attention`). KV-cache
  and attention scopes are flagged `aggressive` and require an explicit owner
  acknowledgement to pass the eval gate, even when the metrics hold.
- **Disclosure is required.** A quantized lane may carry a public identity only if
  EITHER (a) the public alias itself discloses the precision (a *qualified* alias
  like `openagents/khala-code-fp8`), OR (b) the unqualified alias is fronted by a
  quantized lane whose precision + backend are disclosed in the receipt AND the
  lane has passed the quantization eval gate.
- **Honest sentinels.** Precision is `unquantized` (known full precision),
  `not_measured` (honestly unknown — e.g. a managed provider that does not
  disclose how it quantizes), or a concrete reduced-precision value. Unknown is
  never silently treated as full precision. An unqualified alias over an
  `not_measured` lane is rejected: you cannot claim "same model" when you cannot
  even say how it was served.

### Alias-sharing policy (Open Question Q6 — resolved)

Which quantization modes may share a public model alias?

- **An unqualified public alias** (`openagents/khala-code`) may be served by:
  - the unquantized lane (always — same product); or
  - a quantized lane only when its precision + backend are disclosed in the
    receipt AND it has passed the quantization eval gate. Until then the
    quantized lane must use a qualified alias.
- **A qualified alias** (`openagents/khala-code-fp8`) may always front the
  matching reduced-precision lane — the precision is named in the id the caller
  addressed, so the claim is honest by construction. The eval gate still governs
  whether such a lane is *production-promoted*, but the identity claim itself is
  not a leak.
- **`not_measured` precision may NOT share an unqualified alias.** Honest
  ambiguity is treated as undisclosed.

## What shipped (buildable now)

All in `apps/openagents.com/workers/api/src/inference/`:

1. **Quantization metadata + served-model descriptor**
   (`khala-quantization.ts`): the typed `KhalaQuantizationMetadata`
   (precision / backend / backendVersion / scope / evalGatePassed / evalGateRef)
   and `KhalaServedModelDescriptor` (publicAlias / servedModelId /
   aliasQualification / quantization). Builders collapse absent signals to honest
   sentinels (`UNKNOWN_QUANTIZATION`) — never a fabricated full precision. Added
   as a first-class `quantization` field on `KhalaTelemetryRecord`
   (`khala-telemetry.ts`); an unmeasured request records the honest-unknown shape.

2. **Same-model-claim guard** (`khala-quantization-guard.ts`): a typed,
   fail-closed check over the served-model descriptor. `evaluateSameModelClaim`
   returns a verdict; `assertSameModelClaim` throws `KhalaSameModelClaimError`.
   It REJECTS an undisclosed quantized lane under an unqualified alias, a
   `not_measured` lane under an unqualified alias, and a disclosed-but-ungated
   quantized lane under an unqualified alias; it PASSES an unquantized lane, a
   qualified alias, and a disclosed + eval-gate-qualified quantized lane. It is a
   bounded enum/field check over the STRUCTURED descriptor — not intent routing —
   the receipt-disclosure sibling of the Khala identity guard.

3. **Quantization eval gate** (`khala-quantization-eval-gate.ts`): a comparison
   harness that scores a quantized lane vs the original precision on EXECUTED
   acceptance verdicts (reuses the P0-4 verifier's `AcceptanceVerdict`, never a
   regex) and computes the cost-per-accepted-outcome delta (reuses the P1-5
   report's accepted-outcome + cost math shape). `runQuantizationEvalGate` PASSES
   only when accepted-outcome quality HOLDS, or a small drop (within an agreed
   bound) is bought back by a sufficient cost-per-accepted-outcome improvement.
   Deterministic/fixture by default (`decisionGrade:false`).

## Fixture vs owner/compute-gated split

- **Buildable now (in this change):** the metadata schema + telemetry field, the
  same-model-claim guard, and the eval-gate LOGIC scored on deterministic fixture
  comparison sets. No real quantized serving, no network, no spend, no compute.
- **Owner/compute-gated (built, flagged OFF):** the real quantized-vs-original
  sweep that stands up a real reduced-precision serving lane and runs the executed
  Khala verifier on BOTH precisions. `collectRealQuantSweepSamples` throws
  `RealQuantSweepNotArmedError` unless `armRealQuantSweep:true` plus an injected
  executor are owner-supplied. A decision-grade gate result (promoting a real lane
  to a public alias) requires this armed sweep over realistic traffic; the default
  fixture path proves the gate logic only.

## Tests

`khala-quantization.test.ts`, `khala-quantization-guard.test.ts`,
`khala-quantization-eval-gate.test.ts` (37 new): quant metadata populates from a
fixture served-model descriptor and records the honest sentinel when unknown; the
same-model guard rejects an undisclosed/unknown/ungated quantized alias and passes
a disclosed-gated one and a qualified alias; the eval gate passes when quality
holds (or a small drop is offset by a cost-per-accepted win) and fails when the
accepted-outcome rate drops beyond bound or without a cost win, fails an
aggressive scope without ack, and the real sweep is flag-gated off (no real
serving in tests).

## Verification

Inference suites green (779 tests, 37 new), `typecheck`, `check:architecture`,
`check:effect-topology`, `check:public-projection-freshness` all green. Not
deployed; no spend.
