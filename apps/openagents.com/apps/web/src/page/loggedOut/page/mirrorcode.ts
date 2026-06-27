import { Array as Arr } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import type { Message } from '../message'
import type { MirrorCodeRunsModel } from '../model'
import {
  type MirrorCodeComparator,
  type MirrorCodeRun,
  type MirrorCodeRunStatus,
  formatMirrorCodePassRate,
  formatMirrorCodeTokens,
  mirrorCodeGradeLabel,
  mirrorCodeStatusLabel,
} from '../mirrorcode/runs'

type DivAttr = Parameters<ReturnType<typeof html<Message>>['div']>[0][number]

// ---------------------------------------------------------------------------
// Shared class constants (mirror the `/gym` page vocabulary): pure-black panels,
// mono uppercase section titles, thin borders, strips/tables over cards. No
// gradients, no marketing chrome (DESIGN.md).
// ---------------------------------------------------------------------------
const pageClass =
  'mx-auto grid min-w-0 w-full max-w-7xl gap-6 px-4 py-8 font-mono text-[#f1efe8] sm:px-6 lg:px-8'
const panelClass = 'grid min-w-0 gap-4 border border-white/10 bg-[#050505] p-4'
const sectionTitleClass =
  'm-0 font-mono text-[0.78rem] font-semibold uppercase tracking-wide text-white/55'
const cellHeadClass =
  'border-b border-white/10 px-3 py-2 text-left text-[0.7rem] font-semibold uppercase tracking-wide text-white/40'
const cellClass =
  'border-b border-white/5 px-3 py-2 align-top text-[0.8125rem] text-white/80'

// Semantic status accents (DESIGN.md): passed=positive, failed/error=negative,
// running=info, queued=muted.
const statusColor = (status: MirrorCodeRunStatus): string =>
  status === 'passed'
    ? '#00c853'
    : status === 'failed' || status === 'error'
      ? '#d32f2f'
      : status === 'running'
        ? '#2979ff'
        : 'rgba(255,255,255,0.45)'

const statusBadge = (status: MirrorCodeRunStatus): Html => {
  const h = html<Message>()
  const color = statusColor(status)

  return h.span(
    [
      h.DataAttribute('mirrorcode-status', status),
      Ui.className<Message>(
        'inline-flex items-center gap-1.5 text-[0.78rem] font-semibold uppercase tracking-wide',
      ),
      h.Style({ color }),
    ],
    [
      h.span(
        [
          Ui.className<Message>('inline-block size-2 rounded-full'),
          h.Style({ background: color }),
        ],
        [],
      ),
      h.span([], [mirrorCodeStatusLabel(status)]),
    ],
  )
}

// A small "Phase-0 smoke" / "decision-grade" marker. A smoke run is NEVER
// rendered as a published frontier measurement (#6378).
const gradeBadge = (run: MirrorCodeRun): Html => {
  const h = html<Message>()
  const isSmoke = run.grade === 'smoke'

  return h.span(
    [
      h.DataAttribute('mirrorcode-grade', run.grade),
      Ui.className<Message>(
        isSmoke
          ? 'border border-white/15 bg-white/[0.03] px-2 py-0.5 text-[0.68rem] font-semibold uppercase tracking-wide text-white/55'
          : 'border border-[#00c853]/30 bg-[#04130a] px-2 py-0.5 text-[0.68rem] font-semibold uppercase tracking-wide text-[#7be39f]',
      ),
    ],
    [mirrorCodeGradeLabel(run.grade)],
  )
}

const stat = (label: string, value: string): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('grid gap-1 border border-white/10 bg-black p-3')],
    [
      h.span([Ui.className<Message>('text-[0.72rem] text-white/45')], [label]),
      h.span(
        [Ui.className<Message>('text-[1rem] font-semibold text-white')],
        [value],
      ),
    ],
  )
}

const emptyState = (
  attrs: ReadonlyArray<DivAttr>,
  eyebrow: string,
  heading: string,
  body: string,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...attrs,
      Ui.className<Message>(
        'grid place-items-start gap-2 border border-dashed border-white/15 bg-black p-6',
      ),
    ],
    [
      h.p([Ui.className<Message>(sectionTitleClass)], [eyebrow]),
      h.p(
        [Ui.className<Message>('m-0 text-base font-semibold text-white/80 sm:text-lg')],
        [heading],
      ),
      h.p(
        [Ui.className<Message>('m-0 max-w-[78ch] text-base text-white/55 sm:text-sm')],
        [body],
      ),
    ],
  )
}

// Newest-run-first ordering for both the live panel and the leaderboard.
const byStartedAtDesc = (
  runs: ReadonlyArray<MirrorCodeRun>,
): ReadonlyArray<MirrorCodeRun> =>
  [...runs].sort((a, b) =>
    a.startedAt < b.startedAt ? 1 : a.startedAt > b.startedAt ? -1 : 0,
  )

// ---------------------------------------------------------------------------
// Hero
// ---------------------------------------------------------------------------
const hero = (): Html => {
  const h = html<Message>()

  return h.header([Ui.className<Message>('grid gap-3')], [
    h.div(
      [
        h.DataAttribute('mirrorcode-no-spend-banner', ''),
        Ui.className<Message>(
          'w-fit border border-[#7fb0ff]/30 bg-[#07111f] px-3 py-1 text-[0.75rem] font-semibold uppercase tracking-wide text-[#b8d4ff]',
        ),
      ],
      ['Live data only / public tasks only'],
    ),
    h.h1(
      [Ui.className<Message>('m-0 text-3xl font-semibold tracking-tight text-balance sm:text-5xl')],
      ['MirrorCode, powered by Khala'],
    ),
    h.p(
      [Ui.className<Message>('m-0 max-w-3xl text-base text-white/65')],
      [
        'Khala (openagents/khala) reimplements real tools from scratch inside a sandbox, then a held-out test suite scores the result. The benchmark is the Epoch Research MirrorCode set, run here on PUBLIC tasks only — the private set is excluded — so every number below is reproducible and never reads the held-out answers.',
      ],
    ),
  ])
}

// ---------------------------------------------------------------------------
// Live run panel: the single latest Khala run, or an honest empty state.
// ---------------------------------------------------------------------------
const livePanelEmpty = (): Html =>
  emptyState(
    [
      html<Message>().DataAttribute('mirrorcode-live-empty', ''),
      html<Message>().AriaLabel('Latest MirrorCode run'),
    ],
    'Latest run',
    'No runs yet — machinery shipped, awaiting first Phase-0 run',
    'The MirrorCode harness, scorer, and projection are live. As soon as the first Khala run lands, its status, pass-rate, token total, and summary appear here. Nothing on this page is fabricated.',
  )

const livePanelLoading = (): Html =>
  emptyState(
    [html<Message>().DataAttribute('mirrorcode-live-loading', '')],
    'Latest run',
    'Loading the latest run…',
    'Reading the public MirrorCode runs projection.',
  )

const livePanelError = (error: string): Html =>
  emptyState(
    [html<Message>().DataAttribute('mirrorcode-live-error', '')],
    'Latest run',
    'Run feed unavailable',
    `The public runs projection could not be read right now (${error}). No numbers are shown rather than inventing any.`,
  )

const latestRunCard = (run: MirrorCodeRun): Html => {
  const h = html<Message>()

  return h.article(
    [
      h.DataAttribute('mirrorcode-latest-run', run.runId),
      h.AriaLabel(`Latest MirrorCode run: ${run.taskId}`),
      Ui.className<Message>('grid gap-4 border border-white/10 bg-black p-4'),
    ],
    [
      h.div(
        [Ui.className<Message>('flex flex-wrap items-center justify-between gap-3')],
        [
          h.div([Ui.className<Message>('grid gap-0.5')], [
            h.p(
              [Ui.className<Message>('m-0 text-base font-semibold text-white')],
              [run.taskId],
            ),
            h.p(
              [Ui.className<Message>('m-0 text-[0.78rem] text-white/50')],
              [`${run.model} · bucket ${run.bucket}${run.language ? ` · ${run.language}` : ''}`],
            ),
          ]),
          h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
            statusBadge(run.status),
            gradeBadge(run),
          ]),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-3 sm:grid-cols-2 lg:grid-cols-4')],
        [
          stat('Pass rate', formatMirrorCodePassRate(run.passRate)),
          stat('Tokens total', formatMirrorCodeTokens(run.tokensTotal)),
          stat('Bucket', run.bucket),
          stat('Run id', run.runId),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-3 sm:grid-cols-2')],
        [
          stat('Started', run.startedAt),
          stat('Finished', run.finishedAt ?? 'in progress'),
        ],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-[0.85rem] text-white/70')],
        [run.summary],
      ),
    ],
  )
}

const livePanel = (model: MirrorCodeRunsModel): Html => {
  const h = html<Message>()

  const body: Html =
    model._tag === 'MirrorCodeRunsLoaded'
      ? Arr.match(byStartedAtDesc(model.response.runs), {
          onEmpty: () => livePanelEmpty(),
          onNonEmpty: runs => latestRunCard(runs[0]),
        })
      : model._tag === 'MirrorCodeRunsFailed'
        ? livePanelError(model.error)
        : model._tag === 'MirrorCodeRunsLoading'
          ? livePanelLoading()
          : livePanelEmpty()

  return h.section(
    [
      h.DataAttribute('mirrorcode-live-panel', ''),
      Ui.className<Message>(panelClass),
    ],
    [
      h.div([Ui.className<Message>('grid gap-2')], [
        h.p([Ui.className<Message>(sectionTitleClass)], ['Live run']),
        h.h2(
          [Ui.className<Message>('m-0 max-w-[24ch] text-2xl font-semibold tracking-tight text-balance text-white sm:text-3xl')],
          ['The latest Khala run'],
        ),
        h.p([Ui.className<Message>('m-0 max-w-[78ch] text-base text-white/65 sm:text-sm')], [
          'Status, pass-rate over the public scoring suite, total tokens, task, bucket, and the run summary — straight from the public projection.',
        ]),
      ]),
      body,
    ],
  )
}

// ---------------------------------------------------------------------------
// Leaderboard table (real Khala runs).
// ---------------------------------------------------------------------------
const leaderboardRow = (run: MirrorCodeRun): Html => {
  const h = html<Message>()

  return h.tr(
    [h.DataAttribute('mirrorcode-run-row', run.runId)],
    [
      h.td([Ui.className<Message>(cellClass)], [statusBadge(run.status)]),
      h.td(
        [Ui.className<Message>(`${cellClass} font-semibold text-white`)],
        [run.taskId],
      ),
      h.td([Ui.className<Message>(cellClass)], [run.bucket]),
      h.td([Ui.className<Message>(cellClass)], [run.language ?? '—']),
      h.td(
        [Ui.className<Message>(`${cellClass} tabular-nums`)],
        [formatMirrorCodePassRate(run.passRate)],
      ),
      h.td(
        [Ui.className<Message>(`${cellClass} tabular-nums`)],
        [formatMirrorCodeTokens(run.tokensTotal)],
      ),
      h.td([Ui.className<Message>(cellClass)], [gradeBadge(run)]),
    ],
  )
}

const leaderboardTable = (runs: ReadonlyArray<MirrorCodeRun>): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('min-w-0 overflow-x-auto')],
    [
      h.table(
        [
          h.DataAttribute('mirrorcode-leaderboard', ''),
          Ui.className<Message>('w-full min-w-[40rem] border-collapse text-[0.8125rem]'),
        ],
        [
          h.thead([], [
            h.tr([], [
              h.th([Ui.className<Message>(cellHeadClass)], ['Status']),
              h.th([Ui.className<Message>(cellHeadClass)], ['Task']),
              h.th([Ui.className<Message>(cellHeadClass)], ['Bucket']),
              h.th([Ui.className<Message>(cellHeadClass)], ['Language']),
              h.th([Ui.className<Message>(cellHeadClass)], ['Pass rate']),
              h.th([Ui.className<Message>(cellHeadClass)], ['Tokens']),
              h.th([Ui.className<Message>(cellHeadClass)], ['Grade']),
            ]),
          ]),
          h.tbody([], byStartedAtDesc(runs).map(leaderboardRow)),
        ],
      ),
    ],
  )
}

const comparatorTable = (
  comparators: ReadonlyArray<MirrorCodeComparator>,
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('min-w-0 overflow-x-auto')],
    [
      h.table(
        [
          h.DataAttribute('mirrorcode-comparators', ''),
          Ui.className<Message>('w-full min-w-[40rem] border-collapse text-[0.8125rem]'),
        ],
        [
          h.thead([], [
            h.tr([], [
              h.th([Ui.className<Message>(cellHeadClass)], ['Reference']),
              h.th([Ui.className<Message>(cellHeadClass)], ['Model']),
              h.th([Ui.className<Message>(cellHeadClass)], ['Note']),
            ]),
          ]),
          h.tbody(
            [],
            comparators.map(comparator =>
              h.tr(
                [h.DataAttribute('mirrorcode-comparator-row', comparator.label)],
                [
                  h.td(
                    [Ui.className<Message>(`${cellClass} font-semibold text-white`)],
                    [comparator.label],
                  ),
                  h.td([Ui.className<Message>(cellClass)], [comparator.model]),
                  h.td([Ui.className<Message>(cellClass)], [comparator.note]),
                ],
              ),
            ),
          ),
        ],
      ),
    ],
  )
}

const leaderboardPanel = (model: MirrorCodeRunsModel): Html => {
  const h = html<Message>()

  const loaded =
    model._tag === 'MirrorCodeRunsLoaded' ? model.response : null

  const runsBody: Html = loaded
    ? Arr.match(loaded.runs, {
        onEmpty: () =>
          emptyState(
            [h.DataAttribute('mirrorcode-leaderboard-empty', '')],
            'Leaderboard',
            'No scored runs yet',
            'Once the first Khala run completes against the public MirrorCode suite, it appears in this table.',
          ),
        onNonEmpty: runs => leaderboardTable(runs),
      })
    : model._tag === 'MirrorCodeRunsFailed'
      ? emptyState(
          [h.DataAttribute('mirrorcode-leaderboard-error', '')],
          'Leaderboard',
          'Leaderboard unavailable',
          `The public runs projection could not be read right now (${model.error}).`,
        )
      : emptyState(
          [h.DataAttribute('mirrorcode-leaderboard-loading', '')],
          'Leaderboard',
          'Loading runs…',
          'Reading the public MirrorCode runs projection.',
        )

  // The comparators section is ALWAYS clearly separated and labeled as
  // illustrative — never presented as a head-to-head against Khala (#6378).
  const comparatorsBody: Html =
    loaded && loaded.comparators.length > 0
      ? comparatorTable(loaded.comparators)
      : emptyState(
          [h.DataAttribute('mirrorcode-comparators-empty', '')],
          'Paper-reference comparators',
          'No comparators published',
          'Illustrative paper-reference numbers appear here when the projection carries them.',
        )

  return h.section(
    [
      h.DataAttribute('mirrorcode-leaderboard-panel', ''),
      Ui.className<Message>(panelClass),
    ],
    [
      h.div([Ui.className<Message>('grid gap-2')], [
        h.p([Ui.className<Message>(sectionTitleClass)], ['Leaderboard']),
        h.h2(
          [Ui.className<Message>('m-0 text-2xl font-semibold tracking-tight text-white sm:text-3xl')],
          ['Khala runs on the public MirrorCode suite'],
        ),
        h.p([Ui.className<Message>('m-0 max-w-[78ch] text-base text-white/65 sm:text-sm')], [
          'Every real Khala run, newest first. Smoke runs are labeled Phase-0 smoke and are not frontier measurements.',
        ]),
      ]),
      runsBody,
      h.div(
        [
          h.DataAttribute('mirrorcode-comparators-section', ''),
          Ui.className<Message>('grid gap-2 border-t border-white/10 pt-4'),
        ],
        [
          h.p([Ui.className<Message>(sectionTitleClass)], [
            'Paper-reference comparators (illustrative — not a head-to-head)',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-[78ch] text-[0.8125rem] text-white/55')], [
            'These figures are quoted from published papers under different harnesses and task sets. They are context only and are NOT directly comparable to the Khala runs above.',
          ]),
          comparatorsBody,
        ],
      ),
    ],
  )
}

// A small benchmark/freshness strip, populated once the projection loads.
const benchmarkStrip = (model: MirrorCodeRunsModel): Html => {
  const h = html<Message>()

  const text =
    model._tag === 'MirrorCodeRunsLoaded'
      ? `Benchmark: ${model.response.benchmark.name} · scope: ${model.response.benchmark.scope} · model: ${model.response.model} · generated ${model.response.generatedAt}`
      : 'Benchmark: Epoch Research MirrorCode · scope: public tasks only (private set excluded) · model: openagents/khala'

  return h.div(
    [
      h.DataAttribute('mirrorcode-benchmark-strip', ''),
      Ui.className<Message>(
        'border border-white/10 bg-black px-3 py-2 text-[0.78rem] text-white/55',
      ),
    ],
    [text],
  )
}

export const view = (model: MirrorCodeRunsModel): Html => {
  const h = html<Message>()

  return h.main(
    [
      h.DataAttribute('route', 'mirrorcode'),
      h.DataAttribute('mirrorcode-page', ''),
      Ui.className<Message>('min-h-dvh bg-black'),
    ],
    [
      h.div([Ui.className<Message>(pageClass)], [
        hero(),
        benchmarkStrip(model),
        livePanel(model),
        leaderboardPanel(model),
      ]),
    ],
  )
}
