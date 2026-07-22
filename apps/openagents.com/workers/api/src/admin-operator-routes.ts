// Admin operator overview (#9188) — one admin-gated, public-safe, REDACTED
// snapshot of "everything going on at a glance": the live agent chains
// (Pylon/Codex assignments + their event timelines), the token-usage rollup,
// recent agent traces, the Pylon fleet, and lightweight cloud-health signals.
//
// HONESTY DISCIPLINE (mirrors admin-ops-routes.ts): every field is either a
// real read from an exact source table, or an explicit `unavailable`/
// `not_measured` marker with a reason ref — never a fabricated value. This
// route deliberately COMPOSES existing exact readers/tables rather than
// re-implementing their logic:
//   * pylon_api_assignments + pylon_api_events (the agent chains)
//   * token_usage_events (the usage rollup + recent turns)
//   * agent_traces (recent public-safe trajectory summaries)
//   * pylon_api_registrations (the fleet)
// Sources that require owner-scoped Postgres projection authority or an
// external credential (Full Auto runs, FleetRuns, releases, GitHub issues,
// Sarah activity) are returned as honest `unavailable` markers here; the
// admin UI polls their existing owner-scoped endpoints client-side to fill
// them in live. See the closing note in the admin operator UI route.
//
// REDACTION: the whole assembled snapshot is passed through `redactDeep`
// before it leaves the Worker, so even if a stored projection column ever
// carried secret-shaped material it is masked. The Pylon projection JSON
// columns are already scrubbed at ingest; this is defence in depth.

import { forbidden, methodNotAllowed, noStoreJsonResponse, unauthorized } from './http/responses'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

export const ADMIN_OPERATOR_OVERVIEW_PATH = '/api/admin/operator/overview'
export const ADMIN_OPERATOR_OVERVIEW_ROUTE_REF =
  'route.admin_operator.overview.v1'

const ORG_CLOUD_RUNTIME_DEMAND_SOURCE = 'khala_mobile_org_cloud_runtime'

// Non-terminal (live) assignment lease states — the agent chains that are
// actively in flight. Kept in sync with ACTIVE_LEASE_ASSIGNMENT_STATES in
// pylon-api.ts; duplicated as a small literal set to avoid importing the
// large pylon-api module (and its heavy schema graph) into this read route.
const ACTIVE_ASSIGNMENT_STATES: ReadonlySet<string> = new Set([
  'accepted',
  'blocked',
  'offered',
  'proof_submitted',
  'running',
])

type AdminOperatorSession = Readonly<{
  user: Readonly<{ email: string }>
}>

export type AdminOperatorOverviewDependencies<
  Session extends AdminOperatorSession,
  Bindings,
> = Readonly<{
  db: (env: Bindings) => D1Database
  isOpenAgentsAdminEmail: (email: string) => boolean
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  nowIso?: () => string
  /** Bound size for every list in the snapshot. */
  limit?: number
}>

// ---------------------------------------------------------------------------
// Redaction — mask any secret-shaped string anywhere in the snapshot.
// ---------------------------------------------------------------------------

const secretShapedPattern =
  /([A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}|\/Users\/|\/home\/|oa_agent_[A-Za-z0-9._-]+|\bsk-[A-Za-z0-9_-]{16,}\b|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github_pat_[A-Za-z0-9_]+|bearer\s+[A-Za-z0-9._~+/=-]{16,}|access[_-]?token|auth\.json|mnemonic|seed[_-]?phrase|recovery[_-]?phrase|private[_-]?key|macaroon|preimage|lnbc[0-9a-z]+|lnurl[0-9a-z]+|spark1[0-9a-z]{20,})/i

export const REDACTED_PLACEHOLDER = '[redacted]'

export const redactDeep = (value: unknown): unknown => {
  if (typeof value === 'string') {
    return secretShapedPattern.test(value) ? REDACTED_PLACEHOLDER : value
  }
  if (Array.isArray(value)) {
    return value.map(redactDeep)
  }
  if (value !== null && typeof value === 'object') {
    const out: Record<string, unknown> = {}
    for (const [key, entry] of Object.entries(value as Record<string, unknown>)) {
      // A secret-shaped KEY (e.g. "access_token") masks its whole value.
      out[key] = secretShapedPattern.test(key)
        ? REDACTED_PLACEHOLDER
        : redactDeep(entry)
    }
    return out
  }
  return value
}

// ---------------------------------------------------------------------------
// Small typed helpers
// ---------------------------------------------------------------------------

const parseProjection = (raw: unknown): unknown => {
  if (typeof raw !== 'string' || raw.trim() === '') return null
  try {
    return JSON.parse(raw) as unknown
  } catch {
    return null
  }
}

const toNumber = (value: unknown): number => {
  const n = Number(value ?? 0)
  return Number.isFinite(n) ? n : 0
}

const isoHoursAgo = (nowIso: string, hours: number): string => {
  const nowMs = Date.parse(nowIso)
  const base = Number.isFinite(nowMs) ? nowMs : Date.now()
  return new Date(base - hours * 60 * 60 * 1000).toISOString()
}

const placeholders = (count: number): string =>
  Array.from({ length: count }, () => '?').join(', ')

// ---------------------------------------------------------------------------
// Section readers (each is one bounded, exact D1 read)
// ---------------------------------------------------------------------------

type AssignmentRow = Readonly<{
  assignment_ref: string
  pylon_ref: string
  owner_agent_user_id: string
  job_kind: string
  state: string
  lease_expires_at: string
  public_projection_json: string
  created_at: string
  updated_at: string
}>

type EventRow = Readonly<{
  event_ref: string
  pylon_ref: string
  assignment_ref: string | null
  event_kind: string
  status: string
  public_projection_json: string
  created_at: string
}>

const readAgentChains = async (
  db: D1Database,
  limit: number,
): Promise<{
  chains: ReadonlyArray<unknown>
  activeCount: number
  totalRecent: number
}> => {
  const assignmentResult = await db
    .prepare(
      `SELECT assignment_ref, pylon_ref, owner_agent_user_id, job_kind, state,
              lease_expires_at, public_projection_json, created_at, updated_at
         FROM pylon_api_assignments
        WHERE archived_at IS NULL
        ORDER BY updated_at DESC
        LIMIT ?`,
    )
    .bind(limit)
    .all<AssignmentRow>()

  const assignments = assignmentResult.results ?? []
  const refs = assignments.map(row => row.assignment_ref)

  // One bounded query for the event timelines of exactly those assignments.
  const eventsByAssignment = new Map<string, EventRow[]>()
  if (refs.length > 0) {
    const eventResult = await db
      .prepare(
        `SELECT event_ref, pylon_ref, assignment_ref, event_kind, status,
                public_projection_json, created_at
           FROM pylon_api_events
          WHERE assignment_ref IN (${placeholders(refs.length)})
            AND archived_at IS NULL
          ORDER BY created_at DESC
          LIMIT ?`,
      )
      .bind(...refs, refs.length * 12)
      .all<EventRow>()

    for (const event of eventResult.results ?? []) {
      if (event.assignment_ref === null) continue
      const list = eventsByAssignment.get(event.assignment_ref) ?? []
      if (list.length < 8) list.push(event)
      eventsByAssignment.set(event.assignment_ref, list)
    }
  }

  const chains = assignments.map(row => ({
    assignmentRef: row.assignment_ref,
    pylonRef: row.pylon_ref,
    ownerUserId: row.owner_agent_user_id,
    jobKind: row.job_kind,
    state: row.state,
    active: ACTIVE_ASSIGNMENT_STATES.has(row.state),
    leaseExpiresAt: row.lease_expires_at,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    projection: parseProjection(row.public_projection_json),
    events: (eventsByAssignment.get(row.assignment_ref) ?? []).map(event => ({
      eventRef: event.event_ref,
      eventKind: event.event_kind,
      status: event.status,
      createdAt: event.created_at,
      projection: parseProjection(event.public_projection_json),
    })),
  }))

  return {
    chains,
    activeCount: chains.filter(chain => chain.active).length,
    totalRecent: chains.length,
  }
}

const readTokenRollup = async (
  db: D1Database,
  nowIso: string,
  limit: number,
): Promise<unknown> => {
  const since24h = isoHoursAgo(nowIso, 24)

  const totalsRow = await db
    .prepare(
      `SELECT COUNT(*) AS events, COALESCE(SUM(total_tokens), 0) AS tokens
         FROM token_usage_events`,
    )
    .first<{ events: number; tokens: number }>()

  const last24hRow = await db
    .prepare(
      `SELECT COUNT(*) AS events, COALESCE(SUM(total_tokens), 0) AS tokens
         FROM token_usage_events
        WHERE observed_at >= ?`,
    )
    .bind(since24h)
    .first<{ events: number; tokens: number }>()

  const byDemandSource = await db
    .prepare(
      `SELECT COALESCE(demand_source, 'unlabeled') AS demand_source,
              COUNT(*) AS events,
              COALESCE(SUM(total_tokens), 0) AS tokens
         FROM token_usage_events
        WHERE observed_at >= ?
        GROUP BY COALESCE(demand_source, 'unlabeled')
        ORDER BY tokens DESC
        LIMIT ?`,
    )
    .bind(since24h, limit)
    .all<{ demand_source: string; events: number; tokens: number }>()

  const byProvider = await db
    .prepare(
      `SELECT COALESCE(provider, 'unknown') AS provider,
              COUNT(*) AS events,
              COALESCE(SUM(total_tokens), 0) AS tokens
         FROM token_usage_events
        WHERE observed_at >= ?
        GROUP BY COALESCE(provider, 'unknown')
        ORDER BY tokens DESC
        LIMIT ?`,
    )
    .bind(since24h, limit)
    .all<{ provider: string; events: number; tokens: number }>()

  const recent = await db
    .prepare(
      `SELECT observed_at, provider, model, demand_source, demand_kind,
              total_tokens, usage_truth
         FROM token_usage_events
        ORDER BY observed_at DESC
        LIMIT ?`,
    )
    .bind(limit)
    .all<{
      observed_at: string
      provider: string | null
      model: string | null
      demand_source: string | null
      demand_kind: string | null
      total_tokens: number
      usage_truth: string
    }>()

  return {
    total: {
      events: toNumber(totalsRow?.events),
      tokens: toNumber(totalsRow?.tokens),
    },
    last24h: {
      events: toNumber(last24hRow?.events),
      tokens: toNumber(last24hRow?.tokens),
    },
    byDemandSource: (byDemandSource.results ?? []).map(row => ({
      demandSource: row.demand_source,
      events: toNumber(row.events),
      tokens: toNumber(row.tokens),
    })),
    byProvider: (byProvider.results ?? []).map(row => ({
      provider: row.provider,
      events: toNumber(row.events),
      tokens: toNumber(row.tokens),
    })),
    recent: (recent.results ?? []).map(row => ({
      observedAt: row.observed_at,
      provider: row.provider,
      model: row.model,
      demandSource: row.demand_source,
      demandKind: row.demand_kind,
      totalTokens: toNumber(row.total_tokens),
      usageTruth: row.usage_truth,
    })),
  }
}

const readTraces = async (db: D1Database, limit: number): Promise<unknown> => {
  const result = await db
    .prepare(
      `SELECT trace_uuid, owner_user_id, agent_ref, schema_version, visibility,
              step_count, demand_kind, demand_source, created_at
         FROM agent_traces
        ORDER BY created_at DESC
        LIMIT ?`,
    )
    .bind(limit)
    .all<{
      trace_uuid: string
      owner_user_id: string
      agent_ref: string
      schema_version: string
      visibility: string
      step_count: number
      demand_kind: string | null
      demand_source: string | null
      created_at: string
    }>()

  return (result.results ?? []).map(row => ({
    traceUuid: row.trace_uuid,
    ownerUserId: row.owner_user_id,
    agentRef: row.agent_ref,
    schemaVersion: row.schema_version,
    visibility: row.visibility,
    stepCount: toNumber(row.step_count),
    demandKind: row.demand_kind,
    demandSource: row.demand_source,
    createdAt: row.created_at,
  }))
}

const readFleet = async (
  db: D1Database,
  limit: number,
): Promise<{
  pylons: ReadonlyArray<unknown>
  onlineCount: number
  totalCount: number
}> => {
  const result = await db
    .prepare(
      `SELECT pylon_ref, display_name, status, resource_mode, wallet_ready,
              latest_heartbeat_at, public_projection_json, updated_at
         FROM pylon_api_registrations
        WHERE archived_at IS NULL
        ORDER BY updated_at DESC
        LIMIT ?`,
    )
    .bind(limit)
    .all<{
      pylon_ref: string
      display_name: string
      status: string
      resource_mode: string
      wallet_ready: number
      latest_heartbeat_at: string | null
      public_projection_json: string
      updated_at: string
    }>()

  const rows = result.results ?? []

  const totalRow = await db
    .prepare(
      `SELECT COUNT(*) AS total,
              SUM(CASE WHEN status = 'online' THEN 1 ELSE 0 END) AS online
         FROM pylon_api_registrations
        WHERE archived_at IS NULL`,
    )
    .first<{ total: number; online: number }>()

  return {
    pylons: rows.map(row => ({
      pylonRef: row.pylon_ref,
      displayName: row.display_name,
      status: row.status,
      resourceMode: row.resource_mode,
      walletReady: row.wallet_ready === 1,
      latestHeartbeatAt: row.latest_heartbeat_at,
      updatedAt: row.updated_at,
      projection: parseProjection(row.public_projection_json),
    })),
    onlineCount: toNumber(totalRow?.online),
    totalCount: toNumber(totalRow?.total),
  }
}

const readCloudHealth = async (db: D1Database): Promise<unknown> => {
  const lastOrgCloud = await db
    .prepare(
      `SELECT MAX(observed_at) AS last_at
         FROM token_usage_events
        WHERE demand_source = ?`,
    )
    .bind(ORG_CLOUD_RUNTIME_DEMAND_SOURCE)
    .first<{ last_at: string | null }>()

  const lastAny = await db
    .prepare(`SELECT MAX(observed_at) AS last_at FROM token_usage_events`)
    .first<{ last_at: string | null }>()

  return {
    lastOrgCloudTurnAt:
      lastOrgCloud?.last_at === null || lastOrgCloud?.last_at === undefined
        ? {
            status: 'not_measured',
            reasonRef: 'reason.admin_operator.no_org_cloud_turns_yet',
          }
        : { status: 'ok', value: lastOrgCloud.last_at },
    lastTokenUsageAt:
      lastAny?.last_at === null || lastAny?.last_at === undefined
        ? {
            status: 'not_measured',
            reasonRef: 'reason.admin_operator.no_token_usage_yet',
          }
        : { status: 'ok', value: lastAny.last_at },
  }
}

// Sources that need owner-scoped Postgres projection authority or an external
// credential: returned honestly as `unavailable` so the snapshot never
// fabricates them. The admin UI polls their live owner-scoped endpoints.
const unavailableSection = (endpoint: string) =>
  ({
    status: 'unavailable' as const,
    reasonRef: 'reason.admin_operator.composed_client_side',
    liveEndpoint: endpoint,
  }) as const

// ---------------------------------------------------------------------------
// Route handler
// ---------------------------------------------------------------------------

export const makeAdminOperatorOverviewHandler = <
  Session extends AdminOperatorSession,
  Bindings,
>(
  dependencies: AdminOperatorOverviewDependencies<Session, Bindings>,
) => ({
  handleAdminOperatorOverview: async (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Promise<HttpResponse> => {
    if (request.method !== 'GET') return methodNotAllowed(['GET'])

    const session = await dependencies.requireBrowserSession(request, env, ctx)
    if (session === undefined) return unauthorized()
    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return forbidden()
    }

    const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()
    const limit = Math.max(1, Math.min(50, dependencies.limit ?? 20))
    const db = dependencies.db(env)

    const [agentChains, tokens, traces, fleet, cloudHealth] = await Promise.all(
      [
        readAgentChains(db, limit),
        readTokenRollup(db, nowIso, limit),
        readTraces(db, limit),
        readFleet(db, limit),
        readCloudHealth(db),
      ],
    )

    const snapshot = {
      ok: true,
      routeRef: ADMIN_OPERATOR_OVERVIEW_ROUTE_REF,
      generatedAt: nowIso,
      limit,
      agentChains: {
        activeCount: agentChains.activeCount,
        recentCount: agentChains.totalRecent,
        chains: agentChains.chains,
      },
      tokens,
      traces,
      fleet,
      cloudHealth,
      // Honest markers — composed client-side from the live owner-scoped
      // endpoints (see the module header and the admin UI route note).
      fullAuto: unavailableSection('/api/full-auto-runs'),
      fleetRuns: unavailableSection('/api/fleet-runs'),
      opsHealth: unavailableSection('/api/admin/ops/health'),
    }

    const redacted = redactDeep(snapshot)
    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(redacted),
      session,
    )
  },
})
