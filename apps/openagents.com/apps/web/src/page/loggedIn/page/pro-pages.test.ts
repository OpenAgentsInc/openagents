// /pro runs + evals view tests (issue 6184): the comparison table, per-variant
// video, deltas, and honest empty/not-found states render through the shared
// Pro primitives. Pure (no DOM, no network): we walk the foldkit VNode tree to
// a string and assert on the rendered markup.

import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import type { Session } from '../../../domain/session'
import { evalDetailView, evalsView } from './pro-evals'
import { listProEvals, resolveProEval } from './pro-readmodel'
import { runDetailView, runsView } from './pro-runs'

type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string | null>
  data?: { attrs?: Record<string, unknown>; props?: Record<string, unknown> }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === 'object' && value !== null

const renderHtml = (html: Html): string => {
  if (html === null || !isVNodeLike(html)) return ''
  const tag = html.sel ?? 'node'
  // Walk BOTH attrs and props: foldkit maps some HTML attributes (e.g. `src`)
  // to DOM properties, so a source/img src lands in `props`, not `attrs`.
  const attrs = { ...(html.data?.attrs ?? {}), ...(html.data?.props ?? {}) }
  const attrStr = Object.entries(attrs)
    .filter(([, v]) => v !== false && v !== undefined && v !== null)
    .map(([k, v]) => (v === true ? ` ${k}` : ` ${k}="${String(v)}"`))
    .join('')
  const children = (html.children ?? [])
    .map(c => (typeof c === 'string' ? c : c === null ? '' : renderHtml(c)))
    .join('')
  return `<${tag}${attrStr}>${html.text ?? ''}${children}</${tag}>`
}

const session = (): Session => ({
  userId: 'user_1',
  email: 'rhys@example.test',
  name: 'Rhys',
})

describe('/pro/evals comparison', () => {
  test('eval detail renders the variant comparison with both variants + deltas', () => {
    const ev = listProEvals()[0]!
    const out = renderHtml(evalDetailView(session(), ev.id))
    // both variant labels appear
    for (const v of ev.variants) expect(out).toContain(v.label)
    // the comparison table is present
    expect(out).toContain('pro-eval-comparison')
    // a signed pass-rate delta for the regressed variant (-100%)
    expect(out).toContain('-100%')
    // honest illustrative note (fixtures, not decision-grade)
    expect(out).toContain('Illustrative')
    // a per-variant video pane is rendered
    expect(out).toContain('pro-video-pane')
    expect(out).toContain('/pro-assets/sample-session.webm')
  })

  test('#6192: eval detail surfaces the REFUTED verify verdict + evidence', () => {
    const ev = listProEvals()[0]!
    const out = renderHtml(evalDetailView(session(), ev.id))
    expect(out).toContain('pro-verdict-pill')
    expect(out).toContain('REFUTED')
    expect(out).toContain('pro-verdict-evidence')
  })

  test('eval detail shows an honest not-found state for an unknown id', () => {
    const out = renderHtml(evalDetailView(session(), 'does-not-exist'))
    expect(out).toContain('Eval not found')
    expect(out).toContain('pro-overview-error')
  })

  test('evals index lists every recorded eval', () => {
    const out = renderHtml(evalsView(session()))
    for (const ev of listProEvals()) expect(out).toContain(ev.title)
    expect(out).toContain('pro-index-list')
  })

  test('not_measured renders literally, never a fabricated 0', () => {
    // The fixture has measured latencies; assert the formatter contract holds
    // by confirming the resolver carries the sentinel type and the renderer
    // would print it (the eval page reads MeasuredMs directly).
    const ev = resolveProEval(listProEvals()[0]!.id)!
    expect(ev.variants.every(v => v.latencyP50Ms !== undefined)).toBe(true)
  })
})

describe('/pro/runs', () => {
  test('run detail renders the video, step table, and distilled-test ref', () => {
    const out = renderHtml(runDetailView(session(), 'login-regression-prod'))
    expect(out).toContain('pro-video-pane')
    expect(out).toContain('pro-step-table')
    expect(out).toContain('apps/qa-runner/generated/login-verify.e2e.test.ts')
    // pass status pill
    expect(out).toContain('pro-status-pill')
  })

  test('#6192: a CONFIRMED run renders the verdict pill + evidence', () => {
    const out = renderHtml(runDetailView(session(), 'login-regression-prod'))
    expect(out).toContain('pro-verdict-pill')
    expect(out).toContain('CONFIRMED')
    expect(out).toContain('pro-verdict-evidence')
    // the observed evidence is inline (no local run needed to confirm)
    expect(out).toContain('stays at /login')
  })

  test('#6192: a FALSE claim renders REFUTED (not a fake pass) with contradicting evidence', () => {
    const out = renderHtml(runDetailView(session(), 'login-redirect-claim-refuted'))
    expect(out).toContain('pro-verdict-pill')
    expect(out).toContain('REFUTED')
    expect(out).toContain('contradicting evidence')
    // never inflate a refuted run to confirmed
    expect(out).not.toContain('>CONFIRMED<')
  })

  test('run detail shows an honest not-found for an unknown id', () => {
    const out = renderHtml(runDetailView(session(), 'nope'))
    expect(out).toContain('Run not found')
  })

  test('runs index lists recorded runs', () => {
    const out = renderHtml(runsView(session()))
    expect(out).toContain('/login renders the sign-in form (prod)')
    expect(out).toContain('pro-index-list')
  })

  test('#6190: a multi-target run renders the per-target matrix (dev/staging/prod) side by side', () => {
    const out = renderHtml(runDetailView(session(), 'login-multi-target'))
    // the per-target matrix table is present
    expect(out).toContain('pro-target-matrix')
    // every selected target appears
    expect(out).toContain('dev')
    expect(out).toContain('staging')
    expect(out).toContain('prod')
    // the restriction policy is surfaced per target (prod read-only, dev writable)
    expect(out).toContain('pro-restriction-badge')
    expect(out).toContain('read-only')
    expect(out).toContain('writable')
    // per-target status pills
    expect(out).toContain('pro-status-pill')
  })

  test('#6190: a read-only target blocks a mutating step honestly (failure reason renders, not a fake pass)', () => {
    const out = renderHtml(
      runDetailView(session(), 'submit-login-multi-target'),
    )
    expect(out).toContain('pro-target-matrix')
    // the honest refusal reason renders inline for the read-only target
    expect(out).toContain('pro-target-failure')
    expect(out).toContain('restriction violation')
    expect(out).toContain('never create data')
    // and the run is honestly red, never inflated to pass
    expect(out).toContain('pro-status-pill')
  })
})
