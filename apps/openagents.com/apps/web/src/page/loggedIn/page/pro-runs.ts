import type { Html } from 'foldkit/html'

import type { Session } from '../../../domain/session'
import {
  proEvalsRouter,
  proRouter,
  proRunRouter,
  proRunsRouter,
} from '../../../route'
import * as Ui from '../../../ui'
import type { Message } from '../message'
import {
  listProRuns,
  type ProRun,
  resolveProRun,
} from './pro-readmodel'

// ---------------------------------------------------------------------------
// /pro/runs + /pro/runs/<id> (issue 6184)
// ---------------------------------------------------------------------------
//
// The Runs section of the operator console: an index of qa-runner runs and a
// run-detail page with the video, the step table, and the committed distilled
// test link. Rendered as a top-level Pro console page (its own shell), so the
// URLs are stable + shareable — the PR-evidence loop (#6185) links straight to
// /pro/runs/<id>.
//
// All class-bearing markup lives in `@openagentsinc/ui` Pro primitives; this
// page only wires the read model to those components (DESIGN.md + the
// Foldkit-UI-composition guard).

const SECTIONS = (active: 'runs' | 'evals'): ReadonlyArray<Ui.ProConsoleSection> => [
  { label: 'Overview', href: proRouter() },
  { label: 'Runs', active: active === 'runs', href: proRunsRouter() },
  { label: 'Evals', active: active === 'evals', href: proEvalsRouter() },
  { label: 'Sessions', disabled: true },
  { label: 'Settings', disabled: true },
]

const topStrip = (session: Session, breadcrumb: string): Html =>
  Ui.proTopStrip<Message>({
    homeHref: proRouter(),
    breadcrumb,
    creditsLabel: 'credits',
    creditsState: 'soon',
    creditsHint: 'Usage metering is coming to Pro — not active yet.',
    accountLabel: session.email,
  })

const shell = (
  session: Session,
  breadcrumb: string,
  active: 'runs' | 'evals',
  main: Html,
): Html =>
  Ui.proConsoleShell<Message>({
    topStrip: topStrip(session, breadcrumb),
    register: Ui.proRegister<Message>(SECTIONS(active)),
    main: Ui.proMainPane<Message>([main]),
  })

const ms = (value: number): string => `${value}ms`

// ----- Runs index --------------------------------------------------------

const runsIndexBody = (): Html => {
  const runs = listProRuns()
  return Ui.proConsoleStack<Message>([
    Ui.proPageHeader<Message>({
      title: 'Runs',
      meta: [{ label: 'count', value: String(runs.length) }],
    }),
    Ui.proIndexList<Message>({
      rows: runs.map((run: ProRun) => ({
        href: proRunRouter({ runId: run.id }),
        title: run.title,
        status: run.status,
        meta: `${run.targetName} · ${run.brain} · ${ms(run.durationMs)}`,
      })),
      emptyLabel: 'No runs yet. A qa-runner run will appear here once recorded.',
    }),
  ])
}

export const runsView = (session: Session): Html =>
  shell(session, 'Runs', 'runs', runsIndexBody())

// ----- Run detail --------------------------------------------------------

const runDetailBody = (runId: string): Html => {
  const run = resolveProRun(runId)
  if (run === undefined) {
    return Ui.proConsoleStack<Message>([
      Ui.proPageHeader<Message>({
        back: { href: proRunsRouter(), label: 'Runs' },
        title: 'Run not found',
        meta: [{ label: 'id', value: runId }],
      }),
      Ui.proErrorStrip<Message>(
        `No run with id "${runId}". It may not be recorded yet.`,
      ),
    ])
  }

  const children: ReadonlyArray<Html> = [
    Ui.proPageHeader<Message>({
      back: { href: proRunsRouter(), label: 'Runs' },
      title: run.title,
      status: run.status,
      meta: [
        { label: 'target', value: run.targetName },
        { label: 'brain', value: run.brain },
        { label: 'backend', value: run.backend },
        { label: 'duration', value: ms(run.durationMs) },
        { label: 'started', value: run.startedAt },
      ],
      ...(run.failure !== undefined ? { note: run.failure } : {}),
    }),
    ...(run.video !== undefined
      ? [
          Ui.proConsoleSection2<Message>('Video', [
            Ui.proVideoPane<Message>({
              src: run.video.src,
              format: run.video.format,
              label: 'Playable session recording (public-safe).',
            }),
          ]),
        ]
      : []),
    Ui.proConsoleSection2<Message>('Steps', [
      Ui.proRunStepTable<Message>(
        run.steps.map(step => ({
          index: step.index,
          kind: step.kind,
          label: step.label,
          status: step.status,
        })),
      ),
    ]),
    ...(run.distilledTestPath !== undefined
      ? [
          Ui.proConsoleSection2<Message>('Distilled test', [
            Ui.proCodeRef<Message>(run.distilledTestPath),
          ]),
        ]
      : []),
  ]

  return Ui.proConsoleStack<Message>(children)
}

export const runDetailView = (session: Session, runId: string): Html =>
  shell(session, run_breadcrumb(runId), 'runs', runDetailBody(runId))

const run_breadcrumb = (runId: string): string => `Runs / ${runId}`
