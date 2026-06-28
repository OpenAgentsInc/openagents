import { Array } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { userFacingCopy } from '../../../display-copy'
import { friendlyRelativeTime } from '../../../time-format'
import * as Ui from '../../../ui'
import type { Message } from '../message'
import type {
  Model,
  PublicAdjutantActivityMilestone,
  PublicAdjutantActivityModel,
  PublicAdjutantDeployedSite,
  PublicAgentGoal,
  PublicAgentGoalEvent,
  PublicArtanisForumRewardSmoke,
  PublicArtanisForumRewardVisibility,
  PublicArtanisReportActivityTickerEntry,
  PublicArtanisProductionLaunchGate,
  PublicArtanisReportDecisionFailureMode,
  PublicArtanisPylonLaunchCommunication,
  PublicArtanisReport,
  PublicArtanisReportClaimSummary,
  PublicArtanisReportModel,
  PublicKhalaTokensServedHistoryModel,
  PublicKhalaTokensServedHistoryPoint,
  PublicPylonStats,
  PublicPylonStatsModel,
  PublicPylonV02OmegaReleaseGate,
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

const formatNumber = (value: number): string => numberFormatter.format(value)

const formatCompactNumber = (value: number): string =>
  new Intl.NumberFormat('en-US', {
    maximumFractionDigits: value >= 10_000 ? 1 : 0,
    notation: 'compact',
  }).format(value)

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
        'grid min-h-28 content-between gap-3 border border-[#222] bg-[#010102] p-3',
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
            'tabular-nums text-3xl font-semibold tracking-normal text-[#f1efe8]',
          ),
        ],
        [value],
      ),
      h.div([Ui.className<Message>('text-[0.75rem] text-white/35')], [detail]),
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
      h.div(
        [Ui.className<Message>('grid gap-2 sm:grid-cols-2 lg:grid-cols-5')],
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

const compactRefs = (
  refs: ReadonlyArray<string>,
  fallback = 'No public refs',
): string => (refs.length === 0 ? fallback : refs.slice(0, 3).join(', '))

const tickerIssueLabel = (issueNumber: number | null): string =>
  issueNumber === null ? 'No linked issue' : `Issue #${issueNumber}`

const artanisTickerRow = (
  entry: PublicArtanisReportActivityTickerEntry,
): Html => {
  const h = html<Message>()

  return h.li(
    [
      Ui.className<Message>(
        'grid min-w-[16rem] gap-2 border border-[#222] bg-[#010102] p-3 sm:min-w-[20rem]',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex items-center justify-between gap-3 text-[0.6875rem] text-white/35',
          ),
        ],
        [
          h.span([Ui.className<Message>('uppercase')], [entry.state]),
          h.span([Ui.className<Message>('tabular-nums')], [
            entry.createdAtDisplay,
          ]),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.8125rem] text-[#f1efe8]')],
        [entry.label],
      ),
      h.div([Ui.className<Message>('text-[0.75rem] text-white/45')], [
        entry.assignmentRef ?? entry.activityRef,
      ]),
      h.div([Ui.className<Message>('text-[0.75rem] text-white/35')], [
        tickerIssueLabel(entry.issueNumber),
      ]),
    ],
  )
}

const artanisFailureModeRow = (
  mode: PublicArtanisReportDecisionFailureMode,
): Html => {
  const h = html<Message>()

  return h.li(
    [
      Ui.className<Message>(
        'grid gap-2 border-b border-[#1b1b1b] py-3 last:border-b-0',
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
            [mode.label],
          ),
          h.span(
            [Ui.className<Message>('tabular-nums text-[0.75rem] text-white/45')],
            [formatNumber(mode.count)],
          ),
        ],
      ),
      h.div([Ui.className<Message>('text-[0.75rem] text-white/35')], [
        `${tickerIssueLabel(mode.resultingPublicIssueNumber)} / ${
          mode.latestDecisionRef ?? mode.failureModeRef
        }`,
      ]),
    ],
  )
}

const artanisDecisionLogView = (
  report: PublicArtanisReport,
): Html | null => {
  const h = html<Message>()
  const decisionLog = report.decisionLog

  if (decisionLog === undefined) {
    return null
  }

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-3 border border-[#222] bg-[#010102] p-3',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-end justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-1')],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], ['The Log']),
              h.h3(
                [
                  Ui.className<Message>(
                    'text-lg font-semibold tracking-normal text-[#f1efe8]',
                  ),
                ],
                ['Live Artanis decisions'],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            [`Generated ${decisionLog.generatedAtDisplay}`],
          ),
        ],
      ),
      Array.match(decisionLog.ticker, {
        onEmpty: () =>
          h.p(
            [Ui.className<Message>('text-[0.75rem] text-white/35')],
            ['No public Artanis decisions are published yet.'],
          ),
        onNonEmpty: entries =>
          h.ol(
            [
              Ui.className<Message>(
                'flex gap-2 overflow-x-auto pb-1 [scrollbar-width:thin]',
              ),
            ],
            entries.map(artanisTickerRow),
          ),
      }),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-3 border-t border-[#222] pt-3 lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div([Ui.className<Message>(Ui.eyebrowClass)], ['The Brain']),
              h.div(
                [Ui.className<Message>('text-[0.8125rem] text-[#f1efe8]')],
                ['Autonomous triage summary'],
              ),
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/35')],
                [
                  `Dispatches ${formatNumber(decisionLog.countsByState.dispatched ?? 0)} / blocked ${formatNumber(decisionLog.countsByState.blocked ?? 0)} / failed ${formatNumber(decisionLog.countsByState.dispatch_failed ?? 0)}`,
                ],
              ),
            ],
          ),
          Array.match(decisionLog.failureModes, {
            onEmpty: () =>
              h.p(
                [Ui.className<Message>('text-[0.75rem] text-white/35')],
                ['No triaged failure modes in the public decision window.'],
              ),
            onNonEmpty: modes =>
              h.ol([Ui.className<Message>('grid')], modes.map(artanisFailureModeRow)),
          }),
        ],
      ),
      h.div([Ui.className<Message>('text-[0.75rem] text-white/35')], [
        decisionLog.authorityBoundary,
      ]),
    ],
  )
}

const bitcoinPrimary = (value: string): string => value.replace(/ \(.+\)$/, '')

const bitcoinDenomination = (value: string): string | null =>
  value.match(/\((.+)\)$/)?.[1] ?? null

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

const artanisClaimRow = (claim: PublicArtanisReportClaimSummary): Html => {
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
            [claim.label],
          ),
          h.span(
            [Ui.className<Message>('text-[0.6875rem] text-white/45')],
            [claim.stateLabel],
          ),
        ],
      ),
      h.p(
        [Ui.className<Message>('text-[0.75rem] text-white/45')],
        [claim.description],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [compactRefs(claim.blockedByRefs, compactRefs(claim.evidenceRefs))],
      ),
    ],
  )
}

const artanisForumRewardView = (
  visibility: PublicArtanisForumRewardVisibility,
): Html => {
  const h = html<Message>()
  const receiptLabel = `${formatNumber(visibility.contentRewardCount)} content ${
    visibility.contentRewardCount === 1 ? 'reward' : 'rewards'
  }`
  const bridgeLabel = `${formatNumber(visibility.acceptedContributionCount)} accepted ${
    visibility.acceptedContributionCount === 1 ? 'bridge' : 'bridges'
  }`

  return h.div(
    [Ui.className<Message>('grid gap-3 border border-[#222] bg-[#010102] p-3')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            ['Forum bitcoin'],
          ),
          h.span(
            [Ui.className<Message>('text-[0.75rem] text-white/55')],
            [visibility.stateLabel],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]',
          ),
        ],
        [
          statsMetric('Content rewards', receiptLabel, bridgeLabel),
          statsMetric(
            'Live spend',
            visibility.liveWalletSpendAllowed ? 'available' : 'blocked',
            visibility.liveWalletSpendAllowed
              ? 'Wallet authority present'
              : 'Needs wallet authority and spend cap',
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [
          compactRefs(
            visibility.caveatRefs,
            compactRefs(visibility.blockerRefs),
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [
          `Receipts ${compactRefs(visibility.forumReceiptRefs)} / actions ${compactRefs(visibility.paidActionRefs)}`,
        ],
      ),
    ],
  )
}

const artanisForumRewardSmokeView = (
  smoke: PublicArtanisForumRewardSmoke,
): Html => {
  const h = html<Message>()
  const exchangeLabel = `${formatNumber(smoke.exchangeCount)} ${
    smoke.exchangeCount === 1 ? 'exchange' : 'exchanges'
  }`

  return h.div(
    [Ui.className<Message>('grid gap-3 border border-[#222] bg-[#010102] p-3')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            ['Reward check'],
          ),
          h.span(
            [Ui.className<Message>('text-[0.75rem] text-white/55')],
            [smoke.modeLabel],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]',
          ),
        ],
        [
          statsMetric(
            'Registered agents',
            formatNumber(smoke.registeredAgentRefs.length),
            exchangeLabel,
          ),
          statsMetric(
            'Live bitcoin',
            smoke.usedLiveBitcoin ? 'recorded' : 'not used',
            smoke.usedLiveBitcoin
              ? compactRefs(smoke.walletAuthorityRefs)
              : 'Simulation only',
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [compactRefs(smoke.runReasonRefs, compactRefs(smoke.caveatRefs))],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [
          `Receipts ${compactRefs(smoke.receiptProjectionRefs)} / boundary ${compactRefs(smoke.acceptedContributionBoundaryRefs)}`,
        ],
      ),
    ],
  )
}

const artanisPylonLaunchView = (
  launch: PublicArtanisPylonLaunchCommunication,
): Html => {
  const h = html<Message>()
  const stageCount = launch.stageSummaryRefs.length
  const forumTopicPath = launch.primaryForumTopicUrl.replace(
    'https://openagents.com',
    '',
  )

  return h.div(
    [Ui.className<Message>('grid gap-3 border border-[#222] bg-[#010102] p-3')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            ['Pylon launch'],
          ),
          h.a(
            [
              h.Href(forumTopicPath),
              Ui.className<Message>(
                'text-[0.75rem] text-white/55 underline-offset-4 hover:text-[#f1efe8] hover:underline',
              ),
            ],
            ['Forum update'],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]',
          ),
        ],
        [
          statsMetric(
            'Launch brief',
            launch.forumIntentReady ? 'prepared' : 'blocked',
            launch.forumPostTitle,
          ),
          statsMetric(
            'Readiness',
            `${formatNumber(stageCount)} states`,
            compactRefs(launch.stageSummaryRefs),
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [
          compactRefs(
            launch.resourceModeCaveatRefs,
            compactRefs(launch.authorityBoundaryRefs),
          ),
        ],
      ),
    ],
  )
}

const artanisProductionLaunchGateView = (
  gate: PublicArtanisProductionLaunchGate,
): Html => {
  const h = html<Message>()
  const blockerCount = gate.failedOrPendingRequiredCount
  const firstBlockers = gate.blockerRefs.slice(0, 3)

  return h.div(
    [Ui.className<Message>('grid gap-3 border border-[#222] bg-[#010102] p-3')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            ['Production gate'],
          ),
          h.span(
            [Ui.className<Message>('text-[0.75rem] text-white/55')],
            [gate.stateLabel],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]',
          ),
        ],
        [
          statsMetric(
            'Autonomy claim',
            gate.canClaimContinuouslyRunning ? 'allowed' : 'blocked',
            gate.canClaimContinuouslyRunning
              ? 'All required gates passed'
              : `${formatNumber(blockerCount)} required ${
                  blockerCount === 1 ? 'gate' : 'gates'
                } not passed`,
          ),
          statsMetric(
            'Verification',
            `${formatNumber(gate.verificationTargetRefs.length)} targets`,
            compactRefs(gate.verificationTargetRefs),
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [
          firstBlockers.length === 0
            ? compactRefs(gate.docsRefs)
            : compactRefs(firstBlockers, compactRefs(gate.docsRefs)),
        ],
      ),
    ],
  )
}

const artanisOmegaReleaseGateView = (
  gate: PublicPylonV02OmegaReleaseGate,
): Html => {
  const h = html<Message>()
  const blockerCount = gate.failedOrPendingRequiredCount
  const pylonProofLabel = `${formatNumber(gate.multiPylonObservedDistinctPylonCount)} / ${formatNumber(gate.multiPylonRequiredDistinctPylonCount)} distinct Pylons`
  const firstBlockers = gate.blockerRefs.slice(0, 3)

  return h.div(
    [Ui.className<Message>('grid gap-3 border border-[#222] bg-[#010102] p-3')],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            ['Omega release gate'],
          ),
          h.span(
            [Ui.className<Message>('text-[0.75rem] text-white/55')],
            [gate.stateLabel],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]',
          ),
        ],
        [
          statsMetric(
            'Multi-Pylon proof',
            gate.multiPylonPaidWorkProofComplete ? 'complete' : 'blocked',
            pylonProofLabel,
          ),
          statsMetric(
            'Release claim',
            gate.canAnnouncePylonV02Release ? 'allowed' : 'blocked',
            gate.canAnnouncePylonV02Release
              ? 'All required public proof is complete'
              : `${formatNumber(blockerCount)} required ${
                  blockerCount === 1 ? 'item' : 'items'
                } not passed`,
          ),
          statsMetric(
            'Payment mode',
            gate.payoutModeGate.livePayoutClaimAllowed ? 'declared' : 'blocked',
            gate.hostedMdkDirectPayoutClaimAllowed
              ? 'Hosted MDK direct payout'
              : gate.payoutModeGate.modeLabel,
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('text-[0.75rem] text-white/35')],
        [
          firstBlockers.length === 0
            ? compactRefs(gate.runbookRefs)
            : compactRefs(firstBlockers, compactRefs(gate.runbookRefs)),
        ],
      ),
    ],
  )
}

const artanisReportLoadedView = (report: PublicArtanisReport): Html => {
  const h = html<Message>()
  const blockers = report.publicBlockerRefs.slice(0, 5)
  const claims = [...report.standaloneClaims, ...report.r10Claims].slice(0, 7)
  const healthAttentionCount = report.healthSummary.staleOrBlockedSignalCount
  const acceptedWorkDenomination = bitcoinDenomination(
    report.pylonSummary.acceptedWorkBitcoinTotal,
  )
  const acceptedWorkSettlementDetail = report.pylonSummary
    .acceptedWorkSettlementGate.publicPaidWorkTotalsAllowed
    ? `Receipts ${compactRefs(report.pylonSummary.acceptedWorkSettlementReceiptRefs)}`
    : report.pylonSummary.acceptedWorkSettlementGate.stateLabel

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
              h.div(
                [Ui.className<Message>(Ui.eyebrowClass)],
                ['Public report'],
              ),
              h.h2(
                [
                  Ui.className<Message>(
                    'text-xl font-semibold tracking-normal text-[#f1efe8]',
                  ),
                ],
                ['Artanis status report'],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            [`Updated ${report.updatedAtDisplay}`],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-2 sm:grid-cols-2 lg:grid-cols-5')],
        [
          statsMetric(
            'Autonomous loop',
            report.autonomousLoop.state.replace(/_/g, ' '),
            report.autonomousLoop.latestTickState === null
              ? `${formatNumber(report.autonomousLoop.tickCount)} ticks`
              : `${report.autonomousLoop.latestTickState.replace(/_/g, ' ')} tick`,
          ),
          statsMetric(
            'Health',
            report.healthSummary.overallState.replace(/_/g, ' '),
            report.healthSummary.overclaimBlocked
              ? `${formatNumber(healthAttentionCount)} ${
                  healthAttentionCount === 1 ? 'signal' : 'signals'
                } ${healthAttentionCount === 1 ? 'needs' : 'need'} attention`
              : 'No stale signals',
          ),
          statsMetric(
            'Model Lab',
            report.modelLabSummary.readiness.replace(/_/g, ' '),
            `${formatNumber(report.modelLabSummary.completeSectionCount)} / ${formatNumber(report.modelLabSummary.sectionCount)} sections complete`,
          ),
          statsMetric(
            'Pylon feed',
            report.pylonSummary.feedStatus,
            `${formatNumber(report.pylonSummary.pylonsOnlineNow)} online / ${formatNumber(report.pylonSummary.assignmentReadyPylonsOnlineNow)} assignment-ready / ${
              report.pylonSummary.earningLaunchGate.publicEarningCopyAllowed
                ? 'earning ready'
                : 'earning blocked'
            }`,
          ),
          statsMetric(
            'Accepted-work bitcoin',
            bitcoinPrimary(report.pylonSummary.acceptedWorkBitcoinTotal),
            acceptedWorkDenomination === null
              ? acceptedWorkSettlementDetail
              : `${acceptedWorkDenomination} total / ${acceptedWorkSettlementDetail}`,
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
                'grid gap-3 border border-[#222] bg-[#010102] p-3',
              ),
            ],
            [
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/45')],
                ['Public blockers'],
              ),
              blockers.length === 0
                ? h.p(
                    [Ui.className<Message>('text-[0.75rem] text-white/35')],
                    ['No public blockers are listed.'],
                  )
                : h.ul(
                    [Ui.className<Message>('grid gap-1')],
                    blockers.map(blocker =>
                      h.li(
                        [
                          Ui.className<Message>(
                            'break-words text-[0.75rem] text-white/55',
                          ),
                        ],
                        [blocker],
                      ),
                    ),
                  ),
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/35')],
                [
                  `Receipts ${compactRefs(report.receiptRefs)} / artifacts ${compactRefs(report.artifactRefs)}`,
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
                ['Forum refs'],
              ),
              h.div(
                [Ui.className<Message>('flex flex-wrap gap-2')],
                report.forumLinks.map(link =>
                  h.a(
                    [
                      h.Href(link.href),
                      Ui.className<Message>(
                        'border border-[#333] px-3 py-2 text-[0.75rem] text-white/70 underline-offset-4 hover:border-white/35 hover:text-[#f1efe8] hover:underline',
                      ),
                    ],
                    [link.label],
                  ),
                ),
              ),
              h.div(
                [Ui.className<Message>('text-[0.75rem] text-white/35')],
                [compactRefs(report.publicGoalRefs)],
              ),
            ],
          ),
        ],
      ),
      artanisDecisionLogView(report),
      artanisPylonLaunchView(report.pylonLaunchCommunication),
      artanisOmegaReleaseGateView(report.pylonOmegaReleaseGate),
      artanisProductionLaunchGateView(report.productionLaunchGate),
      artanisForumRewardView(report.forumRewardVisibility),
      artanisForumRewardSmokeView(report.forumRewardSmoke),
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 border border-[#222] bg-[#010102] p-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            ['Claim states'],
          ),
          h.ol([Ui.className<Message>('grid')], claims.map(artanisClaimRow)),
        ],
      ),
    ],
  )
}

const artanisReportView = (model: PublicArtanisReportModel): Html => {
  const h = html<Message>()

  if (model._tag === 'PublicArtanisReportLoaded') {
    return artanisReportLoadedView(model.report)
  }

  if (model._tag === 'PublicArtanisReportFailed') {
    return h.section(
      [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
      [
        h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public report']),
        h.p([Ui.className<Message>('text-sm text-[#ff6f00]')], [model.error]),
      ],
    )
  }

  return h.section(
    [Ui.className<Message>('grid gap-3 border-b border-[#222] pb-6')],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Public report']),
      h.p(
        [Ui.className<Message>('text-sm text-white/45')],
        ['Loading Artanis public report.'],
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

const loadedView = (
  agentRef: string,
  goal: PublicAgentGoal | null,
  events: ReadonlyArray<PublicAgentGoalEvent>,
  pylonStats: PublicPylonStatsModel,
  artanisReport: PublicArtanisReportModel,
  khalaTokensServedHistory: PublicKhalaTokensServedHistoryModel,
  adjutantActivity: PublicAdjutantActivityModel,
): Html => {
  const h = html<Message>()
  const agentName = displayName(agentRef)
  const displayedObjective = userFacingCopy(
    goal?.objective ?? fallbackObjective(agentRef),
  )
  const isArtanis = agentRef === 'artanis'
  const isAdjutant = agentRef === 'adjutant'
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
      h.section(
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
                      h.span(
                        [],
                        [`updated ${friendlyRelativeTime(goal.updatedAt)}`],
                      ),
                    ],
              ),
            ],
          ),
        ],
      ),
      isArtanis ? artanisPulseView(khalaTokensServedHistory) : null,
      isArtanis ? artanisReportView(artanisReport) : null,
      isArtanis ? artanisFleetOnboardingView() : null,
      isArtanis ? pylonStatsView(pylonStats) : null,
      isAdjutant ? adjutantActivityView(adjutantActivity) : null,
      h.section(
        [Ui.className<Message>('grid gap-3')],
        [
          h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Activity']),
          Array.match(events, {
            onEmpty: () =>
              h.p(
                [Ui.className<Message>('text-sm text-white/45')],
                ['No public activity has been published yet.'],
              ),
            onNonEmpty: events =>
              h.ol([Ui.className<Message>('grid')], events.map(eventRow)),
          }),
        ],
      ),
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
      model.publicArtanisReport,
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
      model.publicArtanisReport,
      model.publicKhalaTokensServedHistory,
      model.publicAdjutantActivity,
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
