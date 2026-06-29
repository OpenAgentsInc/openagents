// Khala SAME-MODEL-CLAIM guard (book P1-7 / #6090).
//
// THE RULE (book Ch.5 + khala.md §6/§7 disclosure):
// -------------------------------------------------
// A quantized lane is NOT the same product as the unqualified public model id.
// So you may NOT silently serve a reduced-precision variant UNDER the bare public
// alias (`openagents/khala-code`) and let the receipt imply the customer got the
// unqualified model. A quantized lane may carry a public-facing identity ONLY if
// EITHER:
//   (a) the alias itself DISCLOSES the precision (a `qualified` alias, e.g.
//       `openagents/khala-code-fp8`), OR
//   (b) the unqualified alias is fronted by a quantized lane whose precision +
//       backend are DISCLOSED IN THE RECEIPT and the lane has PASSED the
//       quantization eval gate (so "same model" is a proven claim, not a guess).
//
// This is the receipt-disclosure sibling of the Khala IDENTITY guard
// (`khala-identity.ts`): that guard stops Khala from naming the wrong model;
// THIS guard stops Khala from claiming an unqualified model identity for a
// reduced-precision product without disclosing what it actually served.
//
// It is a TYPED, FAIL-CLOSED check over the served-model descriptor — NOT intent
// routing or string matching over user content. It inspects the structured
// descriptor (alias qualification + quantization metadata), per the workspace
// rule that bounded enum/field checks are fine once the structured object exists.
//
// PURE: no Worker, no Effect runtime, no IO. Same descriptor => same verdict.
import {
  type KhalaServedModelDescriptor,
  isAggressiveScope,
  isQuantizedPrecision,
} from './khala-quantization'

// ---------------------------------------------------------------------------
// The verdict.
// ---------------------------------------------------------------------------

// Stable, neutral reason refs for a REJECTED same-model claim. Each names exactly
// WHY the descriptor may not present under its (unqualified) public alias.
export type KhalaSameModelRejectionReason =
  // A quantized lane fronted an UNQUALIFIED alias without disclosing its
  // precision in the receipt (the headline P1-7 leak: silent quantized variant).
  | 'undisclosed_quantization_under_unqualified_alias'
  // The precision is honestly UNKNOWN (`not_measured`) under an unqualified
  // alias — we cannot claim "same model" when we cannot even say how it served.
  | 'unknown_precision_under_unqualified_alias'
  // The quantized lane is disclosed but has NOT passed the eval gate, so its
  // accepted-outcome quality vs the original precision is unproven — it may not
  // claim the unqualified alias (it MAY still serve under a qualified alias).
  | 'quantized_lane_not_eval_gate_qualified'

// The guard verdict over one served-model descriptor.
export type KhalaSameModelVerdict = Readonly<{
  // True when the descriptor may legitimately present under its public alias.
  allowed: boolean
  // The rejection reason when not allowed; null when allowed.
  reason: KhalaSameModelRejectionReason | null
  // A public-safe, human/agent-readable explanation. Never carries secrets.
  detail: string
  // Whether the served lane is a reduced-precision (quantized) lane at all (an
  // unquantized lane is trivially allowed under any alias).
  isQuantizedLane: boolean
  // Whether the lane's quantization metadata is DISCLOSED in the receipt
  // (precision + backend present and not the unknown sentinel). The disclosure
  // half of the rule.
  disclosed: boolean
  // Whether the quantization SCOPE is aggressive (KV-cache / attention). Surfaced
  // as a policy WARNING even when the claim is allowed, so an aggressive lane is
  // never silently normalized.
  aggressiveScopeWarning: boolean
}>

// ---------------------------------------------------------------------------
// Disclosure check (is the quantization recorded in the receipt?).
// ---------------------------------------------------------------------------

// Whether a quantized lane's metadata is DISCLOSED: the precision is a concrete
// reduced-precision value (not the `not_measured` sentinel) AND the backend is
// named (not the `not_measured` sentinel). A reduced-precision lane that records
// neither is NOT disclosed — that is the silent-variant case the guard rejects.
export const isQuantizationDisclosed = (
  descriptor: KhalaServedModelDescriptor,
): boolean => {
  const { precision, backend } = descriptor.quantization
  if (!isQuantizedPrecision(precision)) {
    // Either unquantized (nothing to disclose) or `not_measured` (unknown — that
    // is handled as its own rejection, not "disclosed").
    return false
  }
  return backend !== 'not_measured'
}

// ---------------------------------------------------------------------------
// The guard.
// ---------------------------------------------------------------------------

// Evaluate the same-model claim for a served-model descriptor. FAIL-CLOSED: a
// quantized lane is allowed under an UNQUALIFIED alias ONLY when its precision +
// backend are disclosed AND it passed the eval gate. A `qualified` alias (the id
// itself discloses precision) is always allowed for a quantized lane — the
// customer was told. An unquantized lane is always allowed.
//
// PURE: same descriptor => same verdict.
export const evaluateSameModelClaim = (
  descriptor: KhalaServedModelDescriptor,
): KhalaSameModelVerdict => {
  const { precision, scope } = descriptor.quantization
  const isQuantizedLane = isQuantizedPrecision(precision)
  const disclosed = isQuantizationDisclosed(descriptor)
  const aggressiveScopeWarning = isQuantizedLane && isAggressiveScope(scope)

  // An unquantized lane is the same product as the unqualified alias — allowed.
  // (Note: an `unquantized` precision is not quantized, so this branch covers it.
  // A `not_measured` precision is NOT unquantized — it falls through below.)
  if (precision === 'unquantized') {
    return {
      allowed: true,
      reason: null,
      detail:
        'Lane served at full original precision (unquantized); it is the same product as the public alias.',
      isQuantizedLane: false,
      disclosed: false,
      aggressiveScopeWarning: false,
    }
  }

  // A QUALIFIED alias discloses the precision in the model id itself. The customer
  // explicitly addressed a reduced-precision product, so presenting it is allowed
  // regardless of the receipt (the alias is the disclosure). The eval gate still
  // governs whether the lane is PRODUCTION-promoted, but the claim itself is honest.
  if (descriptor.aliasQualification === 'qualified') {
    return {
      allowed: true,
      reason: null,
      detail:
        'Public alias discloses its precision; the reduced-precision product is named in the model id the caller addressed.',
      isQuantizedLane,
      disclosed,
      aggressiveScopeWarning,
    }
  }

  // From here the alias is UNQUALIFIED (the bare public alias). The lane is either
  // honestly-unknown precision or a disclosed/undisclosed quantized lane.

  // Honestly-unknown precision under the bare alias: we cannot claim "same model"
  // when we cannot even say how it was served. REJECT.
  if (precision === 'not_measured') {
    return {
      allowed: false,
      reason: 'unknown_precision_under_unqualified_alias',
      detail:
        'Served precision is not known (not_measured) under the unqualified public alias. The receipt cannot honestly claim this is the unqualified model; disclose the precision/backend or serve under a qualified alias.',
      isQuantizedLane: false,
      disclosed: false,
      aggressiveScopeWarning: false,
    }
  }

  // A quantized lane under the bare alias with NO disclosure: the silent-variant
  // leak the issue calls out. REJECT.
  if (!disclosed) {
    return {
      allowed: false,
      reason: 'undisclosed_quantization_under_unqualified_alias',
      detail:
        'A reduced-precision (quantized) lane is fronting the unqualified public alias without disclosing its precision/backend in the receipt. A quantized variant cannot silently share an unqualified public model alias.',
      isQuantizedLane,
      disclosed,
      aggressiveScopeWarning,
    }
  }

  // A DISCLOSED quantized lane under the bare alias is allowed ONLY if it has
  // passed the eval gate — otherwise its accepted-outcome quality vs the original
  // precision is unproven and it may not claim to BE the unqualified model.
  if (!descriptor.quantization.evalGatePassed) {
    return {
      allowed: false,
      reason: 'quantized_lane_not_eval_gate_qualified',
      detail:
        'Quantization is disclosed in the receipt, but the quantized lane has not passed the Khala quantization eval gate, so its accepted-outcome quality vs the original precision is unproven. It may serve under a qualified alias but not claim the unqualified public alias.',
      isQuantizedLane,
      disclosed,
      aggressiveScopeWarning,
    }
  }

  // Disclosed AND eval-gate-qualified quantized lane under the bare alias: the
  // claim is proven. Allowed (with an aggressive-scope warning if applicable).
  return {
    allowed: true,
    reason: null,
    detail:
      'Quantization is disclosed in the receipt and the lane passed the Khala quantization eval gate; the claim to the public alias is proven.',
    isQuantizedLane,
    disclosed,
    aggressiveScopeWarning,
  }
}

// A thrown rejection for the route path that wants fail-closed enforcement rather
// than a verdict to branch on. Carries the typed reason + public-safe detail.
export class KhalaSameModelClaimError extends Error {
  readonly _tag = 'KhalaSameModelClaimError'
  readonly reason: KhalaSameModelRejectionReason
  constructor(verdict: KhalaSameModelVerdict & { reason: KhalaSameModelRejectionReason }) {
    super(verdict.detail)
    this.name = 'KhalaSameModelClaimError'
    this.reason = verdict.reason
  }
}

// Assert a descriptor may present under its public alias, throwing the typed
// error otherwise. The fail-closed route entry point.
export const assertSameModelClaim = (
  descriptor: KhalaServedModelDescriptor,
): KhalaSameModelVerdict => {
  const verdict = evaluateSameModelClaim(descriptor)
  if (!verdict.allowed && verdict.reason !== null) {
    throw new KhalaSameModelClaimError({ ...verdict, reason: verdict.reason })
  }
  return verdict
}
