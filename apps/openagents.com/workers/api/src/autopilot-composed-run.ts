// Autopilot all-in-one composed-run model — the typed shape of ONE Autopilot
// run that composes the OpenAgents Cloud primitives + open markets against ONE
// balance with ONE receipt envelope (EPIC #5510, child #5519; promises
// autopilot.all_in_one_business_system.v1 + cloud.primitives_suite.v1).
//
// Episode 239 ("Let's Make Money", docs/transcripts/239.md): Autopilot is the
// all-in-one business system a business runs on, COMPOSED of the OpenAgents
// Cloud primitives (inference, fine-tuning, training, agentic work, sandbox
// compute, web services) and the open markets beneath them, billed from one
// balance.
//
// SCOPE / HONESTY: this is an INERT scaffold that shows the COMPOSITION SHAPE,
// NOT a claim the run is live or billable. It is PURE:
//   - it moves no money, provisions no primitive, opens no wallet, reads no
//     real balance, and writes no real receipt;
//   - it only assembles a typed composed-run PLAN — a set of primitive
//     component plans (each pointing at an existing primitive scaffold's
//     public-safe receipt-ref shape), a single balance ref the components
//     share, and one receipt-envelope ref the run would settle under.
// Both promises STAY `planned`. Nothing here flips them green: there is no
// provisioning, no metering, no unified balance debit, no settlement. A green
// flip stays receipt-first and owner-signed per proof.claim_upgrade_receipts.v1
// and demand-provenance per proof.demand_provenance.v1 (internal use is
// plumbing proof, not market proof).
//
// It deliberately IMPORTS the merged primitive scaffolds rather than
// duplicating them, so the composition references the same receipt-ref shapes,
// schema constants, and composable-primitive vocabulary the primitives own.

import { Schema as S } from 'effect'

import {
  fineTuningJobReceiptRef,
} from './cloud/fine-tuning-service-routes'
import {
  sandboxRentalReceiptRef,
} from './cloud/sandbox-compute-service-routes'
import { inferenceChargeReceiptRef } from './inference/metering-hook'
import type { MarketplaceComposablePrimitive } from './marketplace-product-composition'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const AUTOPILOT_COMPOSED_RUN_SCHEMA =
  'openagents.autopilot_composed_run.v1' as const

// The two capstone promises this composition shape sits under. BOTH stay
// planned; the composed run makes no live-product claim for either.
export const AUTOPILOT_ALL_IN_ONE_PROMISE =
  'autopilot.all_in_one_business_system.v1' as const
export const CLOUD_PRIMITIVES_SUITE_PROMISE =
  'cloud.primitives_suite.v1' as const

// Blockers this scaffold documents (and does NOT clear): a typed shape is not a
// composed product, unified billing, or a real-business receipt.
export const COMPOSED_RUN_COMPOSITION_UNBUILT_REF =
  'blocker.product_promises.autopilot_business_system_composition_unbuilt' as const
export const COMPOSED_RUN_UNIFIED_BILLING_UNBUILT_REF =
  'blocker.product_promises.autopilot_business_system_unified_billing_unbuilt' as const
export const COMPOSED_RUN_REAL_BUSINESS_RECEIPT_MISSING_REF =
  'blocker.product_promises.autopilot_business_system_real_business_receipt_missing' as const

/**
 * The composable primitives a run can include. Reuses the marketplace
 * composable-primitive vocabulary so the composition speaks ONE language with
 * the compose-and-list surface (#5515); a run is built from the SAME layers a
 * product is composed from.
 */
export type ComposedRunPrimitive = MarketplaceComposablePrimitive

/**
 * The public-safe receipt-ref a component would settle under, derived from the
 * owning primitive scaffold's receipt-ref helper where one exists. Never a raw
 * amount, destination, or payment material — only a dereferenceable ref shape.
 */
const componentReceiptRef = (
  primitive: ComposedRunPrimitive,
  componentRunId: string,
): string => {
  switch (primitive) {
    case 'inference':
      return inferenceChargeReceiptRef(componentRunId)
    case 'fine_tuning':
      return fineTuningJobReceiptRef(componentRunId)
    case 'sandbox':
      return sandboxRentalReceiptRef(componentRunId)
    // Primitives whose scaffolds do not yet own a receipt-ref helper get a
    // neutral, namespaced placeholder ref so the envelope shape stays uniform.
    default:
      return `receipt.autopilot.composed_run.component.${primitive}.${componentRunId}`
  }
}

/**
 * One primitive component of a composed run: which primitive it is, the
 * capability ref it points at, its own per-component run id, and the
 * public-safe receipt ref it would settle under. PLAN only — INERT.
 */
export const ComposedRunComponent = S.Struct({
  primitive: S.String,
  /** Neutral capability ref (e.g. a promise id or capability id). */
  capabilityRef: S.String,
  /** Per-component run id (job/sandbox/request id shape). */
  componentRunId: S.String,
  /** Public-safe receipt ref this component would settle under. */
  componentReceiptRef: S.String,
})
export type ComposedRunComponent = typeof ComposedRunComponent.Type

/**
 * The single shared balance a composed run debits. INERT: no real balance is
 * read or debited; this is only the ref + asset shape the unified balance WOULD
 * have. The asset speaks the shared credit<->Bitcoin boundary vocabulary.
 */
export const ComposedRunBalance = S.Struct({
  /** Neutral balance ref (account/agent balance ref). */
  balanceRef: S.String,
  /** The asset the balance is denominated in. */
  asset: S.Literals(['credit', 'bitcoin', 'usd', 'free']),
})
export type ComposedRunBalance = typeof ComposedRunBalance.Type

/**
 * The single receipt envelope a composed run settles under — the ONE receipt
 * that references every component receipt ref. INERT: nothing is written.
 */
export const ComposedRunReceiptEnvelope = S.Struct({
  /** Public-safe envelope ref for the whole run. */
  envelopeRef: S.String,
  /** The component receipt refs this envelope composes. */
  componentReceiptRefs: S.Array(S.String),
})
export type ComposedRunReceiptEnvelope = typeof ComposedRunReceiptEnvelope.Type

/**
 * A typed composed-run plan: a business ref, the one balance the run shares,
 * the primitive components, and the one receipt envelope they settle under.
 * PLAN only — no provisioning, metering, debit, or settlement.
 */
export const ComposedRunPlan = S.Struct({
  schema: S.Literal(AUTOPILOT_COMPOSED_RUN_SCHEMA),
  /** Stable run id. */
  runId: S.String,
  /** Neutral business ref (agent/user/customer ref); no name is required. */
  businessRef: S.String,
  title: S.String,
  summary: S.String,
  balance: ComposedRunBalance,
  components: S.Array(ComposedRunComponent),
  receiptEnvelope: ComposedRunReceiptEnvelope,
  /** Always BOTH capstone promises, planned — the run over-claims neither. */
  promiseIds: S.Tuple([
    S.Literal(AUTOPILOT_ALL_IN_ONE_PROMISE),
    S.Literal(CLOUD_PRIMITIVES_SUITE_PROMISE),
  ]),
  /** Always planned — the run is a shape, not a live/billable product. */
  promiseState: S.Literal('planned'),
  /** Always true — the run is INERT (moves no money, provisions nothing). */
  inert: S.Literal(true),
  /**
   * The blockers this composition shape documents and does NOT clear: a typed
   * shape is not a composed product, unified billing, or a real-business
   * receipt.
   */
  unclearedBlockerRefs: S.Array(S.String),
  createdAt: S.String,
})
export type ComposedRunPlan = typeof ComposedRunPlan.Type

export class ComposedRunValidationError extends S.TaggedErrorClass<ComposedRunValidationError>()(
  'ComposedRunValidationError',
  {
    reason: S.String,
  },
) {}

const isNonEmpty = (value: string): boolean => value.trim().length > 0

/** Raw component input for building a composed-run plan. */
export type ComposedRunComponentInput = {
  primitive: ComposedRunPrimitive
  capabilityRef: string
  componentRunId: string
}

/**
 * Build a typed composed-run plan from raw input. PURE and validating:
 *   - requires non-empty run id / business ref / title;
 *   - requires a non-empty balance ref;
 *   - requires >= 2 components (the COMPOSITION invariant: an all-in-one run is
 *     >= 2 primitives composed on ONE balance, mirroring the promise's "at
 *     least two composed primitives" verification gate);
 *   - requires each component to name a primitive + non-empty capability ref +
 *     non-empty component run id;
 *   - derives each component's public-safe receipt ref from the owning
 *     primitive scaffold and assembles the ONE receipt envelope over them;
 *   - pins BOTH capstone promise ids to planned so the plan can never
 *     over-claim, and records the uncleared blockers.
 */
export const buildComposedRunPlan = (input: {
  runId: string
  businessRef: string
  title: string
  summary: string
  balance: ComposedRunBalance
  components: ReadonlyArray<ComposedRunComponentInput>
  createdAt?: string
}):
  | { ok: true; plan: ComposedRunPlan }
  | { ok: false; error: ComposedRunValidationError } => {
  if (!isNonEmpty(input.runId)) {
    return fail('runId must be non-empty')
  }
  if (!isNonEmpty(input.businessRef)) {
    return fail('businessRef must be non-empty')
  }
  if (!isNonEmpty(input.title)) {
    return fail('title must be non-empty')
  }
  if (!isNonEmpty(input.balance.balanceRef)) {
    return fail('balance.balanceRef must be non-empty')
  }
  if (input.components.length < 2) {
    return fail(
      'an all-in-one composed run must compose at least two primitives on one balance',
    )
  }
  for (const component of input.components) {
    if (!isNonEmpty(component.capabilityRef)) {
      return fail(
        `component for ${component.primitive} must have a non-empty capabilityRef`,
      )
    }
    if (!isNonEmpty(component.componentRunId)) {
      return fail(
        `component for ${component.primitive} must have a non-empty componentRunId`,
      )
    }
  }

  const components: ReadonlyArray<ComposedRunComponent> = input.components.map(
    component => ({
      primitive: component.primitive,
      capabilityRef: component.capabilityRef,
      componentRunId: component.componentRunId,
      componentReceiptRef: componentReceiptRef(
        component.primitive,
        component.componentRunId,
      ),
    }),
  )

  return {
    ok: true,
    plan: {
      schema: AUTOPILOT_COMPOSED_RUN_SCHEMA,
      runId: input.runId,
      businessRef: input.businessRef,
      title: input.title,
      summary: input.summary,
      balance: input.balance,
      components,
      receiptEnvelope: {
        envelopeRef: `receipt.autopilot.composed_run.${input.runId}`,
        componentReceiptRefs: components.map(
          component => component.componentReceiptRef,
        ),
      },
      promiseIds: [
        AUTOPILOT_ALL_IN_ONE_PROMISE,
        CLOUD_PRIMITIVES_SUITE_PROMISE,
      ],
      promiseState: 'planned',
      inert: true,
      unclearedBlockerRefs: [
        COMPOSED_RUN_COMPOSITION_UNBUILT_REF,
        COMPOSED_RUN_UNIFIED_BILLING_UNBUILT_REF,
        COMPOSED_RUN_REAL_BUSINESS_RECEIPT_MISSING_REF,
      ],
      createdAt: input.createdAt ?? currentIsoTimestamp(),
    },
  }

  function fail(reason: string): {
    ok: false
    error: ComposedRunValidationError
  } {
    return { ok: false, error: new ComposedRunValidationError({ reason }) }
  }
}

/**
 * The distinct primitives a composed run includes — useful for a listing
 * surface and for confirming the run spans >= 2 primitive kinds.
 */
export const composedRunPrimitives = (
  plan: ComposedRunPlan,
): ReadonlyArray<string> => [
  ...new Set(plan.components.map(component => component.primitive)),
]

/**
 * A read-only composed-run store. Injected so the surface stays pure and
 * testable; the live Worker passes an empty store while the run is INERT.
 */
export type ComposedRunStore = {
  list: () => ReadonlyArray<ComposedRunPlan>
}

export const emptyComposedRunStore: ComposedRunStore = {
  list: () => [],
}

export const makeInMemoryComposedRunStore = (
  plans: ReadonlyArray<ComposedRunPlan>,
): ComposedRunStore => ({
  list: () => plans,
})

/**
 * Staleness contract for the composed-run projection. Built fresh from the
 * injected store on every request, so it is `live_at_read` (maxStaleness 0).
 */
export const ComposedRunStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness(['autopilot_composed_run_changed'])

/**
 * Public-safe composed-run listing projection. Honest: the surface is a
 * scaffold; both capstone promises stay planned and the run is inert.
 */
export const listComposedRuns = (
  store: ComposedRunStore,
): {
  schema: typeof AUTOPILOT_COMPOSED_RUN_SCHEMA
  promiseIds: readonly [
    typeof AUTOPILOT_ALL_IN_ONE_PROMISE,
    typeof CLOUD_PRIMITIVES_SUITE_PROMISE,
  ]
  promiseState: 'planned'
  inert: true
  generatedAt: string
  maxStalenessSeconds: number
  staleness: PublicProjectionStalenessContract
  unclearedBlockerRefs: ReadonlyArray<string>
  runs: ReadonlyArray<ComposedRunPlan>
} => ({
  schema: AUTOPILOT_COMPOSED_RUN_SCHEMA,
  promiseIds: [AUTOPILOT_ALL_IN_ONE_PROMISE, CLOUD_PRIMITIVES_SUITE_PROMISE],
  promiseState: 'planned',
  inert: true,
  generatedAt: currentIsoTimestamp(),
  maxStalenessSeconds: ComposedRunStaleness.maxStalenessSeconds,
  staleness: ComposedRunStaleness,
  unclearedBlockerRefs: [
    COMPOSED_RUN_COMPOSITION_UNBUILT_REF,
    COMPOSED_RUN_UNIFIED_BILLING_UNBUILT_REF,
    COMPOSED_RUN_REAL_BUSINESS_RECEIPT_MISSING_REF,
  ],
  runs: store.list(),
})

/** Read one composed run by id, or null when absent. */
export const readComposedRun = (
  store: ComposedRunStore,
  runId: string,
): ComposedRunPlan | null =>
  store.list().find(run => run.runId === runId) ?? null
