// Khala M4 — Pylon serving ADMISSION gate (EPIC #6017, #6012; Workstream C).
//
// THE M4 SUPPLY-SIDE GAP this module fills. The serving-fabric dispatch
// (`psionic-fabric-serve.ts`), the per-stage payout decision
// (`serving-node-payout.ts`), and the end-to-end serve->parity->settle loop
// (`khala-loop-integration.ts`) all already exist and are tested. What was
// MISSING is the issue's "Capability/registration" task:
//
//   "route to Pylons advertising the right capability + wallet/assignment
//    readiness (NIP-89 refs + heartbeat)."
//
// Nothing in the inference path consumed the live Pylon registration/heartbeat
// surface (`pylon-api.ts`) to decide whether a target Pylon is actually ADMITTED
// to serve a Khala request before dispatching to it. This module is that gate: a
// PURE, fail-closed admission decision over a public-safe projection of a Pylon's
// registration record. The loop consults it FIRST; a non-admitted Pylon is never
// routed to, so a missing capability / stale heartbeat / unready wallet degrades
// SAFELY (the request falls back to the existing cloud adapters) and never serves
// or pays against an unready node.
//
// BOUNDARIES. This module holds NO money authority and never reaches into Psionic
// execution. It is PURE: it consumes a projection + the required capability + a
// reference clock and returns a typed decision. It never reads D1, never calls a
// transport, never throws, never logs. The actual registration record + Spark
// payout-target readiness are resolved upstream (`pylon-api.ts`) and projected
// into this gate's input; the gate only decides admit/refuse from that snapshot.
//
// PUBLIC-SAFE. The input carries only attribution refs (node ref, capability
// refs, NIP-90 lane refs, the redacted `payout.spark.<digest>` ref) and boolean
// readiness — never a raw `spark1…` address, invoice, preimage, or pubkey-bound
// payment material. The blocker refs are neutral, stable strings.

// ----------------------------------------------------------------------------
// Public-safe blocker reason refs (neutral; never payment material)
// ----------------------------------------------------------------------------

export const PYLON_ADMISSION_NOT_ACTIVE_REF =
  'blocker.pylon_admission.registration_not_active'
export const PYLON_ADMISSION_CAPABILITY_MISSING_REF =
  'blocker.pylon_admission.capability_not_advertised'
export const PYLON_ADMISSION_NO_HEARTBEAT_REF =
  'blocker.pylon_admission.no_heartbeat'
export const PYLON_ADMISSION_HEARTBEAT_UNHEALTHY_REF =
  'blocker.pylon_admission.heartbeat_not_healthy'
export const PYLON_ADMISSION_HEARTBEAT_STALE_REF =
  'blocker.pylon_admission.heartbeat_stale'
export const PYLON_ADMISSION_NO_SERVING_LANE_REF =
  'blocker.pylon_admission.no_serving_lane_advert'
export const PYLON_ADMISSION_WALLET_NOT_READY_REF =
  'blocker.pylon_admission.wallet_not_ready'
export const PYLON_ADMISSION_NO_PAYOUT_TARGET_REF =
  'blocker.pylon_admission.no_spark_payout_target'

// Public-safe policy ref stamped on every admission decision.
export const PYLON_ADMISSION_POLICY_REF = 'policy.khala_pylon_admission.v1'

// The default heartbeat freshness window (ms). A heartbeat older than this is
// STALE — the node is not provably online right now, so it is not admitted to
// take live serving work. Conservative default; tunable per call.
export const DEFAULT_HEARTBEAT_TTL_MS = 90_000

// The heartbeat statuses that count as a HEALTHY, serve-ready node. Anything else
// (degraded / draining / blocked / unknown) is refused. Kept narrow + explicit so
// a new status string fails closed (not admitted) until it is deliberately added.
const HEALTHY_HEARTBEAT_STATUSES: ReadonlySet<string> = new Set([
  'ok',
  'healthy',
  'active',
  'ready',
])

// ----------------------------------------------------------------------------
// The public-safe Pylon serving snapshot the gate consumes
// ----------------------------------------------------------------------------

// A minimal, public-safe projection of a Pylon's live registration + heartbeat +
// payout readiness — exactly the fields the admission decision needs. Built
// upstream from `PylonApiRegistrationRecord` + the Spark payout-target store
// (`pylon-api.ts`); this gate never sees the raw record or any payment material.
export type PylonServingSnapshot = Readonly<{
  // Public-safe node id of the candidate serving Pylon (the would-be payout
  // recipient). Attribution ref only, never wallet material.
  pylonRef: string
  // The registration status. Only `active` is admissible.
  status: 'active' | 'blocked' | 'retired'
  // The capability refs the node ADVERTISES (NIP-89 handler-information style).
  // The required serving capability must be present for admission.
  capabilityRefs: ReadonlyArray<string>
  // The NIP-90 serving lane refs the node advertises (its serving offer). At
  // least one must be present so the node is actually offering serving work.
  servingLaneRefs: ReadonlyArray<string>
  // ISO timestamp of the node's latest heartbeat, or null if it never sent one.
  // null / stale (older than the TTL) => not provably online => refused.
  latestHeartbeatAt: string | null
  // The node's latest heartbeat status string (e.g. 'ok'). Must be a healthy,
  // serve-ready status. null / unknown => refused (fails closed).
  latestHeartbeatStatus: string | null
  // Whether the node's wallet is receive-ready (published by the heartbeat,
  // recomputed from the live payout-target store). Must be true to admit — an
  // admitted serving node must be able to RECEIVE its Bitcoin payout.
  walletReady: boolean
  // The redacted `payout.spark.<digest>` ref when a Spark payout target is
  // registered + ready, else null. A ready target is required so the eventual
  // RL-2/RL-3 settlement has a registered destination (never the raw address).
  sparkPayoutTargetRef: string | null
}>

// ----------------------------------------------------------------------------
// The admission decision
// ----------------------------------------------------------------------------

export type PylonAdmissionInput = Readonly<{
  // The candidate Pylon's public-safe serving snapshot.
  snapshot: PylonServingSnapshot
  // The capability ref the Khala request REQUIRES (e.g. the small-model serving
  // capability for `openagents/khala-mini`). The node must advertise it.
  requiredCapabilityRef: string
  // The reference clock (ms since epoch) heartbeat freshness is measured against.
  // Injected (the gate never reads the wall clock itself) so it stays PURE + the
  // freshness check is deterministic in tests.
  nowMs: number
  // Heartbeat freshness window (ms). Defaults to DEFAULT_HEARTBEAT_TTL_MS.
  heartbeatTtlMs?: number | undefined
}>

export type PylonAdmissionDecision = Readonly<{
  schema: 'openagents.khala_pylon_admission.v1'
  pylonRef: string
  // Whether the Pylon is admitted to take a live Khala serving request. True ONLY
  // when EVERY gate passes; any failure => false + the neutral blocker refs.
  admitted: boolean
  // Public-safe blocker refs (empty iff admitted). Neutral, stable strings.
  blockerRefs: ReadonlyArray<string>
  // Public-safe policy refs.
  policyRefs: ReadonlyArray<string>
}>

// Parse an ISO timestamp to ms since epoch, or undefined if unparseable. Pure;
// no exceptions (an invalid timestamp is treated as "no usable heartbeat").
const isoToMs = (iso: string | null): number | undefined => {
  if (iso === null) return undefined
  const ms = Date.parse(iso)
  return Number.isFinite(ms) ? ms : undefined
}

// Decide whether a candidate Pylon is admitted to serve a Khala request. PURE +
// FAIL-CLOSED: runs every readiness gate and admits ONLY when all pass. The gates,
// each contributing a neutral blocker ref on failure:
//   1. registration is `active`;
//   2. the required serving capability is advertised (NIP-89 refs);
//   3. a serving lane is advertised (the node is actually offering serving work);
//   4. a heartbeat exists, is a healthy status, and is FRESH within the TTL
//      (the node is provably online right now);
//   5. the wallet is receive-ready AND a Spark payout target is registered
//      (the node can RECEIVE its eventual Bitcoin payout).
// Never throws, never logs, never moves money.
export const decidePylonAdmission = (
  input: PylonAdmissionInput,
): PylonAdmissionDecision => {
  const { snapshot } = input
  const ttl =
    typeof input.heartbeatTtlMs === 'number' && input.heartbeatTtlMs > 0
      ? input.heartbeatTtlMs
      : DEFAULT_HEARTBEAT_TTL_MS

  const blockerRefs: string[] = []

  // GATE 1 — registration is active.
  if (snapshot.status !== 'active') {
    blockerRefs.push(PYLON_ADMISSION_NOT_ACTIVE_REF)
  }

  // GATE 2 — the required serving capability is advertised.
  if (!snapshot.capabilityRefs.includes(input.requiredCapabilityRef)) {
    blockerRefs.push(PYLON_ADMISSION_CAPABILITY_MISSING_REF)
  }

  // GATE 3 — a serving lane is advertised (the node is offering serving work).
  if (snapshot.servingLaneRefs.length === 0) {
    blockerRefs.push(PYLON_ADMISSION_NO_SERVING_LANE_REF)
  }

  // GATE 4 — a fresh, healthy heartbeat (provably online right now).
  const heartbeatMs = isoToMs(snapshot.latestHeartbeatAt)
  if (heartbeatMs === undefined) {
    blockerRefs.push(PYLON_ADMISSION_NO_HEARTBEAT_REF)
  } else {
    const status = snapshot.latestHeartbeatStatus
    if (status === null || !HEALTHY_HEARTBEAT_STATUSES.has(status)) {
      blockerRefs.push(PYLON_ADMISSION_HEARTBEAT_UNHEALTHY_REF)
    }
    // Stale if older than the TTL. A future-dated heartbeat (clock skew) is NOT
    // stale; only past-the-window staleness fails this gate.
    if (input.nowMs - heartbeatMs > ttl) {
      blockerRefs.push(PYLON_ADMISSION_HEARTBEAT_STALE_REF)
    }
  }

  // GATE 5 — wallet/payout readiness (the node can receive its Bitcoin payout).
  if (!snapshot.walletReady) {
    blockerRefs.push(PYLON_ADMISSION_WALLET_NOT_READY_REF)
  }
  if (
    snapshot.sparkPayoutTargetRef === null ||
    snapshot.sparkPayoutTargetRef.trim() === ''
  ) {
    blockerRefs.push(PYLON_ADMISSION_NO_PAYOUT_TARGET_REF)
  }

  return {
    admitted: blockerRefs.length === 0,
    blockerRefs,
    policyRefs: [PYLON_ADMISSION_POLICY_REF],
    pylonRef: snapshot.pylonRef,
    schema: 'openagents.khala_pylon_admission.v1',
  }
}
