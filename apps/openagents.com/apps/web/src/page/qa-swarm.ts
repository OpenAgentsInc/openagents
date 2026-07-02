import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'
import {
  type QaSwarmCoverageFrontierItem,
  type QaSwarmPerfBudgetItem,
  type QaSwarmRunProjection,
  type QaSwarmVerdict,
  type QaSwarmVerdictItem,
  lookupQaSwarmRunProjection,
} from './qa-swarm/projection'

export type QaSwarmRouteLike = Readonly<{
  _tag: 'QaSwarm'
  runRef: string
}>

const pageShellClass = 'h-dvh overflow-auto bg-[#000] text-[#f1efe8]'
const mono = "font-['Commit_Mono',_'Berkeley_Mono',_ui-monospace,_monospace]"
const shell = 'mx-auto grid w-full max-w-6xl gap-6 px-5 py-8 sm:px-8 sm:py-12'
const panel = 'border border-[#222] bg-[#010102] p-4 sm:p-5'
const sectionLabel =
  'm-0 text-[0.68rem] font-semibold uppercase leading-none tracking-[0.14em] text-white/42'
const body = 'm-0 text-sm leading-6 text-white/62'

const verdictTextClass = (verdict: QaSwarmVerdict): string => {
  switch (verdict) {
    case 'passed':
      return 'text-[#00c853]'
    case 'failed':
      return 'text-[#d32f2f]'
    case 'warning':
      return 'text-[#ff6f00]'
    case 'inconclusive':
      return 'text-white/58'
  }
}

const verdictBorderClass = (verdict: QaSwarmVerdict): string => {
  switch (verdict) {
    case 'passed':
      return 'border-[#00c853]/45'
    case 'failed':
      return 'border-[#d32f2f]/45'
    case 'warning':
      return 'border-[#ff6f00]/45'
    case 'inconclusive':
      return 'border-white/15'
  }
}

const verdictDotClass = (verdict: QaSwarmVerdict): string => {
  switch (verdict) {
    case 'passed':
      return 'bg-[#00c853]'
    case 'failed':
      return 'bg-[#d32f2f]'
    case 'warning':
      return 'bg-[#ff6f00]'
    case 'inconclusive':
      return 'bg-white/42'
  }
}

const label = <Message>(text: string): Html => {
  const h = html<Message>()
  return h.span([Ui.className<Message>(sectionLabel)], [text])
}

const code = <Message>(text: string): Html => {
  const h = html<Message>()
  return h.code(
    [
      Ui.className<Message>(
        `break-all border border-white/10 bg-white/[0.04] px-1.5 py-0.5 text-[0.82em] text-white/78 ${mono}`,
      ),
    ],
    [text],
  )
}

const verdictBadge = <Message>(verdict: QaSwarmVerdict): Html => {
  const h = html<Message>()
  return h.span(
    [
      Ui.className<Message>(
        `inline-flex w-fit items-center gap-2 border px-2.5 py-1 ${verdictBorderClass(verdict)}`,
      ),
    ],
    [
      h.span(
        [
          Ui.className<Message>(
            `size-2 shrink-0 rounded-full ${verdictDotClass(verdict)}`,
          ),
          h.AriaHidden(true),
        ],
        [],
      ),
      h.span(
        [
          Ui.className<Message>(
            `text-[0.7rem] font-semibold uppercase leading-none tracking-wide ${verdictTextClass(verdict)} ${mono}`,
          ),
        ],
        [verdict],
      ),
    ],
  )
}

const metric = <Message>(
  name: string,
  value: string,
  detail: string,
): Html => {
  const h = html<Message>()
  return h.div(
    [
      Ui.className<Message>(
        'grid min-h-24 gap-2 border border-[#222] bg-[#080808] p-3',
      ),
    ],
    [
      h.span([Ui.className<Message>(sectionLabel)], [name]),
      h.span(
        [
          Ui.className<Message>(
            `text-2xl font-semibold leading-none text-[#f1efe8] ${mono}`,
          ),
        ],
        [value],
      ),
      h.span([Ui.className<Message>('text-xs leading-4 text-white/45')], [
        detail,
      ]),
    ],
  )
}

const verdictRow = <Message>(item: QaSwarmVerdictItem): Html => {
  const h = html<Message>()
  return h.li(
    [
      Ui.className<Message>(
        'grid gap-3 border border-[#222] bg-[#060606] p-3 sm:grid-cols-[10rem_1fr_auto] sm:items-start',
      ),
    ],
    [
      h.div([Ui.className<Message>('grid gap-2')], [
        verdictBadge<Message>(item.verdict),
        h.span([Ui.className<Message>(`text-xs text-white/40 ${mono}`)], [
          item.receiptRef,
        ]),
      ]),
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h3(
          [Ui.className<Message>('m-0 text-sm font-semibold text-[#f1efe8]')],
          [item.label],
        ),
        h.p([Ui.className<Message>(body)], [item.summary]),
      ]),
      h.a(
        [
          h.Href(`#${item.receiptRef}`),
          Ui.className<Message>(
            `text-xs leading-none text-white/45 underline decoration-white/20 underline-offset-4 hover:text-[#f1efe8] hover:decoration-[#2979ff] focus-visible:outline focus-visible:outline-1 focus-visible:outline-[#2979ff] ${mono}`,
          ),
        ],
        ['receipt'],
      ),
    ],
  )
}

const coverageRow = <Message>(item: QaSwarmCoverageFrontierItem): Html => {
  const h = html<Message>()
  const pct = Math.round((item.current / item.frontier) * 100)
  return h.li(
    [Ui.className<Message>('grid gap-2 border border-[#222] bg-[#060606] p-3')],
    [
      h.div([Ui.className<Message>('flex items-center justify-between gap-3')], [
        h.span([Ui.className<Message>('text-sm font-semibold text-[#f1efe8]')], [
          item.label,
        ]),
        h.span([Ui.className<Message>(`text-xs text-white/50 ${mono}`)], [
          `${item.current}/${item.frontier}`,
        ]),
      ]),
      h.div(
        [
          Ui.className<Message>(
            'h-2 overflow-hidden border border-white/10 bg-black',
          ),
          h.Role('meter'),
          h.AriaLabel(item.label),
          h.Attribute('aria-valuemin', '0'),
          h.Attribute('aria-valuemax', String(item.frontier)),
          h.Attribute('aria-valuenow', String(item.current)),
        ],
        [
          h.div(
            [
              Ui.className<Message>('h-full bg-[#2979ff]'),
              h.Style({ width: `${Math.min(100, pct)}%` }),
            ],
            [],
          ),
        ],
      ),
      h.span([Ui.className<Message>(`text-xs text-white/38 ${mono}`)], [
        item.receiptRef,
      ]),
    ],
  )
}

const perfRow = <Message>(item: QaSwarmPerfBudgetItem): Html => {
  const h = html<Message>()
  return h.li(
    [
      Ui.className<Message>(
        'grid gap-3 border border-[#222] bg-[#060606] p-3 sm:grid-cols-[1fr_auto_auto] sm:items-center',
      ),
    ],
    [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.span([Ui.className<Message>('text-sm font-semibold text-[#f1efe8]')], [
          item.label,
        ]),
        h.span([Ui.className<Message>(`text-xs text-white/38 ${mono}`)], [
          item.receiptRef,
        ]),
      ]),
      h.span([Ui.className<Message>(`text-sm text-white/70 ${mono}`)], [
        `${item.actualMs}ms / ${item.budgetMs}ms`,
      ]),
      verdictBadge<Message>(item.verdict),
    ],
  )
}

const runArticle = <Message>(projection: QaSwarmRunProjection): Html => {
  const h = html<Message>()

  return h.main(
    [
      Ui.className<Message>(shell),
      h.AriaLabel('QA Swarm run projection'),
      h.DataAttribute('component', 'qa-swarm-run-page'),
    ],
    [
      h.section([Ui.className<Message>('grid gap-5 border-b border-[#222] pb-6')], [
        h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-4')], [
          h.div([Ui.className<Message>('grid max-w-3xl gap-3')], [
            h.p([Ui.className<Message>(sectionLabel)], ['QA Swarm run']),
            h.h1(
              [
                Ui.className<Message>(
                  'm-0 text-3xl font-semibold leading-tight text-[#f1efe8] sm:text-4xl',
                ),
              ],
              [projection.title],
            ),
            h.p([Ui.className<Message>(body)], [
              'Public-safe projection from the nightly status artifact, trace receipts, coverage frontier, perf budgets, videos, and distilled regression tests.',
            ]),
          ]),
          verdictBadge<Message>(projection.verdict),
        ]),
        h.div(
          [
            Ui.className<Message>(
              'grid gap-2 border border-[#222] bg-[#010102] p-3 text-sm sm:grid-cols-[1fr_1fr]',
            ),
          ],
          [
            h.div([Ui.className<Message>('grid gap-1')], [
              label<Message>('Run ref'),
              code<Message>(projection.runRef),
            ]),
            h.div([Ui.className<Message>('grid gap-1')], [
              label<Message>('Target'),
              h.span([Ui.className<Message>('text-[#f1efe8]')], [
                projection.target.label,
                projection.target.visibility === 'opaque'
                  ? ' · opaque ref'
                  : ' · public ref',
              ]),
              code<Message>(projection.target.ref),
            ]),
          ],
        ),
      ]),

      h.section(
        [Ui.className<Message>('grid grid-cols-2 gap-3 lg:grid-cols-4')],
        [
          metric<Message>(
            'Verdicts',
            String(projection.verdictWall.length),
            'Receipt-backed checks',
          ),
          metric<Message>(
            'Coverage',
            String(projection.coverageFrontier.length),
            'Current vs frontier rows',
          ),
          metric<Message>(
            'Budgets',
            String(projection.perfBudgets.length),
            'Named perf thresholds',
          ),
          metric<Message>(
            'Videos',
            String(projection.videoRefs.length),
            'Public trace media refs',
          ),
        ],
      ),

      h.section([Ui.className<Message>(panel)], [
        h.div([Ui.className<Message>('grid gap-4')], [
          label<Message>('Verdict wall'),
          h.ul(
            [Ui.className<Message>('grid gap-3 p-0')],
            projection.verdictWall.map(verdictRow),
          ),
        ]),
      ]),

      h.section([Ui.className<Message>('grid gap-4 lg:grid-cols-2')], [
        h.div([Ui.className<Message>(panel)], [
          h.div([Ui.className<Message>('grid gap-4')], [
            label<Message>('Coverage + frontier'),
            h.ul(
              [Ui.className<Message>('grid gap-3 p-0')],
              projection.coverageFrontier.map(coverageRow),
            ),
          ]),
        ]),
        h.div([Ui.className<Message>(panel)], [
          h.div([Ui.className<Message>('grid gap-4')], [
            label<Message>('Perf budgets'),
            h.ul(
              [Ui.className<Message>('grid gap-3 p-0')],
              projection.perfBudgets.map(perfRow),
            ),
          ]),
        ]),
      ]),

      h.section([Ui.className<Message>(panel)], [
        h.div([Ui.className<Message>('grid gap-4')], [
          label<Message>('Videos and traces'),
          h.div(
            [Ui.className<Message>('grid gap-3 sm:grid-cols-2')],
            projection.videoRefs.map(video =>
              h.a(
                [
                  h.Href(video.traceHref),
                  Ui.className<Message>(
                    'grid min-h-36 gap-3 border border-[#222] bg-[#060606] p-3 text-left transition hover:border-[#2979ff]/70 focus-visible:outline focus-visible:outline-1 focus-visible:outline-[#2979ff]',
                  ),
                ],
                [
                  h.span(
                    [Ui.className<Message>('text-sm font-semibold text-[#f1efe8]')],
                    [video.label],
                  ),
                  h.span([Ui.className<Message>(body)], [
                    'Open the trace page for the dereferenceable QA run video receipt.',
                  ]),
                  code<Message>(video.videoRef),
                ],
              ),
            ),
          ),
        ]),
      ]),

      h.section([Ui.className<Message>(panel)], [
        h.div([Ui.className<Message>('grid gap-4')], [
          label<Message>('Distilled tests'),
          h.div(
            [Ui.className<Message>('flex flex-wrap gap-2')],
            projection.distilledTests.map(test =>
              h.a(
                [
                  h.Href(test.href),
                  Ui.className<Message>(
                    `border border-[#222] bg-[#060606] px-3 py-2 text-sm text-white/68 underline decoration-white/15 underline-offset-4 hover:text-[#f1efe8] hover:decoration-[#2979ff] focus-visible:outline focus-visible:outline-1 focus-visible:outline-[#2979ff] ${mono}`,
                  ),
                ],
                [test.label],
              ),
            ),
          ),
        ]),
      ]),

      h.section([Ui.className<Message>('grid gap-2 border-t border-[#222] pt-4')], [
        h.p([Ui.className<Message>(sectionLabel)], ['Projection boundary']),
        h.p([Ui.className<Message>(body)], [
          'Schema ',
          projection.schemaVersion,
          '. Generated ',
          projection.generatedAt,
          '. Artifact snapshot max age ',
          String(projection.staleness.maxAgeHours),
          'h. Public safety refs: ',
          projection.publicSafetyRefs.join(', '),
          '.',
        ]),
      ]),
    ],
  )
}

const notFoundArticle = <Message>(runRef: string): Html => {
  const h = html<Message>()
  return h.main(
    [
      Ui.className<Message>(shell),
      h.DataAttribute('component', 'qa-swarm-not-found'),
    ],
    [
      h.section([Ui.className<Message>(panel)], [
        h.div([Ui.className<Message>('grid gap-4')], [
          label<Message>('QA Swarm run'),
          h.h1(
            [Ui.className<Message>('m-0 text-3xl font-semibold text-[#f1efe8]')],
            ['Run unavailable'],
          ),
          h.p([Ui.className<Message>(body)], [
            'No public-safe projection is published for ',
            runRef,
            '. Private or owner-only targets are not disclosed by this route.',
          ]),
        ]),
      ]),
    ],
  )
}

export const view = <Message>(
  route: QaSwarmRouteLike,
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()
  const projection = lookupQaSwarmRunProjection(route.runRef)

  return h.div(
    [Ui.className<Message>(pageShellClass), h.DataAttribute('route', 'qa-swarm')],
    [
      PublicHeader.view(authState),
      projection === null
        ? notFoundArticle<Message>(route.runRef)
        : runArticle<Message>(projection),
    ],
  )
}

export const title = (route: QaSwarmRouteLike): string => {
  const projection = lookupQaSwarmRunProjection(route.runRef)
  return projection === null
    ? 'QA Swarm run - OpenAgents'
    : `${projection.title} - OpenAgents`
}
