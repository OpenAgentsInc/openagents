// Cache-aware routing for the Khala coordinator (book P0-2, item 6 / #6084).
//
// THE BOOK'S RULE (Inference Engineering §5.3.3 "Cache-Aware Routing"): when an
// inference server makes heavy use of prefix caching, routing must stop dividing
// traffic evenly. "A user in a long conversation with a chatbot or asking
// multiple questions about a codebase should have their request routed to the
// same replica whenever possible so that they get a cache hit for a faster, less
// expensive request."
//
// Khala's `model-router.ts` resolves an ORDERED lane plan (cheapest viable lane
// first, then overflow fallbacks). This module REORDERS that plan for a
// session/codebase/account follow-up so the cache-WARM lane is tried first —
// WITHOUT widening the plan (it only reorders lanes already in the plan, so a
// cache hint can never route to an unviable, unhealthy, or disallowed lane) and
// WITHOUT defeating overflow (the rest of the plan stays as the fallback tail).
//
// TYPED, NOT AD-HOC (workspace semantic-routing rule): the warm lane is looked up
// from a typed `CacheWarmthOracle` keyed by the public-safe cache-affinity HASH —
// never a string match on user intent or prompt text. The oracle is an
// INJECTED capability (a tiny KV/DO-backed map of `affinityHash → warm lane`),
// so this module is pure config + a typed lookup with no I/O of its own.
//
// CONSTRAINTS (the book's "subject to health/privacy/region/spend"): a warm lane
// is only promoted when it is (a) still in the viable plan, (b) reported HEALTHY,
// (c) ALLOWED by the account's privacy/region posture. A warm hint never
// overrides any of these — it only reorders among the lanes that already passed
// every gate.

// A lane id as it appears in a `model-router` plan (e.g. 'fireworks',
// 'vertex-anthropic', 'passthrough-openai'). Kept as a string alias so this
// module does not couple to the adapter-id constants.
export type LaneId = string

// Health posture of a lane, as the coordinator/health scorer reports it. A
// `degraded` or `unhealthy` lane is never promoted by a cache hint (the book's
// "subject to provider health" — a warm cache on a sick replica is a bad trade).
export type LaneHealth = 'healthy' | 'degraded' | 'unhealthy'

// The injected warmth oracle: given the public-safe affinity HASH, return the
// lane that previously served this affinity key (and is therefore cache-warm),
// or undefined when the affinity key has no recorded warm lane (first turn, or
// the record expired). Keyed by the HASH, never the raw key — the oracle never
// sees raw account/session/codebase identifiers.
export type CacheWarmthOracle = (
  affinityHash: string,
) => LaneId | undefined

// Health lookup for a lane. Defaults to `healthy` for any lane the scorer has no
// signal on (absence of a degradation signal is not evidence of degradation).
export type LaneHealthOracle = (lane: LaneId) => LaneHealth

// Privacy / region posture gate: may THIS account's traffic pin to THIS lane?
// `false` forbids the cache hint for a lane the account may not use for
// data-residency / privacy reasons (the book's "subject to privacy, region").
// Defaults to allow-all when the route does not wire a posture.
export type CachePinPolicy = (lane: LaneId) => boolean

// Inputs to a cache-aware reorder decision.
export type CacheAwareRoutingInput = Readonly<{
  // The viable, ordered lane plan from `model-router` (already health/registry
  // filtered upstream; cheapest-viable first). REORDERED, never widened.
  plannedLanes: ReadonlyArray<LaneId>
  // The public-safe cache-affinity hash for this request (from
  // `hashCacheAffinityKey`). `null` when no affinity key applied (no session /
  // codebase / account context) → no reorder, plan returned unchanged.
  affinityHash: string | null
  // The injected warmth oracle. Absent → no warm hint → plan unchanged.
  warmthOracle?: CacheWarmthOracle | undefined
  // The injected health oracle. Absent → every lane treated as healthy.
  healthOracle?: LaneHealthOracle | undefined
  // The injected privacy/region pin policy. Absent → allow every lane.
  pinPolicy?: CachePinPolicy | undefined
}>

// The outcome of a cache-aware reorder: the (possibly reordered) plan plus a
// neutral, public-safe explanation of WHY — so telemetry/tests can assert the
// decision without exposing any raw key.
export type CacheAwareRoutingDecision = Readonly<{
  // The lane plan to dispatch. Same SET as `plannedLanes` (a permutation), so
  // overflow still covers every viable lane; only the ORDER may change.
  lanes: ReadonlyArray<LaneId>
  // The warm lane that was promoted to the front, or null when none was (no
  // affinity hash, no warm record, lane fell out of the plan, or a constraint
  // blocked it).
  warmLane: LaneId | null
  // Neutral reason ref for the decision (public-safe; never a raw key):
  //   'no_affinity'        — no affinity hash → no reorder
  //   'no_warm_record'     — oracle had no warm lane for this hash
  //   'warm_not_in_plan'   — warm lane is not among the viable lanes
  //   'warm_unhealthy'     — warm lane is degraded/unhealthy → not promoted
  //   'warm_pin_forbidden' — privacy/region policy forbids pinning this lane
  //   'already_warm_first' — warm lane was already first → no change needed
  //   'promoted_warm_lane' — warm lane promoted to the front of the plan
  reason:
    | 'no_affinity'
    | 'no_warm_record'
    | 'warm_not_in_plan'
    | 'warm_unhealthy'
    | 'warm_pin_forbidden'
    | 'already_warm_first'
    | 'promoted_warm_lane'
}>

// Decide the cache-aware lane order. PURE + deterministic. Promotes the
// cache-warm lane to the FRONT of the plan when — and only when — every gate
// passes; otherwise returns the plan unchanged with the neutral reason.
//
// The reorder is a stable move-to-front: the warm lane is pulled to index 0 and
// the remaining lanes keep their original relative order (so the cheapest-viable
// fallback tail is preserved exactly behind the warm lane).
export const decideCacheAwareRouting = (
  input: CacheAwareRoutingInput,
): CacheAwareRoutingDecision => {
  const unchanged = (
    reason: CacheAwareRoutingDecision['reason'],
  ): CacheAwareRoutingDecision => ({
    lanes: input.plannedLanes,
    reason,
    warmLane: null,
  })

  if (input.affinityHash === null) {
    return unchanged('no_affinity')
  }
  if (input.warmthOracle === undefined) {
    return unchanged('no_warm_record')
  }

  const warm = input.warmthOracle(input.affinityHash)
  if (warm === undefined) {
    return unchanged('no_warm_record')
  }
  if (!input.plannedLanes.includes(warm)) {
    // The warm lane is no longer viable (dropped by registry/health upstream).
    // Never widen the plan to re-add it — fall back to the cheapest-viable plan.
    return unchanged('warm_not_in_plan')
  }

  const health = (input.healthOracle ?? (() => 'healthy'))(warm)
  if (health !== 'healthy') {
    return unchanged('warm_unhealthy')
  }

  const allowed = (input.pinPolicy ?? (() => true))(warm)
  if (!allowed) {
    return unchanged('warm_pin_forbidden')
  }

  if (input.plannedLanes[0] === warm) {
    // Already the primary lane — cache-aware order matches cheapest-viable order.
    return {
      lanes: input.plannedLanes,
      reason: 'already_warm_first',
      warmLane: warm,
    }
  }

  // Stable move-to-front: warm lane first, the rest in original relative order.
  const rest = input.plannedLanes.filter(lane => lane !== warm)
  return {
    lanes: [warm, ...rest],
    reason: 'promoted_warm_lane',
    warmLane: warm,
  }
}
