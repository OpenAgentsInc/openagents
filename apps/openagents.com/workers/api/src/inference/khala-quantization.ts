// Khala quantization metadata + served-model descriptor (book P1-7 / #6090).
//
// THE PRINCIPLE (book Ch.5, in our own words)
// -------------------------------------------
// A model served at a reduced numeric precision (FP8 / MXFP8 / NVFP4 / INT4 / …)
// is NOT the same product as the unqualified model id. Quantization can win
// prefill/decode throughput AND lower the accepted-outcome rate; a throughput
// number is not a quality proof. So precision is a FIRST-CLASS, disclosed
// property of the served-model descriptor — not an invisible serving detail.
//
// This module owns ONLY the typed quantization metadata and the served-model
// DESCRIPTOR it hangs on. It is the single place that turns "how was this model
// actually served at the bit level" into a typed, public-safe, receipt-bearing
// fact. The same-model-claim GUARD lives in `khala-quantization-guard.ts`; the
// quality/cost eval gate lives in `khala-quantization-eval-gate.ts`. Both read
// this descriptor; neither re-invents the vocabulary.
//
// HONESTY CONTRACT (mirrors the telemetry schema's `not_measured` discipline):
//   - `unquantized` means we KNOW the lane served at full original precision.
//   - `not_measured` means we do NOT know the precision (e.g. a managed provider
//     that does not disclose how it quantizes behind its API). It is a typed,
//     first-class sentinel — never a fabricated "fp16" guess and never a missing
//     field. "We do not know the precision" is a real value, distinct from "we
//     know it is full precision".
//   - A descriptor whose precision is `not_measured` is HONESTLY ambiguous; the
//     guard treats that ambiguity as undisclosed for an UNQUALIFIED alias (you
//     cannot claim "same model" when you cannot even say how it was served).
//
// PUBLIC-SAFE (INVARIANTS: no secret/private leakage): the descriptor carries
// only neutral classifiers (precision mode, backend/engine id, a coarse method
// label) and public refs. No prompt, account, key, price, or weights material.
import { Schema as S } from 'effect'

// The honest "no measurement exists" sentinel. Defined locally (equal to the
// telemetry module's `NOT_MEASURED`) so this module — which the telemetry schema
// imports — carries no back-import to telemetry and there is no import cycle.
export const NOT_MEASURED = 'not_measured' as const

// ---------------------------------------------------------------------------
// Precision mode (book Ch.5: the numeric format the weights/activations use).
// ---------------------------------------------------------------------------

// The served numeric precision. `unquantized` is a real value (full original
// precision — FP16/BF16/FP32, i.e. NOT quantized). `not_measured` is the honest
// sentinel for an undisclosed managed lane. Every other value is a concrete
// reduced-precision format the book discusses. A closed union so a new format is
// added deliberately (and the guard/gate stay exhaustive).
export const KhalaPrecisionMode = S.Literals([
  // Full original precision — the lane is NOT quantized.
  'unquantized',
  // Weights-only / activation reduced-precision formats (book Ch.5 §5.x).
  'fp8',
  'mxfp8',
  'nvfp4',
  'int8',
  'int4',
  'awq',
  'gptq',
  // The honest "we do not know how this lane quantizes" sentinel.
  'not_measured',
])
export type KhalaPrecisionMode = typeof KhalaPrecisionMode.Type

// Whether a precision mode is a reduced-precision (quantized) format. Used by the
// guard + gate to decide whether disclosure / an eval gate is even required.
// `unquantized` and `not_measured` are NOT "quantized": the first is known-full,
// the second is honestly unknown (the guard handles unknown separately).
export const isQuantizedPrecision = (mode: KhalaPrecisionMode): boolean => {
  switch (mode) {
    case 'unquantized':
    case 'not_measured':
      return false
    case 'fp8':
    case 'mxfp8':
    case 'nvfp4':
    case 'int8':
    case 'int4':
    case 'awq':
    case 'gptq':
      return true
  }
}

// ---------------------------------------------------------------------------
// What was quantized (book Ch.5: weights-only is safest; KV-cache / attention
// quantization is more aggressive and riskier — POLICY, see the doc).
// ---------------------------------------------------------------------------

// The SCOPE of the quantization — which tensors carry the reduced precision. The
// book's policy ladder (and our adopted policy): weights-only / FP8 BEFORE
// aggressive KV-cache or attention quantization. Recording the scope lets the
// policy be enforced (the eval-gate flags an aggressive scope) instead of assumed.
export const KhalaQuantizationScope = S.Literals([
  // The lane is not quantized; scope is not applicable.
  'none',
  // Weights only (the safest first step the book recommends).
  'weights_only',
  // Weights + activations.
  'weights_and_activations',
  // Includes KV-cache quantization (more aggressive — book flags quality risk).
  'kv_cache',
  // Includes attention quantization (most aggressive — flagged).
  'attention',
  // The honest sentinel: we know it is quantized but not which tensors.
  'not_measured',
])
export type KhalaQuantizationScope = typeof KhalaQuantizationScope.Type

// Whether a scope is one the policy considers AGGRESSIVE (KV-cache / attention).
// The eval-gate requires an explicit policy acknowledgement for these.
export const isAggressiveScope = (scope: KhalaQuantizationScope): boolean =>
  scope === 'kv_cache' || scope === 'attention'

// ---------------------------------------------------------------------------
// The serving backend / engine that produced the precision (book §6 notes:
// receipt fields for engine + version + quantization).
// ---------------------------------------------------------------------------

// The serving backend behind the precision. Reuses the same engine vocabulary as
// the benchmark matrix (provider-native for managed lanes; concrete open engines
// for self-hosted) plus `not_measured`. We do NOT fork the benchmark's
// `BenchmarkEngine` union here — we ADD the sentinel — because a served request
// may have an unknown backend where a benchmark cell always names one.
export const KhalaQuantizationBackend = S.Literals([
  'provider-native',
  'vllm',
  'sglang',
  'tensorrt-llm',
  'not_measured',
])
export type KhalaQuantizationBackend = typeof KhalaQuantizationBackend.Type

// ---------------------------------------------------------------------------
// The quantization metadata struct (the typed receipt/telemetry fields).
// ---------------------------------------------------------------------------

// The quantization metadata recorded on the served-model descriptor / receipt.
// Every field is a concrete value or the honest sentinel. This is the typed
// answer to "at what precision, on what backend, over which tensors, was this
// request actually served, and has the quantized lane been eval-gate qualified?"
export const KhalaQuantizationMetadata = S.Struct({
  schemaVersion: S.Literal('openagents.khala.quantization.v1'),
  // The served numeric precision (or `unquantized` / `not_measured`).
  precision: KhalaPrecisionMode,
  // The serving backend/engine that applied the precision.
  backend: KhalaQuantizationBackend,
  // The serving engine VERSION when the lane exposes it (book §6: engine +
  // version in receipts). `not_measured` sentinel string otherwise.
  backendVersion: S.Union([S.String, S.Literal(NOT_MEASURED)]),
  // Which tensors carry the reduced precision (the policy-relevant scope).
  scope: KhalaQuantizationScope,
  // Whether this quantized lane has PASSED a Khala quantization eval gate
  // (`khala-quantization-eval-gate.ts`) against the original precision. `false`
  // for an unquantized lane (not applicable) AND for a quantized lane that has
  // not been gated. The guard reads this to decide if a quantized lane may carry
  // a public alias.
  evalGatePassed: S.Boolean,
  // The dereferenceable ref to the eval-gate result, when one exists. Null when
  // the lane has not been gated (unquantized or ungated quantized).
  evalGateRef: S.NullOr(S.String),
})
export type KhalaQuantizationMetadata = typeof KhalaQuantizationMetadata.Type

// The canonical UNQUANTIZED metadata: a lane we KNOW served at full original
// precision. Distinct from `UNKNOWN_QUANTIZATION` (we do not know).
export const UNQUANTIZED: KhalaQuantizationMetadata = {
  schemaVersion: 'openagents.khala.quantization.v1',
  precision: 'unquantized',
  backend: 'not_measured',
  backendVersion: NOT_MEASURED,
  scope: 'none',
  evalGatePassed: false,
  evalGateRef: null,
}

// The canonical honest-unknown metadata: we do NOT know how this lane was served
// at the bit level (a managed provider that does not disclose). The guard treats
// this ambiguity as undisclosed for an UNQUALIFIED public alias.
export const UNKNOWN_QUANTIZATION: KhalaQuantizationMetadata = {
  schemaVersion: 'openagents.khala.quantization.v1',
  precision: 'not_measured',
  backend: 'not_measured',
  backendVersion: NOT_MEASURED,
  scope: 'not_measured',
  evalGatePassed: false,
  evalGateRef: null,
}

// ---------------------------------------------------------------------------
// The served-model descriptor (the typed "what model, served how" record).
// ---------------------------------------------------------------------------

// Whether a public model alias is QUALIFIED — i.e. it discloses precision in the
// alias itself (e.g. `openagents/khala-code-fp8`) — or UNQUALIFIED (the bare
// public alias `openagents/khala-code`). The same-model guard uses this to
// decide whether an alias may front a quantized lane silently.
export const KhalaAliasQualification = S.Literals([
  'qualified', // the alias discloses precision (e.g. ...-fp8)
  'unqualified', // the bare public alias
])
export type KhalaAliasQualification = typeof KhalaAliasQualification.Type

// The served-model descriptor: the public alias a request was served UNDER, the
// concrete served-model id, and the quantization metadata of how it was served.
// This is the object the guard inspects and the telemetry/receipt carries.
export const KhalaServedModelDescriptor = S.Struct({
  schemaVersion: S.Literal('openagents.khala.served-model.v1'),
  // The PUBLIC alias the caller addressed (e.g. `openagents/khala-code`). This is
  // the brand-promise identity — what the receipt claims the customer bought.
  publicAlias: S.String,
  // The concrete served-model id the lane actually ran (provider/engine-specific).
  servedModelId: S.String,
  // How the public alias qualifies (does it disclose precision?). DERIVED from
  // the alias by `aliasQualification` so it is never hand-set inconsistently.
  aliasQualification: KhalaAliasQualification,
  // The quantization metadata of how this request was served.
  quantization: KhalaQuantizationMetadata,
})
export type KhalaServedModelDescriptor =
  typeof KhalaServedModelDescriptor.Type

// ---------------------------------------------------------------------------
// Decoders.
// ---------------------------------------------------------------------------

export const decodeKhalaQuantizationMetadata = S.decodeUnknownOption(
  KhalaQuantizationMetadata,
)
export const decodeKhalaServedModelDescriptor = S.decodeUnknownOption(
  KhalaServedModelDescriptor,
)

// ---------------------------------------------------------------------------
// Builders — assemble metadata honestly from what the lane disclosed.
// ---------------------------------------------------------------------------

// The raw, possibly-partial signals a serving lane CAN disclose about precision.
// Everything optional; absence collapses to the honest sentinel — never a guess.
export type KhalaQuantizationInput = Readonly<{
  precision?: KhalaPrecisionMode | undefined
  backend?: KhalaQuantizationBackend | undefined
  backendVersion?: string | undefined
  scope?: KhalaQuantizationScope | undefined
  evalGatePassed?: boolean | undefined
  evalGateRef?: string | null | undefined
}>

// Normalize a possibly-absent precision into a concrete value or the sentinel.
// Absence => `not_measured` (honest unknown), NEVER a fabricated full-precision.
const normalizePrecision = (
  precision: KhalaPrecisionMode | undefined,
): KhalaPrecisionMode => precision ?? 'not_measured'

// Derive the scope honestly. If the lane is unquantized, scope is `none`. If the
// caller gave a scope, use it. Otherwise: an unknown precision => `not_measured`;
// a known quantized precision with no scope => `not_measured` (we know it is
// quantized but not which tensors — honest, not a fabricated `weights_only`).
const deriveScope = (
  precision: KhalaPrecisionMode,
  scope: KhalaQuantizationScope | undefined,
): KhalaQuantizationScope => {
  if (precision === 'unquantized') {
    return 'none'
  }
  if (scope !== undefined) {
    return scope
  }
  return 'not_measured'
}

// Build the quantization metadata from raw lane signals. PURE: same input => same
// metadata. Unknown precision yields the honest-unknown shape; an explicitly
// unquantized lane yields the unquantized shape with any disclosed backend.
export const buildKhalaQuantizationMetadata = (
  input: KhalaQuantizationInput,
): KhalaQuantizationMetadata => {
  const precision = normalizePrecision(input.precision)
  return {
    schemaVersion: 'openagents.khala.quantization.v1',
    precision,
    backend: input.backend ?? 'not_measured',
    backendVersion:
      input.backendVersion === undefined || input.backendVersion.trim() === ''
        ? NOT_MEASURED
        : input.backendVersion,
    scope: deriveScope(precision, input.scope),
    // An unquantized lane is never "eval-gate passed" (the gate is for proving a
    // quantized lane holds quality); default to the caller's flag only for a
    // quantized lane.
    evalGatePassed:
      precision === 'unquantized' || precision === 'not_measured'
        ? false
        : (input.evalGatePassed ?? false),
    evalGateRef: input.evalGateRef ?? null,
  }
}

// ---------------------------------------------------------------------------
// Alias qualification (does the public alias disclose precision?).
// ---------------------------------------------------------------------------

// The disclosed-precision suffixes an alias may carry to QUALIFY itself. An alias
// ending in one of these (case-insensitive, after the final `-`) is `qualified`:
// it tells the caller, in the model id itself, that this is a reduced-precision
// product. The bare alias is `unqualified`. This is a bounded, documented
// classification of the alias STRING (not intent routing): it maps a precision
// suffix to the qualification flag the guard reads.
const QUALIFYING_PRECISION_SUFFIXES: ReadonlyArray<string> = [
  'fp8',
  'mxfp8',
  'nvfp4',
  'int8',
  'int4',
  'awq',
  'gptq',
]

// Classify a public alias as qualified (discloses precision in the id) or
// unqualified (the bare public alias). PURE string classification.
export const aliasQualification = (
  publicAlias: string,
): KhalaAliasQualification => {
  const lower = publicAlias.toLowerCase()
  const tail = lower.slice(lower.lastIndexOf('-') + 1)
  return QUALIFYING_PRECISION_SUFFIXES.includes(tail)
    ? 'qualified'
    : 'unqualified'
}

// Assemble the full served-model descriptor. The alias qualification is DERIVED
// from the alias so it is always consistent with the public id. PURE.
export const buildKhalaServedModelDescriptor = (input: {
  readonly publicAlias: string
  readonly servedModelId: string
  readonly quantization: KhalaQuantizationInput
}): KhalaServedModelDescriptor => ({
  schemaVersion: 'openagents.khala.served-model.v1',
  publicAlias: input.publicAlias,
  servedModelId: input.servedModelId,
  aliasQualification: aliasQualification(input.publicAlias),
  quantization: buildKhalaQuantizationMetadata(input.quantization),
})
