import { Schema as S } from 'effect'

import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import {
  businessSourceKindForSourceRef,
  coerceStoredBusinessSourceRef,
  decodeBusinessSourceRef,
} from './business-source-attribution'
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

export class BusinessFunnelValidationError extends S.TaggedErrorClass<BusinessFunnelValidationError>()(
  'BusinessFunnelValidationError',
  { reason: S.String },
) {}

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
  source_ref: string | null
  count: number
  last_occurred_at: string | null
}>

type BusinessFunnelTotalRow = Readonly<{
  count: number
}>

export type BusinessFunnelMetricStatus = 'measured' | 'not_measured'

export type BusinessFunnelRateMetric = Readonly<{
  denominator: number
  numerator: number
  status: BusinessFunnelMetricStatus
  value: number | null
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
  sourceRefBreakdown: ReadonlyArray<
    Readonly<{
      sourceKind: BusinessFunnelSourceKind
      sourceRef: string
      count: number
    }>
  >
  lastOccurredAt: string | null
}>

export type BusinessFunnelSourceRefSummary = Readonly<{
  sourceKind: BusinessFunnelSourceKind
  sourceRef: string
  eventCount: number
  stageCounts: ReadonlyArray<
    Readonly<{
      stage: BusinessFunnelStage
      count: number
    }>
  >
  rates: Readonly<{
    visitToSignup: BusinessFunnelRateMetric
    signupToSpec: BusinessFunnelRateMetric
    specToPayment: BusinessFunnelRateMetric
  }>
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
  sourceRefs: ReadonlyArray<BusinessFunnelSourceRefSummary>
  privacyBoundary: Readonly<{
    aggregateOnly: true
    excludes: ReadonlyArray<string>
  }>
  evidenceRefs: ReadonlyArray<string>
}>

const normalizeEventRef = (value: string): string => value.trim().slice(0, 240)

export const businessFunnelSourceKindForSignup = (
  input: Readonly<{ sourceRef: string }>,
): BusinessFunnelSourceKind => {
  return businessSourceKindForSourceRef(input.sourceRef)
}

export const recordBusinessFunnelEvent = async (
  db: D1Database,
  input: BusinessFunnelEventInput,
  runtime: BusinessFunnelRuntime = systemBusinessFunnelRuntime,
): Promise<BusinessFunnelEventRecord> => {
  const decodedSourceRef = decodeBusinessSourceRef(input.sourceRef)
  if ('reason' in decodedSourceRef) {
    throw new BusinessFunnelValidationError({ reason: decodedSourceRef.reason })
  }
  const sourceRef = decodedSourceRef.sourceRef
  const record: BusinessFunnelEventRecord = {
    id: runtime.makeId('business_funnel_event'),
    eventRef: normalizeEventRef(input.eventRef),
    stage: input.stage,
    sourceKind: businessSourceKindForSourceRef(sourceRef),
    sourceRef,
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
    sourceRefCounts: Map<
      string,
      Readonly<{ count: number; sourceKind: BusinessFunnelSourceKind }>
    >
    lastOccurredAt: string | null
  }
> => ({
  visit: {
    count: 0,
    sourceCounts: emptySourceCounts(),
    sourceRefCounts: new Map(),
    lastOccurredAt: null,
  },
  signup: {
    count: 0,
    sourceCounts: emptySourceCounts(),
    sourceRefCounts: new Map(),
    lastOccurredAt: null,
  },
  intake_spec: {
    count: 0,
    sourceCounts: emptySourceCounts(),
    sourceRefCounts: new Map(),
    lastOccurredAt: null,
  },
  payment: {
    count: 0,
    sourceCounts: emptySourceCounts(),
    sourceRefCounts: new Map(),
    lastOccurredAt: null,
  },
  provisioned: {
    count: 0,
    sourceCounts: emptySourceCounts(),
    sourceRefCounts: new Map(),
    lastOccurredAt: null,
  },
  first_outcome: {
    count: 0,
    sourceCounts: emptySourceCounts(),
    sourceRefCounts: new Map(),
    lastOccurredAt: null,
  },
  retained: {
    count: 0,
    sourceCounts: emptySourceCounts(),
    sourceRefCounts: new Map(),
    lastOccurredAt: null,
  },
  referred_engagement: {
    count: 0,
    sourceCounts: emptySourceCounts(),
    sourceRefCounts: new Map(),
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

const rateMetric = (
  numerator: number,
  denominator: number,
): BusinessFunnelRateMetric => ({
  denominator,
  numerator,
  status: denominator === 0 ? 'not_measured' : 'measured',
  value: denominator === 0 ? null : Number((numerator / denominator).toFixed(4)),
})

const sourceRefBreakdownForStage = (
  stage: BusinessFunnelStage,
  stageMap: ReturnType<typeof buildEmptyStageMap>,
): BusinessFunnelStageSummary['sourceRefBreakdown'] =>
  [...stageMap[stage].sourceRefCounts.entries()]
    .map(([sourceRef, entry]) => ({
      count: entry.count,
      sourceKind: entry.sourceKind,
      sourceRef,
    }))
    .sort((a, b) =>
      b.count === a.count
        ? a.sourceRef.localeCompare(b.sourceRef)
        : b.count - a.count,
    )

const sourceRefSummaries = (
  sourceStageCounts: Map<
    string,
    Readonly<{
      sourceKind: BusinessFunnelSourceKind
      stageCounts: Map<BusinessFunnelStage, number>
    }>
  >,
): ReadonlyArray<BusinessFunnelSourceRefSummary> =>
  [...sourceStageCounts.entries()]
    .map(([sourceRef, entry]) => {
      const stageCount = (stage: BusinessFunnelStage): number =>
        entry.stageCounts.get(stage) ?? 0
      const eventCount = BUSINESS_FUNNEL_STAGE_ORDER.reduce(
        (sum, stage) => sum + stageCount(stage),
        0,
      )

      return {
        eventCount,
        rates: {
          signupToSpec: rateMetric(stageCount('intake_spec'), stageCount('signup')),
          specToPayment: rateMetric(stageCount('payment'), stageCount('intake_spec')),
          visitToSignup: rateMetric(stageCount('signup'), stageCount('visit')),
        },
        sourceKind: entry.sourceKind,
        sourceRef,
        stageCounts: BUSINESS_FUNNEL_STAGE_ORDER.map(stage => ({
          count: stageCount(stage),
          stage,
        })),
      }
    })
    .sort((a, b) =>
      b.eventCount === a.eventCount
        ? a.sourceRef.localeCompare(b.sourceRef)
        : b.eventCount - a.eventCount,
    )

export const readBusinessFunnelDashboard = async (
  db: D1Database,
  nowIso: string,
): Promise<PublicBusinessFunnelDashboardResponse> => {
  const rows = await db
    .prepare(
      `SELECT
          stage,
          source_kind,
          COALESCE(NULLIF(TRIM(source_ref), ''), 'direct') AS source_ref,
          COUNT(*) AS count,
          MAX(occurred_at) AS last_occurred_at
        FROM business_funnel_events
       GROUP BY stage, source_kind, COALESCE(NULLIF(TRIM(source_ref), ''), 'direct')`,
    )
    .all<BusinessFunnelAggregateRow>()
  const total = await db
    .prepare(`SELECT COUNT(*) AS count FROM business_funnel_events`)
    .first<BusinessFunnelTotalRow>()

  const stageMap = buildEmptyStageMap()
  const sourceStageCounts = new Map<
    string,
    {
      sourceKind: BusinessFunnelSourceKind
      stageCounts: Map<BusinessFunnelStage, number>
    }
  >()

  for (const row of rows.results ?? []) {
    const stage = stageMap[row.stage]
    const count = Number(row.count)
    const sourceRef = coerceStoredBusinessSourceRef(row.source_ref)
    const sourceKind = businessSourceKindForSourceRef(sourceRef)
    stage.count += count
    stage.sourceCounts[sourceKind] += count
    const existingSourceRef = stage.sourceRefCounts.get(sourceRef)
    stage.sourceRefCounts.set(sourceRef, {
      count: (existingSourceRef?.count ?? 0) + count,
      sourceKind,
    })
    const existingSource = sourceStageCounts.get(sourceRef) ?? {
      sourceKind,
      stageCounts: new Map<BusinessFunnelStage, number>(),
    }
    existingSource.stageCounts.set(
      row.stage,
      (existingSource.stageCounts.get(row.stage) ?? 0) + count,
    )
    sourceStageCounts.set(sourceRef, existingSource)
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
      sourceRefBreakdown: sourceRefBreakdownForStage(stage, stageMap),
      lastOccurredAt: stageMap[stage].lastOccurredAt,
    })),
    sourceKinds: BUSINESS_FUNNEL_SOURCE_KINDS,
    sourceRefs: sourceRefSummaries(sourceStageCounts),
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
      'issue:8267',
      'roadmap:BF-1.4',
    ],
  }
}

export const handleBusinessFunnelDashboardApi = (
  db: D1Database,
  runtime: BusinessFunnelRuntime = systemBusinessFunnelRuntime,
): Promise<PublicBusinessFunnelDashboardResponse> =>
  readBusinessFunnelDashboard(db, runtime.nowIso())
