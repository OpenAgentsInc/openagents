import type {
  PublicActivityTimelineEnvelope,
  PublicActivityTimelineEvent,
} from '@openagentsinc/public-activity-timeline'
import { Array } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { userFacingCopy } from '../../../display-copy'
import { currentUnixMs, friendlyRelativeTime } from '../../../time-format'
import * as Ui from '../../../ui'
import type { Message } from '../message'
import type {
  Model,
  PublicActivityTimelineModel,
  PublicAdjutantActivityMilestone,
  PublicAdjutantActivityModel,
  PublicAdjutantDeployedSite,
  PublicAgentGoal,
  PublicAgentGoalEvent,
  PublicKhalaTokensServedHistoryModel,
  PublicKhalaTokensServedHistoryPoint,
  PublicPylonStats,
  PublicPylonStatsModel,
  PublicRecentPylon,
} from '../model'

const campaignObjective =
  'Release the next version of Pylon, connect it deeply to Omega, and route more inference and fine-tuning work to the live Pylon wave using the new Bitcoin infrastructure.'

const adjutantObjective =
  'Supervise public software-order fulfillment and Autopilot Sites delivery from order assignment through reviewable versions, deployment, customer-visible status, and public-safe progress.'

const numberFormatter = new Intl.NumberFormat('en-US')

const displayName = (agentRef: string): string =>
  agentRef === 'artanis'
    ? 'Artanis'
    : agentRef === 'adjutant'
      ? 'Autopilot'
      : agentRef

const fallbackObjective = (agentRef: string): string =>
  agentRef === 'artanis'
    ? campaignObjective
    : agentRef === 'adjutant'
      ? adjutantObjective
      : 'This public agent has not published a durable goal yet.'

const statusText = (goal: PublicAgentGoal | null): string =>
  goal === null ? 'No public goal' : goal.status.replace(/_/g, ' ')

const usageText = (goal: PublicAgentGoal): string =>
  goal.remainingTokens === null
    ? `${goal.tokensUsed} tokens`
    : `${goal.tokensUsed} / ${goal.tokenBudget ?? 0} tokens`

const tokenProgressPercent = (goal: PublicAgentGoal | null): number =>
  goal === null || goal.tokenBudget === null || goal.tokenBudget <= 0
    ? 0
    : Math.max(0, Math.min(100, (goal.tokensUsed / goal.tokenBudget) * 100))

const formatNumber = (value: number): string => numberFormatter.format(value)

const formatCompactNumber = (value: number): string =>
  new Intl.NumberFormat('en-US', {
    maximumFractionDigits: value >= 10_000 ? 1 : 0,
    notation: 'compact',
  }).format(value)

// Freshness windows for the LIVE /artanis recruitment console. This surface
// must never present old data under a live frame, so a stale goal or a stale
// per-goal activity row is dropped rather than shown as if current.
const GOAL_FRESH_WINDOW_MS = 24 * 60 * 60 * 1000
const ACTIVITY_FRESH_WINDOW_MS = 60 * 60 * 1000

const parseMs = (value: string | null | undefined): number | null => {
  if (value === null || value === undefined) {
    return null
  }
  const parsed = Date.parse(value)
  return Number.isNaN(parsed) ? null : parsed
}

// Treat a public goal as fresh only when it was updated within the window. An
// unparseable timestamp fails closed (stale) so we never render an
// unbounded-age "current goal" with a 23-day-old run id.
const isGoalFresh = (goal: PublicAgentGoal | null): boolean => {
  if (goal === null) {
    return false
  }
  const updatedMs = parseMs(goal.updatedAt)
  if (updatedMs === null) {
    return false
  }
  return currentUnixMs() - updatedMs <= GOAL_FRESH_WINDOW_MS
}

// Strip raw operational jargon (doc paths, "at commit <hash>", bare commit
// hashes, and run/goal ids) from a public objective so the recruitment console
// shows a human goal, never internal refs.
const sanitizeObjective = (value: string): string =>
  value
    .replace(
      /\bfollowing\s+\S+\.md(?:\s+at\s+commit\s+[0-9a-f]{7,40})?/gi,
      '',
    )
    .replace(/\bat\s+commit\s+[0-9a-f]{7,40}/gi, '')
    .replace(/\b[\w./-]+\.md\b/gi, '')
    .replace(/\b(?:run|commit|goal)[ _-]?[0-9a-f]{6,}\b/gi, '')
    .replace(/\b[0-9a-f]{7,40}\b/gi, '')
    .replace(/\(\s*\)/g, '')
    .replace(/\s{2,}/g, ' ')
    .replace(/\s+([.,;:])/g, '$1')
    .trim()

const tokenPaceDaySeconds = 24 * 60 * 60

type TokenPaceParts = Readonly<{
  day: string
  elapsedSeconds: number
  priorDay: string
}>

type TokenPace = Readonly<{
  day: string
  todayTokens: number
  yesterdayTokens: number
  paceProjection: number
  target4x: number
  target10x: number
  progressPct: number
  floorPct: number
  behindPace: boolean
}>

const pad2 = (value: number): string => value.toString().padStart(2, '0')

const isoTimestampMs = (value: string | undefined): number | undefined => {
  if (value === undefined) {
    return undefined
  }

  const match =
    /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,3}))?Z$/.exec(
      value,
    )
  if (match === null) {
    return undefined
  }

  const year = Number(match[1])
  const month = Number(match[2])
  const day = Number(match[3])
  const hour = Number(match[4])
  const minute = Number(match[5])
  const second = Number(match[6])
  const millisecond = Number((match[7] ?? '0').padEnd(3, '0'))

  if (
    month < 1 ||
    month > 12 ||
    day < 1 ||
    day > 31 ||
    hour > 23 ||
    minute > 59 ||
    second > 59 ||
    millisecond > 999
  ) {
    return undefined
  }

  return Date.UTC(year, month - 1, day, hour, minute, second, millisecond)
}

const timezoneParts = (
  timestampMs: number,
  timezone: string,
): Readonly<{
  year: number
  month: number
  day: number
  hour: number
  minute: number
  second: number
}> | undefined => {
  try {
    const formatter = new Intl.DateTimeFormat('en-US', {
      day: '2-digit',
      hour: '2-digit',
      hourCycle: 'h23',
      minute: '2-digit',
      month: '2-digit',
      second: '2-digit',
      timeZone: timezone,
      year: 'numeric',
    })
    const parts = formatter.formatToParts(timestampMs)
    const numberPart = (type: Intl.DateTimeFormatPartTypes): number =>
      Number(parts.find(part => part.type === type)?.value ?? '0')

    return {
      day: numberPart('day'),
      hour: numberPart('hour'),
      minute: numberPart('minute'),
      month: numberPart('month'),
      second: numberPart('second'),
      year: numberPart('year'),
    }
  } catch {
    return undefined
  }
}

const dayString = (
  parts: Readonly<{ year: number; month: number; day: number }>,
): string => `${parts.year}-${pad2(parts.month)}-${pad2(parts.day)}`

const tokenPaceParts = (
  generatedAt: string | undefined,
  timezone: string,
): TokenPaceParts | undefined => {
  const timestampMs = isoTimestampMs(generatedAt)
  if (timestampMs === undefined) {
    return undefined
  }

  const today = timezoneParts(timestampMs, timezone)
  const prior = timezoneParts(
    timestampMs - tokenPaceDaySeconds * 1000,
    timezone,
  )
  if (today === undefined || prior === undefined) {
    return undefined
  }

  return {
    day: dayString(today),
    elapsedSeconds: today.hour * 60 * 60 + today.minute * 60 + today.second,
    priorDay: dayString(prior),
  }
}

const maybePointTokensForDay = (
  series: ReadonlyArray<PublicKhalaTokensServedHistoryPoint>,
  day: string,
): number | undefined => {
  const point = series.find(
    entry => entry.day === day && Number.isFinite(entry.tokensServed),
  )

  return point === undefined
    ? undefined
    : Math.max(0, Math.trunc(point.tokensServed))
}

const latestEarlierTokens = (
  series: ReadonlyArray<PublicKhalaTokensServedHistoryPoint>,
  day: string,
): number => {
  const latest = series
    .filter(point => point.day < day && Number.isFinite(point.tokensServed))
    .sort((left, right) => left.day.localeCompare(right.day))
    .at(-1)

  return Math.max(0, Math.trunc(latest?.tokensServed ?? 0))
}

const tokenPaceFromHistory = (
  model: PublicKhalaTokensServedHistoryModel,
): TokenPace | null => {
  if (model._tag !== 'PublicKhalaTokensServedHistoryLoaded') {
    return null
  }

  const parts = tokenPaceParts(model.history.generatedAt, model.history.timezone)
  if (parts === undefined) {
    return null
  }

  const todayTokens =
    maybePointTokensForDay(model.history.series, parts.day) ?? 0
  const exactYesterday = maybePointTokensForDay(
    model.history.series,
    parts.priorDay,
  )
  const yesterdayTokens =
    exactYesterday ?? latestEarlierTokens(model.history.series, parts.day)
  const fraction =
    parts.elapsedSeconds <= 0 || parts.elapsedSeconds >= tokenPaceDaySeconds
      ? 0
      : parts.elapsedSeconds / tokenPaceDaySeconds
  const paceProjection =
    fraction <= 0
      ? todayTokens
      : Math.max(todayTokens, Math.round(todayTokens / fraction))
  const target4x = 4 * yesterdayTokens
  const target10x = 10 * yesterdayTokens
  const progressPct =
    target10x <= 0 ? 0 : Math.min(100, (todayTokens / target10x) * 100)
  const floorPct =
    target10x <= 0 ? 0 : Math.min(100, (target4x / target10x) * 100)

  return {
    behindPace: paceProjection < target4x,
    day: parts.day,
    floorPct,
    paceProjection,
    progressPct,
    target10x,
    target4x,
    todayTokens,
    yesterdayTokens,
  }
}

const tokenPaceProgressStyle = (pct: number): string =>
  `width: ${Math.max(0, Math.min(100, pct)).toFixed(2)}%;`

const tokenPaceFloorStyle = (pct: number): string =>
  `left: ${Math.max(0, Math.min(100, pct)).toFixed(2)}%;`

const tokenPaceSparklinePoints = (
  series: ReadonlyArray<PublicKhalaTokensServedHistoryPoint>,
): string => {
  const values = series.slice(-14).map(point => Math.max(0, point.tokensServed))
  const points = values.length < 2 ? [0, 0, ...values] : values
  const max = Math.max(...points, 1)
  const min = Math.min(...points, 0)
  const span = max - min || 1
  const width = 240
  const height = 64
  const step = points.length <= 1 ? width : width / (points.length - 1)

  return points
    .map((value, index) => {
      const x = (index * step).toFixed(1)
      const y = (height - ((value - min) / span) * height).toFixed(1)
      return `${x},${y}`
    })
    .join(' ')
}

const tokenPaceSparkline = (
  series: ReadonlyArray<PublicKhalaTokensServedHistoryPoint>,
): Html => {
  const h = html<Message>()
  const recent = series.slice(-14)
  const peak = recent.reduce(
    (max, point) => (point.tokensServed > max ? point.tokensServed : max),
    0,
  )
  const ariaLabel = `Recent daily token burn sparkline, ${recent.length} ${
    recent.length === 1 ? 'day' : 'days'
  }, peak ${formatNumber(peak)} tokens.`

  return h.div(
    [Ui.className<Message>('grid gap-2')],
    [
      h.svg(
        [
          h.ViewBox('0 0 240 64'),
          h.Role('img'),
          h.AriaLabel(ariaLabel),
          h.Attribute('preserveAspectRatio', 'none'),
          Ui.className<Message>(
            'h-16 w-full border border-[#222] bg-[#050505]',
          ),
        ],
        [
          h.line(
            [
              h.Attribute('x1', '0'),
              h.Attribute('y1', '32'),
              h.Attribute('x2', '240'),
              h.Attribute('y2', '32'),
              h.Attribute('stroke', '#1d1d1d'),
              h.Attribute('stroke-width', '1'),
            ],
            [],
          ),
          h.polyline(
            [
              h.Attribute('points', tokenPaceSparklinePoints(recent)),
              h.Attribute('fill', 'none'),
              h.Attribute('stroke', '#00c853'),
              h.Attribute('stroke-width', '2'),
              h.Attribute('stroke-linecap', 'round'),
              h.Attribute('stroke-linejoin', 'round'),
            ],
            [],
          ),
        ],
      ),
      h.ul(
        [Ui.className<Message>('sr-only')],
        recent.map(point =>
          h.li([], [
            `${point.day}: ${formatNumber(point.tokensServed)} tokens`,
          ]),
        ),
      ),
    ],
  )
}

const artanisPulsePlaceholder = (label: string): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['The Pulse']),
      h.div(
        [
          h.Role('status'),
          Ui.className<Message>(
            'border border-[#222] bg-[#010102] p-4 text-sm text-white/45',
          ),
        ],
        [label],
      ),
    ],
  )
}

const artanisPulseLoadedView = (
  history: PublicKhalaTokensServedHistoryModel,
  pace: TokenPace,
): Html => {
  const h = html<Message>()
  const series =
    history._tag === 'PublicKhalaTokensServedHistoryLoaded'
      ? history.history.series
      : []
  const status = pace.behindPace ? 'Behind 4x floor' : 'On 4x floor'
  const gap = Math.max(0, pace.target4x - pace.paceProjection)

  return h.section(
    [Ui.className<Message>('grid gap-4 border-b border-[#222] pb-6')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-end justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], ['The Pulse']),
              h.h2(
                [
                  Ui.className<Message>(
                    'text-xl font-semibold tracking-normal text-[#f1efe8]',
                  ),
                ],
                ['Live token burn'],
              ),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                pace.behindPace
                  ? 'border border-[#ff6f00]/60 px-2.5 py-1 text-[0.75rem] text-[#ffb26b]'
                  : 'border border-[#00c853]/60 px-2.5 py-1 text-[0.75rem] text-[#9ad6b7]',
              ),
            ],
            [status],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(18rem,0.74fr)]',
          ),
        ],
        [
          h.div(
            [
              Ui.className<Message>(
                'grid gap-3 border border-[#222] bg-[#010102] p-3',
              ),
            ],
            [
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/45')],
                ['Burn rate'],
              ),
              tokenPaceSparkline(series),
              h.div(
                [
                  Ui.className<Message>(
                    'grid gap-2 text-[0.75rem] text-white/45 sm:grid-cols-3',
                  ),
                ],
                [
                  h.div([], [
                    `Today ${formatCompactNumber(pace.todayTokens)}`,
                  ]),
                  h.div([], [
                    `Projected ${formatCompactNumber(pace.paceProjection)}`,
                  ]),
                  h.div([], [
                    `Yesterday ${formatCompactNumber(pace.yesterdayTokens)}`,
                  ]),
                ],
              ),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'grid content-between gap-4 border border-[#222] bg-[#010102] p-3',
              ),
            ],
            [
              h.div(
                [Ui.className<Message>('grid gap-1')],
                [
                  h.div(
                    [Ui.className<Message>('text-[0.75rem] text-white/45')],
                    ['Daily target'],
                  ),
                  h.div(
                    [
                      Ui.className<Message>(
                        'text-3xl font-semibold tabular-nums tracking-normal text-[#f1efe8]',
                      ),
                    ],
                    [formatCompactNumber(pace.target10x)],
                  ),
                  h.div(
                    [Ui.className<Message>('text-[0.75rem] text-white/35')],
                    [`10x yesterday / 4x floor ${formatCompactNumber(pace.target4x)}`],
                  ),
                ],
              ),
              h.div([Ui.className<Message>('grid gap-2')], [
                h.div(
                  [
                    Ui.className<Message>(
                      'relative h-3 overflow-hidden border border-[#333] bg-[#050505]',
                    ),
                  ],
                  [
                    h.div(
                      [
                        h.Attribute(
                          'style',
                          tokenPaceProgressStyle(pace.progressPct),
                        ),
                        Ui.className<Message>('h-full bg-[#00c853]'),
                      ],
                      [],
                    ),
                    h.div(
                      [
                        h.Attribute('style', tokenPaceFloorStyle(pace.floorPct)),
                        Ui.className<Message>(
                          'absolute top-[-0.25rem] h-5 border-l border-[#ffb400]',
                        ),
                      ],
                      [],
                    ),
                  ],
                ),
                h.div(
                  [
                    Ui.className<Message>(
                      'flex flex-wrap justify-between gap-2 text-[0.75rem] text-white/45',
                    ),
                  ],
                  [
                    h.span([], [`${Math.round(pace.progressPct)}% of 10x`]),
                    h.span([], [
                      pace.behindPace
                        ? `Gap ${formatCompactNumber(gap)}`
                        : '4x floor cleared',
                    ]),
                  ],
                ),
              ]),
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/35')],
                [
                  `${pace.day} / aggregate public token ledger / no user, prompt, or provider rows exposed`,
                ],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}

const artanisPulseView = (
  history: PublicKhalaTokensServedHistoryModel,
): Html => {
  if (history._tag === 'PublicKhalaTokensServedHistoryLoading') {
    return artanisPulsePlaceholder('Loading public token-burn history.')
  }

  if (history._tag === 'PublicKhalaTokensServedHistoryFailed') {
    return artanisPulsePlaceholder('Token-burn history unavailable.')
  }

  const pace = tokenPaceFromHistory(history)
  if (pace === null) {
    return artanisPulsePlaceholder('Token pace unavailable.')
  }

  return artanisPulseLoadedView(history, pace)
}

const publicRefsLabel = (
  label: string,
  refs: ReadonlyArray<string>,
): string | null =>
  Array.match(refs, {
    onEmpty: () => null,
    onNonEmpty: values => `${label} ${values.slice(0, 3).join(', ')}`,
  })

const trimProtocol = (url: string | null): string =>
  url === null
    ? 'none'
    : url.replace(/^https?:\/\//, '').replace(/^wss:\/\//, '')

const statsStatusText = (stats: PublicPylonStats | null): string =>
  stats === null ? 'loading' : stats.available ? 'live' : 'down'

const pylonStatsFromModel = (
  model: PublicPylonStatsModel,
): PublicPylonStats | null =>
  model._tag === 'PublicPylonStatsLoaded' ? model.stats : null

const artanisFleetHealthText = (stats: PublicPylonStats | null): string =>
  stats === null
    ? 'Active slots loading'
    : `Active ${formatNumber(stats.pylonsAssignmentReadyNow)}/${formatNumber(stats.pylonsOnlineNow)} slots`

const pylonStatsError = (model: PublicPylonStatsModel): string | null =>
  model._tag === 'PublicPylonStatsFailed'
    ? model.error
    : model._tag === 'PublicPylonStatsLoaded'
      ? model.stats.error
      : null

const deployedSiteRow = (site: PublicAdjutantDeployedSite): Html => {
  const h = html<Message>()

  return h.li(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#1b1b1b] py-2 last:border-b-0 sm:grid-cols-[minmax(0,1fr)_minmax(0,1.2fr)_auto]',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('min-w-0')],
        [
          h.div(
            [Ui.className<Message>('truncate text-[0.8125rem] text-[#f1efe8]')],
            [site.title],
          ),
          h.div(
            [Ui.className<Message>('truncate text-[0.75rem] text-white/35')],
            [site.publicRef],
          ),
        ],
      ),
      h.a(
        [
          h.Href(site.url),
          h.Target('_blank'),
          h.Rel('noreferrer'),
          Ui.className<Message>(
            'min-w-0 truncate text-[0.75rem] text-white/55 underline underline-offset-[3px] hover:text-[#f1efe8]',
          ),
        ],
        [trimProtocol(site.url)],
      ),
      h.div(
        [
          Ui.className<Message>(
            'tabular-nums text-[0.75rem] text-white/45 sm:text-right',
          ),
        ],
        [site.status],
      ),
    ],
  )
}

const adjutantMilestoneRow = (
  milestone: PublicAdjutantActivityMilestone,
): Html => {
  const h = html<Message>()

  return h.li(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#1b1b1b] py-3 last:border-b-0',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.span(
            [Ui.className<Message>('text-[0.8125rem] text-[#f1efe8]')],
            [userFacingCopy(milestone.label)],
          ),
          h.span(
            [Ui.className<Message>('text-[0.6875rem] text-white/35')],
            [milestone.stage],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/45')],
        [userFacingCopy(milestone.summary)],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [`${milestone.publicRef} / ${milestone.updatedAt}`],
      ),
      milestone.siteUrl === null
        ? null
        : h.a(
            [
              h.Href(milestone.siteUrl),
              h.Target('_blank'),
              h.Rel('noreferrer'),
              Ui.className<Message>(
                'text-[0.75rem] text-white/55 underline underline-offset-[3px] hover:text-[#f1efe8]',
              ),
            ],
            [trimProtocol(milestone.siteUrl)],
          ),
    ],
  )
}

const adjutantActivityView = (model: PublicAdjutantActivityModel): Html => {
  const h = html<Message>()

  if (model._tag === 'PublicAdjutantActivityLoading') {
    return h.section(
      [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
      [
        h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public Sites']),
        h.p(
          [Ui.className<Message>('text-sm text-white/45')],
          ['Loading public Autopilot activity.'],
        ),
      ],
    )
  }

  if (model._tag === 'PublicAdjutantActivityFailed') {
    return h.section(
      [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
      [
        h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public Sites']),
        h.p([Ui.className<Message>('text-sm text-[#ff6f00]')], [model.error]),
      ],
    )
  }

  if (model._tag !== 'PublicAdjutantActivityLoaded') {
    return h.section(
      [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
      [
        h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public Sites']),
        h.p(
          [Ui.className<Message>('text-sm text-white/45')],
          ['No public Autopilot activity has been published yet.'],
        ),
      ],
    )
  }

  return h.section(
    [Ui.className<Message>('grid gap-4 border-b border-[#222] pb-6')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-end justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public Sites']),
              h.h2(
                [
                  Ui.className<Message>(
                    'text-xl font-semibold tracking-normal text-[#f1efe8]',
                  ),
                ],
                ['Autopilot activity'],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            [
              `${model.activity.milestones.length} public milestone${model.activity.milestones.length === 1 ? '' : 's'}`,
            ],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]',
          ),
        ],
        [
          h.div(
            [
              Ui.className<Message>(
                'grid gap-2 border border-[#222] bg-[#010102] p-3',
              ),
            ],
            [
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/45')],
                ['Deployed public Sites'],
              ),
              Array.match(model.activity.deployedSites.slice(0, 4), {
                onEmpty: () =>
                  h.p(
                    [Ui.className<Message>('text-[0.75rem] text-white/35')],
                    ['No public Sites are deployed yet.'],
                  ),
                onNonEmpty: sites =>
                  h.ol(
                    [Ui.className<Message>('grid')],
                    sites.map(deployedSiteRow),
                  ),
              }),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'grid gap-2 border border-[#222] bg-[#010102] p-3',
              ),
            ],
            [
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/45')],
                ['Recent milestones'],
              ),
              Array.match(model.activity.milestones.slice(0, 5), {
                onEmpty: () =>
                  h.p(
                    [Ui.className<Message>('text-[0.75rem] text-white/35')],
                    ['No public milestones are available.'],
                  ),
                onNonEmpty: milestones =>
                  h.ol(
                    [Ui.className<Message>('grid')],
                    milestones.map(adjutantMilestoneRow),
                  ),
              }),
            ],
          ),
        ],
      ),
    ],
  )
}

const statsMetric = (label: string, value: string, detail: string): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid min-h-28 min-w-0 content-between gap-3 overflow-hidden border border-[#222] bg-[#010102] p-3',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('truncate text-[0.6875rem] text-white/45')],
        [label],
      ),
      h.div(
        [
          Ui.className<Message>(
            'min-w-0 break-words text-2xl font-semibold leading-tight tracking-normal text-[#f1efe8] tabular-nums sm:text-3xl',
          ),
        ],
        [value],
      ),
      h.div(
        [Ui.className<Message>('break-words text-[0.75rem] text-white/35')],
        [detail],
      ),
    ],
  )
}

const pylonRow = (pylon: PublicRecentPylon): Html => {
  const h = html<Message>()

  return h.li(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#1b1b1b] py-2 last:border-b-0 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto]',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('min-w-0')],
        [
          h.div(
            [Ui.className<Message>('truncate text-[0.8125rem] text-[#f1efe8]')],
            [pylon.nodeLabel ?? pylon.nostrPubkeyShort],
          ),
          h.div(
            [Ui.className<Message>('truncate text-[0.75rem] text-white/35')],
            [pylon.nostrPubkeyShort],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('min-w-0 text-[0.75rem] text-white/45')],
        [
          `${pylon.runtimeState ?? 'unknown'} / ${pylon.readyModel ?? 'unknown'}`,
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'tabular-nums text-[0.75rem] text-white/45 sm:text-right',
          ),
        ],
        [formatNumber(pylon.eligibleProductCount)],
      ),
    ],
  )
}

const pylonStatsView = (model: PublicPylonStatsModel): Html => {
  const h = html<Message>()
  const stats = pylonStatsFromModel(model)
  const error = pylonStatsError(model)
  const recentPylons = stats?.recentPylons.slice(0, 4) ?? []

  return h.section(
    [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-end justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div(
                [Ui.className<Message>(Ui.eyebrowClass)],
                ['Pylon network'],
              ),
              h.h2(
                [
                  Ui.className<Message>(
                    'text-xl font-semibold tracking-normal text-[#f1efe8]',
                  ),
                ],
                ['Omega Pylon stats'],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            [`Feed ${statsStatusText(stats)}`],
          ),
        ],
      ),
      // This panel lives in the narrow right rail of the /artanis 3-column
      // layout (≈18rem at xl), so a 5-up metric row would crush values like
      // "v0.2.5+" and "Ready" into per-character wraps. Cap the grid so each
      // metric card stays wide enough to render its value on one line.
      h.div(
        [Ui.className<Message>('grid grid-cols-2 gap-2 sm:grid-cols-3 xl:grid-cols-2')],
        [
          statsMetric(
            'Pylons online',
            stats === null ? '-' : formatNumber(stats.pylonsOnlineNow),
            'v0.2.5+ heartbeat window',
          ),
          statsMetric(
            'Wallet ready',
            stats === null ? '-' : formatNumber(stats.pylonsWalletReadyNow),
            'Public readiness',
          ),
          statsMetric(
            'Seen in 24h',
            stats === null ? '-' : formatNumber(stats.pylonsSeen24h),
            'Recent check-ins',
          ),
          statsMetric(
            'Earning gate',
            stats === null
              ? '-'
              : stats.earningLaunchGate.publicEarningCopyAllowed
                ? 'Ready'
                : 'Blocked',
            stats === null
              ? 'Stats loading'
              : stats.earningLaunchGate.publicEarningCopyAllowed
                ? 'Bounded copy'
                : 'Unsafe copy blocked',
          ),
          statsMetric(
            'Version floor',
            stats === null ? '-' : `v${stats.minimumClientVersion}+`,
            stats === null ? 'Stats loading' : (stats.asOfLabel ?? 'Fresh'),
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 border border-[#222] bg-[#010102] p-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.25fr)]',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2 text-[0.75rem] text-white/45')],
            [
              h.div(
                [Ui.className<Message>('text-[#f1efe8]')],
                [`Source ${trimProtocol(stats?.sourceUrl ?? null)}`],
              ),
              h.div(
                [],
                [`Relay ${trimProtocol(stats?.hostedNexusRelayUrl ?? null)}`],
              ),
              h.div(
                [],
                [
                  stats?.asOfLabel === null || stats?.asOfLabel === undefined
                    ? 'Timestamp unavailable'
                    : `As of ${friendlyRelativeTime(stats.asOfLabel)}`,
                ],
              ),
              h.div(
                [],
                [
                  `Training participants ${formatNumber(stats?.trainingAcceptedContributors ?? 0)} / assigned ${formatNumber(stats?.trainingAssignedContributors ?? 0)}`,
                ],
              ),
              error === null
                ? null
                : h.div([Ui.className<Message>('text-[#ff6f00]')], [error]),
            ],
          ),
          Array.match(recentPylons, {
            onEmpty: () =>
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/35')],
                [
                  stats === null
                    ? 'Loading recent Pylon presence.'
                    : 'No recent Pylon rows published.',
                ],
              ),
            onNonEmpty: rows =>
              h.ol([Ui.className<Message>('grid')], rows.map(pylonRow)),
          }),
        ],
      ),
    ],
  )
}

const fleetOnboardingCommands = [
  'npm install -g @openagentsinc/khala',
  'khala fleet connect',
  'khala fleet status',
] as const

const artanisFleetOnboardingView = (): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-4 border-b border-[#222] pb-6')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-end justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Join fleet']),
              h.h2(
                [
                  Ui.className<Message>(
                    'text-xl font-semibold tracking-normal text-[#f1efe8]',
                  ),
                ],
                ['Have Codex or Claude? Join the fleet.'],
              ),
            ],
          ),
          h.a(
            [
              h.Href('/docs/connect-codex-fleet'),
              Ui.className<Message>(
                'border border-[#333] px-3 py-2 text-[0.75rem] text-white/70 underline-offset-4 hover:border-white/35 hover:text-[#f1efe8] hover:underline',
              ),
            ],
            ['Fleet docs'],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-3 lg:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)]',
          ),
        ],
        [
          h.div(
            [
              Ui.className<Message>(
                'grid gap-3 border border-[#222] bg-[#010102] p-3',
              ),
            ],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'max-w-3xl text-sm leading-6 text-[#f1efe8]',
                  ),
                ],
                [
                  'Connect your own coding-agent capacity so a per-user Artanis can burn down public issue backlogs through your local Pylon. Credentials stay on your machine; public projections use generic fleet labels and refs only.',
                ],
              ),
              h.div(
                [
                  Ui.className<Message>(
                    'grid gap-2 text-[0.75rem] text-white/45 sm:grid-cols-3',
                  ),
                ],
                [
                  h.div(
                    [Ui.className<Message>('border border-[#1b1b1b] p-2')],
                    ['Paste-free device login'],
                  ),
                  h.div(
                    [Ui.className<Message>('border border-[#1b1b1b] p-2')],
                    ['Isolated Codex account homes'],
                  ),
                  h.div(
                    [Ui.className<Message>('border border-[#1b1b1b] p-2')],
                    ['More accounts, more throughput'],
                  ),
                ],
              ),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'grid gap-3 border border-[#222] bg-[#010102] p-3',
              ),
            ],
            [
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/45')],
                ['Start here'],
              ),
              h.ol(
                [Ui.className<Message>('grid gap-1')],
                fleetOnboardingCommands.map(command =>
                  h.li(
                    [
                      Ui.className<Message>(
                        'overflow-x-auto border border-[#1b1b1b] bg-black px-3 py-2 text-[0.75rem] leading-6 text-[#f1efe8]',
                      ),
                    ],
                    [h.code([], [command])],
                  ),
                ),
              ),
              h.div(
                [Ui.className<Message>('flex flex-wrap gap-2')],
                [
                  h.a(
                    [
                      h.Href('/docs/connect-codex-fleet'),
                      Ui.className<Message>(
                        'border border-[#333] px-3 py-2 text-[0.75rem] text-white/70 underline-offset-4 hover:border-white/35 hover:text-[#f1efe8] hover:underline',
                      ),
                    ],
                    ['Read the setup guide'],
                  ),
                ],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}

// Live fleet-shipping feed (#6534). Reads the read-only public activity
// timeline and renders TODAY's real fleet work in human language (no ref-IDs,
// no ledger jargon, no stale "updated N days ago" report). The stale demo
// `artanis_tick` rows and `projection_gap` markers are intentionally excluded;
// what remains is genuine live work: forum, inference, settlement,
// verification, training windows, and Pylon presence.
const FLEET_FEED_FRESH_WINDOW_MS = 24 * 60 * 60 * 1000

const fleetEventKindLabel: Readonly<
  Record<PublicActivityTimelineEvent['kind'], string>
> = {
  artanis_tick: 'Decision',
  assignment_ready: 'Assignment ready',
  capacity_snapshot: 'Capacity',
  forum_posted: 'Forum reply',
  forum_topic_created: 'Forum topic',
  khala_inference_served: 'Inference served',
  projection_gap: 'Projection gap',
  pylon_heartbeat: 'Pylon heartbeat',
  pylon_registered: 'Pylon joined',
  real_bitcoin_moved: 'Bitcoin moved',
  settlement_recorded: 'Settlement',
  trace_submitted: 'Trace submitted',
  verification_queued: 'Verification queued',
  verification_rejected: 'Verification rejected',
  verification_verified: 'Verified',
  wallet_ready: 'Wallet ready',
  window_closed: 'Window closed',
  window_opened: 'Window opened',
  work_claimed: 'Work claimed',
}

const fleetExcludedKinds = new Set<PublicActivityTimelineEvent['kind']>([
  'artanis_tick',
  'projection_gap',
])

const fleetFeedEvents = (
  events: ReadonlyArray<PublicActivityTimelineEvent>,
): ReadonlyArray<PublicActivityTimelineEvent> =>
  [...events]
    .filter(
      event => !fleetExcludedKinds.has(event.kind) && event.text.trim() !== '',
    )
    .sort((left, right) => right.ts.localeCompare(left.ts))

const fleetFeedIsFresh = (
  events: ReadonlyArray<PublicActivityTimelineEvent>,
  generatedAt: string,
): boolean => {
  const newest = events[0]
  if (newest === undefined) {
    return false
  }
  const newestMs = parseMs(newest.ts)
  const generatedMs = parseMs(generatedAt)
  if (newestMs === null || generatedMs === null) {
    return true
  }
  return generatedMs - newestMs <= FLEET_FEED_FRESH_WINDOW_MS
}

const fleetFreshCount = (
  events: ReadonlyArray<PublicActivityTimelineEvent>,
  generatedAt: string,
): number => {
  const generatedMs = parseMs(generatedAt)
  if (generatedMs === null) {
    return events.length
  }
  return events.filter(event => {
    const ms = parseMs(event.ts)
    return ms !== null && generatedMs - ms <= FLEET_FEED_FRESH_WINDOW_MS
  }).length
}

const fleetStatusBadge = (
  label: string,
  tone: 'live' | 'idle' | 'muted',
): Html => {
  const h = html<Message>()
  const cls =
    tone === 'live'
      ? 'inline-flex items-center gap-2 border border-[#00c853]/60 px-2.5 py-1 text-[0.75rem] text-[#9ad6b7]'
      : tone === 'idle'
        ? 'inline-flex items-center gap-2 border border-[#ff6f00]/60 px-2.5 py-1 text-[0.75rem] text-[#ffb26b]'
        : 'inline-flex items-center gap-2 border border-[#333] px-2.5 py-1 text-[0.75rem] text-white/55'

  return h.span(
    [Ui.className<Message>(cls)],
    [
      tone === 'live'
        ? h.span(
            [
              Ui.className<Message>(
                'h-2 w-2 rounded-full bg-[#00c853] motion-safe:animate-pulse',
              ),
            ],
            [],
          )
        : h.span([], ['▶']),
      h.span([], [label]),
    ],
  )
}

const fleetHeader = (right: Html): Html => {
  const h = html<Message>()
  return h.div(
    [Ui.className<Message>('flex flex-wrap items-end justify-between gap-3')],
    [
      h.div(
        [Ui.className<Message>('grid gap-2')],
        [
          h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Fleet shipping']),
          h.h2(
            [
              Ui.className<Message>(
                'text-lg font-semibold tracking-normal text-[#f1efe8]',
              ),
            ],
            ['What the fleet is doing now'],
          ),
        ],
      ),
      right,
    ],
  )
}

const fleetEventRow = (event: PublicActivityTimelineEvent): Html => {
  const h = html<Message>()
  const kindLabel = fleetEventKindLabel[event.kind] ?? event.kind
  const amount =
    typeof event.amountSats === 'number' && Number.isFinite(event.amountSats)
      ? `${formatNumber(event.amountSats)} sats`
      : null

  return h.li(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#1b1b1b] py-3 last:border-b-0',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('flex flex-wrap items-baseline gap-x-3 gap-y-1')],
        [
          h.span(
            [
              Ui.className<Message>(
                'inline-flex shrink-0 border border-[#2a2a2a] bg-[#0a0a0a] px-1.5 py-0.5 text-[0.625rem] uppercase tracking-wide text-white/55',
              ),
            ],
            [kindLabel],
          ),
          h.span(
            [
              Ui.className<Message>(
                'min-w-0 flex-1 text-[0.8125rem] leading-5 text-[#f1efe8]',
              ),
            ],
            [event.text],
          ),
          h.span(
            [
              Ui.className<Message>(
                'shrink-0 tabular-nums text-[0.6875rem] text-white/35',
              ),
            ],
            [friendlyRelativeTime(event.ts)],
          ),
        ],
      ),
      amount === null
        ? null
        : h.div(
            [Ui.className<Message>('text-[0.6875rem] text-white/45')],
            [
              event.realBitcoinMoved === true
                ? `${amount} / real Bitcoin`
                : `${amount} / simulated`,
            ],
          ),
    ],
  )
}

const fleetIdleState = (
  newest: PublicActivityTimelineEvent | undefined,
): Html => {
  const h = html<Message>()
  return h.div(
    [
      h.Role('status'),
      Ui.className<Message>(
        'flex items-center gap-3 border border-[#222] bg-[#010102] p-4 text-sm text-white/45',
      ),
    ],
    [
      h.span([], ['▶']),
      h.span(
        [],
        [
          newest === undefined
            ? 'The fleet is idle right now. New work will appear here live.'
            : `Quiet right now — last fleet activity ${friendlyRelativeTime(newest.ts)}. New work will appear here live.`,
        ],
      ),
    ],
  )
}

const fleetShippingLoadedView = (
  envelope: PublicActivityTimelineEnvelope,
): Html => {
  const h = html<Message>()
  const events = fleetFeedEvents(envelope.events)
  const fresh = fleetFeedIsFresh(events, envelope.generatedAt)
  const fresh24h = fleetFreshCount(events, envelope.generatedAt)
  const liveSources = envelope.sourceLag.filter(
    lag => lag.status === 'current',
  ).length
  const newest = events[0]

  return h.section(
    [Ui.className<Message>('grid gap-4 border-b border-[#222] pb-6')],
    [
      fleetHeader(
        h.div(
          [Ui.className<Message>('flex items-center gap-3')],
          [
            fresh
              ? fleetStatusBadge('Live', 'live')
              : fleetStatusBadge('Idle', 'idle'),
            h.a(
              [
                h.Href('/activity'),
                Ui.className<Message>(
                  'border border-[#333] px-3 py-2 text-[0.75rem] text-white/70 underline-offset-4 hover:border-white/35 hover:text-[#f1efe8] hover:underline',
                ),
              ],
              ['Full activity'],
            ),
          ],
        ),
      ),
      h.div(
        [Ui.className<Message>('grid gap-2 sm:grid-cols-3')],
        [
          statsMetric(
            'Shipped (24h)',
            formatNumber(fresh24h),
            'Live fleet events in the last day',
          ),
          statsMetric(
            'Latest',
            newest === undefined ? '-' : friendlyRelativeTime(newest.ts),
            newest === undefined
              ? 'No recent events'
              : (fleetEventKindLabel[newest.kind] ?? newest.kind),
          ),
          statsMetric(
            'Live feeds',
            `${formatNumber(liveSources)} / ${formatNumber(envelope.sourceLag.length)}`,
            'Source streams reporting current',
          ),
        ],
      ),
      // Honesty rule (#6534): never present old rows under a live frame. If the
      // freshest event is older than the live window, show an honest idle state
      // (with the real last-activity time in the metric above) instead of
      // listing stale rows.
      fresh
        ? Array.match(events.slice(0, 8), {
            onEmpty: () => fleetIdleState(newest),
            onNonEmpty: rows =>
              h.ol(
                [
                  Ui.className<Message>(
                    'grid border border-[#222] bg-[#010102] px-3',
                  ),
                ],
                rows.map(fleetEventRow),
              ),
          })
        : fleetIdleState(newest),
    ],
  )
}

const fleetShippingMessageView = (message: string): Html => {
  const h = html<Message>()
  return h.section(
    [Ui.className<Message>('grid gap-4 border-b border-[#222] pb-6')],
    [
      fleetHeader(fleetStatusBadge('Loading', 'muted')),
      h.div(
        [
          h.Role('status'),
          Ui.className<Message>(
            'border border-[#222] bg-[#010102] p-4 text-sm text-white/45',
          ),
        ],
        [message],
      ),
    ],
  )
}

// A failed timeline fetch is NOT a dead end on this recruitment surface. Show
// an honest reconnecting state (never the scary "Unavailable" wall): the feed
// is transiently unreachable and fresh fleet work returns on the next load.
const fleetShippingReconnectingView = (): Html => {
  const h = html<Message>()
  return h.section(
    [Ui.className<Message>('grid gap-4 border-b border-[#222] pb-6')],
    [
      fleetHeader(fleetStatusBadge('Reconnecting', 'muted')),
      h.div(
        [
          h.Role('status'),
          Ui.className<Message>(
            'flex items-center gap-3 border border-[#222] bg-[#010102] p-4 text-sm text-white/45',
          ),
        ],
        [
          h.span([], ['▶']),
          h.span(
            [],
            [
              'Reconnecting to the live fleet feed. Fresh fleet work will appear here as it ships.',
            ],
          ),
        ],
      ),
    ],
  )
}

const fleetShippingView = (model: PublicActivityTimelineModel): Html => {
  if (model._tag === 'PublicActivityTimelineLoaded') {
    return fleetShippingLoadedView(model.envelope)
  }
  if (model._tag === 'PublicActivityTimelineFailed') {
    return fleetShippingReconnectingView()
  }
  return fleetShippingMessageView('Loading live fleet activity.')
}

type VirtualMergeQueueLane = Readonly<{
  label: string
  value: string
  detail: string
  tone: 'ready' | 'active' | 'blocked' | 'muted'
}>

type VirtualMergeQueueStep = Readonly<{
  label: string
  detail: string
}>

const virtualMergeQueueLanes: ReadonlyArray<VirtualMergeQueueLane> = [
  {
    label: 'Actual head',
    value: 'origin/main',
    detail: 'Pinned commit from GitHub branch protection',
    tone: 'muted',
  },
  {
    label: 'Virtual head',
    value: 'projection',
    detail: 'Advances after each verified non-conflicting candidate',
    tone: 'active',
  },
  {
    label: 'Next branch base',
    value: 'virtual head',
    detail: 'New fleet work starts from the projected post-merge tree',
    tone: 'ready',
  },
  {
    label: 'Conflict lane',
    value: 'blocked',
    detail: 'Duplicate issue, stale base, closed issue, or path conflict',
    tone: 'blocked',
  },
] as const

const virtualMergeQueueSteps: ReadonlyArray<VirtualMergeQueueStep> = [
  {
    label: '1. Admit',
    detail: 'Issue is open, one PR per issue is preserved, and verification passed.',
  },
  {
    label: '2. Project',
    detail: 'Candidate patch becomes the next virtual head before another agent branches.',
  },
  {
    label: '3. Promote',
    detail: 'Only the front ready entry moves to the real protected branch.',
  },
]

const vmqToneClass: Record<VirtualMergeQueueLane['tone'], string> = {
  active: 'border-[#3a7bff]/45 bg-[#07101f] text-[#8fb6ff]',
  blocked: 'border-[#ff6f00]/45 bg-[#160b03] text-[#ffb26b]',
  muted: 'border-[#333] bg-[#0a0a0a] text-white/55',
  ready: 'border-[#00c853]/45 bg-[#06140a] text-[#9ad6b7]',
}

const virtualMergeQueueLaneView = (lane: VirtualMergeQueueLane): Html => {
  const h = html<Message>()

  return h.li(
    [
      Ui.className<Message>(
        `grid min-h-28 content-between gap-3 border p-3 ${vmqToneClass[lane.tone]}`,
      ),
    ],
    [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.span(
          [Ui.className<Message>('text-[0.6875rem] uppercase tracking-wide text-white/40')],
          [lane.label],
        ),
        h.span(
          [Ui.className<Message>('text-lg font-semibold leading-6 text-[#f1efe8]')],
          [lane.value],
        ),
      ]),
      h.p([Ui.className<Message>('m-0 text-[0.75rem] leading-5')], [
        lane.detail,
      ]),
    ],
  )
}

const virtualMergeQueueStepView = (step: VirtualMergeQueueStep): Html => {
  const h = html<Message>()

  return h.li(
    [Ui.className<Message>('grid gap-1 border border-[#1b1b1b] bg-black p-3')],
    [
      h.div(
        [Ui.className<Message>('text-[0.75rem] font-semibold leading-5 text-[#f1efe8]')],
        [step.label],
      ),
      h.p([Ui.className<Message>('m-0 text-[0.75rem] leading-5 text-white/45')], [
        step.detail,
      ]),
    ],
  )
}

const artanisVirtualMergeQueueView = (): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.DataAttribute('component', 'artanis-virtual-merge-queue'),
      Ui.className<Message>('grid gap-4 border-b border-[#222] pb-6'),
    ],
    [
      h.div(
        [Ui.className<Message>('flex flex-wrap items-end justify-between gap-3')],
        [
          h.div([Ui.className<Message>('grid gap-2')], [
            h.div([Ui.className<Message>(Ui.eyebrowClass)], [
              'Virtual merge queue',
            ]),
            h.h2(
              [
                Ui.className<Message>(
                  'm-0 text-lg font-semibold tracking-normal text-[#f1efe8]',
                ),
              ],
              ['Projected branch base for parallel agents'],
            ),
          ]),
          h.a(
            [
              h.Href(
                '/docs/artanis/2026-06-28-gitafter-cloudflare-artifacts-coordination-audit',
              ),
              Ui.className<Message>(
                'border border-[#333] px-3 py-2 text-[0.75rem] text-white/70 underline-offset-4 hover:border-white/35 hover:text-[#f1efe8] hover:underline',
              ),
            ],
            ['Coordination audit'],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-4 xl:grid-cols-[minmax(0,1.2fr)_minmax(18rem,0.8fr)]',
          ),
        ],
        [
          h.div([Ui.className<Message>('grid gap-3')], [
            h.ol(
              [
                h.AriaLabel('Virtual merge queue projection lanes'),
                Ui.className<Message>('grid gap-2 sm:grid-cols-2 xl:grid-cols-4'),
              ],
              virtualMergeQueueLanes.map(virtualMergeQueueLaneView),
            ),
            h.div(
              [
                Ui.className<Message>(
                  'grid gap-2 border border-[#222] bg-[#010102] p-3',
                ),
              ],
              [
                h.div(
                  [
                    Ui.className<Message>(
                      'flex flex-wrap items-center justify-between gap-3',
                    ),
                  ],
                  [
                    h.span(
                      [
                        Ui.className<Message>(
                          'text-[0.75rem] font-semibold text-[#f1efe8]',
                        ),
                      ],
                      ['Public proof fixture'],
                    ),
                    h.span(
                      [
                        Ui.className<Message>(
                          'tabular-nums text-[0.6875rem] text-[#8fb6ff]',
                        ),
                      ],
                      ['24 accepted / 0 conflicts'],
                    ),
                  ],
                ),
                h.div(
                  [
                    Ui.className<Message>(
                      'h-2 overflow-hidden border border-[#222] bg-black',
                    ),
                  ],
                  [
                    h.div(
                      [
                        Ui.className<Message>('h-full bg-[#3a7bff]'),
                        h.Style({ width: '100%' }),
                      ],
                      [],
                    ),
                  ],
                ),
                h.p(
                  [
                    Ui.className<Message>(
                      'm-0 text-[0.75rem] leading-5 text-white/45',
                    ),
                  ],
                  [
                    'The shipped simulator proves a 20+ item queue can advance one virtual head without opening duplicate work for the same issue.',
                  ],
                ),
              ],
            ),
          ]),
          h.div([Ui.className<Message>('grid content-start gap-3')], [
            h.ol(
              [Ui.className<Message>('grid gap-2')],
              virtualMergeQueueSteps.map(virtualMergeQueueStepView),
            ),
            h.div(
              [
                Ui.className<Message>(
                  'border border-[#222] bg-[#010102] p-3 text-[0.75rem] leading-5 text-white/45',
                ),
              ],
              [
                'Public-safe only: no raw patches, local workspace paths, provider payloads, or private prompts are exposed on this page.',
              ],
            ),
          ]),
        ],
      ),
    ],
  )
}

const eventRow = (event: PublicAgentGoalEvent): Html => {
  const h = html<Message>()
  const publicRefs = [
    publicRefsLabel('commits', event.commitRefs),
    publicRefsLabel('artifacts', event.artifactRefs),
    publicRefsLabel('receipts', event.receiptRefs),
  ].filter((value): value is string => value !== null)

  return h.li(
    [
      Ui.className<Message>(
        'grid gap-1 border-b border-[#1b1b1b] py-3 last:border-b-0',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('flex items-center justify-between gap-3')],
        [
          h.span(
            [Ui.className<Message>('text-[0.8125rem] text-[#f1efe8]')],
            [userFacingCopy(event.summary)],
          ),
          h.span(
            [Ui.className<Message>('text-[0.6875rem] text-white/35')],
            [event.status ?? event.type],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [
          event.runId === null
            ? friendlyRelativeTime(event.createdAt)
            : `${event.runId} / ${friendlyRelativeTime(event.createdAt)}`,
        ],
      ),
      Array.match(publicRefs, {
        onEmpty: () => null,
        onNonEmpty: refs =>
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/35')],
            [refs.join(' / ')],
          ),
      }),
    ],
  )
}

const publicAgentActivityView = (
  events: ReadonlyArray<PublicAgentGoalEvent>,
): Html => {
  const h = html<Message>()
  const now = currentUnixMs()
  // Freshness guard: never render an old wall of rows as if it were live. Keep
  // only rows inside the activity window; rows whose timestamp cannot be parsed
  // (e.g. the synthetic load-error row) are kept so genuine errors still
  // surface.
  const fresh = events.filter(event => {
    const ms = parseMs(event.createdAt)
    return ms === null || now - ms <= ACTIVITY_FRESH_WINDOW_MS
  })

  return h.section(
    [Ui.className<Message>('grid gap-3')],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Activity']),
      Array.match(fresh.slice(0, 20), {
        onEmpty: () =>
          h.div(
            [
              h.Role('status'),
              Ui.className<Message>(
                'flex items-center gap-3 border border-[#222] bg-[#010102] p-4 text-sm text-white/45',
              ),
            ],
            [h.span([], ['▶']), h.span([], ['No fresh activity right now.'])],
          ),
        onNonEmpty: rows =>
          h.ol([Ui.className<Message>('grid')], rows.map(eventRow)),
      }),
    ],
  )
}

const publicAgentGoalView = (
  agentName: string,
  goal: PublicAgentGoal | null,
  displayedObjective: string,
): Html => {
  const h = html<Message>()

  return h.section(
    [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
    [
      h.div(
        [Ui.className<Message>(Ui.eyebrowClass)],
        [goal === null ? 'Campaign objective' : 'Current goal'],
      ),
      h.div(
        [Ui.className<Message>('grid gap-3')],
        [
          h.p(
            [
              Ui.className<Message>(
                'max-w-4xl whitespace-pre-wrap text-base leading-7 text-[#f1efe8]',
              ),
            ],
            [displayedObjective],
          ),
          h.div(
            [
              Ui.className<Message>(
                'flex flex-wrap gap-x-6 gap-y-2 text-[0.75rem] text-white/45',
              ),
            ],
            goal === null
              ? [`Awaiting the first public durable ${agentName} goal.`]
              : [
                  h.span([], [usageText(goal)]),
                  h.span(
                    [],
                    [
                      goal.currentRunId === null
                        ? 'no active run'
                        : `current run ${goal.currentRunId}`,
                    ],
                  ),
                  h.span([], [`updated ${friendlyRelativeTime(goal.updatedAt)}`]),
                ],
          ),
        ],
      ),
    ],
  )
}

const artanisConsoleHeader = (
  goal: PublicAgentGoal | null,
  pylonStats: PublicPylonStatsModel,
): Html => {
  const h = html<Message>()
  const stats = pylonStatsFromModel(pylonStats)
  const progress = tokenProgressPercent(goal)

  return h.header(
    [
      Ui.className<Message>(
        'grid gap-4 border-b border-[#222] bg-[#010102] px-4 py-4 sm:px-5',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('flex flex-wrap items-center gap-3')],
            [
              h.div(
                [
                  Ui.className<Message>(
                    'text-lg font-semibold tracking-normal text-[#f1efe8] sm:text-xl',
                  ),
                ],
                ['ARTANIS console'],
              ),
              h.span(
                [
                  Ui.className<Message>(
                    'inline-flex items-center gap-2 border border-[#21462e] bg-[#06140a] px-2.5 py-1 text-[0.6875rem] font-semibold text-[#00c853]',
                  ),
                ],
                [
                  h.span(
                    [
                      Ui.className<Message>(
                        'h-2 w-2 animate-pulse rounded-full bg-[#00c853]',
                      ),
                    ],
                    [],
                  ),
                  'LIVE',
                ],
              ),
            ],
          ),
          h.a(
            [
              h.Href('/'),
              Ui.className<Message>(
                'text-[0.75rem] text-white/45 underline-offset-4 hover:text-[#f1efe8] hover:underline',
              ),
            ],
            ['Start your own agent'],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(13rem,0.32fr)]',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.h1(
                [
                  Ui.className<Message>(
                    'text-3xl font-semibold leading-none tracking-normal text-[#f1efe8] sm:text-4xl',
                  ),
                ],
                ['Artanis'],
              ),
              h.div(
                [
                  Ui.className<Message>(
                    'flex flex-wrap gap-x-5 gap-y-2 text-[0.75rem] text-white/55',
                  ),
                ],
                [
                  h.span([], [statusText(goal)]),
                  h.span([], [artanisFleetHealthText(stats)]),
                  h.span(
                    [],
                    [
                      goal?.currentRunId === null || goal === null
                        ? 'no active public run'
                        : `run ${goal.currentRunId}`,
                    ],
                  ),
                ],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('grid content-end gap-2')],
            [
              h.div(
                [
                  Ui.className<Message>(
                    'flex items-center justify-between gap-3 text-[0.6875rem] text-white/45',
                  ),
                ],
                [
                  h.span([], ['Daily token pace']),
                  h.span(
                    [Ui.className<Message>('tabular-nums text-white/55')],
                    [`${Math.round(progress)}%`],
                  ),
                ],
              ),
              h.div(
                [
                  Ui.className<Message>(
                    'h-2 overflow-hidden border border-[#222] bg-black',
                  ),
                ],
                [
                  h.div(
                    [
                      Ui.className<Message>('h-full bg-[#ffb400]'),
                      h.Style({ width: `${progress}%` }),
                    ],
                    [],
                  ),
                ],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}

const artanisLoadedView = (
  goal: PublicAgentGoal | null,
  pylonStats: PublicPylonStatsModel,
  activityTimeline: PublicActivityTimelineModel,
  khalaTokensServedHistory: PublicKhalaTokensServedHistoryModel,
): Html => {
  const h = html<Message>()
  // Freshness-gate the goal: a stale goal (e.g. updated 23 days ago) must not
  // drive the live console header, the current-goal panel, or the token-pace
  // meter, so we fall back to the campaign objective with no stale run id.
  const freshGoal = isGoalFresh(goal) ? goal : null
  const sanitizedObjective = sanitizeObjective(
    userFacingCopy(freshGoal?.objective ?? campaignObjective),
  )
  const displayedObjective =
    sanitizedObjective.length > 0 ? sanitizedObjective : campaignObjective

  return h.main(
    [
      h.DataAttribute('component', 'public-agent-page'),
      h.DataAttribute('agent', 'artanis'),
      Ui.className<Message>(
        'mx-auto grid min-h-screen max-w-[96rem] content-start gap-5 px-4 py-5 font-mono text-[#f1efe8] sm:px-6 lg:px-8',
      ),
    ],
    [
      artanisConsoleHeader(freshGoal, pylonStats),
      // HERO: the live token-burn Pulse is the strongest signal that an
      // autonomous fleet is building in real time, so it spans the console.
      artanisPulseView(khalaTokensServedHistory),
      artanisVirtualMergeQueueView(),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-5 xl:grid-cols-[minmax(17rem,0.75fr)_minmax(0,1.45fr)_minmax(18rem,0.8fr)]',
          ),
        ],
        [
          // The stale per-goal "ACTIVITY" wall is gone: the live fleet-shipping
          // feed is the single source of recent work. The left rail now shows
          // only the (freshness-gated) current goal.
          h.div(
            [Ui.className<Message>('grid content-start gap-5')],
            [publicAgentGoalView('Artanis', freshGoal, displayedObjective)],
          ),
          // Live fleet-shipping feed replaces the stale status report + the
          // 11-day-old admin-tick decision log.
          h.div(
            [Ui.className<Message>('grid content-start gap-5')],
            [fleetShippingView(activityTimeline)],
          ),
          h.div(
            [Ui.className<Message>('grid content-start gap-5')],
            [pylonStatsView(pylonStats), artanisFleetOnboardingView()],
          ),
        ],
      ),
    ],
  )
}

const loadedView = (
  agentRef: string,
  goal: PublicAgentGoal | null,
  events: ReadonlyArray<PublicAgentGoalEvent>,
  pylonStats: PublicPylonStatsModel,
  activityTimeline: PublicActivityTimelineModel,
  khalaTokensServedHistory: PublicKhalaTokensServedHistoryModel,
  adjutantActivity: PublicAdjutantActivityModel,
): Html => {
  const h = html<Message>()
  const agentName = displayName(agentRef)
  const displayedObjective = userFacingCopy(
    goal?.objective ?? fallbackObjective(agentRef),
  )
  const isAdjutant = agentRef === 'adjutant'

  if (agentRef === 'artanis') {
    return artanisLoadedView(
      goal,
      pylonStats,
      activityTimeline,
      khalaTokensServedHistory,
    )
  }

  return h.main(
    [
      h.DataAttribute('component', 'public-agent-page'),
      Ui.className<Message>(
        'mx-auto grid min-h-screen max-w-5xl content-start gap-8 px-6 py-10 font-mono text-[#f1efe8] sm:px-8',
      ),
    ],
    [
      h.header(
        [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
        [
          h.div(
            [
              Ui.className<Message>(
                'flex flex-wrap items-center justify-between gap-3',
              ),
            ],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public agent']),
              h.a(
                [
                  h.Href('/'),
                  Ui.className<Message>(
                    'text-[0.75rem] text-white/45 underline-offset-4 hover:text-[#f1efe8] hover:underline',
                  ),
                ],
                ['Start your own agent'],
              ),
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'flex flex-wrap items-end justify-between gap-4',
              ),
            ],
            [
              h.h1(
                [
                  Ui.className<Message>(
                    'text-3xl font-semibold leading-none tracking-normal sm:text-4xl',
                  ),
                ],
                [agentName],
              ),
              h.span(
                [
                  Ui.className<Message>(
                    'border border-[#333] px-2.5 py-1 text-[0.75rem] text-white/55',
                  ),
                ],
                [statusText(goal)],
              ),
            ],
          ),
        ],
      ),
      publicAgentGoalView(agentName, goal, displayedObjective),
      isAdjutant ? adjutantActivityView(adjutantActivity) : null,
      publicAgentActivityView(events),
    ],
  )
}

export const view = (model: Model, agentRef: string): Html => {
  const h = html<Message>()

  if (
    model.publicAgent._tag === 'PublicAgentLoaded' &&
    model.publicAgent.agentRef === agentRef
  ) {
    return loadedView(
      agentRef,
      model.publicAgent.response.goal,
      model.publicAgent.response.events,
      model.publicPylonStats,
      model.publicActivityTimeline,
      model.publicKhalaTokensServedHistory,
      model.publicAdjutantActivity,
    )
  }

  if (
    model.publicAgent._tag === 'PublicAgentFailed' &&
    model.publicAgent.agentRef === agentRef
  ) {
    return loadedView(
      agentRef,
      null,
      [
        {
          id: 'public-agent-load-error',
          goalId: 'unknown',
          runId: null,
          type: 'LoadFailed',
          status: 'failed',
          summary: model.publicAgent.error,
          tokenDelta: 0,
          timeDeltaSeconds: 0,
          artifactRefs: [],
          receiptRefs: [],
          commitRefs: [],
          createdAt: 'now',
        },
      ],
      model.publicPylonStats,
      model.publicActivityTimeline,
      model.publicKhalaTokensServedHistory,
      model.publicAdjutantActivity,
    )
  }

  if (agentRef === 'artanis') {
    return artanisLoadedView(
      null,
      model.publicPylonStats,
      model.publicActivityTimeline,
      model.publicKhalaTokensServedHistory,
    )
  }

  return h.main(
    [
      h.DataAttribute('component', 'public-agent-page'),
      Ui.className<Message>(
        'mx-auto grid min-h-screen max-w-5xl content-start gap-6 px-6 py-10 font-mono text-[#f1efe8] sm:px-8',
      ),
    ],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public agent']),
      h.h1(
        [Ui.className<Message>('text-3xl font-semibold')],
        [displayName(agentRef)],
      ),
      h.p(
        [Ui.className<Message>('text-sm text-white/45')],
        ['Loading public goal.'],
      ),
    ],
  )
}
