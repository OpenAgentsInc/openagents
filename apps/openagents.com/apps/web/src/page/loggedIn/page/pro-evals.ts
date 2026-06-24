import type { Html } from 'foldkit/html'

import type { Session } from '../../../domain/session'
import {
  proEvalRouter,
  proEvalsRouter,
  proRouter,
  proRunsRouter,
} from '../../../route'
import * as Ui from '../../../ui'
import type { Message } from '../message'
import {
  listProEvals,
  type MeasuredMs,
  type ProEval,
  type ProEvalVariant,
  resolveProEval,
} from './pro-readmodel'

// ---------------------------------------------------------------------------
// /pro/evals + /pro/evals/<id> (issue 6184)
// ---------------------------------------------------------------------------
//
// The chill-eval surface: an index of comparisons and the eval-detail page —
// the headline variant comparison table (pass-rate / p50 / p90 / deltas) plus a
// per-variant video. This is the URL the PR-evidence loop (#6185) posts:
// /pro/evals/<id>. Stable + shareable; rendered through the shared Pro
// primitives (DESIGN.md + Foldkit-UI-composition guard).

const EVALS_HREF = proEvalsRouter()

const SECTIONS = (): ReadonlyArray<Ui.ProConsoleSection> => [
  { label: 'Overview', href: proRouter() },
  { label: 'Runs', href: proRunsRouter() },
  { label: 'Evals', active: true, href: EVALS_HREF },
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

const shell = (session: Session, breadcrumb: string, main: Html): Html =>
  Ui.proConsoleShell<Message>({
    topStrip: topStrip(session, breadcrumb),
    register: Ui.proRegister<Message>(SECTIONS()),
    main: Ui.proMainPane<Message>([main]),
  })

// ----- formatters (honest not_measured; signed deltas) -------------------

const fmtMs = (value: MeasuredMs): string =>
  value === 'not_measured' ? 'not_measured' : `${Math.round(value)}ms`

const fmtPctDelta = (delta: number): string => {
  if (delta === 0) return '0%'
  const sign = delta > 0 ? '+' : ''
  return `${sign}${Math.round(delta * 100)}%`
}

const fmtMsDelta = (value: MeasuredMs): string => {
  if (value === 'not_measured') return 'not_measured'
  if (value === 0) return '0ms'
  const sign = value > 0 ? '+' : ''
  return `${sign}${Math.round(value)}ms`
}

// ----- Evals index -------------------------------------------------------

const evalsIndexBody = (): Html => {
  const evals = listProEvals()
  return Ui.proConsoleStack<Message>([
    Ui.proPageHeader<Message>({
      title: 'Evals',
      meta: [{ label: 'count', value: String(evals.length) }],
    }),
    Ui.proIndexList<Message>({
      rows: evals.map((ev: ProEval) => ({
        href: proEvalRouter({ evalId: ev.id }),
        title: ev.title,
        meta: `${ev.variants.length} variants · scenario ${ev.scenarioId}`,
      })),
      emptyLabel:
        'No evals yet. Run `bun --filter @openagentsinc/qa-runner evals` to record a comparison.',
    }),
  ])
}

export const evalsView = (session: Session): Html =>
  shell(session, 'Evals', evalsIndexBody())

// ----- Eval detail (the comparison) --------------------------------------

const evalDetailBody = (evalId: string): Html => {
  const ev = resolveProEval(evalId)
  if (ev === undefined) {
    return Ui.proConsoleStack<Message>([
      Ui.proPageHeader<Message>({
        back: { href: EVALS_HREF, label: 'Evals' },
        title: 'Eval not found',
        meta: [{ label: 'id', value: evalId }],
      }),
      Ui.proErrorStrip<Message>(
        `No eval with id "${evalId}". It may not be recorded yet.`,
      ),
    ])
  }

  const comparisonRows = ev.variants.map((v: ProEvalVariant) => ({
    label: v.label,
    ...(v.note !== undefined ? { note: v.note } : {}),
    baseline: v.variantId === ev.baselineVariantId,
    passRate: v.passRate,
    passCount: v.passCount,
    runCount: v.runCount,
    latencyP50: fmtMs(v.latencyP50Ms),
    latencyP90: fmtMs(v.latencyP90Ms),
    deltaPass:
      v.variantId === ev.baselineVariantId ? '0%' : fmtPctDelta(v.passRateDelta),
    deltaP50:
      v.variantId === ev.baselineVariantId
        ? '0ms'
        : fmtMsDelta(v.latencyP50DeltaMs),
  }))

  const videoSections = ev.variants
    .filter((v: ProEvalVariant) => v.video !== undefined)
    .map((v: ProEvalVariant) =>
      Ui.proVideoPane<Message>({
        src: v.video!.src,
        format: v.video!.format,
        label: v.label,
      }),
    )

  return Ui.proConsoleStack<Message>([
    Ui.proPageHeader<Message>({
      back: { href: EVALS_HREF, label: 'Evals' },
      title: ev.title,
      meta: [
        { label: 'scenario', value: ev.scenarioLabel },
        { label: 'target', value: ev.targetName },
        { label: 'reps', value: String(ev.repetitions) },
        { label: 'variants', value: String(ev.variants.length) },
      ],
      ...(ev.decisionGrade
        ? {}
        : {
            note: 'Illustrative run (fixtures / no spend). Numbers prove the harness, not the lanes.',
          }),
    }),
    Ui.proConsoleSection2<Message>('Comparison', [
      Ui.proEvalComparisonTable<Message>(comparisonRows),
    ]),
    ...(videoSections.length > 0
      ? [Ui.proConsoleSection2<Message>('Videos', videoSections)]
      : []),
  ])
}

export const evalDetailView = (session: Session, evalId: string): Html =>
  shell(session, `Evals / ${evalId}`, evalDetailBody(evalId))
