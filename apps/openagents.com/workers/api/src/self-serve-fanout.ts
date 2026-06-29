// Self-serve control-center fanout — the typed, customer-initiated shape of ONE
// unit of Autopilot work being fanned out across the open agent labor market in
// a SINGLE self-serve action (promise
// autopilot.control_center_fanout_marketplace.v1, yellow).
//
// THE GAP THIS CLOSES: the promise's first-live evidence (#4783, P7 lane-C)
// proved a single work order can burst its owned capacity dark and fan out to
// the open market, but that run was OPERATOR-STAGED — the operator manually
// staged the order and a second manual step listed the linked market work
// request. The promise's safeCopy is explicit: "Self-serve customer-initiated
// fanout (the run was operator-staged) ... [is] not live", carried as
// blocker.product_promises.self_serve_fanout_missing.
//
// This module is the missing self-serve capability: a customer (not an operator)
// initiates a fanout for their own work order in ONE action, the server-side
// lane-C gate is evaluated (reusing evaluateLaneCFanoutForWorkOrder so the
// public-trust floor + opt-in + budget cap stay enforced server-side), and the
// linked market work-request the fanout WOULD list is assembled in the same
// step. No more operator staging and no second manual listing call.
//
// SCOPE / HONESTY: this is FLAG-GATED INERT where it would touch real dispatch.
// The plan is PURE: it lists nothing on the market, opens no escrow, and moves
// no money — it assembles a typed fanout PLAN over the existing lane-C gate. The
// dispatch seam (`dispatchSelfServeFanout`) is INERT by default (`enabled:
// false` => `disabled`, no market write); only when armed AND the gate is ready
// AND the customer opted in does it surface the market work-request input the
// requester would list. The promise STAYS yellow: a self-serve plan + an inert
// dispatch seam is not a broad live marketplace. A green flip stays
// receipt-first and owner-signed per proof.claim_upgrade_receipts.v1.

import { Effect, Schema as S } from 'effect'

import {
  type LaneCFanoutBridgeInput,
  type LaneCFanoutBridgeResult,
  evaluateLaneCFanoutForWorkOrder,
  laneCFanoutObjectiveRef,
} from './lane-c-fanout-bridge'
import {
  MARKETPLACE_LIVE_WORK_CLASS,
  MarketplaceWorkClassId,
  getMarketplaceWorkClass,
  isMarketplaceWorkClassLive,
} from './marketplace-work-class-catalog'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const SELF_SERVE_FANOUT_SCHEMA =
  'openagents.self_serve_fanout.v1' as const

// The yellow promise this capability sits under. It STAYS yellow; the plan makes
// no broad-live-marketplace claim.
export const SELF_SERVE_FANOUT_PROMISE =
  'autopilot.control_center_fanout_marketplace.v1' as const

// Default work class for backward-compatible fanout requests.
export const SELF_SERVE_FANOUT_WORK_CLASS = MARKETPLACE_LIVE_WORK_CLASS

// The default public capability the market job requires (a local coding agent).
export const SELF_SERVE_FANOUT_CAPABILITY_REF =
  'capability.pylon.local_claude_agent' as const

// The default public verification command the market job carries.
export const SELF_SERVE_FANOUT_VERIFICATION_COMMAND_REF =
  'command.public.pylon.labor.bun_test' as const

// The blocker this self-serve capability CLEARS once deployed + dereferenceable:
// a customer-initiated single-action fanout planner/route exists, vs the
// previously operator-staged run.
export const SELF_SERVE_FANOUT_CLEARED_BLOCKER_REF =
  'blocker.product_promises.self_serve_fanout_missing' as const

// Historical blocker ref retained for old receipts and public projections.
export const SELF_SERVE_FANOUT_PLUGIN_MARKETPLACE_BLOCKER_REF =
  'blocker.product_promises.plugin_marketplace_beyond_code_task_missing' as const

/**
 * The customer-initiated request to fan one of their OWN work orders out across
 * the open agent labor market in a single self-serve action. Neutral refs only;
 * the customer supplies their opt-in and a budget cap. The placement/policy
 * facts come from the work order projection on the server side — the customer
 * never asserts them.
 */
export const SelfServeFanoutInput = S.Struct({
  /** The customer's own work order ref being fanned out. */
  workOrderRef: S.String,
  /** Neutral customer ref initiating the fanout (e.g. "agent:<id>"). */
  customerRef: S.String,
  /** The customer's explicit opt-in to leave the first-party lanes. */
  customerOptIn: S.Boolean,
  /** Public-safe budget cap in whole sats (>= 0). */
  budgetCapSats: S.Number,
  /** Short public-safe title for the market listing. */
  title: S.String,
  /** Typed marketplace work class. Defaults to code_task when omitted. */
  workClass: S.optionalKey(MarketplaceWorkClassId),
})
export type SelfServeFanoutInput = typeof SelfServeFanoutInput.Type

/**
 * The market work-request the fanout WOULD list. INERT in the plan — nothing is
 * listed on the market here; this is the exact input the requester would POST to
 * /api/forum/work-requests, assembled in the SAME self-serve step.
 */
export const SelfServeFanoutMarketWorkRequest = S.Struct({
  /** Public-safe objective ref derived from the work order. */
  objectiveRef: S.String,
  /** Budget for the market job in whole sats. */
  budgetSats: S.Number,
  /** Public-safe listing title. */
  title: S.String,
  /** Capabilities the market job requires (local coding agent). */
  requiredCapabilityRefs: S.Array(S.String),
  /** Public-safe verification command the validator re-runs. */
  verificationCommandRef: S.String,
  /** Public-safe deadline ref. */
  deadlineRef: S.String,
  /** Typed supported marketplace work class. */
  workClass: MarketplaceWorkClassId,
})
export type SelfServeFanoutMarketWorkRequest =
  typeof SelfServeFanoutMarketWorkRequest.Type

/**
 * A typed self-serve fanout plan: who initiated it, the lane-C gate decision,
 * whether the gate authorized a market fanout, and the market work-request the
 * fanout would list. PLAN only — no market listing, escrow, dispatch, or
 * settlement is performed.
 */
export const SelfServeFanoutPlan = S.Struct({
  schema: S.Literal(SELF_SERVE_FANOUT_SCHEMA),
  /** Stable plan id (derived from the work order ref). */
  planId: S.String,
  workOrderRef: S.String,
  /** Neutral customer ref that initiated the self-serve fanout. */
  customerRef: S.String,
  /** True — initiated by the customer, not operator-staged. */
  selfServe: S.Literal(true),
  /** Typed supported marketplace work class. */
  workClass: MarketplaceWorkClassId,
  /** The lane-C gate decision (server-side floor + opt-in + budget cap). */
  gate: S.Struct({
    lane: S.Literals(['owned_capacity', 'public_market']),
    state: S.Literals(['blocked', 'ready']),
    ownedCapacityState: S.Literals(['available', 'dark', 'limited']),
    reasonRefs: S.Array(S.String),
  }),
  /** Whether the gate authorized a market fanout (ready + public_market). */
  readyForMarket: S.Boolean,
  /**
   * The market work-request the fanout would list, present only when the gate is
   * ready. Null when the gate blocked the fanout (e.g. opt-out, private tier,
   * budget cap exceeded).
   */
  marketWorkRequest: S.NullOr(SelfServeFanoutMarketWorkRequest),
  /** Always the yellow promise — the plan over-claims nothing. */
  promiseIds: S.Tuple([S.Literal(SELF_SERVE_FANOUT_PROMISE)]),
  /** Always yellow — a self-serve plan is not a broad live marketplace. */
  promiseState: S.Literal('yellow'),
  /** Always true — the plan is INERT (lists nothing, dispatches nothing). */
  inert: S.Literal(true),
  /** The blocker this capability clears (for evidence). */
  clearedBlockerRefs: S.Array(S.String),
  /** The blocker this capability documents and does NOT clear. */
  unclearedBlockerRefs: S.Array(S.String),
  createdAt: S.String,
})
export type SelfServeFanoutPlan = typeof SelfServeFanoutPlan.Type

export class SelfServeFanoutValidationError extends S.TaggedErrorClass<SelfServeFanoutValidationError>()(
  'SelfServeFanoutValidationError',
  {
    reason: S.String,
  },
) {}

const isNonEmpty = (value: string): boolean => value.trim().length > 0

const isWholeNonNegative = (value: number): boolean =>
  Number.isInteger(value) && value >= 0

/** Stable, public-safe plan id derived from a work order ref and work class. */
export const selfServeFanoutPlanId = (
  workOrderRef: string,
  workClass: MarketplaceWorkClassId = SELF_SERVE_FANOUT_WORK_CLASS,
): string => {
  const safeWorkOrderRef = workOrderRef.replace(/[^a-z0-9._-]+/giu, '_')
  if (workClass === SELF_SERVE_FANOUT_WORK_CLASS) {
    return `self_serve_fanout.${safeWorkOrderRef}`
  }
  const safeWorkClass = workClass.replace(/[^a-z0-9._-]+/giu, '_')
  return `self_serve_fanout.${safeWorkOrderRef}.${safeWorkClass}`
}

/** Public-safe deadline ref for a self-serve fanout market job. */
export const SELF_SERVE_FANOUT_DEADLINE_REF =
  'deadline.public.self_serve_fanout.20261231' as const

/**
 * The server-side placement/policy + readiness facts the work order projection
 * supplies for the gate. Customer input is layered on top of these — the
 * customer can never assert the placement source, privacy tier, or readiness.
 */
export type SelfServeFanoutWorkOrderFacts = Readonly<{
  placementSource: string
  placementAvailabilityState: string
  privacyTier: string
  settlementBridgeReady: boolean
  marketInventoryReady: boolean
  artifactAuthorityReady: boolean
  validatorPolicyReady: boolean
  missionWorkOrderUnified: boolean
  providerTrustTier: LaneCFanoutBridgeInput['providerTrustTier']
}>

/**
 * Build a typed self-serve fanout plan. PURE and validating:
 *   - requires non-empty work order ref / customer ref / title;
 *   - requires a whole non-negative budget cap in sats;
 *   - evaluates the EXISTING lane-C gate over the server-supplied placement +
 *     policy facts and the customer's opt-in + budget cap (so the public-trust
 *     floor stays enforced server-side and a private order can never fan out);
 *   - assembles the linked market work-request in the same step when the gate is
 *     ready, leaving it null when the gate blocked the fanout;
 *   - pins the yellow promise, records the cleared self-serve blocker and the
 *     current blocker state so the plan can never over-claim.
 *
 * Never throws.
 */
export const buildSelfServeFanoutPlan = (
  input: SelfServeFanoutInput,
  facts: SelfServeFanoutWorkOrderFacts,
  createdAt?: string,
):
  | { ok: true; plan: SelfServeFanoutPlan }
  | { ok: false; error: SelfServeFanoutValidationError } => {
  if (!isNonEmpty(input.workOrderRef)) {
    return fail('workOrderRef must be non-empty')
  }
  if (!isNonEmpty(input.customerRef)) {
    return fail('customerRef must be non-empty')
  }
  if (!isNonEmpty(input.title)) {
    return fail('title must be non-empty')
  }
  if (!isWholeNonNegative(input.budgetCapSats)) {
    return fail('budgetCapSats must be a whole non-negative number of sats')
  }
  const workClass = input.workClass ?? SELF_SERVE_FANOUT_WORK_CLASS
  const workClassContract = getMarketplaceWorkClass(workClass)
  if (
    workClassContract === null ||
    !isMarketplaceWorkClassLive(workClass)
  ) {
    return fail(`workClass must be a live marketplace work class: ${workClass}`)
  }

  const fanout: LaneCFanoutBridgeResult = evaluateLaneCFanoutForWorkOrder({
    placementSource: facts.placementSource,
    placementAvailabilityState: facts.placementAvailabilityState,
    privacyTier: facts.privacyTier,
    customerOptIn: input.customerOptIn,
    budgetCapSats: input.budgetCapSats,
    // The fanout authorizes market quotes up to the budget cap; the per-quote
    // budget check is enforced again at escrow-reserve time on acceptance.
    quotedSats: input.budgetCapSats,
    settlementBridgeReady: facts.settlementBridgeReady,
    marketInventoryReady: facts.marketInventoryReady,
    artifactAuthorityReady: facts.artifactAuthorityReady,
    validatorPolicyReady: facts.validatorPolicyReady,
    missionWorkOrderUnified: facts.missionWorkOrderUnified,
    providerTrustTier: facts.providerTrustTier,
  })

  const objectiveRef = laneCFanoutObjectiveRef(input.workOrderRef)
  const marketWorkRequest: SelfServeFanoutMarketWorkRequest | null =
    fanout.readyForMarket
      ? {
          objectiveRef,
          budgetSats: input.budgetCapSats,
          title: input.title.slice(0, 160),
          requiredCapabilityRefs: [...workClassContract.requiredCapabilityRefs],
          verificationCommandRef: workClassContract.verificationCommandRef,
          deadlineRef: SELF_SERVE_FANOUT_DEADLINE_REF,
          workClass,
        }
      : null

  return {
    ok: true,
    plan: {
      schema: SELF_SERVE_FANOUT_SCHEMA,
      planId: selfServeFanoutPlanId(input.workOrderRef, workClass),
      workOrderRef: input.workOrderRef,
      customerRef: input.customerRef,
      selfServe: true,
      workClass,
      gate: {
        lane: fanout.decision.lane,
        state: fanout.decision.state,
        ownedCapacityState: fanout.ownedCapacityState,
        reasonRefs: fanout.decision.reasonRefs,
      },
      readyForMarket: fanout.readyForMarket,
      marketWorkRequest,
      promiseIds: [SELF_SERVE_FANOUT_PROMISE],
      promiseState: 'yellow',
      inert: true,
      clearedBlockerRefs: [SELF_SERVE_FANOUT_CLEARED_BLOCKER_REF],
      unclearedBlockerRefs: [],
      createdAt: createdAt ?? currentIsoTimestamp(),
    },
  }

  function fail(reason: string): {
    ok: false
    error: SelfServeFanoutValidationError
  } {
    return {
      ok: false,
      error: new SelfServeFanoutValidationError({ reason }),
    }
  }
}

// ---------------------------------------------------------------------------
// Dispatch seam (FLAG-GATED INERT)
// ---------------------------------------------------------------------------

export const SELF_SERVE_FANOUT_DISPATCH_DISABLED_REF =
  'blocker.self_serve_fanout.dispatch_flag_disabled' as const
export const SELF_SERVE_FANOUT_DISPATCH_GATE_BLOCKED_REF =
  'blocker.self_serve_fanout.gate_blocked' as const

export type DispatchSelfServeFanoutInput = Readonly<{
  /** The plan whose fanout would be dispatched. Must be ready for market. */
  plan: SelfServeFanoutPlan
}>

export type DispatchSelfServeFanoutResult =
  // Flag off: planned the dispatch, listed NOTHING. The default path.
  | Readonly<{ _tag: 'disabled'; planId: string }>
  // The gate blocked the fanout (opt-out, private tier, budget cap, ...): no
  // market listing.
  | Readonly<{ _tag: 'blocked'; planId: string; reasonRefs: ReadonlyArray<string> }>
  // Armed + gate-ready: surfaces the market work-request input the requester
  // would list. This is still INERT at this layer — it lists nothing itself; it
  // returns the authorized input the caller would POST to the market route.
  | Readonly<{
      _tag: 'authorized'
      planId: string
      marketWorkRequest: SelfServeFanoutMarketWorkRequest
    }>

export type DispatchSelfServeFanoutDeps = Readonly<{
  // FLAG: the seam is INERT unless this is true. Off => `disabled`, no listing.
  enabled: boolean
}>

/**
 * Dispatch ONE self-serve fanout plan to the open market.
 *
 * Flow:
 * 1. When the flag is off, return `disabled` — plan only, nothing listed. (Even
 *    before inspecting the gate, so an inert seam never acts on gates it will not
 *    use.)
 * 2. When the gate blocked the fanout (or the market work-request is absent),
 *    return `blocked` with the gate's reason refs — no listing.
 * 3. Otherwise return `authorized` with the market work-request input the
 *    requester would POST to /api/forum/work-requests.
 *
 * INERT by default. Never throws. Even when authorized, THIS layer writes no
 * market listing, opens no escrow, and moves no money — it surfaces the
 * authorized input only.
 */
export const dispatchSelfServeFanout = (
  deps: DispatchSelfServeFanoutDeps,
  input: DispatchSelfServeFanoutInput,
): Effect.Effect<DispatchSelfServeFanoutResult> => {
  const planId = input.plan.planId

  // FLAG-GATED INERT: by default the seam plans but lists nothing.
  if (!deps.enabled) {
    return Effect.succeed({ _tag: 'disabled', planId } as const)
  }

  if (!input.plan.readyForMarket || input.plan.marketWorkRequest === null) {
    return Effect.succeed({
      _tag: 'blocked',
      planId,
      reasonRefs: input.plan.gate.reasonRefs,
    } as const)
  }

  return Effect.succeed({
    _tag: 'authorized',
    planId,
    marketWorkRequest: input.plan.marketWorkRequest,
  } as const)
}

// ---------------------------------------------------------------------------
// Read-only store + public projection
// ---------------------------------------------------------------------------

/**
 * A read-only self-serve fanout plan store. Injected so the surface stays pure
 * and testable; the live Worker passes an empty store while INERT.
 */
export type SelfServeFanoutStore = {
  list: () => ReadonlyArray<SelfServeFanoutPlan>
}

export const emptySelfServeFanoutStore: SelfServeFanoutStore = {
  list: () => [],
}

export const makeInMemorySelfServeFanoutStore = (
  plans: ReadonlyArray<SelfServeFanoutPlan>,
): SelfServeFanoutStore => ({
  list: () => plans,
})

/**
 * Staleness contract for the self-serve fanout projection. Built fresh from the
 * injected store on every request, so it is `live_at_read` (maxStaleness 0).
 */
export const SelfServeFanoutStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness(['self_serve_fanout_plan_changed'])

/**
 * Public-safe self-serve fanout listing projection. Honest: the capability is a
 * self-serve planner; the promise stays yellow and every plan is inert.
 */
export const listSelfServeFanoutPlans = (
  store: SelfServeFanoutStore,
): {
  schema: typeof SELF_SERVE_FANOUT_SCHEMA
  promiseIds: readonly [typeof SELF_SERVE_FANOUT_PROMISE]
  promiseState: 'yellow'
  inert: true
  selfServe: true
  workClass: typeof SELF_SERVE_FANOUT_WORK_CLASS
  generatedAt: string
  maxStalenessSeconds: number
  staleness: PublicProjectionStalenessContract
  clearedBlockerRefs: ReadonlyArray<string>
  unclearedBlockerRefs: ReadonlyArray<string>
  plans: ReadonlyArray<SelfServeFanoutPlan>
} => ({
  schema: SELF_SERVE_FANOUT_SCHEMA,
  promiseIds: [SELF_SERVE_FANOUT_PROMISE],
  promiseState: 'yellow',
  inert: true,
  selfServe: true,
  workClass: SELF_SERVE_FANOUT_WORK_CLASS,
  generatedAt: currentIsoTimestamp(),
  maxStalenessSeconds: SelfServeFanoutStaleness.maxStalenessSeconds,
  staleness: SelfServeFanoutStaleness,
  clearedBlockerRefs: [SELF_SERVE_FANOUT_CLEARED_BLOCKER_REF],
  unclearedBlockerRefs: [],
  plans: store.list(),
})

/** Read one self-serve fanout plan by plan id, or null when absent. */
export const readSelfServeFanoutPlan = (
  store: SelfServeFanoutStore,
  planId: string,
): SelfServeFanoutPlan | null =>
  store.list().find(plan => plan.planId === planId) ?? null
