import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import {
  RequestedLoadTokenUsageStats,
  UpdatedTokenUsageStatsFilter,
  type Message,
} from '../message'
import type {
  Model,
  TokenUsageStatsFilterKey,
  TokenUsageStatsFilters,
} from '../model'

type TokenCounts = Readonly<{
  cacheReadTokens: number
  cacheWrite1hTokens: number
  cacheWrite5mTokens: number
  inputTokens: number
  outputTokens: number
  reasoningTokens: number
  totalTokens: number
}>

type AggregateRow = Readonly<{
  key: string
  label: string
  tokenCounts: TokenCounts
  usageEvents: number
}>

type AnalyticsRow = Readonly<{
  costUsd: number
  inputTokens: number
  key: string
  label: string
  outputTokens: number
  totalTokens: number
  usageEvents: number
}>

type AnalyticsDayPoint = Readonly<{
  costUsd: number
  day: string
  inputTokens: number
  outputTokens: number
  totalTokens: number
  usageEvents: number
}>

type AnalyticsDemandClientDayPoint = AnalyticsDayPoint &
  Readonly<{
    key: string
    label: string
  }>

type ActorRow = Readonly<{
  accountRef: string | null
  anonymous: boolean
  teamId: string | null
  tokenCounts: TokenCounts
  usageEvents: number
  userId: string | null
}>

type EventRecord = Readonly<{
  actor: Readonly<{
    accountRef?: string
    teamId?: string
    userId?: string
  }>
  eventId: string
  model: string | null
  observedAt: string
  privacy: Readonly<{
    leaderboardEligible: boolean
    privacyOptOut: boolean
  }>
  producerSystem: string
  provider: string | null
  safeMetadata: Record<string, unknown>
  sourceRefs: Readonly<{
    anonymizedSourceRef?: string
    repositoryRef?: string
    runRef?: string
    sessionRef?: string
    taskRef?: string
  }>
  sourceRoute: string
  tokenCounts: TokenCounts
  usageTruth: string
}>

const numberFormatter = new Intl.NumberFormat('en-US')
const usdFormatter = new Intl.NumberFormat('en-US', {
  currency: 'USD',
  maximumFractionDigits: 4,
  minimumFractionDigits: 2,
  style: 'currency',
})

const emptyCounts: TokenCounts = {
  cacheReadTokens: 0,
  cacheWrite1hTokens: 0,
  cacheWrite5mTokens: 0,
  inputTokens: 0,
  outputTokens: 0,
  reasoningTokens: 0,
  totalTokens: 0,
}

const unsafeLabelPattern =
  /(access[_-]?token|api[_-]?key|authorization|bearer|completion|cookie|credential|private|prompt|provider[_-]?(payload|secret|token)|raw|refresh[_-]?token|secret|source[_-]?code)/i

const unsafeValuePattern =
  /(\/Users\/|\/home\/|Bearer\s+[A-Za-z0-9._-]{8,}|authorization:\s*bearer|api[_-]?key=|callback[_-]?token|cookie=|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|private[_-]?(repo|source)|raw[_-]?(completion|payload|prompt|provider|response|source|text)|secret|sk-[a-z0-9])/i

const formatNumber = (value: number): string =>
  numberFormatter.format(Math.max(0, Math.trunc(value)))

const formatUsd = (value: number): string =>
  usdFormatter.format(Math.max(0, value))

const safeText = (value: string | null | undefined): string => {
  const text = value?.trim()

  if (text === undefined || text === '') {
    return 'Unknown'
  }

  return unsafeValuePattern.test(text) ? '[redacted]' : text
}

const analyticsLabel = (row: Pick<AnalyticsRow, 'key' | 'label'>): string =>
  row.key === 'unlabeled' || row.key.startsWith('unlabeled:')
    ? safeText(row.label.replace(/^unlabeled/i, 'Unlabeled traffic'))
    : safeText(row.label)

const truthEventCount = (
  rows: ReadonlyArray<AggregateRow>,
  truth: string,
): number => rows.find(row => row.key === truth)?.usageEvents ?? 0

const actorLabel = (row: ActorRow): string =>
  row.anonymous
    ? 'Anonymous/anonymized source'
    : safeText(row.userId ?? row.teamId ?? row.accountRef)

const eventActorLabel = (event: EventRecord): string =>
  event.privacy.privacyOptOut ||
  (event.actor.userId === undefined &&
    event.actor.teamId === undefined &&
    event.actor.accountRef === undefined)
    ? 'Anonymous/anonymized source'
    : safeText(event.actor.userId ?? event.actor.teamId ?? event.actor.accountRef)

const eventRefLabel = (event: EventRecord): string => {
  const refs = (
    [
      ['repo', event.sourceRefs.repositoryRef],
      ['run', event.sourceRefs.runRef],
      ['session', event.sourceRefs.sessionRef],
      ['task', event.sourceRefs.taskRef],
      ['anon', event.sourceRefs.anonymizedSourceRef],
    ] as ReadonlyArray<readonly [string, string | undefined]>
  )
    .filter((entry): entry is [string, string] => entry[1] !== undefined)
    .map(([label, value]) => `${label}:${safeText(value)}`)

  return refs.length === 0 ? 'No safe refs' : refs.join(' | ')
}

const privacyLabel = (event: EventRecord): string =>
  event.privacy.privacyOptOut
    ? 'Privacy opt-out'
    : event.privacy.leaderboardEligible
      ? 'Leaderboard eligible'
      : 'Leaderboard excluded'

const metadataSnippet = (metadata: Record<string, unknown>): string => {
  const entries = Object.entries(metadata)
    .filter(([key, value]) => {
      if (unsafeLabelPattern.test(key)) {
        return false
      }

      if (
        typeof value !== 'string' &&
        typeof value !== 'number' &&
        typeof value !== 'boolean'
      ) {
        return false
      }

      return !unsafeValuePattern.test(String(value))
    })
    .slice(0, 3)
    .map(([key, value]) => `${key}:${String(value)}`)

  return entries.length === 0 ? 'No safe metadata' : entries.join(' | ')
}

const fieldLabelClass =
  'm-0 text-[0.6875rem] font-medium uppercase tracking-normal text-white/45'
const inputClass =
  'w-full min-w-0 border border-[#222] bg-[#030303] px-3 py-2 font-mono text-sm leading-5 text-[#f1efe8] outline-none focus:border-[#ffb400] focus:ring-1 focus:ring-[#ffb400]'
const tableHeaderClass =
  'grid gap-3 border-b border-[#222] bg-[#050505] px-3 py-2 text-[0.6875rem] uppercase tracking-normal text-white/45'
const tableRowClass =
  'grid gap-3 border-b border-[#181818] bg-[#010102] px-3 py-3 text-sm text-white/70 last:border-b-0'

const filterInput = (
  filters: TokenUsageStatsFilters,
  field: TokenUsageStatsFilterKey,
  label: string,
  placeholder: string,
): Html => {
  const h = html<Message>()

  return h.label(
    [Ui.className<Message>('grid gap-1.5')],
    [
      h.span([Ui.className<Message>(fieldLabelClass)], [label]),
      h.input([
        h.Type('text'),
        h.Value(filters[field]),
        h.Placeholder(placeholder),
        Ui.className<Message>(inputClass),
        h.OnInput(value => UpdatedTokenUsageStatsFilter({ field, value })),
      ]),
    ],
  )
}

const filterSelect = (
  filters: TokenUsageStatsFilters,
  field: TokenUsageStatsFilterKey,
  label: string,
  options: ReadonlyArray<readonly [string, string]>,
): Html => {
  const h = html<Message>()

  return h.label(
    [Ui.className<Message>('grid gap-1.5')],
    [
      h.span([Ui.className<Message>(fieldLabelClass)], [label]),
      h.select(
        [
          h.Value(filters[field]),
          Ui.className<Message>(inputClass),
          h.OnInput(value => UpdatedTokenUsageStatsFilter({ field, value })),
        ],
        options.map(([value, optionLabel]) =>
          h.option(
            [
              h.Value(value),
              ...(filters[field] === value ? [h.Selected(true)] : []),
            ],
            [optionLabel],
          ),
        ),
      ),
    ],
  )
}

const filterPanel = (filters: TokenUsageStatsFilters): Html => {
  const h = html<Message>()

  return Ui.section<Message>(
    [
      Ui.headingBlock<Message>({
        eyebrow: 'Filters',
        title: 'Token usage scope',
        body: 'Blank filters include all canonical ledger events for the selected time window.',
        level: 2,
      }),
      h.div(
        [Ui.className<Message>('mt-4 grid gap-3 md:grid-cols-2 xl:grid-cols-4')],
        [
          filterInput(filters, 'since', 'Since', '2026-06-08T00:00:00.000Z'),
          filterInput(filters, 'until', 'Until', '2026-06-08T23:59:59.999Z'),
          filterInput(filters, 'provider', 'Provider', 'google_gemini'),
          filterInput(filters, 'model', 'Model', 'gemini-3.5-flash'),
          filterSelect(filters, 'producerSystem', 'Source system', [
            ['', 'All systems'],
            ['probe', 'Probe'],
            ['omega', 'Omega'],
            ['provider_broker', 'Provider broker'],
            ['shc_runner', 'Benchmark runner'],
            ['manual', 'Manual'],
            ['unknown', 'Unknown'],
          ]),
          filterSelect(filters, 'sourceRoute', 'Source route', [
            ['', 'All routes'],
            ['probe_direct_provider', 'Probe direct provider'],
            ['probe_local_model', 'Probe local model'],
            ['omega_provider_broker', 'Omega provider broker'],
            ['omega_hosted_gemini', 'Omega hosted Gemini'],
            ['shc_runner_callback', 'Benchmark runner callback'],
            ['manual', 'Manual'],
            ['unknown', 'Unknown'],
          ]),
          filterInput(filters, 'actorUserId', 'Actor user', 'user id'),
          filterInput(filters, 'actorTeamId', 'Actor team', 'team id'),
          filterSelect(filters, 'leaderboardWindow', 'Leaderboard window', [
            ['today', 'Today'],
            ['7d', '7 days'],
            ['30d', '30 days'],
            ['all', 'All time'],
          ]),
          filterSelect(filters, 'usageTruth', 'Usage truth', [
            ['', 'All truth states'],
            ['exact', 'Exact'],
            ['estimated', 'Estimated'],
            ['unknown', 'Unknown'],
          ]),
          filterSelect(filters, 'leaderboardEligible', 'Leaderboard', [
            ['', 'All events'],
            ['true', 'Eligible only'],
            ['false', 'Excluded only'],
          ]),
        ],
      ),
      h.div(
        [Ui.className<Message>('mt-4 flex justify-end')],
        [
          Ui.button<Message>({
            label: 'Apply filters',
            size: 'sm',
            attrs: [
              h.Type('button'),
              h.OnClick(RequestedLoadTokenUsageStats()),
            ],
          }),
        ],
      ),
    ],
    [Ui.className<Message>('mt-4')],
  )
}

const aggregateRowsTable = (input: {
  title: string
  body: string
  rows: ReadonlyArray<AggregateRow>
}): Html => {
  const h = html<Message>()

  return Ui.section<Message>(
    [
      Ui.headingBlock<Message>({
        eyebrow: 'Breakdown',
        title: input.title,
        body: input.body,
        level: 2,
      }),
      h.div(
        [Ui.className<Message>('mt-4 overflow-x-auto border border-[#222]')],
        [
          h.div(
            [Ui.className<Message>(`${tableHeaderClass} grid-cols-[minmax(14rem,1fr)_repeat(3,minmax(6rem,auto))]`)],
            [h.span([], ['Label']), h.span([], ['Tokens']), h.span([], ['Events']), h.span([], ['In / Out'])],
          ),
          ...(input.rows.length === 0
            ? [
                h.div(
                  [Ui.className<Message>('bg-[#010102] px-3 py-4 text-sm text-white/45')],
                  ['No rows for this filter.'],
                ),
              ]
            : input.rows.map(row =>
                h.div(
                  [Ui.className<Message>(`${tableRowClass} grid-cols-[minmax(14rem,1fr)_repeat(3,minmax(6rem,auto))]`)],
                  [
                    h.span([Ui.className<Message>('min-w-0 truncate text-[#f1efe8]')], [
                      safeText(row.label),
                    ]),
                    h.span([], [formatNumber(row.tokenCounts.totalTokens)]),
                    h.span([], [formatNumber(row.usageEvents)]),
                    h.span([], [
                      `${formatNumber(row.tokenCounts.inputTokens)} / ${formatNumber(row.tokenCounts.outputTokens)}`,
                    ]),
                  ],
                ),
              )),
        ],
      ),
    ],
    [Ui.className<Message>('min-w-0')],
  )
}

const actorRowsTable = (input: {
  body: string
  rows: ReadonlyArray<ActorRow>
  title: string
}): Html => {
  const h = html<Message>()

  return Ui.section<Message>(
    [
      Ui.headingBlock<Message>({
        eyebrow: 'Identity',
        title: input.title,
        body: input.body,
        level: 2,
      }),
      h.div(
        [Ui.className<Message>('mt-4 overflow-x-auto border border-[#222]')],
        [
          h.div(
            [Ui.className<Message>(`${tableHeaderClass} grid-cols-[minmax(14rem,1fr)_repeat(3,minmax(6rem,auto))]`)],
            [h.span([], ['Actor']), h.span([], ['Tokens']), h.span([], ['Events']), h.span([], ['Team'])],
          ),
          ...(input.rows.length === 0
            ? [
                h.div(
                  [Ui.className<Message>('bg-[#010102] px-3 py-4 text-sm text-white/45')],
                  ['No actor rows for this filter.'],
                ),
              ]
            : input.rows.map(row =>
                h.div(
                  [Ui.className<Message>(`${tableRowClass} grid-cols-[minmax(14rem,1fr)_repeat(3,minmax(6rem,auto))]`)],
                  [
                    h.span([Ui.className<Message>('min-w-0 truncate text-[#f1efe8]')], [
                      actorLabel(row),
                    ]),
                    h.span([], [formatNumber(row.tokenCounts.totalTokens)]),
                    h.span([], [formatNumber(row.usageEvents)]),
                    h.span([], [safeText(row.teamId)]),
                  ],
                ),
              )),
        ],
      ),
    ],
    [Ui.className<Message>('min-w-0')],
  )
}

const analyticsRowsTable = (input: {
  body: string
  eyebrow: string
  firstColumn: string
  rows: ReadonlyArray<AnalyticsRow>
  title: string
}): Html => {
  const h = html<Message>()

  return Ui.section<Message>(
    [
      Ui.headingBlock<Message>({
        eyebrow: input.eyebrow,
        title: input.title,
        body: input.body,
        level: 2,
      }),
      h.div(
        [Ui.className<Message>('mt-4 overflow-x-auto border border-[#222]')],
        [
          h.div(
            [
              Ui.className<Message>(
                `${tableHeaderClass} grid-cols-[minmax(14rem,1fr)_repeat(4,minmax(6rem,auto))]`,
              ),
            ],
            [
              h.span([], [input.firstColumn]),
              h.span([], ['Tokens']),
              h.span([], ['Events']),
              h.span([], ['Cost']),
              h.span([], ['In / Out']),
            ],
          ),
          ...(input.rows.length === 0
            ? [
                h.div(
                  [
                    Ui.className<Message>(
                      'bg-[#010102] px-3 py-4 text-sm text-white/45',
                    ),
                  ],
                  ['No owner analytics rows for this window.'],
                ),
              ]
            : input.rows.map(row =>
                h.div(
                  [
                    Ui.className<Message>(
                      `${tableRowClass} grid-cols-[minmax(14rem,1fr)_repeat(4,minmax(6rem,auto))]`,
                    ),
                  ],
                  [
                    h.span(
                      [
                        Ui.className<Message>(
                          'min-w-0 truncate text-[#f1efe8]',
                        ),
                      ],
                      [analyticsLabel(row)],
                    ),
                    h.span([], [formatNumber(row.totalTokens)]),
                    h.span([], [formatNumber(row.usageEvents)]),
                    h.span([], [formatUsd(row.costUsd)]),
                    h.span([], [
                      `${formatNumber(row.inputTokens)} / ${formatNumber(row.outputTokens)}`,
                    ]),
                  ],
                ),
              )),
        ],
      ),
    ],
    [Ui.className<Message>('min-w-0')],
  )
}

const analyticsDayRowsTable = (rows: ReadonlyArray<AnalyticsDayPoint>): Html => {
  const h = html<Message>()

  return Ui.section<Message>(
    [
      Ui.headingBlock<Message>({
        eyebrow: 'History',
        title: 'Daily served-token history',
        body: 'UTC daily buckets for the selected owner analytics window.',
        level: 2,
      }),
      h.div(
        [Ui.className<Message>('mt-4 overflow-x-auto border border-[#222]')],
        [
          h.div(
            [
              Ui.className<Message>(
                `${tableHeaderClass} grid-cols-[minmax(8rem,1fr)_repeat(4,minmax(6rem,auto))]`,
              ),
            ],
            [
              h.span([], ['Day']),
              h.span([], ['Tokens']),
              h.span([], ['Events']),
              h.span([], ['Cost']),
              h.span([], ['In / Out']),
            ],
          ),
          ...(rows.length === 0
            ? [
                h.div(
                  [
                    Ui.className<Message>(
                      'bg-[#010102] px-3 py-4 text-sm text-white/45',
                    ),
                  ],
                  ['No daily analytics rows for this window.'],
                ),
              ]
            : rows.map(row =>
                h.div(
                  [
                    Ui.className<Message>(
                      `${tableRowClass} grid-cols-[minmax(8rem,1fr)_repeat(4,minmax(6rem,auto))]`,
                    ),
                  ],
                  [
                    h.span([Ui.className<Message>('text-[#f1efe8]')], [
                      safeText(row.day),
                    ]),
                    h.span([], [formatNumber(row.totalTokens)]),
                    h.span([], [formatNumber(row.usageEvents)]),
                    h.span([], [formatUsd(row.costUsd)]),
                    h.span([], [
                      `${formatNumber(row.inputTokens)} / ${formatNumber(row.outputTokens)}`,
                    ]),
                  ],
                ),
              )),
        ],
      ),
    ],
    [Ui.className<Message>('min-w-0')],
  )
}

const analyticsDemandClientDayTable = (
  rows: ReadonlyArray<AnalyticsDemandClientDayPoint>,
): Html => {
  const h = html<Message>()

  return Ui.section<Message>(
    [
      Ui.headingBlock<Message>({
        eyebrow: 'Adoption',
        title: 'Per-tool adoption over time',
        body: 'UTC daily demand-client buckets for attributed and unlabeled traffic.',
        level: 2,
      }),
      h.div(
        [Ui.className<Message>('mt-4 overflow-x-auto border border-[#222]')],
        [
          h.div(
            [
              Ui.className<Message>(
                `${tableHeaderClass} grid-cols-[8rem_minmax(14rem,1fr)_repeat(3,minmax(6rem,auto))]`,
              ),
            ],
            [
              h.span([], ['Day']),
              h.span([], ['Tool']),
              h.span([], ['Tokens']),
              h.span([], ['Events']),
              h.span([], ['Cost']),
            ],
          ),
          ...(rows.length === 0
            ? [
                h.div(
                  [
                    Ui.className<Message>(
                      'bg-[#010102] px-3 py-4 text-sm text-white/45',
                    ),
                  ],
                  ['No per-tool daily analytics rows for this window.'],
                ),
              ]
            : rows.map(row =>
                h.div(
                  [
                    Ui.className<Message>(
                      `${tableRowClass} grid-cols-[8rem_minmax(14rem,1fr)_repeat(3,minmax(6rem,auto))]`,
                    ),
                  ],
                  [
                    h.span([], [safeText(row.day)]),
                    h.span(
                      [
                        Ui.className<Message>(
                          'min-w-0 truncate text-[#f1efe8]',
                        ),
                      ],
                      [analyticsLabel(row)],
                    ),
                    h.span([], [formatNumber(row.totalTokens)]),
                    h.span([], [formatNumber(row.usageEvents)]),
                    h.span([], [formatUsd(row.costUsd)]),
                  ],
                ),
              )),
        ],
      ),
    ],
    [Ui.className<Message>('min-w-0')],
  )
}

const eventRowsTable = (rows: ReadonlyArray<EventRecord>): Html => {
  const h = html<Message>()

  return Ui.section<Message>(
    [
      Ui.headingBlock<Message>({
        eyebrow: 'Drilldown',
        title: 'Recent safe token events',
        body: 'Rows show normalized ledger fields and safe metadata snippets only.',
        level: 2,
      }),
      h.div(
        [Ui.className<Message>('mt-4 overflow-x-auto border border-[#222]')],
        [
          h.div(
            [Ui.className<Message>(`${tableHeaderClass} min-w-[72rem] grid-cols-[10rem_12rem_14rem_14rem_8rem_10rem_1fr]`)],
            [h.span([], ['Observed']), h.span([], ['Model']), h.span([], ['Source']), h.span([], ['Actor']), h.span([], ['Tokens']), h.span([], ['Privacy']), h.span([], ['Safe refs / metadata'])],
          ),
          ...(rows.length === 0
            ? [
                h.div(
                  [Ui.className<Message>('bg-[#010102] px-3 py-4 text-sm text-white/45')],
                  ['No recent token events for this filter.'],
                ),
              ]
            : rows.map(event =>
                h.div(
                  [Ui.className<Message>(`${tableRowClass} min-w-[72rem] grid-cols-[10rem_12rem_14rem_14rem_8rem_10rem_1fr]`)],
                  [
                    h.span([], [safeText(event.observedAt)]),
                    h.span([Ui.className<Message>('truncate')], [
                      `${safeText(event.provider)} / ${safeText(event.model)}`,
                    ]),
                    h.span([Ui.className<Message>('truncate')], [
                      `${safeText(event.producerSystem)} / ${safeText(event.sourceRoute)} / ${safeText(event.usageTruth)}`,
                    ]),
                    h.span([Ui.className<Message>('truncate text-[#f1efe8]')], [
                      eventActorLabel(event),
                    ]),
                    h.span([], [formatNumber(event.tokenCounts.totalTokens)]),
                    h.span([], [privacyLabel(event)]),
                    h.span([Ui.className<Message>('min-w-0 truncate')], [
                      `${eventRefLabel(event)} | ${metadataSnippet(event.safeMetadata)}`,
                    ]),
                  ],
                ),
              )),
        ],
      ),
    ],
    [Ui.className<Message>('mt-4')],
  )
}

const statusBanner = (model: Model): Html => {
  const h = html<Message>()
  const stats = model.tokenUsageStats

  if (stats._tag === 'TokenUsageStatsLoading') {
    return h.div(
      [Ui.className<Message>('mt-4 border border-[#222] bg-[#050505] px-4 py-3 text-sm text-white/55')],
      ['Loading token usage ledger...'],
    )
  }

  if (stats._tag === 'TokenUsageStatsFailed') {
    return h.div(
      [Ui.className<Message>('mt-4 border border-[#5a1f1f] bg-[#170606] px-4 py-3 text-sm text-[#ffb4a8]')],
      [`Stats load failed: ${stats.error}`],
    )
  }

  if (stats._tag !== 'TokenUsageStatsLoaded') {
    return h.div(
      [Ui.className<Message>('mt-4 border border-[#222] bg-[#050505] px-4 py-3 text-sm text-white/55')],
      ['No token usage request has been loaded yet.'],
    )
  }

  return stats.response.usageEvents === 0
    ? h.div(
        [Ui.className<Message>('mt-4 border border-[#222] bg-[#050505] px-4 py-3 text-sm text-white/55')],
        ['No canonical token usage events match these filters.'],
      )
    : h.div(
        [Ui.className<Message>('mt-4 border border-[#1e3a2b] bg-[#07130d] px-4 py-3 text-sm text-[#b8f7d1]')],
        [`Generated ${safeText(stats.response.generatedAt)} from canonical token usage aggregates.`],
      )
}

const loadedResponse = (model: Model) =>
  model.tokenUsageStats._tag === 'TokenUsageStatsLoaded'
    ? model.tokenUsageStats.response
    : undefined

const loadedLeaderboards = (model: Model) =>
  model.tokenUsageStats._tag === 'TokenUsageStatsLoaded'
    ? model.tokenUsageStats.leaderboards
    : undefined

const loadedPreference = (model: Model) =>
  model.tokenUsageStats._tag === 'TokenUsageStatsLoaded'
    ? model.tokenUsageStats.preference.preference
    : undefined

const loadedAnalytics = (model: Model) =>
  model.tokenUsageStats._tag === 'TokenUsageStatsLoaded'
    ? model.tokenUsageStats.analytics
    : undefined

export const view = (model: Model): Html => {
  const response = loadedResponse(model)
  const leaderboards = loadedLeaderboards(model)
  const preference = loadedPreference(model)
  const analytics = loadedAnalytics(model)
  const totals = response?.totals ?? emptyCounts
  const usageEvents = response?.usageEvents ?? 0
  const byUsageTruth = (response?.byUsageTruth ?? []) as ReadonlyArray<AggregateRow>
  const byDemandKind = (analytics?.byDemandKind ?? []) as ReadonlyArray<AnalyticsRow>
  const byDemandSource = (analytics?.byDemandSource ?? []) as ReadonlyArray<AnalyticsRow>
  const byDemandClient = (analytics?.byDemandClient ?? []) as ReadonlyArray<AnalyticsRow>
  const byDay = (analytics?.byDay ?? []) as ReadonlyArray<AnalyticsDayPoint>
  const byDemandClientDay = (analytics?.byDemandClientDay ?? []) as ReadonlyArray<AnalyticsDemandClientDayPoint>
  const h = html<Message>()

  return Ui.container<Message>(
    [
      Ui.pageHeader<Message>({
        eyebrow: 'Stats',
        title: 'Token ledger',
        body: 'Canonical token totals across Probe, Omega, provider brokers, benchmarks, local routes, and future model producers.',
      }),
      filterPanel(model.tokenUsageStats.filters),
      statusBanner(model),
      Ui.section<Message>(
        [
          Ui.headingBlock<Message>({
            eyebrow: 'Preference',
            title: 'Current leaderboard participation',
            body:
              preference === undefined
                ? 'Load Stats to read your current leaderboard preference.'
                : `${preference.leaderboardParticipation} / ${preference.leaderboardVisibility}`,
            level: 2,
          }),
        ],
        [Ui.className<Message>('mt-4')],
      ),
      Ui.statGrid<Message>(
        [
          { label: 'Total tokens', value: formatNumber(totals.totalTokens), tone: 'accent' },
          { label: 'Input tokens', value: formatNumber(totals.inputTokens), tone: 'info' },
          { label: 'Output tokens', value: formatNumber(totals.outputTokens), tone: 'positive' },
          { label: 'Reasoning tokens', value: formatNumber(totals.reasoningTokens), tone: 'warning' },
          { label: 'Cache read', value: formatNumber(totals.cacheReadTokens), tone: 'neutral' },
          { label: 'Cache write', value: formatNumber(totals.cacheWrite5mTokens + totals.cacheWrite1hTokens), tone: 'neutral' },
          { label: 'Estimated events', value: formatNumber(truthEventCount(byUsageTruth, 'estimated')), tone: 'warning' },
          { label: 'Unknown events', value: formatNumber(truthEventCount(byUsageTruth, 'unknown')), tone: 'negative' },
          ...(analytics === undefined
            ? []
            : [
                { label: 'Owner cost', value: formatUsd(analytics.totals.costUsd), tone: 'warning' as const },
                {
                  label: 'Cost coverage',
                  value: `${formatNumber(analytics.totals.costCoverage * 100)}%`,
                  tone: analytics.totals.costCoverage < 1 ? 'negative' as const : 'positive' as const,
                },
              ]),
          { label: 'Usage events', value: formatNumber(usageEvents), tone: 'neutral' },
        ],
        [Ui.className<Message>('mt-4')],
      ),
      ...(analytics === undefined
        ? []
        : [
            h.div(
              [Ui.className<Message>('mt-4 grid gap-4 2xl:grid-cols-2')],
              [
                analyticsRowsTable({
                  body: 'External, internal, and unlabeled demand stay separated for GTM measurement.',
                  eyebrow: 'Demand',
                  firstColumn: 'Demand',
                  rows: byDemandKind,
                  title: 'Demand split',
                }),
                analyticsRowsTable({
                  body: 'Source tags show which channel or campaign produced the served tokens.',
                  eyebrow: 'Attribution',
                  firstColumn: 'Source',
                  rows: byDemandSource,
                  title: 'Per-source adoption',
                }),
                analyticsRowsTable({
                  body: 'Client tags identify the tool, SDK, runner, or app using Khala capacity.',
                  eyebrow: 'Adoption',
                  firstColumn: 'Tool',
                  rows: byDemandClient,
                  title: 'Per-tool adoption',
                }),
                analyticsDayRowsTable(byDay),
                analyticsDemandClientDayTable(byDemandClientDay),
              ],
            ),
          ]),
      h.div(
        [Ui.className<Message>('mt-4 grid gap-4 2xl:grid-cols-2')],
        [
          aggregateRowsTable({
            title: 'Provider and model',
            body: 'Totals grouped by provider/model pair.',
            rows: (response?.byProviderModel ?? []) as ReadonlyArray<AggregateRow>,
          }),
          aggregateRowsTable({
            title: 'Source system and route',
            body: 'Totals grouped by producer system and canonical source route.',
            rows: (response?.bySourceRoute ?? []) as ReadonlyArray<AggregateRow>,
          }),
          aggregateRowsTable({
            title: 'Usage truth',
            body: 'Exact, estimated, and unknown accounting buckets.',
            rows: byUsageTruth,
          }),
          aggregateRowsTable({
            title: 'Run/session/task/repository refs',
            body: 'Safe source references only; raw private paths and provider payloads are not rendered.',
            rows: (response?.bySourceRef ?? []) as ReadonlyArray<AggregateRow>,
          }),
          actorRowsTable({
            title: 'Actor and anonymous totals',
            body: 'Anonymous and privacy opt-out events stay in global totals without implying identity.',
            rows: (response?.byActor ?? []) as ReadonlyArray<ActorRow>,
          }),
          actorRowsTable({
            title: 'Top users',
            body: 'Opt-out-aware identified user rankings for the selected leaderboard window.',
            rows: (leaderboards?.topUsers ?? []) as ReadonlyArray<ActorRow>,
          }),
          actorRowsTable({
            title: 'Top teams',
            body: 'Opt-out-aware team rankings for the selected leaderboard window.',
            rows: (leaderboards?.topTeams ?? []) as ReadonlyArray<ActorRow>,
          }),
          aggregateRowsTable({
            title: 'Top runs',
            body: 'Coding-agent run rankings using safe run refs only.',
            rows: (leaderboards?.topRuns ?? []) as ReadonlyArray<AggregateRow>,
          }),
          aggregateRowsTable({
            title: 'Top projects',
            body: 'Project/repository rankings use safe refs and exclude opted-out identities.',
            rows: (leaderboards?.topProjects ?? []) as ReadonlyArray<AggregateRow>,
          }),
          aggregateRowsTable({
            title: 'Provider/model leaderboard',
            body: 'Identity-independent provider/model rankings include global usage.',
            rows: (leaderboards?.topProviderModels ?? []) as ReadonlyArray<AggregateRow>,
          }),
        ],
      ),
      eventRowsTable((response?.recentEvents ?? []) as ReadonlyArray<EventRecord>),
    ],
    [Ui.className<Message>('py-4')],
  )
}
