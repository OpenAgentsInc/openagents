import { Schema as S } from 'effect'

import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const BusinessFunnelDashboardEndpoint =
  '/api/public/business/funnel-dashboard' as const

export const BusinessFunnelStage = S.Literals([
  'visit',
  'signup',
  'intake_spec',
  'payment',
  'provisioned',
  'first_outcome',
  'retained',
  'referred_engagement',
])
export type BusinessFunnelStage = typeof BusinessFunnelStage.Type

export const BusinessFunnelSourceKind = S.Literals([
  'content',
  'outbound',
  'ai_search',
  'referral',
  'direct',
  'unknown',
])
export type BusinessFunnelSourceKind =
  typeof BusinessFunnelSourceKind.Type

export const BUSINESS_FUNNEL_STAGE_ORDER: ReadonlyArray<BusinessFunnelStage> = [
  'visit',
  'signup',
  'intake_spec',
  'payment',
  'provisioned',
  'first_outcome',
  'retained',
  'referred_engagement',
]

export const BUSINESS_FUNNEL_SOURCE_KINDS: ReadonlyArray<BusinessFunnelSourceKind> =
  ['content', 'outbound', 'ai_search', 'referral', 'direct', 'unknown']

export type BusinessFunnelEventInput = Readonly<{
  eventRef: string
  stage: BusinessFunnelStage
  sourceKind: BusinessFunnelSourceKind
  sourceRef: string | null
  occurredAt: string
}>

export type BusinessFunnelEventRecord = BusinessFunnelEventInput &
  Readonly<{
    id: string
    observedAt: string
  }>

export type BusinessFunnelRuntime = Readonly<{
  makeId: (prefix: string) => string
  nowIso: () => string
}>

export const systemBusinessFunnelRuntime: BusinessFunnelRuntime = {
  makeId: compactRandomId,
  nowIso: currentIsoTimestamp,
}

type BusinessFunnelAggregateRow = Readonly<{
  stage: BusinessFunnelStage
  source_kind: BusinessFunnelSourceKind
  count: number
  last_occurred_at: string | null
}>

type BusinessFunnelTotalRow = Readonly<{
  count: number
}>

export type BusinessFunnelStageSummary = Readonly<{
  stage: BusinessFunnelStage
  count: number
  sourceBreakdown: ReadonlyArray<
    Readonly<{
      sourceKind: BusinessFunnelSourceKind
      count: number
    }>
  >
  lastOccurredAt: string | null
}>

export type PublicBusinessFunnelDashboardResponse = Readonly<{
  schemaVersion: 'openagents.business_funnel_dashboard.v1'
  generatedAt: string
  staleness: PublicProjectionStalenessContract
  stageOrder: ReadonlyArray<BusinessFunnelStage>
  totals: Readonly<{
    eventCount: number
  }>
  stages: ReadonlyArray<BusinessFunnelStageSummary>
  sourceKinds: ReadonlyArray<BusinessFunnelSourceKind>
  privacyBoundary: Readonly<{
    aggregateOnly: true
    excludes: ReadonlyArray<string>
  }>
  evidenceRefs: ReadonlyArray<string>
}>

const normalizeEventRef = (value: string): string => value.trim().slice(0, 240)

const normalizeSourceRef = (value: string | null): string | null => {
  if (value === null) {
    return null
  }
  const trimmed = value.trim().slice(0, 240)
  return trimmed === '' ? null : trimmed
}

export const businessFunnelSourceKindFromAttribution = (
  sourceAttribution: string | null,
): BusinessFunnelSourceKind => {
  if (sourceAttribution === null) {
    return 'direct'
  }

  const normalized = sourceAttribution.trim().toLowerCase()

  if (normalized === 'content') {
    return 'content'
  }
  if (normalized === 'outbound') {
    return 'outbound'
  }
  if (
    normalized === 'ai-search' ||
    normalized === 'ai_search' ||
    normalized === 'aisearch'
  ) {
    return 'ai_search'
  }
  if (normalized === 'referral') {
    return 'referral'
  }
  if (normalized === 'direct') {
    return 'direct'
  }

  return 'unknown'
}

export const businessFunnelSourceKindForSignup = (
  input: Readonly<{
    sourceAttribution: string | null
    referralCode: string | null
  }>,
): BusinessFunnelSourceKind => {
  const attributed = businessFunnelSourceKindFromAttribution(
    input.sourceAttribution,
  )

  if (attributed !== 'direct' || input.referralCode === null) {
    return attributed
  }

  return 'referral'
}

export const recordBusinessFunnelEvent = async (
  db: D1Database,
  input: BusinessFunnelEventInput,
  runtime: BusinessFunnelRuntime = systemBusinessFunnelRuntime,
): Promise<BusinessFunnelEventRecord> => {
  const record: BusinessFunnelEventRecord = {
    id: runtime.makeId('business_funnel_event'),
    eventRef: normalizeEventRef(input.eventRef),
    stage: input.stage,
    sourceKind: input.sourceKind,
    sourceRef: normalizeSourceRef(input.sourceRef),
    occurredAt: input.occurredAt,
    observedAt: runtime.nowIso(),
  }

  await db
    .prepare(
      `INSERT OR IGNORE INTO business_funnel_events (
        id,
        event_ref,
        stage,
        source_kind,
        source_ref,
        occurred_at,
        observed_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      record.id,
      record.eventRef,
      record.stage,
      record.sourceKind,
      record.sourceRef,
      record.occurredAt,
      record.observedAt,
    )
    .run()

  return record
}

const emptySourceCounts = (): Record<BusinessFunnelSourceKind, number> => ({
  content: 0,
  outbound: 0,
  ai_search: 0,
  referral: 0,
  direct: 0,
  unknown: 0,
})

const buildEmptyStageMap = (): Record<
  BusinessFunnelStage,
  {
    count: number
    sourceCounts: Record<BusinessFunnelSourceKind, number>
    lastOccurredAt: string | null
  }
> => ({
  visit: { count: 0, sourceCounts: emptySourceCounts(), lastOccurredAt: null },
  signup: { count: 0, sourceCounts: emptySourceCounts(), lastOccurredAt: null },
  intake_spec: {
    count: 0,
    sourceCounts: emptySourceCounts(),
    lastOccurredAt: null,
  },
  payment: { count: 0, sourceCounts: emptySourceCounts(), lastOccurredAt: null },
  provisioned: {
    count: 0,
    sourceCounts: emptySourceCounts(),
    lastOccurredAt: null,
  },
  first_outcome: {
    count: 0,
    sourceCounts: emptySourceCounts(),
    lastOccurredAt: null,
  },
  retained: {
    count: 0,
    sourceCounts: emptySourceCounts(),
    lastOccurredAt: null,
  },
  referred_engagement: {
    count: 0,
    sourceCounts: emptySourceCounts(),
    lastOccurredAt: null,
  },
})

const latestIso = (a: string | null, b: string | null): string | null => {
  if (a === null) {
    return b
  }
  if (b === null) {
    return a
  }
  return a >= b ? a : b
}

export const readBusinessFunnelDashboard = async (
  db: D1Database,
  nowIso: string,
): Promise<PublicBusinessFunnelDashboardResponse> => {
  const rows = await db
    .prepare(
      `SELECT
          stage,
          source_kind,
          COUNT(*) AS count,
          MAX(occurred_at) AS last_occurred_at
        FROM business_funnel_events
       GROUP BY stage, source_kind`,
    )
    .all<BusinessFunnelAggregateRow>()
  const total = await db
    .prepare(`SELECT COUNT(*) AS count FROM business_funnel_events`)
    .first<BusinessFunnelTotalRow>()

  const stageMap = buildEmptyStageMap()

  for (const row of rows.results ?? []) {
    const stage = stageMap[row.stage]
    stage.count += Number(row.count)
    stage.sourceCounts[row.source_kind] += Number(row.count)
    stage.lastOccurredAt = latestIso(stage.lastOccurredAt, row.last_occurred_at)
  }

  return {
    schemaVersion: 'openagents.business_funnel_dashboard.v1',
    generatedAt: nowIso,
    staleness: liveAtReadStaleness(['business_funnel_events.insert']),
    stageOrder: BUSINESS_FUNNEL_STAGE_ORDER,
    totals: {
      eventCount: Number(total?.count ?? 0),
    },
    stages: BUSINESS_FUNNEL_STAGE_ORDER.map(stage => ({
      stage,
      count: stageMap[stage].count,
      sourceBreakdown: BUSINESS_FUNNEL_SOURCE_KINDS.map(sourceKind => ({
        sourceKind,
        count: stageMap[stage].sourceCounts[sourceKind],
      })),
      lastOccurredAt: stageMap[stage].lastOccurredAt,
    })),
    sourceKinds: BUSINESS_FUNNEL_SOURCE_KINDS,
    privacyBoundary: {
      aggregateOnly: true,
      excludes: [
        'contact_email',
        'phone',
        'business_name',
        'user_id',
        'payment_payload',
        'raw_provider_payload',
      ],
    },
    evidenceRefs: [
      'table:business_funnel_events',
      'issue:8077',
      'roadmap:BF-1.4',
    ],
  }
}

export const handleBusinessFunnelDashboardApi = (
  db: D1Database,
  runtime: BusinessFunnelRuntime = systemBusinessFunnelRuntime,
): Promise<PublicBusinessFunnelDashboardResponse> =>
  readBusinessFunnelDashboard(db, runtime.nowIso())
