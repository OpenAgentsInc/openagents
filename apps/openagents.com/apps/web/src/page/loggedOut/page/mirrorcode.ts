import { Array as Arr } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import type { Message } from '../message'
import {
  type MirrorCodeComparator,
  type MirrorCodeRun,
  type MirrorCodeRunStatus,
  formatMirrorCodePassRate,
  formatMirrorCodeTokens,
  mirrorCodeGradeLabel,
  mirrorCodeStatusLabel,
} from '../mirrorcode/runs'
import type { MirrorCodeRunsModel } from '../model'

type DivAttr = Parameters<ReturnType<typeof html<Message>>['div']>[0][number]
type ExecutionPhaseState = 'complete' | 'active' | 'pending' | 'blocked'
type ExecutionPhase = Readonly<{
  key: string
  label: string
  detail: string
  state: ExecutionPhaseState
}>

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

const phaseStateClass = (state: ExecutionPhaseState): string =>
  state === 'complete'
    ? 'border-[#00c853]/35 bg-[#031006] text-[#99efb0]'
    : state === 'active'
      ? 'border-[#2979ff]/40 bg-[#040d1b] text-[#b7d3ff]'
      : state === 'blocked'
        ? 'border-[#d32f2f]/35 bg-[#160505] text-[#ffb3b3]'
        : 'border-white/10 bg-black text-white/45'

const phaseBarClass = (state: ExecutionPhaseState): string =>
  state === 'complete'
    ? 'bg-[#00c853]'
    : state === 'active'
      ? 'bg-[#2979ff]'
      : state === 'blocked'
        ? 'bg-[#d32f2f]'
        : 'bg-white/18'

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

const codeBlock = (label: string, lines: ReadonlyArray<string>): Html => {
  const h = html<Message>()

  return h.figure(
    [
      Ui.className<Message>(
        'm-0 grid gap-2 border border-white/10 bg-black p-3',
      ),
    ],
    [
      h.figcaption(
        [
          Ui.className<Message>(
            'text-[0.72rem] font-semibold uppercase tracking-wide text-white/45',
          ),
        ],
        [label],
      ),
      h.pre(
        [
          Ui.className<Message>(
            'm-0 overflow-x-auto whitespace-pre rounded-none text-[0.75rem] leading-5 text-white/70',
          ),
        ],
        [lines.join('\n')],
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
        [
          Ui.className<Message>(
            'm-0 text-base font-semibold text-white/80 sm:text-lg',
          ),
        ],
        [heading],
      ),
      h.p(
        [
          Ui.className<Message>(
            'm-0 max-w-[78ch] text-base text-white/55 sm:text-sm',
          ),
        ],
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

const executionPhasesFor = (
  run: MirrorCodeRun,
): ReadonlyArray<ExecutionPhase> => {
  const scored = run.status === 'passed' || run.status === 'failed'
  const terminal = scored || run.status === 'error'

  return [
    {
      key: 'queued',
      label: 'Queued',
      detail: `${run.bucket} bucket · ${run.language ?? 'default runtime'}`,
      state: 'complete',
    },
    {
      key: 'implementation',
      label: 'Implement',
      detail:
        run.status === 'queued'
          ? 'Waiting for owner-runner pickup'
          : 'Khala builds the tool from scratch in sandbox',
      state: run.status === 'queued' ? 'pending' : 'complete',
    },
    {
      key: 'scoring',
      label: 'Score',
      detail:
        run.status === 'running'
          ? 'Held-out public suite is in flight'
          : scored
            ? `${formatMirrorCodePassRate(run.passRate)} public-suite pass rate`
            : run.status === 'error'
              ? 'Harness error before scored result'
              : 'Awaiting scored result',
      state:
        run.status === 'running'
          ? 'active'
          : scored
            ? 'complete'
            : run.status === 'error'
              ? 'blocked'
              : 'pending',
    },
    {
      key: 'closeout',
      label: 'Closeout',
      detail: terminal
        ? `${formatMirrorCodeTokens(run.tokensTotal)} tokens · ${
            run.decisionGrade ? 'exact refs required' : 'smoke evidence'
          }`
        : 'Token total and summary update at completion',
      state: terminal
        ? run.status === 'error'
          ? 'blocked'
          : 'complete'
        : 'pending',
    },
  ]
}

// ---------------------------------------------------------------------------
// Hero
// ---------------------------------------------------------------------------
const hero = (): Html => {
  const h = html<Message>()

  return h.header(
    [Ui.className<Message>('grid gap-3')],
    [
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
        [
          Ui.className<Message>(
            'm-0 text-3xl font-semibold tracking-tight text-balance sm:text-5xl',
          ),
        ],
        ['MirrorCode, powered by Khala'],
      ),
      h.p(
        [Ui.className<Message>('m-0 max-w-3xl text-base text-white/65')],
        [
          'Khala (openagents/khala) reimplements real tools from scratch inside a sandbox, then a held-out test suite scores the result. The benchmark is the Epoch Research MirrorCode set, run here on PUBLIC tasks only — the private set is excluded — so every number below is reproducible and never reads the held-out answers.',
        ],
      ),
    ],
  )
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
        [
          Ui.className<Message>(
            'flex flex-wrap items-center justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-0.5')],
            [
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 text-base font-semibold text-white',
                  ),
                ],
                [run.taskId],
              ),
              h.p(
                [Ui.className<Message>('m-0 text-[0.78rem] text-white/50')],
                [
                  `${run.model} · bucket ${run.bucket}${run.language ? ` · ${run.language}` : ''}`,
                ],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('flex flex-wrap items-center gap-2')],
            [statusBadge(run.status), gradeBadge(run)],
          ),
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
      h.div(
        [Ui.className<Message>('grid gap-2')],
        [
          h.p([Ui.className<Message>(sectionTitleClass)], ['Live run']),
          h.h2(
            [
              Ui.className<Message>(
                'm-0 max-w-[24ch] text-2xl font-semibold tracking-tight text-balance text-white sm:text-3xl',
              ),
            ],
            ['The latest Khala run'],
          ),
          h.p(
            [
              Ui.className<Message>(
                'm-0 max-w-[78ch] text-base text-white/65 sm:text-sm',
              ),
            ],
            [
              'Status, pass-rate over the public scoring suite, total tokens, task, bucket, and the run summary — straight from the public projection.',
            ],
          ),
        ],
      ),
      body,
    ],
  )
}

const executionPhaseView = (
  run: MirrorCodeRun,
  phase: ExecutionPhase,
  index: number,
): Html => {
  const h = html<Message>()

  return h.li(
    [
      h.DataAttribute('mirrorcode-execution-phase', phase.key),
      h.DataAttribute('mirrorcode-execution-phase-state', phase.state),
      Ui.className<Message>(
        `grid min-w-[8.5rem] content-start gap-2 border p-3 ${phaseStateClass(phase.state)}`,
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('flex items-center justify-between gap-3')],
        [
          h.span(
            [
              Ui.className<Message>(
                'text-[0.68rem] font-semibold uppercase tracking-wide text-current/70',
              ),
            ],
            [`0${index + 1}`],
          ),
          h.span(
            [
              Ui.className<Message>('h-1.5 w-12 rounded-full'),
              h.Style({ background: statusColor(run.status) }),
            ],
            [],
          ),
        ],
      ),
      h.h3(
        [Ui.className<Message>('m-0 text-sm font-semibold text-white')],
        [phase.label],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-[0.75rem] leading-5 text-current/75')],
        [phase.detail],
      ),
    ],
  )
}

const executionRail = (run: MirrorCodeRun): Html => {
  const h = html<Message>()
  const phases = executionPhasesFor(run)

  return h.article(
    [
      h.DataAttribute('mirrorcode-execution-run', run.runId),
      h.AriaLabel(`MirrorCode execution visualizer for ${run.taskId}`),
      Ui.className<Message>('grid gap-3 border border-white/10 bg-black p-4'),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'flex flex-wrap items-start justify-between gap-3',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-1')],
            [
              h.h3(
                [
                  Ui.className<Message>(
                    'm-0 text-base font-semibold text-white',
                  ),
                ],
                [run.taskId],
              ),
              h.p(
                [Ui.className<Message>('m-0 text-[0.78rem] text-white/50')],
                [
                  `${run.runId} · started ${run.startedAt} · ${
                    run.finishedAt ?? 'in progress'
                  }`,
                ],
              ),
            ],
          ),
          h.div(
            [Ui.className<Message>('flex flex-wrap items-center gap-2')],
            [statusBadge(run.status), gradeBadge(run)],
          ),
        ],
      ),
      h.ol(
        [
          Ui.className<Message>(
            'grid list-none gap-2 p-0 m-0 sm:grid-cols-2 xl:grid-cols-4',
          ),
        ],
        phases.map((phase, index) => executionPhaseView(run, phase, index)),
      ),
      h.div(
        [
          h.DataAttribute('mirrorcode-execution-token-band', ''),
          Ui.className<Message>(
            'grid gap-2 border border-white/10 bg-[#050505] p-3 sm:grid-cols-[9rem_minmax(0,1fr)] sm:items-center',
          ),
        ],
        [
          h.span(
            [
              Ui.className<Message>(
                'text-[0.72rem] font-semibold uppercase tracking-wide text-white/45',
              ),
            ],
            ['Token burn'],
          ),
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.div(
                [Ui.className<Message>('h-2 overflow-hidden bg-white/10')],
                [
                  h.span(
                    [
                      Ui.className<Message>(
                        `block h-full ${phaseBarClass(
                          phases[phases.length - 1]?.state ?? 'pending',
                        )}`,
                      ),
                      h.Style({
                        width:
                          run.tokensTotal === 0
                            ? '2%'
                            : `${Math.min(100, Math.max(8, run.tokensTotal / 100000))}%`,
                      }),
                    ],
                    [],
                  ),
                ],
              ),
              h.span(
                [Ui.className<Message>('text-[0.78rem] text-white/55')],
                [
                  `${formatMirrorCodeTokens(run.tokensTotal)} exact-token total from the public row; raw events and task contents stay private.`,
                ],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}

const executionVisualizerEmpty = (): Html =>
  emptyState(
    [html<Message>().DataAttribute('mirrorcode-execution-empty', '')],
    'Execution visualizer',
    'No execution rows to visualize yet',
    'When a MirrorCode run exists, this panel renders the queued, implementation, scoring, and closeout phases from public-safe status rows only.',
  )

const executionVisualizerPanel = (model: MirrorCodeRunsModel): Html => {
  const h = html<Message>()
  const body: Html =
    model._tag === 'MirrorCodeRunsLoaded'
      ? Arr.match(byStartedAtDesc(model.response.runs), {
          onEmpty: () => executionVisualizerEmpty(),
          onNonEmpty: runs =>
            h.div(
              [Ui.className<Message>('grid gap-3')],
              runs.slice(0, 3).map(executionRail),
            ),
        })
      : model._tag === 'MirrorCodeRunsFailed'
        ? emptyState(
            [h.DataAttribute('mirrorcode-execution-error', '')],
            'Execution visualizer',
            'Execution feed unavailable',
            `The public runs projection could not be read right now (${model.error}).`,
          )
        : model._tag === 'MirrorCodeRunsLoading'
          ? emptyState(
              [h.DataAttribute('mirrorcode-execution-loading', '')],
              'Execution visualizer',
              'Loading execution rows…',
              'Reading public-safe MirrorCode run status rows.',
            )
          : executionVisualizerEmpty()

  return h.section(
    [
      h.DataAttribute('mirrorcode-execution-visualizer', ''),
      Ui.className<Message>(panelClass),
    ],
    [
      h.div(
        [Ui.className<Message>('grid gap-2')],
        [
          h.p(
            [Ui.className<Message>(sectionTitleClass)],
            ['Live run execution visualizer'],
          ),
          h.h2(
            [
              Ui.className<Message>(
                'm-0 max-w-[28ch] text-2xl font-semibold tracking-tight text-balance text-white sm:text-3xl',
              ),
            ],
            ['Follow MirrorCode tasks from queue to closeout'],
          ),
          h.p(
            [
              Ui.className<Message>(
                'm-0 max-w-[78ch] text-base text-white/65 sm:text-sm',
              ),
            ],
            [
              'A compact execution rail for the newest public runs. It shows lifecycle state, scoring posture, token burn, and finish status without exposing prompts, raw logs, private benchmark material, or canary strings.',
            ],
          ),
        ],
      ),
      body,
    ],
  )
}

const playgroundStep = (
  title: string,
  body: string,
  attrs: ReadonlyArray<DivAttr>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      ...attrs,
      Ui.className<Message>(
        'grid content-start gap-2 border border-white/10 bg-black p-3',
      ),
    ],
    [
      h.h3(
        [Ui.className<Message>('m-0 text-sm font-semibold text-white')],
        [title],
      ),
      h.p(
        [Ui.className<Message>('m-0 text-[0.8125rem] leading-5 text-white/60')],
        [body],
      ),
    ],
  )
}

const playgroundPanel = (): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.DataAttribute('mirrorcode-playground-panel', ''),
      Ui.className<Message>(panelClass),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'grid gap-2 lg:grid-cols-[minmax(0,1fr)_18rem] lg:items-start',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-2')],
            [
              h.p(
                [Ui.className<Message>(sectionTitleClass)],
                ['MirrorCode-as-a-Service playground'],
              ),
              h.h2(
                [
                  Ui.className<Message>(
                    'm-0 max-w-[26ch] text-2xl font-semibold tracking-tight text-balance text-white sm:text-3xl',
                  ),
                ],
                ['Queue a public-task run, then read status by run id'],
              ),
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 max-w-[78ch] text-base text-white/65 sm:text-sm',
                  ),
                ],
                [
                  'The public playground shows the exact API surface without opening public dispatch. Launch is owner-gated; status and leaderboard reads are public-safe and never expose task source, prompts, logs, canary strings, keys, or private-set answers.',
                ],
              ),
            ],
          ),
          h.div(
            [
              h.DataAttribute('mirrorcode-owner-gated-launch', ''),
              Ui.className<Message>(
                'grid gap-1 border border-[#ffb400]/30 bg-[#120d02] p-3 text-[0.78rem] text-[#ffd884]',
              ),
            ],
            [
              h.span(
                [
                  Ui.className<Message>(
                    'font-semibold uppercase tracking-wide',
                  ),
                ],
                ['Owner-gated launch'],
              ),
              h.span(
                [],
                [
                  'POST requires an admin bearer token; public visitors can inspect the contract and read results only.',
                ],
              ),
            ],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-3 md:grid-cols-3')],
        [
          playgroundStep(
            '1. Choose a public target',
            'Select an S, M, or L public MirrorCode task. Private tasks are excluded from this service surface.',
            [h.DataAttribute('mirrorcode-playground-step', 'target')],
          ),
          playgroundStep(
            '2. Queue a launch intent',
            'The owner-operated runner creates a queued row first, then the external MirrorCode/Inspect executor updates status and result.',
            [h.DataAttribute('mirrorcode-playground-step', 'launch')],
          ),
          playgroundStep(
            '3. Poll public status',
            'Read a single run or the leaderboard. The response carries public-safe fields only, with exact-token refs when a decision-grade result exists.',
            [h.DataAttribute('mirrorcode-playground-step', 'status')],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-3 lg:grid-cols-2')],
        [
          codeBlock('Launch intent', [
            'POST /api/gym/mirrorcode/runs',
            'Authorization: Bearer <admin-token>',
            '',
            '{',
            '  "kind": "launch",',
            '  "taskId": "cal",',
            '  "bucket": "S",',
            '  "language": "python",',
            '  "grade": "smoke"',
            '}',
          ]),
          codeBlock('Status read', [
            'GET /api/gym/mirrorcode/runs/{runId}',
            'GET /api/gym/mirrorcode/runs',
            '',
            'returns:',
            '  status, passRate, tokensTotal,',
            '  exactTokenUsageEventRefs,',
            '  tokenAttributionProofRef',
          ]),
        ],
      ),
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
          Ui.className<Message>(
            'w-full min-w-[40rem] border-collapse text-[0.8125rem]',
          ),
        ],
        [
          h.thead(
            [],
            [
              h.tr(
                [],
                [
                  h.th([Ui.className<Message>(cellHeadClass)], ['Status']),
                  h.th([Ui.className<Message>(cellHeadClass)], ['Task']),
                  h.th([Ui.className<Message>(cellHeadClass)], ['Bucket']),
                  h.th([Ui.className<Message>(cellHeadClass)], ['Language']),
                  h.th([Ui.className<Message>(cellHeadClass)], ['Pass rate']),
                  h.th([Ui.className<Message>(cellHeadClass)], ['Tokens']),
                  h.th([Ui.className<Message>(cellHeadClass)], ['Grade']),
                ],
              ),
            ],
          ),
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
          Ui.className<Message>(
            'w-full min-w-[40rem] border-collapse text-[0.8125rem]',
          ),
        ],
        [
          h.thead(
            [],
            [
              h.tr(
                [],
                [
                  h.th([Ui.className<Message>(cellHeadClass)], ['Reference']),
                  h.th([Ui.className<Message>(cellHeadClass)], ['Model']),
                  h.th([Ui.className<Message>(cellHeadClass)], ['Note']),
                ],
              ),
            ],
          ),
          h.tbody(
            [],
            comparators.map(comparator =>
              h.tr(
                [
                  h.DataAttribute(
                    'mirrorcode-comparator-row',
                    comparator.label,
                  ),
                ],
                [
                  h.td(
                    [
                      Ui.className<Message>(
                        `${cellClass} font-semibold text-white`,
                      ),
                    ],
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

  const loaded = model._tag === 'MirrorCodeRunsLoaded' ? model.response : null

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
      h.div(
        [Ui.className<Message>('grid gap-2')],
        [
          h.p([Ui.className<Message>(sectionTitleClass)], ['Leaderboard']),
          h.h2(
            [
              Ui.className<Message>(
                'm-0 text-2xl font-semibold tracking-tight text-white sm:text-3xl',
              ),
            ],
            ['Khala runs on the public MirrorCode suite'],
          ),
          h.p(
            [
              Ui.className<Message>(
                'm-0 max-w-[78ch] text-base text-white/65 sm:text-sm',
              ),
            ],
            [
              'Every real Khala run, newest first. Smoke runs are labeled Phase-0 smoke and are not frontier measurements.',
            ],
          ),
        ],
      ),
      runsBody,
      h.div(
        [
          h.DataAttribute('mirrorcode-comparators-section', ''),
          Ui.className<Message>('grid gap-2 border-t border-white/10 pt-4'),
        ],
        [
          h.p(
            [Ui.className<Message>(sectionTitleClass)],
            ['Paper-reference comparators (illustrative — not a head-to-head)'],
          ),
          h.p(
            [
              Ui.className<Message>(
                'm-0 max-w-[78ch] text-[0.8125rem] text-white/55',
              ),
            ],
            [
              'These figures are quoted from published papers under different harnesses and task sets. They are context only and are NOT directly comparable to the Khala runs above.',
            ],
          ),
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
      h.div(
        [Ui.className<Message>(pageClass)],
        [
          hero(),
          benchmarkStrip(model),
          playgroundPanel(),
          livePanel(model),
          executionVisualizerPanel(model),
          leaderboardPanel(model),
        ],
      ),
    ],
  )
}
