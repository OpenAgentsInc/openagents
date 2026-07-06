// Agent Computer launch surface — the "our cloud" autonomous-execution lane
// SCAFFOLD behind `autopilot.cloud_coding_sessions.v1` (red).
//
// This is the typed Worker-side request/lifecycle surface for LAUNCHING a managed
// OpenAgents Agent Computer — the isolated Firecracker microVM that lets a
// coding turn run in OUR cloud (OpenAgents-owned GCE capacity) instead of on the
// user's laptop. It mirrors the flag-gated INERT pattern proven by the inference
// gateway (`inference/chat-completions-routes.ts`) and the sibling Cloud-primitive
// scaffolds (`cloud/sandbox-compute-service-routes.ts`,
// `cloud/fine-tuning-service-routes.ts`):
//
//   - flag-gated INERT by default (CLOUD_CODING_SESSIONS_ENABLED, default off ->
//     every route returns 404, so NOTHING changes on the live Worker)
//   - typed launch request (lane + repo trust tier + objective -> typed session)
//   - a MANAGED-RUNTIME ADAPTER seam (`CloudCodingRuntimeAdapter`) where the real
//     OpenAgents Cloud control plane plugs in (the cloud repo's POST /v1/placement
//     + per-context Firecracker microVM lease); the production default fails
//     closed until that real provisioner is explicitly armed
//   - a placement policy that honors repo trust tiers BEFORE any dispatch
//     (regulated -> SHC-only, private -> own/verified, public -> any), an
//     authority boundary the promise already commits to
//   - a usage/receipt seam (`CloudCodingMeteringHook`) for the
//     `openagents.resource_usage_receipt.v1` round-trip; lifecycle/resource
//     receipt refs come from the control-plane `cloud.gce.*` events; ships a
//     no-op/log stub, with a real receipt-first ledger hook available
//     (`makeLedgerCloudCodingMeteringHook`) the way fine-tuning does
//   - a lifecycle read (GET by id) that resolves a session for the AUTHENTICATED
//     account only (cross-account isolation), like the fine-tuning job read
//
// HONEST SCOPE: `autopilot.cloud_coding_sessions.v1` STAYS red until a
// mobile-dispatched turn runs inside a real Firecracker microVM on OpenAgents
// GCE, streams to the timeline, and produces content-addressed artifacts plus
// dereferenceable lifecycle + resource usage receipts with owner sign-off per
// `proof.claim_upgrade_receipts.v1`. This route fails closed when live GCE
// provisioning is unarmed instead of faking an Agent Computer.

import { Effect, Schema as S } from 'effect'

import { AgentRateLimitPolicy } from '../agent-rate-limit-policy'
import { noStoreJsonResponse } from '../http/responses'
import { parseJsonRecord } from '../json-boundary'
import { workerLogEntry } from '../observability'
import { readAgentBalance } from '../payments-ledger'
import {
  compactRandomId,
  currentEpochMillis,
  currentIsoTimestamp,
} from '../runtime-primitives'
import {
  type CloudMeteringDeps,
  type CloudMeteringOutcome,
  settleCloudPrimitiveCharge,
} from './cloud-metering'

// FLAG ---------------------------------------------------------------------
// Parse CLOUD_CODING_SESSIONS_ENABLED. Default OFF: anything other than an
// explicit truthy token leaves the surface inert. Same parser shape as the
// inference + sandbox + fine-tuning flags so operators have one mental model.
export const isCloudCodingSessionsEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

// LANE ---------------------------------------------------------------------
// The cloud execution lane a session is launched onto. This is the CLOUD subset
// of the shared Autopilot control-protocol `SessionLane` (#4998): the surface
// only accepts cloud lanes (a `local` session never reaches this Worker route).
//   - `cloud-gcp` — OpenAgents Cloud on Google GCE (the default cloud lane)
//   - `cloud-shc` — OpenAgents Cloud SHC capacity (the cloud fallback / regulated)
export const CloudCodingLane = S.Literals(['cloud-gcp', 'cloud-shc'])
export type CloudCodingLane = typeof CloudCodingLane.Type
export const DEFAULT_CLOUD_CODING_LANE: CloudCodingLane = 'cloud-gcp'

// REPO TRUST TIER ----------------------------------------------------------
// Drives placement: which cloud capacity may run a session for a repo of this
// classification. The promise's authority boundary commits to exactly this:
// regulated -> SHC-only, private -> own/verified, public -> any.
export const RepoTrustTier = S.Literals(['public', 'private', 'regulated'])
export type RepoTrustTier = typeof RepoTrustTier.Type
export const DEFAULT_REPO_TRUST_TIER: RepoTrustTier = 'private'

// ADAPTER (coding agent) ---------------------------------------------------
// The coding-agent runtime the cloud session runs. Matches the control-protocol
// adapters that make sense in the cloud lane (Codex is the documented Phase-1
// cloud agent; claude_agent is the parallel cloud agent).
export const CloudCodingAdapter = S.Literals(['codex', 'claude_agent'])
export type CloudCodingAdapter = typeof CloudCodingAdapter.Type
export const DEFAULT_CLOUD_CODING_ADAPTER: CloudCodingAdapter = 'codex'

// REQUEST SCHEMA -----------------------------------------------------------
// Typed intake for launching a managed cloud coding session. `repoRef` and
// `objective` are refs/intent only (no raw creds/paths). `verify` is the bounded
// verification command list the session must pass. Unknown extra options are
// preserved verbatim for the runtime adapter (e.g. branch hints).
const CloudCodingSessionRequestBody = S.Struct({
  repoRef: S.String,
  objective: S.String,
  lane: S.optionalKey(CloudCodingLane),
  repoTrustTier: S.optionalKey(RepoTrustTier),
  adapter: S.optionalKey(CloudCodingAdapter),
  // Public-safe work context refs. The mobile thread<->repo binding from #8472
  // is the stable work context; until it is supplied, this route derives a
  // session-scoped fallback so the projection stays honest.
  workContextRef: S.optionalKey(S.String),
  threadRef: S.optionalKey(S.String),
  repoBindingRef: S.optionalKey(S.String),
  verify: S.optionalKey(S.Array(S.String)),
  // Bounds the session wall-clock (cost/abuse control). Optional; a default +
  // hard ceiling apply below.
  timeoutSeconds: S.optionalKey(S.Number),
})

// Default + ceiling session windows. The ceiling is a hard cost/abuse control: a
// request over it is rejected before any placement, so one launch cannot pin a
// cloud VM indefinitely.
export const DEFAULT_CLOUD_CODING_TIMEOUT_SECONDS = 1800
export const MAX_CLOUD_CODING_TIMEOUT_SECONDS = 14400
export const DEFAULT_AGENT_COMPUTER_IDLE_RECLAIM_SECONDS = 1800
export const AGENT_COMPUTER_ISOLATION_POLICY_SCHEMA =
  'openagents.agent_computer_isolation_policy.v1'

const USER_CAPACITY_OPTION_KEYS = new Set([
  'pylonRef',
  'pylon_ref',
  'userPylonRef',
  'user_pylon_ref',
  'runnerId',
  'runner_id',
  'capacityRef',
  'capacity_ref',
])

const hasUserCapacityOption = (
  raw: Readonly<Record<string, unknown>>,
): boolean =>
  Object.keys(raw).some(key => USER_CAPACITY_OPTION_KEYS.has(key))

export type CloudCodingSessionRequest = Readonly<{
  repoRef: string
  objective: string
  lane: CloudCodingLane
  repoTrustTier: RepoTrustTier
  adapter: CloudCodingAdapter
  verify: ReadonlyArray<string>
  timeoutSeconds: number
  workContextRef?: string | undefined
  threadRef?: string | undefined
  repoBindingRef?: string | undefined
  // Extra placement/launch options forwarded verbatim to the runtime adapter.
  options: Readonly<Record<string, unknown>>
}>

// SESSION MODEL ------------------------------------------------------------
// The typed cloud coding session the surface returns and the runtime adapter
// drives. `state` mirrors the control-protocol `SessionState` lifecycle the
// managed runtime advances; the scaffold only ever emits `queued` (the runtime
// moves it to running/completed/failed/cancelled once wired).
export const CloudCodingSessionState = S.Literals([
  'queued',
  'running',
  'completed',
  'failed',
  'cancelled',
])
export type CloudCodingSessionState = typeof CloudCodingSessionState.Type

export const AgentComputerLifecycleState = S.Literals([
  'requested',
  'provisioning',
  'ready',
  'active',
  'idle',
  'reclaiming',
  'reclaimed',
  'quarantined',
  'failed',
  'cancelled',
])
export type AgentComputerLifecycleState =
  typeof AgentComputerLifecycleState.Type

export type CloudCodingSession = Readonly<{
  sessionId: string
  accountRef: string
  lane: CloudCodingLane
  adapter: CloudCodingAdapter
  repoRef: string
  repoTrustTier: RepoTrustTier
  timeoutSeconds: number
  state: CloudCodingSessionState
  // Public-safe placement ref for the bound cloud VM/lease (e.g. the cloud
  // repo's placement id). Null until a real VM is leased. NEVER raw creds.
  placementRef: string | null
  // Refs returned by the cloud placement/lease path. Public-safe only.
  leaseRefs: ReadonlyArray<string>
  // Agent Computer projection. Refs only: no raw GCE instance names, host paths,
  // guest IPs, SSH keys, provider tokens, repo content, or wallet material.
  workContextRef: string
  agentComputerRef: string | null
  agentComputerState: AgentComputerLifecycleState
  lifecycleReceiptRefs: ReadonlyArray<string>
  resourceUsageReceiptRefs: ReadonlyArray<string>
  // Content-addressed artifact ref produced by a completed session. Null until a
  // real repo-edit produces one. NEVER raw diff/secret material.
  artifactRef: string | null
  createdAt: string
}>

// PLACEMENT POLICY ---------------------------------------------------------
// Pure decision: which cloud lanes may run a session for a repo of this trust
// tier, and whether the REQUESTED lane is admissible. This encodes the promise's
// authority boundary as a checkable function, BEFORE any adapter dispatch:
//   - regulated -> SHC-only        (cloud-shc)
//   - private   -> own/verified    (cloud-gcp or cloud-shc)
//   - public    -> any             (cloud-gcp or cloud-shc)
// A regulated repo requesting cloud-gcp is REFUSED here; nothing reaches a VM.
export const admissibleLanesForTrustTier = (
  tier: RepoTrustTier,
): ReadonlyArray<CloudCodingLane> =>
  tier === 'regulated' ? ['cloud-shc'] : ['cloud-gcp', 'cloud-shc']

export type PlacementDecision =
  | Readonly<{ allowed: true; lane: CloudCodingLane }>
  | Readonly<{
      allowed: false
      requestedLane: CloudCodingLane
      tier: RepoTrustTier
      admissibleLanes: ReadonlyArray<CloudCodingLane>
    }>

export const decidePlacement = (
  input: Readonly<{ lane: CloudCodingLane; tier: RepoTrustTier }>,
): PlacementDecision => {
  const admissible = admissibleLanesForTrustTier(input.tier)
  if (admissible.includes(input.lane)) {
    return { allowed: true, lane: input.lane }
  }
  return {
    admissibleLanes: admissible,
    allowed: false,
    requestedLane: input.lane,
    tier: input.tier,
  }
}

// ADMISSION POLICY ---------------------------------------------------------
// Credit-gated org-cloud admission for Khala Code mobile. This is additive to
// the owner-self/user-Pylon dispatch lane: it admits ONLY to OpenAgents-owned
// Agent Computer capacity, never to another user's Pylon or caller-supplied
// capacity selectors.
export const CloudCodingAdmissionRefusalReason = S.Literals([
  'insufficient_credit',
  'rate_limited',
  'org_capacity_unavailable',
])
export type CloudCodingAdmissionRefusalReason =
  typeof CloudCodingAdmissionRefusalReason.Type

export type CloudCodingAdmissionLimits = Readonly<{
  maxConcurrentSessions: number
  maxRequests: number
  windowSeconds: number
}>

export const DEFAULT_CLOUD_CODING_ADMISSION_LIMITS: CloudCodingAdmissionLimits =
  {
    maxConcurrentSessions: 1,
    maxRequests: AgentRateLimitPolicy.limit,
    windowSeconds: AgentRateLimitPolicy.windowSeconds,
  }

export type CloudCodingAdmissionSnapshot = Readonly<{
  accountRef: string
  activeSessions: number
  agentComputerCapacityAvailable: boolean
  availableBalanceMsat: number
  capacityRef: string
  requestsInWindow: number
}>

export type CloudCodingAdmissionAllowed = Readonly<{
  allowed: true
  availableBalanceMsat: number
  capacityRef: string
  limit: number
  remainingConcurrentSessions: number
  remainingRequests: number
  windowSeconds: number
}>

export type CloudCodingAdmissionRefused = Readonly<{
  allowed: false
  availableBalanceMsat: number
  capacityRef: string
  reason: CloudCodingAdmissionRefusalReason
  reasonRef: string
  statusCode: 402 | 429 | 503
  limit: number
  remainingConcurrentSessions: number
  remainingRequests: number
  windowSeconds: number
}>

export type CloudCodingAdmissionDecision =
  | CloudCodingAdmissionAllowed
  | CloudCodingAdmissionRefused

export const decideCloudCodingAdmission = (
  input: Readonly<{
    limits?: CloudCodingAdmissionLimits
    snapshot: CloudCodingAdmissionSnapshot
  }>,
): CloudCodingAdmissionDecision => {
  const limits = input.limits ?? DEFAULT_CLOUD_CODING_ADMISSION_LIMITS
  const availableBalanceMsat = Math.max(
    0,
    Math.trunc(input.snapshot.availableBalanceMsat),
  )
  const activeSessions = Math.max(0, Math.trunc(input.snapshot.activeSessions))
  const requestsInWindow = Math.max(
    0,
    Math.trunc(input.snapshot.requestsInWindow),
  )
  const remainingConcurrentSessions = Math.max(
    0,
    limits.maxConcurrentSessions - activeSessions,
  )
  const remainingRequests = Math.max(0, limits.maxRequests - requestsInWindow)
  const base = {
    availableBalanceMsat,
    capacityRef: input.snapshot.capacityRef,
    limit: limits.maxRequests,
    remainingConcurrentSessions,
    remainingRequests,
    windowSeconds: limits.windowSeconds,
  }

  if (availableBalanceMsat <= 0) {
    return {
      ...base,
      allowed: false,
      reason: 'insufficient_credit',
      reasonRef: 'reason.agent_computer_admission.insufficient_credit',
      statusCode: 402,
    }
  }

  if (
    activeSessions >= limits.maxConcurrentSessions ||
    requestsInWindow >= limits.maxRequests
  ) {
    return {
      ...base,
      allowed: false,
      reason: 'rate_limited',
      reasonRef: 'reason.agent_computer_admission.rate_limited',
      statusCode: 429,
    }
  }

  if (!input.snapshot.agentComputerCapacityAvailable) {
    return {
      ...base,
      allowed: false,
      reason: 'org_capacity_unavailable',
      reasonRef: 'reason.agent_computer_admission.org_capacity_unavailable',
      statusCode: 503,
    }
  }

  return {
    ...base,
    allowed: true,
  }
}

export type CloudCodingAdmissionContext = Readonly<{
  accountRef: string
  lane: CloudCodingLane
  request: CloudCodingSessionRequest
  sessionId: string
  workContextRef: string
}>

export type CloudCodingAdmissionGate = (
  context: CloudCodingAdmissionContext,
) => Effect.Effect<CloudCodingAdmissionDecision>

export const allowCloudCodingAdmissionGate: CloudCodingAdmissionGate = () =>
  Effect.succeed({
    allowed: true,
    availableBalanceMsat: 1,
    capacityRef: 'capacity.agent_computer.test.available',
    limit: DEFAULT_CLOUD_CODING_ADMISSION_LIMITS.maxRequests,
    remainingConcurrentSessions: 1,
    remainingRequests: DEFAULT_CLOUD_CODING_ADMISSION_LIMITS.maxRequests,
    windowSeconds: DEFAULT_CLOUD_CODING_ADMISSION_LIMITS.windowSeconds,
  })

const unconfiguredCloudCodingAdmissionGate: CloudCodingAdmissionGate = () =>
  Effect.succeed(
    decideCloudCodingAdmission({
      snapshot: {
        accountRef: 'agent:unknown',
        activeSessions: 0,
        agentComputerCapacityAvailable: false,
        availableBalanceMsat: 1,
        capacityRef: 'capacity.agent_computer.unconfigured',
        requestsInWindow: 0,
      },
    }),
  )

export type AgentComputerCapacitySnapshot = Readonly<{
  available: boolean
  availableSlots: number
  capacityRef: string
}>

export const configuredAgentComputerCapacitySnapshot = (
  input: Readonly<{
    baseUrl: string | undefined
    bearerToken: string | undefined
    gceProvisioningArmed: boolean
  }>,
): AgentComputerCapacitySnapshot => {
  const available =
    input.gceProvisioningArmed &&
    isNonEmptyString(input.baseUrl) &&
    isNonEmptyString(input.bearerToken)
  return {
    available,
    availableSlots: available ? 1 : 0,
    capacityRef: available
      ? 'capacity.agent_computer.control_plane.armed'
      : 'capacity.agent_computer.control_plane.unavailable',
  }
}

export type CloudCodingAdmissionLedgerDeps = Readonly<{
  capacity: () => Promise<AgentComputerCapacitySnapshot>
  db: D1Database
  limits?: CloudCodingAdmissionLimits
  nowMs?: () => number
}>

type AdmissionCountRow = Readonly<{
  active_sessions: unknown
  requests_in_window: unknown
}>

const readAdmissionCounts = async (
  db: D1Database,
  input: Readonly<{
    accountRef: string
    nowMs: number
    windowStartMs: number
  }>,
): Promise<Readonly<{ activeSessions: number; requestsInWindow: number }>> => {
  const row = (await db
    .prepare(
      `SELECT
         (SELECT COUNT(*) FROM cloud_coding_admission_reservations
          WHERE account_ref = ? AND expires_at_ms > ?) AS active_sessions,
         (SELECT COUNT(*) FROM cloud_coding_admission_events
          WHERE account_ref = ? AND created_at_ms >= ?) AS requests_in_window`,
    )
    .bind(
      input.accountRef,
      input.nowMs,
      input.accountRef,
      input.windowStartMs,
    )
    .first()) as AdmissionCountRow | null

  return {
    activeSessions: Math.max(0, Number(row?.active_sessions ?? 0)),
    requestsInWindow: Math.max(0, Number(row?.requests_in_window ?? 0)),
  }
}

const recordAdmissionReservation = async (
  db: D1Database,
  input: Readonly<{
    accountRef: string
    capacityRef: string
    eventId: string
    expiresAtMs: number
    lane: CloudCodingLane
    nowMs: number
    sessionId: string
    workContextRef: string
  }>,
): Promise<void> => {
  await db.batch([
    db
      .prepare(
        `INSERT INTO cloud_coding_admission_events
           (id, session_id, account_ref, work_context_ref, lane, event_kind,
            capacity_ref, created_at_ms)
         VALUES (?, ?, ?, ?, ?, 'admitted', ?, ?)`,
      )
      .bind(
        input.eventId,
        input.sessionId,
        input.accountRef,
        input.workContextRef,
        input.lane,
        input.capacityRef,
        input.nowMs,
      ),
    db
      .prepare(
        `INSERT INTO cloud_coding_admission_reservations
           (session_id, account_ref, work_context_ref, lane, state,
            capacity_ref, created_at_ms, expires_at_ms)
         VALUES (?, ?, ?, ?, 'admitted', ?, ?, ?)
         ON CONFLICT(session_id) DO UPDATE SET
           account_ref = excluded.account_ref,
           work_context_ref = excluded.work_context_ref,
           lane = excluded.lane,
           state = excluded.state,
           capacity_ref = excluded.capacity_ref,
           expires_at_ms = excluded.expires_at_ms`,
      )
      .bind(
        input.sessionId,
        input.accountRef,
        input.workContextRef,
        input.lane,
        input.capacityRef,
        input.nowMs,
        input.expiresAtMs,
      ),
  ])
}

export const makeD1CloudCodingAdmissionGate = (
  deps: CloudCodingAdmissionLedgerDeps,
): CloudCodingAdmissionGate => {
  const limits = deps.limits ?? DEFAULT_CLOUD_CODING_ADMISSION_LIMITS
  return context =>
    Effect.promise(async () => {
      try {
        const nowMs = deps.nowMs?.() ?? currentEpochMillis()
        const [balance, capacity, counts] = await Promise.all([
          readAgentBalance(deps.db, context.accountRef),
          deps.capacity(),
          readAdmissionCounts(deps.db, {
            accountRef: context.accountRef,
            nowMs,
            windowStartMs: nowMs - limits.windowSeconds * 1000,
          }),
        ])
        const decision = decideCloudCodingAdmission({
          limits,
          snapshot: {
            accountRef: context.accountRef,
            activeSessions: counts.activeSessions,
            agentComputerCapacityAvailable:
              capacity.available && capacity.availableSlots > 0,
            availableBalanceMsat: balance?.availableMsat ?? 0,
            capacityRef: capacity.capacityRef,
            requestsInWindow: counts.requestsInWindow,
          },
        })

        if (decision.allowed) {
          await recordAdmissionReservation(deps.db, {
            accountRef: context.accountRef,
            capacityRef: decision.capacityRef,
            eventId: `admission.${refPart(context.sessionId)}.${nowMs}`,
            expiresAtMs: nowMs + context.request.timeoutSeconds * 1000,
            lane: context.lane,
            nowMs,
            sessionId: context.sessionId,
            workContextRef: context.workContextRef,
          })
        }

        return decision
      } catch {
        return decideCloudCodingAdmission({
          limits,
          snapshot: {
            accountRef: context.accountRef,
            activeSessions: limits.maxConcurrentSessions,
            agentComputerCapacityAvailable: false,
            availableBalanceMsat: 0,
            capacityRef: 'capacity.agent_computer.admission_store_unavailable',
            requestsInWindow: limits.maxRequests,
          },
        })
      }
    })
}

// MANAGED-RUNTIME ADAPTER SEAM ---------------------------------------------
// The provider/runtime seam the real OpenAgents Cloud control plane plugs into.
// A live adapter wires `launch` to the cloud repo's POST /v1/placement + GCE VM
// lease (cloud #86/#87/#88/#90) and `get` to the cloud session store. Adapters
// NEVER touch credits, payment, or public projection — that is the metering
// hook's job. Production must fail closed when this real adapter is not armed.
export type CloudCodingRuntimeAdapter = Readonly<{
  id: string
  launch: (
    input: Readonly<{
      sessionId: string
      accountRef: string
      request: CloudCodingSessionRequest
      // The placement-resolved lane (== request.lane, re-stated so the adapter
      // never has to re-derive admissibility).
      lane: CloudCodingLane
    }>,
  ) => Effect.Effect<CloudCodingSession, CloudCodingAdapterError>
  // Resolve the current state of a session, scoped to the requesting account
  // (cross-account isolation: a session is visible only to the account that
  // launched it). Resolves to undefined when no such session exists for the
  // account (the route maps that to 404). The stub has no persistence and always
  // resolves to undefined.
  get: (
    input: Readonly<{ sessionId: string; accountRef: string }>,
  ) => Effect.Effect<CloudCodingSession | undefined, CloudCodingAdapterError>
}>

// Typed adapter failure so the route maps runtime problems to a stable JSON
// error instead of throwing.
export class CloudCodingAdapterError extends Error {
  readonly _tag = 'CloudCodingAdapterError'
  readonly adapterId: string
  readonly reason: string

  constructor(input: Readonly<{ adapterId: string; reason: string }>) {
    super(`[${input.adapterId}] ${input.reason}`)
    this.name = 'CloudCodingAdapterError'
    this.adapterId = input.adapterId
    this.reason = input.reason
  }
}

export const STUB_CLOUD_CODING_ADAPTER_ID = 'stub-cloud-coding'
export const NOT_ARMED_CLOUD_CODING_ADAPTER_ID = 'not-armed-cloud-gce'
export const LIVE_CLOUD_CODING_ADAPTER_ID = 'openagents-cloud-control'

// Stub/accepting runtime adapter. Accepts a launch and returns a `queued`
// session with no real placement, so the route + metering seams are exercisable
// without provisioning real cloud compute. It NEVER leases a VM, runs a repo
// edit, or produces an artifact — `placementRef`/`artifactRef` stay null. A live
// EPIC build replaces dispatch to this with the real cloud control-plane adapter.
export const stubCloudCodingAdapter: CloudCodingRuntimeAdapter = {
  id: STUB_CLOUD_CODING_ADAPTER_ID,
  get: () => Effect.sync((): CloudCodingSession | undefined => undefined),
  launch: ({ sessionId, accountRef, request, lane }) =>
    Effect.sync(
      (): CloudCodingSession => ({
        accountRef,
        adapter: request.adapter,
        artifactRef: null,
        createdAt: currentIsoTimestamp(),
        lane,
        lifecycleReceiptRefs: [],
        leaseRefs: [],
        agentComputerRef: null,
        agentComputerState: 'requested',
        placementRef: null,
        repoRef: request.repoRef,
        repoTrustTier: request.repoTrustTier,
        resourceUsageReceiptRefs: [],
        sessionId,
        state: 'queued',
        timeoutSeconds: request.timeoutSeconds,
        workContextRef: workContextRefForSession(request, sessionId),
      }),
    ),
}

export type CloudCodingControlPlaneConfig = Readonly<{
  baseUrl: string
  bearerToken: string
  gceProvisioningArmed: boolean
  fetch?: typeof fetch
}>

const isNonEmptyString = (value: string | undefined): value is string =>
  value !== undefined && value.trim().length > 0

export const isCloudGceProvisioningArmed = (
  value: string | undefined,
): boolean => value?.trim().toLowerCase() === 'live'

const publicRefFromUnknown = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim().length > 0 ? value : undefined

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> => [
  ...new Set(refs.filter(ref => ref.trim().length > 0)),
]

const refPart = (value: string): string =>
  value.replace(/[^a-zA-Z0-9_.:-]/g, '_')

const workContextRefForSession = (
  request: CloudCodingSessionRequest,
  sessionId: string,
): string =>
  request.workContextRef ?? `work-context.agent-computer.${refPart(sessionId)}`

const sessionStateFromCloudStatus = (status: string): CloudCodingSessionState => {
  if (status === 'completed') {
    return 'completed'
  }
  if (status === 'failed' || status === 'timeout') {
    return 'failed'
  }
  if (status === 'cancelled') {
    return 'cancelled'
  }
  if (status === 'running' || status === 'started') {
    return 'running'
  }
  return 'queued'
}

const agentComputerStateFromCloudStatus = (
  status: string,
  events: ReadonlyArray<CloudPlacementEvent>,
): AgentComputerLifecycleState => {
  const eventKinds = new Set(events.map(event => event.kind))
  if (eventKinds.has('cloud.gce.cleanup')) {
    return 'reclaimed'
  }
  if (eventKinds.has('cloud.gce.degraded')) {
    return 'quarantined'
  }
  if (status === 'completed') {
    return 'idle'
  }
  if (status === 'failed' || status === 'timeout') {
    return 'failed'
  }
  if (status === 'cancelled') {
    return 'cancelled'
  }
  if (status === 'running' || status === 'started') {
    return 'active'
  }
  if (status === 'ready') {
    return 'ready'
  }
  return 'provisioning'
}

const stringOption = (
  options: Readonly<Record<string, unknown>>,
  key: string,
): string | undefined => publicRefFromUnknown(options[key])

const defaultProviderAccountRef = (accountRef: string): string =>
  `provider-account.cloud-coding.${accountRef.replace(/[^a-zA-Z0-9_.:-]/g, '_')}`

const defaultAuthGrantRef = (sessionId: string): string =>
  `grant.cloud-coding-session.${sessionId}`

const agentComputerIsolationPolicy = (
  request: CloudCodingSessionRequest,
) => ({
  schema_version: AGENT_COMPUTER_ISOLATION_POLICY_SCHEMA,
  unit: 'one_firecracker_microvm_per_work_context',
  lifecycle: {
    hard_timeout_seconds: request.timeoutSeconds,
    idle_reclaim_seconds: DEFAULT_AGENT_COMPUTER_IDLE_RECLAIM_SECONDS,
    microvm_destroy_required: true,
    reclaim_on: ['idle_timeout', 'hard_timeout', 'sign_out', 'thread_delete'],
    scratch_wipe_required: true,
  },
  credentials: {
    credential_scanner_required: true,
    no_provider_master_keys: true,
    no_raw_user_oauth_tokens: true,
    no_wallet_material: true,
    scm_broker_only: true,
  },
  network: {
    egress_policy_ref: 'egress.agent_computer.coding_mvp.restricted.v1',
    no_inbound: true,
  },
  projection: {
    public_refs_only: true,
  },
})

type CloudPlacementEvent = Readonly<{
  kind: string
  receiptRefs: ReadonlyArray<string>
  artifactRefs: ReadonlyArray<string>
  data: Readonly<Record<string, unknown>>
  digest: string | null
}>

const recordFromUnknown = (
  value: unknown,
): Record<string, unknown> | undefined =>
  value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined

const publicRefsFromUnknownArray = (
  value: unknown,
): ReadonlyArray<string> =>
  Array.isArray(value)
    ? value.flatMap(item => {
        const ref = publicRefFromUnknown(item)
        return ref === undefined ? [] : [ref]
      })
    : []

const parseEventData = (value: unknown): Readonly<Record<string, unknown>> => {
  const direct = recordFromUnknown(value)
  if (direct !== undefined) {
    return direct
  }
  if (typeof value !== 'string' || value.trim() === '') {
    return {}
  }
  return parseJsonRecord(value) ?? {}
}

const cloudPlacementEventKind = (
  record: Record<string, unknown>,
): string | undefined => {
  const candidates = [record.type, record.kind, record.eventType].flatMap(
    value => {
      const ref = publicRefFromUnknown(value)
      return ref === undefined ? [] : [ref]
    },
  )
  return (
    candidates.find(candidate => candidate.startsWith('cloud.gce.')) ??
    candidates[0]
  )
}

const normalizeCloudPlacementEvent = (
  value: unknown,
): CloudPlacementEvent | undefined => {
  const record = recordFromUnknown(value)
  if (record === undefined) {
    return undefined
  }
  const kind = cloudPlacementEventKind(record)
  if (kind === undefined) {
    return undefined
  }
  return {
    artifactRefs: uniqueRefs([
      ...publicRefsFromUnknownArray(record.artifactRefs),
      ...publicRefsFromUnknownArray(record.artifact_refs),
    ]),
    data: parseEventData(record.dataJson ?? record.data_json ?? record.data),
    digest: publicRefFromUnknown(record.digest) ?? null,
    kind,
    receiptRefs: uniqueRefs([
      ...publicRefsFromUnknownArray(record.receiptRefs),
      ...publicRefsFromUnknownArray(record.receipt_refs),
    ]),
  }
}

const publicRefFromEventData = (
  event: CloudPlacementEvent,
  key: string,
): string | undefined => publicRefFromUnknown(event.data[key])

const lifecycleReceiptRefsFromEvents = (
  events: ReadonlyArray<CloudPlacementEvent>,
): ReadonlyArray<string> =>
  uniqueRefs(
    events.flatMap(event => {
      if (
        event.kind !== 'cloud.gce.provisioned' &&
        event.kind !== 'cloud.gce.cleanup' &&
        event.kind !== 'cloud.gce.degraded'
      ) {
        return []
      }
      return [
        ...event.receiptRefs,
        publicRefFromEventData(event, 'provisionReceiptRef'),
        publicRefFromEventData(event, 'cleanupReceiptRef'),
        publicRefFromEventData(event, 'scratchWipeReceiptRef'),
        publicRefFromEventData(event, 'scratch_wipe_receipt_ref'),
        publicRefFromEventData(event, 'microvmDestroyReceiptRef'),
        publicRefFromEventData(event, 'microvm_destroy_receipt_ref'),
        publicRefFromEventData(event, 'quarantineReceiptRef'),
      ].flatMap(ref => (ref === undefined ? [] : [ref]))
    }),
  )

const resourceUsageReceiptRefsFromEvents = (
  events: ReadonlyArray<CloudPlacementEvent>,
): ReadonlyArray<string> =>
  uniqueRefs(
    events.flatMap(event => {
      if (
        event.kind !== 'cloud.gce.resource_usage_receipt' &&
        event.kind !== 'cloud.gce.resource'
      ) {
        return []
      }
      return [
        ...event.receiptRefs,
        publicRefFromEventData(event, 'resourceUsageReceiptRef'),
      ].flatMap(ref => (ref === undefined ? [] : [ref]))
    }),
  )

const leaseRefsFromEvents = (
  events: ReadonlyArray<CloudPlacementEvent>,
): ReadonlyArray<string> =>
  uniqueRefs(
    events.flatMap(event => {
      const leaseRef = publicRefFromEventData(event, 'leaseRef')
      return leaseRef === undefined ? [] : [leaseRef]
    }),
  )

type CloudPlacementResponse = Readonly<{
  externalRunId: string
  status: string
  events: ReadonlyArray<CloudPlacementEvent>
  binding: Readonly<{
    contractVersion: string
    runId: string
    externalRunId: string
    lane: CloudCodingLane
    providerLane: string
    runnerId: string
    workContextRef: string | null
    capacityClassId: string | null
    sandboxMode: string
    reason: string
    costDriven: boolean
    caps?: Record<string, unknown>
  }>
}>

const normalizeCloudPlacementResponse = (
  payload: unknown,
  fallbackRunId: string,
  fallbackLane: CloudCodingLane,
): CloudPlacementResponse => {
  const record = (payload ?? {}) as Record<string, unknown>
  const rawBinding =
    record.binding !== null && typeof record.binding === 'object'
      ? (record.binding as Record<string, unknown>)
      : {}
  const lane = rawBinding.lane === 'cloud-shc' ? 'cloud-shc' : fallbackLane
  const externalRunId =
    publicRefFromUnknown(record.externalRunId) ??
    publicRefFromUnknown(rawBinding.externalRunId) ??
    ''
  const caps =
    rawBinding.caps !== null && typeof rawBinding.caps === 'object'
      ? (rawBinding.caps as Record<string, unknown>)
      : undefined
  const workContextRef =
    publicRefFromUnknown(rawBinding.workContextRef) ??
    publicRefFromUnknown(rawBinding.work_context_ref) ??
    publicRefFromUnknown(record.workContextRef) ??
    publicRefFromUnknown(record.work_context_ref) ??
    null
  const events = Array.isArray(record.events)
    ? record.events.flatMap(event => {
        const normalized = normalizeCloudPlacementEvent(event)
        return normalized === undefined ? [] : [normalized]
      })
    : []
  return {
    binding: {
      capacityClassId: publicRefFromUnknown(rawBinding.capacityClassId) ?? null,
      contractVersion:
        publicRefFromUnknown(rawBinding.contractVersion) ??
        'openagents.codex_placement_assignment.v1',
      costDriven: rawBinding.costDriven === true,
      externalRunId,
      lane,
      providerLane:
        publicRefFromUnknown(rawBinding.providerLane) ??
        (lane === 'cloud-shc' ? 'shc' : 'gcp'),
      reason: publicRefFromUnknown(rawBinding.reason) ?? '',
      runId: publicRefFromUnknown(rawBinding.runId) ?? fallbackRunId,
      runnerId: publicRefFromUnknown(rawBinding.runnerId) ?? '',
      sandboxMode:
        publicRefFromUnknown(rawBinding.sandboxMode) ?? 'danger_full_access',
      workContextRef,
      ...(caps === undefined ? {} : { caps }),
    },
    events,
    externalRunId,
    status: publicRefFromUnknown(record.status) ?? 'queued',
  }
}

const hasVerifiedReclaimEvidence = (event: CloudPlacementEvent): boolean =>
  event.kind === 'cloud.gce.cleanup' &&
  (publicRefFromEventData(event, 'scratchWipeReceiptRef') ??
    publicRefFromEventData(event, 'scratch_wipe_receipt_ref')) !== undefined &&
  (publicRefFromEventData(event, 'microvmDestroyReceiptRef') ??
    publicRefFromEventData(event, 'microvm_destroy_receipt_ref')) !== undefined

const validateAgentComputerPlacement = (
  placement: CloudPlacementResponse,
  expectedWorkContextRef: string,
): string | undefined => {
  if (placement.binding.workContextRef === null) {
    return 'agent_computer_work_context_binding_missing'
  }
  if (placement.binding.workContextRef !== expectedWorkContextRef) {
    return 'agent_computer_work_context_binding_mismatch'
  }
  if (
    placement.events.some(
      event => event.kind === 'cloud.gce.cleanup' && !hasVerifiedReclaimEvidence(event),
    )
  ) {
    return 'agent_computer_reclaim_evidence_missing'
  }
  return undefined
}

export const makeCloudControlCloudCodingAdapter = (
  config: CloudCodingControlPlaneConfig,
): CloudCodingRuntimeAdapter => {
  const fetchImpl = config.fetch ?? fetch
  const baseUrl = config.baseUrl.replace(/\/+$/, '')
  const notArmed = (reason: string) =>
    new CloudCodingAdapterError({
      adapterId: NOT_ARMED_CLOUD_CODING_ADAPTER_ID,
      reason,
    })
  return {
    id: LIVE_CLOUD_CODING_ADAPTER_ID,
    get: () =>
      Effect.fail(
        notArmed('cloud_coding_lifecycle_read_requires_cloud_session_store'),
      ),
    launch: ({ sessionId, accountRef, request, lane }) =>
      Effect.gen(function* () {
        if (!config.gceProvisioningArmed) {
          return yield* Effect.fail(notArmed('cloud_gce_provisioning_not_armed'))
        }
        if (!isNonEmptyString(config.baseUrl)) {
          return yield* Effect.fail(notArmed('cloud_control_url_not_configured'))
        }
        if (!isNonEmptyString(config.bearerToken)) {
          return yield* Effect.fail(
            notArmed('cloud_control_token_not_configured'),
          )
        }
        const placementResult = yield* Effect.tryPromise({
          catch: error =>
            new CloudCodingAdapterError({
              adapterId: LIVE_CLOUD_CODING_ADAPTER_ID,
              reason:
                error instanceof Error
                  ? error.message
                  : 'cloud_placement_request_failed',
            }),
          try: async () => {
            const workContextRef = workContextRefForSession(request, sessionId)
            const response = await fetchImpl(`${baseUrl}/v1/placement`, {
              body: JSON.stringify({
                auth_grant_ref:
                  stringOption(request.options, 'authGrantRef') ??
                  defaultAuthGrantRef(sessionId),
                agent_computer_isolation_policy:
                  agentComputerIsolationPolicy(request),
                contract_version: 'openagents.codex_placement_assignment.v1',
                created_at_ms: currentEpochMillis(),
                goal: request.objective,
                lane,
                owner_ref: accountRef,
                provider_account_ref:
                  stringOption(request.options, 'providerAccountRef') ??
                  defaultProviderAccountRef(accountRef),
                repository: request.repoRef,
                ...(request.repoBindingRef === undefined
                  ? {}
                  : { repo_binding_ref: request.repoBindingRef }),
                run_id: sessionId,
                sandbox_mode: 'danger_full_access',
                ...(request.threadRef === undefined
                  ? {}
                  : { thread_ref: request.threadRef }),
                timeout_seconds: request.timeoutSeconds,
                wallet_authority: false,
                work_context_ref: workContextRef,
              }),
              headers: {
                Authorization: `Bearer ${config.bearerToken}`,
                'Content-Type': 'application/json',
              },
              method: 'POST',
            })
            if (!response.ok) {
              return {
                ok: false as const,
                reason: `cloud_placement_http_${response.status}`,
              }
            }
            return {
              ok: true as const,
              placement: normalizeCloudPlacementResponse(
                await response.json(),
                sessionId,
                lane,
              ),
            }
          },
        })
        if (!placementResult.ok) {
          return yield* Effect.fail(
            new CloudCodingAdapterError({
              adapterId: LIVE_CLOUD_CODING_ADAPTER_ID,
              reason: placementResult.reason,
            }),
          )
        }
        const placement = placementResult.placement
        const workContextRef = workContextRefForSession(request, sessionId)
        const isolationValidationReason = validateAgentComputerPlacement(
          placement,
          workContextRef,
        )
        if (isolationValidationReason !== undefined) {
          return yield* Effect.fail(
            new CloudCodingAdapterError({
              adapterId: LIVE_CLOUD_CODING_ADAPTER_ID,
              reason: isolationValidationReason,
            }),
          )
        }
        const lifecycleReceiptRefs = lifecycleReceiptRefsFromEvents(
          placement.events,
        )
        const resourceUsageReceiptRefs = resourceUsageReceiptRefsFromEvents(
          placement.events,
        )
        const placementRef =
          placement.externalRunId !== ''
            ? `placement.cloud-coding.${placement.externalRunId}`
            : `placement.cloud-coding.${sessionId}`
        const agentComputerRef =
          placement.externalRunId !== ''
            ? `agent-computer.${refPart(placement.externalRunId)}`
            : `agent-computer.${refPart(sessionId)}`
        return {
          accountRef,
          adapter: request.adapter,
          agentComputerRef,
          agentComputerState: agentComputerStateFromCloudStatus(
            placement.status,
            placement.events,
          ),
          artifactRef: null,
          createdAt: currentIsoTimestamp(),
          lane: placement.binding.lane,
          lifecycleReceiptRefs,
          leaseRefs: uniqueRefs([
            placementRef,
            ...(placement.externalRunId === ''
              ? []
              : [`cloud-run.${placement.externalRunId}`]),
            ...(placement.binding.runnerId === ''
              ? []
              : [`cloud-runner.${placement.binding.runnerId}`]),
            ...(placement.binding.capacityClassId === null
              ? []
              : [`cloud-capacity-class.${placement.binding.capacityClassId}`]),
            ...leaseRefsFromEvents(placement.events),
          ]),
          placementRef,
          repoRef: request.repoRef,
          repoTrustTier: request.repoTrustTier,
          resourceUsageReceiptRefs,
          sessionId,
          state: sessionStateFromCloudStatus(placement.status),
          timeoutSeconds: request.timeoutSeconds,
          workContextRef,
        } satisfies CloudCodingSession
      }),
  }
}

// METERING / RECEIPT HOOK SEAM ---------------------------------------------
// The single typed point where the cloud coding session's usage round-trips into
// an `openagents.resource_usage_receipt.v1` and (when live) a credit debit. The
// usage object is exactly the refs-only receipt the cloud GCE lane already emits
// as `cloud.gce.resource_usage_receipt` (issue #5005); this seam is where that
// usage becomes a dereferenceable receipt + credit charge. Ships a no-op/log stub.
export const CLOUD_CODING_PRIMITIVE = 'cloud.coding_session.run'

export type CloudCodingMeteringContext = Readonly<{
  accountRef: string
  sessionId: string
  lane: CloudCodingLane
  // Metered runtime usage the charge is computed from once the session ends
  // (VM wall-seconds, CPU-seconds, memory-GB-seconds, egress, etc.). Absent at
  // launch time (no usage yet), so the launch-time hook reports metered:false.
  usage?: Readonly<Record<string, number>> | undefined
}>

export type CloudCodingMeteringOutcome = Readonly<{
  metered: boolean
  // Public-safe receipt ref when metering is live; null for the stub. Never a
  // raw amount, destination, or payment material.
  receiptRef: string | null
}>

export type CloudCodingMeteringHook = (
  context: CloudCodingMeteringContext,
) => Effect.Effect<CloudCodingMeteringOutcome>

// Public-safe receipt ref for a cloud coding-session run charge, resolvable
// without exposing any payment material. This is the
// `openagents.resource_usage_receipt.v1` projection ref the desktop/mobile
// timeline surfaces (the #5005 round-trip target).
export const cloudCodingSessionReceiptRef = (sessionId: string): string =>
  `receipt.cloud.coding_session.run.${sessionId}`

// No-op stub. Logs (public-safe: account, lane, session only) and reports
// `metered: false`. Used on the inert path and as the default in tests.
export const stubCloudCodingMeteringHook: CloudCodingMeteringHook = context =>
  Effect.gen(function* () {
    yield* Effect.logInfo(
      workerLogEntry('cloud.coding_session.metering.stub', {
        accountRef: context.accountRef,
        lane: context.lane,
        sessionId: context.sessionId,
      }),
    )
    return {
      metered: false,
      receiptRef: null,
    } satisfies CloudCodingMeteringOutcome
  })

// REAL, receipt-first ledger metering hook (parity with
// `makeLedgerFineTuningMeteringHook`). Charges the account through the SAME
// atomic credit ledger the inference gateway uses, from REAL runtime usage via an
// INJECTED pure pricing function. No default pricing — a live hook MUST supply
// the real price basis. The promise STAYS red; this is the seam, not a live
// billed product, and the scaffold defaults to the no-op stub above.
export type CloudCodingLedgerMeteringDeps = Readonly<
  CloudMeteringDeps & {
    // Pure pricing: REAL runtime usage -> USD charge. No default.
    priceUsd: (context: CloudCodingMeteringContext) => number
    // USD -> integer msat. Shares the inference gateway's single-source
    // conversion so a cloud charge and an inference charge convert identically.
    usdToMsat: (chargeUsd: number) => number
  }
>

export const makeLedgerCloudCodingMeteringHook = (
  deps: CloudCodingLedgerMeteringDeps,
): CloudCodingMeteringHook => {
  return context =>
    Effect.gen(function* () {
      // At launch there is no runtime usage yet (no VM run), so there is nothing
      // to charge — report metered:false without writing a ledger row. The
      // per-session charge fires only once the runtime reports real usage.
      if (context.usage === undefined) {
        return {
          metered: false,
          receiptRef: null,
        } satisfies CloudCodingMeteringOutcome
      }
      const chargeMsat = Math.max(
        0,
        Math.ceil(deps.usdToMsat(deps.priceUsd(context))),
      )
      const outcome: CloudMeteringOutcome = yield* settleCloudPrimitiveCharge(
        {
          db: deps.db,
          ...(deps.nowIso === undefined ? {} : { nowIso: deps.nowIso }),
          ...(deps.mirror === undefined ? {} : { mirror: deps.mirror }),
        },
        {
          accountRef: context.accountRef,
          adapterId: 'cloud-coding-runtime',
          chargeId: context.sessionId,
          chargeMsat,
          primitive: CLOUD_CODING_PRIMITIVE,
        },
      )
      return {
        metered: outcome.metered,
        receiptRef: outcome.metered
          ? cloudCodingSessionReceiptRef(context.sessionId)
          : null,
      } satisfies CloudCodingMeteringOutcome
    })
}

// AUTH SEAM ----------------------------------------------------------------
// Resolves the per-account API key to an account ref. Returns undefined when the
// key is missing/invalid. The Worker wires this to the same programmatic-agent
// auth the inference gateway / sandbox / fine-tuning surfaces use; tests inject a
// fake.
export type CloudCodingAuth = (
  request: Request,
) => Promise<Readonly<{ accountRef: string }> | undefined>

export type CloudCodingSessionServiceDeps = Readonly<{
  // Whether the surface is enabled (env.CLOUD_CODING_SESSIONS_ENABLED, default
  // OFF).
  enabled: boolean
  authenticate: CloudCodingAuth
  // Credit/concurrency/capacity admission. Production wires this to the mobile
  // bearer session's agent balance plus Agent Computer control-plane readiness.
  admissionGate?: CloudCodingAdmissionGate
  // Runtime adapter. Defaults to a fail-closed adapter; tests may inject the
  // stub explicitly, but production must never silently fake success.
  adapter?: CloudCodingRuntimeAdapter
  // Metering/receipt hook. Defaults to the no-op/log stub.
  meteringHook?: CloudCodingMeteringHook
  // Deterministic id injection for tests.
  newId?: () => string
}>

const decodeBody = (value: unknown) => {
  try {
    return S.decodeUnknownSync(CloudCodingSessionRequestBody)(value)
  } catch {
    return undefined
  }
}

const toSessionRequest = (
  body: typeof CloudCodingSessionRequestBody.Type,
  raw: Record<string, unknown>,
): CloudCodingSessionRequest => {
  const {
    adapter: _a,
    lane: _l,
    objective: _o,
    repoRef: _r,
    repoTrustTier: _t,
    workContextRef: _wc,
    threadRef: _tr,
    repoBindingRef: _rb,
    timeoutSeconds: _ts,
    verify: _v,
    ...rest
  } = raw
  const workContextRef = publicRefFromUnknown(body.workContextRef)
  const threadRef = publicRefFromUnknown(body.threadRef)
  const repoBindingRef = publicRefFromUnknown(body.repoBindingRef)
  return {
    adapter: body.adapter ?? DEFAULT_CLOUD_CODING_ADAPTER,
    lane: body.lane ?? DEFAULT_CLOUD_CODING_LANE,
    objective: body.objective,
    options: rest,
    repoRef: body.repoRef,
    repoTrustTier: body.repoTrustTier ?? DEFAULT_REPO_TRUST_TIER,
    timeoutSeconds: body.timeoutSeconds ?? DEFAULT_CLOUD_CODING_TIMEOUT_SECONDS,
    verify: body.verify ?? [],
    ...(workContextRef === undefined ? {} : { workContextRef }),
    ...(threadRef === undefined ? {} : { threadRef }),
    ...(repoBindingRef === undefined ? {} : { repoBindingRef }),
  }
}

// Public-safe JSON projection of a session. NEVER raw creds/diff material.
const projectSession = (session: CloudCodingSession) => ({
  object: 'cloud.coding_session',
  product_object: 'agent.computer_session',
  id: session.sessionId,
  lane: session.lane,
  adapter: session.adapter,
  repo_ref: session.repoRef,
  repo_trust_tier: session.repoTrustTier,
  timeout_seconds: session.timeoutSeconds,
  state: session.state,
  placement_ref: session.placementRef,
  lease_refs: session.leaseRefs,
  work_context_ref: session.workContextRef,
  agent_computer_ref: session.agentComputerRef,
  agent_computer_state: session.agentComputerState,
  lifecycle_receipt_refs: session.lifecycleReceiptRefs,
  resource_usage_receipt_refs: session.resourceUsageReceiptRefs,
  agent_computer: {
    ref: session.agentComputerRef,
    state: session.agentComputerState,
    work_context_ref: session.workContextRef,
    lifecycle_receipt_refs: session.lifecycleReceiptRefs,
    resource_usage_receipt_refs: session.resourceUsageReceiptRefs,
  },
  artifact_ref: session.artifactRef,
  created_at: session.createdAt,
})

const admissionRefusalHeaders = (
  decision: CloudCodingAdmissionRefused,
): Headers => {
  const headers = new Headers()
  if (decision.reason === 'rate_limited') {
    headers.set('ratelimit-limit', String(decision.limit))
    headers.set(
      'ratelimit-policy',
      `${decision.limit};w=${decision.windowSeconds}`,
    )
    headers.set('ratelimit-reset', String(decision.windowSeconds))
  }
  return headers
}

const admissionRefusalResponse = (
  decision: CloudCodingAdmissionRefused,
): Response =>
  noStoreJsonResponse(
    {
      error: decision.reason,
      available_balance_msat: decision.availableBalanceMsat,
      capacity_ref: decision.capacityRef,
      reason: decision.reason,
      reason_ref: decision.reasonRef,
      remaining_concurrent_sessions: decision.remainingConcurrentSessions,
      remaining_requests: decision.remainingRequests,
      window_seconds: decision.windowSeconds,
    },
    {
      headers: admissionRefusalHeaders(decision),
      status: decision.statusCode,
    },
  )

// ROUTE: POST /v1/cloud-coding-sessions (launch a managed cloud coding session).
// INERT (404) by default until the EPIC lands.
export const handleCloudCodingSessionLaunch = (
  request: Request,
  deps: CloudCodingSessionServiceDeps,
) =>
  Effect.gen(function* () {
    // INERT GATE.
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'cloud_coding_sessions_disabled' },
        { status: 404 },
      )
    }

    if (request.method !== 'POST') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
    }

    const session = yield* Effect.promise(() => deps.authenticate(request))
    if (session === undefined) {
      const headers = new Headers({ 'www-authenticate': 'Bearer' })
      return noStoreJsonResponse(
        { error: 'unauthorized' },
        { headers, status: 401 },
      )
    }

    const rawBody = yield* Effect.promise(async () => {
      try {
        return (await request.json()) as Record<string, unknown>
      } catch {
        return undefined
      }
    })
    if (rawBody === undefined) {
      return noStoreJsonResponse({ error: 'invalid_json' }, { status: 400 })
    }
    if (hasUserCapacityOption(rawBody)) {
      return noStoreJsonResponse(
        { error: 'user_pylon_capacity_not_admissible' },
        { status: 400 },
      )
    }

    const body = decodeBody(rawBody)
    if (
      body === undefined ||
      body.repoRef.trim() === '' ||
      body.objective.trim() === ''
    ) {
      return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
    }

    const sessionRequest = toSessionRequest(body, rawBody)

    // COST / ABUSE CONTROL: a non-positive or over-ceiling timeout is rejected
    // before any placement, so a single launch cannot pin a cloud VM forever.
    if (
      sessionRequest.timeoutSeconds <= 0 ||
      sessionRequest.timeoutSeconds > MAX_CLOUD_CODING_TIMEOUT_SECONDS
    ) {
      return noStoreJsonResponse(
        {
          error: 'invalid_timeout',
          maxTimeoutSeconds: MAX_CLOUD_CODING_TIMEOUT_SECONDS,
        },
        { status: 400 },
      )
    }

    // PLACEMENT POLICY (authority boundary): honor the repo trust tier BEFORE any
    // dispatch. A regulated repo requesting cloud-gcp is refused here; nothing
    // reaches a VM.
    const placement = decidePlacement({
      lane: sessionRequest.lane,
      tier: sessionRequest.repoTrustTier,
    })
    if (!placement.allowed) {
      return noStoreJsonResponse(
        {
          admissibleLanes: placement.admissibleLanes,
          error: 'lane_not_admissible_for_trust_tier',
          repoTrustTier: placement.tier,
          requestedLane: placement.requestedLane,
        },
        { status: 403 },
      )
    }

    const newId = deps.newId ?? (() => compactRandomId('ccs'))
    const sessionId = newId()
    const workContextRef = workContextRefForSession(sessionRequest, sessionId)
    const admissionGate =
      deps.admissionGate ?? unconfiguredCloudCodingAdmissionGate
    const admission = yield* admissionGate({
      accountRef: session.accountRef,
      lane: placement.lane,
      request: sessionRequest,
      sessionId,
      workContextRef,
    })
    if (!admission.allowed) {
      return admissionRefusalResponse(admission)
    }

    const adapter =
      deps.adapter ??
      makeCloudControlCloudCodingAdapter({
        baseUrl: '',
        bearerToken: '',
        gceProvisioningArmed: false,
      })
    const meteringHook = deps.meteringHook ?? stubCloudCodingMeteringHook

    const launched = yield* adapter
      .launch({
        accountRef: session.accountRef,
        lane: placement.lane,
        request: sessionRequest,
        sessionId,
      })
      .pipe(
        Effect.map(launchedSession => ({
          ok: true as const,
          session: launchedSession,
        })),
        Effect.catch(error =>
          Effect.succeed({ ok: false as const, reason: error.reason }),
        ),
      )
    if (!launched.ok) {
      return noStoreJsonResponse(
        { error: 'runtime_error', reason: launched.reason },
        { status: 502 },
      )
    }

    // Metering/receipt hook. At launch there is no metered usage yet, so the stub
    // reports `metered: false`; a live hook records the per-session charge once
    // the runtime reports real usage (the resource_usage_receipt round-trip).
    const metering = yield* meteringHook({
      accountRef: session.accountRef,
      lane: placement.lane,
      sessionId,
    })

    return noStoreJsonResponse({
      ...projectSession(launched.session),
      // Honest receipt projection: reports whether metering is live (stub =>
      // metered:false). It NEVER claims a paid/completed cloud result.
      metered: metering.metered,
      receipt_ref: metering.receiptRef,
    })
  })

// ROUTE: GET /v1/cloud-coding-sessions/:sessionId (lifecycle read). INERT (404)
// by default. Resolves the current state of a session for the AUTHENTICATED
// account only — the adapter's `get` enforces cross-account isolation. The stub
// adapter has no persistence, so it always resolves to 404; a live adapter reads
// the cloud session store.
export const handleCloudCodingSessionGet = (
  request: Request,
  sessionId: string,
  deps: CloudCodingSessionServiceDeps,
) =>
  Effect.gen(function* () {
    // INERT GATE.
    if (!deps.enabled) {
      return noStoreJsonResponse(
        { error: 'cloud_coding_sessions_disabled' },
        { status: 404 },
      )
    }

    if (request.method !== 'GET') {
      return noStoreJsonResponse({ error: 'method_not_allowed' }, { status: 405 })
    }

    if (sessionId.trim() === '') {
      return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
    }

    const session = yield* Effect.promise(() => deps.authenticate(request))
    if (session === undefined) {
      const headers = new Headers({ 'www-authenticate': 'Bearer' })
      return noStoreJsonResponse(
        { error: 'unauthorized' },
        { headers, status: 401 },
      )
    }

    const adapter = deps.adapter ?? stubCloudCodingAdapter
    const resolved = yield* adapter
      .get({ accountRef: session.accountRef, sessionId })
      .pipe(
        Effect.map(resolvedSession => ({
          ok: true as const,
          session: resolvedSession,
        })),
        Effect.catch(error =>
          Effect.succeed({ ok: false as const, reason: error.reason }),
        ),
      )
    if (!resolved.ok) {
      return noStoreJsonResponse(
        { error: 'runtime_error', reason: resolved.reason },
        { status: 502 },
      )
    }

    if (resolved.session === undefined) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    return noStoreJsonResponse(projectSession(resolved.session))
  })

// DYNAMIC DISPATCHER -------------------------------------------------------
// Single OptionalEffectRoute the Worker wires: routes POST /v1/cloud-coding-
// sessions (launch) and GET /v1/cloud-coding-sessions/:sessionId (lifecycle
// read). Returns undefined for any non-matching path so the main router falls
// through. INERT-gating lives in the handlers, so an unmatched method on a
// matching path still returns the typed 405 (not a fall-through 404).
const CLOUD_CODING_SESSIONS_BASE = '/v1/cloud-coding-sessions'

export const routeCloudCodingSessionRequest = (
  request: Request,
  deps: CloudCodingSessionServiceDeps,
): Effect.Effect<Response> | undefined => {
  const pathname = new URL(request.url).pathname
  if (pathname === CLOUD_CODING_SESSIONS_BASE) {
    return handleCloudCodingSessionLaunch(request, deps)
  }
  const prefix = `${CLOUD_CODING_SESSIONS_BASE}/`
  if (pathname.startsWith(prefix)) {
    const sessionId = decodeURIComponent(pathname.slice(prefix.length))
    // A trailing-slash-only or nested path is not a valid session id.
    if (sessionId === '' || sessionId.includes('/')) {
      return undefined
    }
    return handleCloudCodingSessionGet(request, sessionId, deps)
  }
  return undefined
}
