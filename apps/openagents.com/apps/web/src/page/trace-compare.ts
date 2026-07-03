import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'
import {
  formatCost,
  formatDuration,
  shortId,
  type VerdictTone,
} from './trace/atif'
import { toneBorderClass, toneDotClass, toneTextClass } from './trace'
import {
  type CompareDelta,
  type CompareTrace,
  type Comparison,
  type MeasuredNumber,
  buildComparison,
  isMeasured,
  parseCompareIds,
} from './trace-compare/model'
import {
  SAMPLE_COMPARE_PATH_IDS,
  lookupTrajectoryForCompare,
} from './trace-compare/sample'

// Public, shareable comparison of N ATIF traces at `/trace/compare/{ids}`
// (issue #6211 — the real "chill-evals": "see how agents perform with these
// MCP/config changes"). It holds a scenario fixed and shows N traces side by
// side: per-trace verdict, latency (duration), step count, and cost, plus the
// behavior deltas of each variant relative to the BASELINE (the first id). Every
// variant deep-links its full `/trace/{uuid}` render. No auth to view.
//
// DESIGN.md + the sibling `/trace` page: dark/pure-black, warm off-white
// (#f1efe8), Commit Mono, command/table surfaces (NOT cards). The verdict tone
// helpers are reused verbatim from `page/trace.ts` so the two surfaces agree on
// semantic state color. The comparison is a real spec-comparison TABLE (metrics
// as rows, variants as columns), the densest honest read for "what changed".
//
// HONEST: nothing is fabricated. Metrics are read straight off each trajectory's
// real `final_metrics`; an unmeasured metric renders as the `not_measured`
// marker, never a fake 0. An unknown uuid renders an explicit "unknown id"
// column rather than inventing a trace.

// `h-dvh overflow-auto` (NOT `min-h-dvh`): the global reset pins
// `html, body, #root` to `height: 100%; overflow: hidden` (see styles.css), so
// a `min-h-dvh` shell would grow past the clipped body and the page could never
// scroll — leaving the header stuck at the top with the content below the fold
// unreachable. A fixed `h-dvh` shell is the real scroll container, so the
// (non-sticky) header scrolls off normally. Matches the sibling `/trace` page.
const pageShellClass = 'h-dvh overflow-auto bg-[#000] text-[#f1efe8]'

const mono = "font-['Commit_Mono',_'Berkeley_Mono',_ui-monospace,_monospace]"

const articleClass = 'mx-auto w-full max-w-5xl px-6 py-10 sm:px-8 sm:py-14'

// ---------------------------------------------------------------------------
// Small shared atoms (consistent vocabulary with `/trace`).
// ---------------------------------------------------------------------------

const sectionHeading = <Message>(text: string): Html => {
  const h = html<Message>()
  return h.h2(
    [
      Ui.className<Message>(
        'text-[0.7rem] font-semibold uppercase leading-none tracking-[0.18em] text-white/45',
      ),
    ],
    [text],
  )
}

const verdictBadge = <Message>(
  label: string,
  tone: VerdictTone,
): Html => {
  const h = html<Message>()
  return h.div(
    [
      Ui.className<Message>(
        `inline-flex items-center gap-2 self-start border px-2.5 py-1 ${toneBorderClass(tone)}`,
      ),
    ],
    [
      h.span(
        [
          Ui.className<Message>(
            `size-2 shrink-0 rounded-full ${toneDotClass(tone)}`,
          ),
          h.AriaHidden(true),
        ],
        [],
      ),
      h.span(
        [
          Ui.className<Message>(
            `text-[0.7rem] font-semibold uppercase leading-none tracking-wide ${toneTextClass(tone)}`,
          ),
        ],
        [label],
      ),
    ],
  )
}

// ---------------------------------------------------------------------------
// Delta formatting (honest, signed, with semantic tone).
// ---------------------------------------------------------------------------

const NOT_MEASURED = 'not_measured'

// A signed delta value + the tone it should render in. The caller decides which
// direction is "good" for the metric (faster/fewer/cheaper = positive tone).
type DeltaRender = { text: string; tone: VerdictTone }

const neutralDelta: DeltaRender = { text: '·', tone: 'neutral' }

const signed = (value: number, render: (abs: number) => string): string => {
  if (value === 0) return `±${render(0)}`
  return value > 0 ? `+${render(value)}` : `−${render(Math.abs(value))}`
}

// Lower-is-better metric delta (duration, steps, cost): negative = faster /
// fewer / cheaper, rendered positive (green); positive = slower (warning).
const lowerIsBetterDelta = (
  value: MeasuredNumber,
  render: (abs: number) => string,
): DeltaRender => {
  if (!isMeasured(value)) return { text: NOT_MEASURED, tone: 'neutral' }
  if (value === 0) return neutralDelta
  return {
    text: signed(value, render),
    tone: value < 0 ? 'positive' : 'warning',
  }
}

const passDeltaRender = (value: MeasuredNumber): DeltaRender => {
  if (!isMeasured(value)) return { text: NOT_MEASURED, tone: 'neutral' }
  if (value === 0) return neutralDelta
  // +1 = this passed where baseline did not (good); -1 = regression (bad).
  return value > 0
    ? { text: '+pass', tone: 'positive' }
    : { text: '−pass', tone: 'negative' }
}

const wholeNumber = (n: number): string => String(Math.round(n))

const deltaLine = <Message>(render: DeltaRender): Html => {
  const h = html<Message>()
  return h.span(
    [
      Ui.className<Message>(
        `mt-1 block text-[0.7rem] leading-none ${toneTextClass(render.tone)} ${mono}`,
      ),
      h.DataAttribute('component', 'trace-compare-delta'),
    ],
    [render.text],
  )
}

// ---------------------------------------------------------------------------
// The comparison table. Metrics are ROWS; variants are COLUMNS. The first data
// column is the row label; each variant column shows the metric value and, for
// non-baseline variants, the delta vs baseline beneath it.
// ---------------------------------------------------------------------------

const measuredCell = (value: MeasuredNumber, render: (n: number) => string) =>
  isMeasured(value) ? render(value) : NOT_MEASURED

const columnHeader = <Message>(
  trace: CompareTrace,
  index: number,
): Html => {
  const h = html<Message>()
  const label = trace.isBaseline ? 'Baseline' : `Variant ${index}`

  if (!trace.found) {
    return h.th(
      [
        h.Scope('col'),
        Ui.className<Message>(
          'border-b border-[#222] px-4 py-3 text-left align-bottom',
        ),
      ],
      [
        h.div(
          [Ui.className<Message>('grid gap-2')],
          [
            h.span(
              [
                Ui.className<Message>(
                  `text-[0.625rem] font-semibold uppercase leading-none tracking-wide text-white/30 ${mono}`,
                ),
              ],
              [label],
            ),
            verdictBadge<Message>('Unknown id', 'neutral'),
            h.span(
              [
                Ui.className<Message>(
                  `text-xs leading-none text-white/40 ${mono}`,
                ),
              ],
              [shortId(trace.uuid)],
            ),
          ],
        ),
      ],
    )
  }

  return h.th(
    [
      h.Scope('col'),
      Ui.className<Message>(
        'border-b border-[#222] px-4 py-3 text-left align-bottom',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('grid gap-2')],
        [
          h.div(
            [Ui.className<Message>('flex items-center gap-2')],
            [
              h.span(
                [
                  Ui.className<Message>(
                    `text-[0.625rem] font-semibold uppercase leading-none tracking-wide ${trace.isBaseline ? 'text-[#ffb400]' : 'text-white/30'} ${mono}`,
                  ),
                ],
                [label],
              ),
            ],
          ),
          verdictBadge<Message>(trace.verdictLabel, trace.verdictTone),
          h.span(
            [Ui.className<Message>(`text-sm leading-none text-[#f1efe8] ${mono}`)],
            [trace.target ?? trace.agentName],
          ),
          // Deep link to the full single-trace render. The single best primitive
          // of the comparison: each column opens its own `/trace/{uuid}`.
          h.a(
            [
              h.Href(`/trace/${trace.uuid}`),
              Ui.className<Message>(
                `inline-flex w-fit items-center gap-1 text-xs leading-none text-white/45 underline decoration-white/20 underline-offset-4 transition hover:text-[#f1efe8] hover:decoration-[#ffb400] focus-visible:outline focus-visible:outline-1 focus-visible:outline-[#ffb400] ${mono}`,
              ),
              h.DataAttribute('component', 'trace-compare-deeplink'),
            ],
            [`trace · ${shortId(trace.uuid)} →`],
          ),
        ],
      ),
    ],
  )
}

// A metric row: a row label + one cell per variant. `cell` renders the value
// for a found trace; non-baseline found traces also get the delta beneath.
const metricRow = <Message>(
  options: Readonly<{
    label: string
    traces: ReadonlyArray<CompareTrace>
    deltaByUuid: ReadonlyMap<string, CompareDelta>
    value: (t: Extract<CompareTrace, { found: true }>) => string
    valueTone?: (t: Extract<CompareTrace, { found: true }>) => VerdictTone
    delta: (d: CompareDelta) => DeltaRender
  }>,
): Html => {
  const h = html<Message>()

  const cells = options.traces.map(t => {
    if (!t.found) {
      return h.td(
        [
          Ui.className<Message>(
            `border-b border-[#161616] px-4 py-3 align-top text-sm text-white/30 ${mono}`,
          ),
        ],
        ['—'],
      )
    }
    const tone = options.valueTone?.(t)
    const valueClass = tone === undefined ? 'text-[#f1efe8]' : toneTextClass(tone)
    const children: Html[] = [
      h.span(
        [Ui.className<Message>(`text-sm leading-none ${valueClass} ${mono}`)],
        [options.value(t)],
      ),
    ]
    if (!t.isBaseline) {
      const d = options.deltaByUuid.get(t.uuid)
      if (d !== undefined) children.push(deltaLine<Message>(options.delta(d)))
    }
    return h.td(
      [
        Ui.className<Message>('border-b border-[#161616] px-4 py-3 align-top'),
      ],
      [h.div([Ui.className<Message>('grid')], children)],
    )
  })

  return h.tr(
    [],
    [
      h.th(
        [
          h.Scope('row'),
          Ui.className<Message>(
            'border-b border-[#161616] px-4 py-3 text-left align-top text-[0.625rem] font-semibold uppercase leading-none tracking-wide text-white/40',
          ),
        ],
        [options.label],
      ),
      ...cells,
    ],
  )
}

const videoRow = <Message>(
  traces: ReadonlyArray<CompareTrace>,
): Html => {
  const h = html<Message>()
  const cells = traces.map(t => {
    if (!t.found || t.videoSrc === undefined) {
      return h.td(
        [
          Ui.className<Message>(
            `border-b border-[#161616] px-4 py-3 align-top text-sm text-white/30 ${mono}`,
          ),
        ],
        ['—'],
      )
    }
    return h.td(
      [
        Ui.className<Message>('border-b border-[#161616] px-4 py-3 align-top'),
      ],
      [
        h.a(
          [
            h.Href(t.videoSrc),
            Ui.className<Message>(
              `inline-flex w-fit text-sm leading-none text-[#f1efe8] underline decoration-white/20 underline-offset-4 transition hover:decoration-[#ffb400] focus-visible:outline focus-visible:outline-1 focus-visible:outline-[#ffb400] ${mono}`,
            ),
            h.DataAttribute('component', 'trace-compare-video'),
          ],
          ['Watch video'],
        ),
      ],
    )
  })

  return h.tr(
    [],
    [
      h.th(
        [
          h.Scope('row'),
          Ui.className<Message>(
            'border-b border-[#161616] px-4 py-3 text-left align-top text-[0.625rem] font-semibold uppercase leading-none tracking-wide text-white/40',
          ),
        ],
        ['Video'],
      ),
      ...cells,
    ],
  )
}

const comparisonTable = <Message>(comparison: Comparison): Html => {
  const h = html<Message>()
  const deltaByUuid = new Map(comparison.deltas.map(d => [d.uuid, d]))

  const rows: Html[] = [
    metricRow<Message>({
      label: 'Verdict',
      traces: comparison.traces,
      deltaByUuid,
      value: t => t.verdictLabel,
      valueTone: t => t.verdictTone,
      delta: d => passDeltaRender(d.passDelta),
    }),
    metricRow<Message>({
      label: 'Latency',
      traces: comparison.traces,
      deltaByUuid,
      value: t => measuredCell(t.durationMs, formatDuration),
      delta: d => lowerIsBetterDelta(d.durationDeltaMs, formatDuration),
    }),
    metricRow<Message>({
      label: 'Steps',
      traces: comparison.traces,
      deltaByUuid,
      value: t => String(t.stepCount),
      delta: d => lowerIsBetterDelta(d.stepCountDelta, wholeNumber),
    }),
    metricRow<Message>({
      label: 'Cost',
      traces: comparison.traces,
      deltaByUuid,
      value: t => measuredCell(t.costUsd, formatCost),
      delta: d => lowerIsBetterDelta(d.costDeltaUsd, formatCost),
    }),
    videoRow<Message>(comparison.traces),
    metricRow<Message>({
      label: 'Model',
      traces: comparison.traces,
      deltaByUuid,
      value: t => t.model,
      // Model is descriptive, not a delta metric.
      delta: () => neutralDelta,
    }),
  ]

  return h.div(
    [Ui.className<Message>('mt-6 overflow-x-auto')],
    [
      h.table(
        [
          Ui.className<Message>(
            'w-full min-w-[36rem] border-collapse border border-[#222] text-left',
          ),
          h.DataAttribute('component', 'trace-compare-table'),
        ],
        [
          h.thead(
            [],
            [
              h.tr(
                [],
                [
                  // Empty top-left corner cell (the row-label column header).
                  h.th(
                    [
                      h.Scope('col'),
                      Ui.className<Message>(
                        'border-b border-[#222] px-4 py-3 text-left align-bottom text-[0.625rem] font-semibold uppercase leading-none tracking-wide text-white/30',
                      ),
                    ],
                    ['Metric'],
                  ),
                  ...comparison.traces.map((t, i) =>
                    columnHeader<Message>(t, i),
                  ),
                ],
              ),
            ],
          ),
          h.tbody([], rows),
        ],
      ),
    ],
  )
}

// ---------------------------------------------------------------------------
// Header + legend.
// ---------------------------------------------------------------------------

const header = <Message>(comparison: Comparison): Html => {
  const h = html<Message>()
  const count = comparison.traces.length
  const subtitleBits = [
    `${comparison.foundCount} trace${comparison.foundCount === 1 ? '' : 's'}`,
    ...(comparison.unknownCount > 0
      ? [`${comparison.unknownCount} unknown`]
      : []),
    'deltas vs baseline',
  ]

  return h.header(
    [Ui.className<Message>('grid gap-5 border-b border-[#222] pb-8')],
    [
      h.span(
        [
          Ui.className<Message>(
            `text-[0.625rem] uppercase leading-none tracking-[0.2em] text-white/35 ${mono}`,
          ),
        ],
        [`trace compare · ${count} variant${count === 1 ? '' : 's'}`],
      ),
      h.h1(
        [
          Ui.className<Message>(
            'm-0 text-2xl font-semibold leading-tight tracking-tight text-[#f1efe8] sm:text-3xl',
          ),
          h.Attribute('style', 'text-wrap: balance'),
        ],
        ['Compare agent traces'],
      ),
      h.p(
        [Ui.className<Message>('m-0 max-w-prose text-sm leading-6 text-white/55')],
        [
          'One scenario, several agent configurations, side by side. See how the ' +
            'agent performs across MCP and config changes — verdict, latency, ' +
            'steps, and cost, with every variant measured against the baseline.',
        ],
      ),
      h.span(
        [Ui.className<Message>(`text-xs leading-none text-white/35 ${mono}`)],
        [subtitleBits.join(' · ')],
      ),
    ],
  )
}

const legend = <Message>(): Html => {
  const h = html<Message>()
  const item = (dotTone: VerdictTone, text: string): Html =>
    h.div(
      [Ui.className<Message>('flex items-center gap-2')],
      [
        h.span(
          [
            Ui.className<Message>(
              `size-2 shrink-0 rounded-full ${toneDotClass(dotTone)}`,
            ),
            h.AriaHidden(true),
          ],
          [],
        ),
        h.span(
          [Ui.className<Message>(`text-xs leading-none text-white/45 ${mono}`)],
          [text],
        ),
      ],
    )
  return h.div(
    [Ui.className<Message>('mt-6 flex flex-wrap gap-x-6 gap-y-2')],
    [
      item('positive', 'better than baseline (faster / fewer / cheaper / passes)'),
      item('warning', 'worse than baseline'),
      item('neutral', 'same, descriptive, or not_measured'),
    ],
  )
}

// ---------------------------------------------------------------------------
// Bodies.
// ---------------------------------------------------------------------------

const compareArticle = <Message>(comparison: Comparison): Html => {
  const h = html<Message>()
  return h.article(
    [
      Ui.className<Message>(articleClass),
      h.DataAttribute('component', 'trace-compare-page'),
    ],
    [
      header<Message>(comparison),
      h.section(
        [Ui.className<Message>('mt-10')],
        [sectionHeading<Message>('Comparison'), comparisonTable<Message>(comparison)],
      ),
      legend<Message>(),
    ],
  )
}

// Shown when the `ids` path resolved to zero usable uuids (e.g. an empty or
// malformed list). Honest, not a fabricated comparison.
const emptyArticle = <Message>(): Html => {
  const h = html<Message>()
  return h.article(
    [
      Ui.className<Message>(
        `${articleClass} flex min-h-[60dvh] flex-col items-center justify-center text-center`,
      ),
      h.DataAttribute('component', 'trace-compare-empty'),
    ],
    [
      h.span(
        [
          Ui.className<Message>(
            `text-[0.625rem] uppercase leading-none tracking-[0.2em] text-white/30 ${mono}`,
          ),
        ],
        ['trace compare · no traces'],
      ),
      h.h1(
        [
          Ui.className<Message>(
            'mt-4 text-2xl font-semibold tracking-tight text-[#f1efe8] sm:text-3xl',
          ),
        ],
        ['Nothing to compare'],
      ),
      h.p(
        [
          Ui.className<Message>(
            `mt-3 max-w-md text-sm leading-6 text-white/50 ${mono}`,
          ),
        ],
        ['This comparison link has no trace ids.'],
      ),
      h.p(
        [Ui.className<Message>('mt-2 max-w-md text-sm leading-6 text-white/45')],
        [
          'A comparison URL lists trace uuids, e.g. /trace/compare/a,b,c — the ' +
            'first id is the baseline the others are measured against.',
        ],
      ),
      h.a(
        [
          h.Href(`/trace/compare/${SAMPLE_COMPARE_PATH_IDS}`),
          Ui.className<Message>(
            'mt-8 inline-flex items-center border border-[#222] px-4 py-2 text-sm text-[#f1efe8] transition hover:border-[#333] hover:bg-[#080808] focus-visible:outline focus-visible:outline-1 focus-visible:outline-[#ffb400]',
          ),
        ],
        ['See the sample comparison'],
      ),
    ],
  )
}

export type TraceCompareRouteLike = Readonly<{
  _tag: 'TraceCompare'
  ids: string
}>

// The page view. Public, no auth. Renders the committed sample trajectories for
// the known uuids; unknown ids resolve to an explicit "unknown id" column rather
// than a fabricated trace. When the worker read API lands, the lookup passed to
// `buildComparison` becomes an async fetch + decode; this render is unchanged.
export const view = <Message>(
  route: TraceCompareRouteLike,
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()
  const uuids = parseCompareIds(route.ids)
  const body =
    uuids.length === 0
      ? emptyArticle<Message>()
      : compareArticle<Message>(
          buildComparison(uuids, lookupTrajectoryForCompare),
        )

  return h.div(
    [Ui.className<Message>(pageShellClass)],
    [PublicHeader.view(authState), body],
  )
}

// The document title. Keep it shareable-friendly + honest about the count.
export const title = (route: TraceCompareRouteLike): string => {
  const uuids = parseCompareIds(route.ids)
  if (uuids.length === 0) return 'Trace comparison - OpenAgents'
  const comparison = buildComparison(uuids, lookupTrajectoryForCompare)
  return `Compare ${comparison.foundCount} agent trace${
    comparison.foundCount === 1 ? '' : 's'
  } - OpenAgents`
}
