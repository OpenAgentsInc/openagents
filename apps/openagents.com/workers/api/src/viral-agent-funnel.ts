import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export type ViralAgentFunnelEventKind =
  | 'capability_manifest_read'
  | 'openapi_read'
  | 'agent_doc_read'
  | 'public_proof_read'
  | 'public_challenge_read'
  | 'first_scoped_action_attempt'

export type ViralAgentFunnelActorClass =
  | 'public_anonymous'
  | 'signed_in_browser_possible'
  | 'scoped_agent_possible'

export type ViralAgentFunnelUserAgentClass =
  | 'agent_or_cli'
  | 'browser'
  | 'crawler'
  | 'unknown'

export type ViralAgentFunnelRuntime = Readonly<{
  makeEventId: () => string
  nowIso: () => string
}>

export const systemViralAgentFunnelRuntime: ViralAgentFunnelRuntime = {
  makeEventId: () => compactRandomId('viral_funnel_event'),
  nowIso: currentIsoTimestamp,
}

export type ViralAgentFunnelEventInput = Readonly<{
  eventKind: ViralAgentFunnelEventKind
  metadata?: Record<string, string | number | boolean | null> | undefined
  proofRef?: string | null | undefined
  route: string
  siteSlug?: string | null | undefined
}>

export type ViralAgentFunnelEventRecord = Readonly<{
  actorClass: ViralAgentFunnelActorClass
  createdAt: string
  eventKind: ViralAgentFunnelEventKind
  id: string
  metadataJson: string
  proofRef: string | null
  route: string
  siteSlug: string | null
  userAgentClass: ViralAgentFunnelUserAgentClass
}>

export type ViralAgentFunnelAggregateRow = Readonly<{
  actorClass: ViralAgentFunnelActorClass
  count: number
  eventKind: ViralAgentFunnelEventKind
  route: string
  userAgentClass: ViralAgentFunnelUserAgentClass
}>

const metadataValue = (
  value: string | number | boolean | null,
): string | number | boolean | null =>
  typeof value === 'string' ? value.trim().slice(0, 160) : value

const safeMetadataJson = (
  metadata: Record<string, string | number | boolean | null> | undefined,
): string => {
  const entries = Object.entries(metadata ?? {})
    .slice(0, 12)
    .map(([key, value]) => [
      key.trim().replace(/[^a-zA-Z0-9_.:-]/g, '_').slice(0, 80),
      metadataValue(value),
    ])
    .filter(([key]) => key !== '')

  return JSON.stringify(Object.fromEntries(entries))
}

export const classifyViralFunnelUserAgent = (
  userAgent: string | null,
): ViralAgentFunnelUserAgentClass => {
  if (userAgent === null || userAgent.trim() === '') {
    return 'unknown'
  }

  const normalized = userAgent.toLowerCase()

  if (
    normalized.includes('codex') ||
    normalized.includes('openai') ||
    normalized.includes('anthropic') ||
    normalized.includes('claude') ||
    normalized.includes('python') ||
    normalized.includes('node') ||
    normalized.includes('curl') ||
    normalized.includes('wget') ||
    normalized.includes('go-http-client')
  ) {
    return 'agent_or_cli'
  }

  if (
    normalized.includes('bot') ||
    normalized.includes('crawler') ||
    normalized.includes('spider')
  ) {
    return 'crawler'
  }

  if (
    normalized.includes('mozilla') ||
    normalized.includes('safari') ||
    normalized.includes('chrome') ||
    normalized.includes('firefox')
  ) {
    return 'browser'
  }

  return 'unknown'
}

export const classifyViralFunnelActor = (
  request: Request,
): ViralAgentFunnelActorClass => {
  const authorization = request.headers.get('authorization')

  if (authorization?.toLowerCase().startsWith('bearer ') === true) {
    return 'scoped_agent_possible'
  }

  const cookie = request.headers.get('cookie')

  if (
    cookie?.includes('openagents') === true ||
    cookie?.includes('openauth') === true
  ) {
    return 'signed_in_browser_possible'
  }

  return 'public_anonymous'
}

export const makeViralAgentFunnelEventRecord = (
  request: Request,
  input: ViralAgentFunnelEventInput,
  runtime: ViralAgentFunnelRuntime = systemViralAgentFunnelRuntime,
): ViralAgentFunnelEventRecord => ({
  actorClass: classifyViralFunnelActor(request),
  createdAt: runtime.nowIso(),
  eventKind: input.eventKind,
  id: runtime.makeEventId(),
  metadataJson: safeMetadataJson(input.metadata),
  proofRef: input.proofRef ?? null,
  route: input.route.slice(0, 240),
  siteSlug: input.siteSlug ?? null,
  userAgentClass: classifyViralFunnelUserAgent(
    request.headers.get('user-agent'),
  ),
})

export const recordViralAgentFunnelEvent = async (
  db: D1Database,
  request: Request,
  input: ViralAgentFunnelEventInput,
  runtime: ViralAgentFunnelRuntime = systemViralAgentFunnelRuntime,
): Promise<void> => {
  const event = makeViralAgentFunnelEventRecord(request, input, runtime)

  await db
    .prepare(
      `INSERT INTO viral_agent_funnel_events
        (id, event_kind, route, actor_class, user_agent_class, site_slug,
         proof_ref, metadata_json, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      event.id,
      event.eventKind,
      event.route,
      event.actorClass,
      event.userAgentClass,
      event.siteSlug,
      event.proofRef,
      event.metadataJson,
      event.createdAt,
    )
    .run()
}

type ViralAgentFunnelAggregateRowRaw = Readonly<{
  actor_class: ViralAgentFunnelActorClass
  count: number
  event_kind: ViralAgentFunnelEventKind
  route: string
  user_agent_class: ViralAgentFunnelUserAgentClass
}>

export const summarizeViralAgentFunnelEvents = async (
  db: D1Database,
  limit = 50,
): Promise<ReadonlyArray<ViralAgentFunnelAggregateRow>> => {
  const rows = await db
    .prepare(
      `SELECT event_kind,
              route,
              actor_class,
              user_agent_class,
              COUNT(*) AS count
         FROM viral_agent_funnel_events
        GROUP BY event_kind, route, actor_class, user_agent_class
        ORDER BY count DESC, event_kind ASC
        LIMIT ?`,
    )
    .bind(Math.max(1, Math.min(200, limit)))
    .all<ViralAgentFunnelAggregateRowRaw>()

  return (rows.results ?? []).map(row => ({
    actorClass: row.actor_class,
    count: row.count,
    eventKind: row.event_kind,
    route: row.route,
    userAgentClass: row.user_agent_class,
  }))
}
