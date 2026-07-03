// Public `/trace/compare/{ids}` render tests (issue #6211 — the real
// "chill-evals": compare N agent traces, shareable). Pure: we parse the route,
// confirm the public (no-auth-bootstrap) startup posture via `init`, and walk
// the foldkit VNode tree to a string to assert the rendered markup — the
// comparison table, per-trace verdict/latency/steps/cost, the honest deltas vs
// the baseline, each variant's `/trace/{uuid}` deep link, and the honest
// handling of unknown ids + an empty id list. Also unit-tests the pure
// comparison model math.

import { Option } from 'effect'
import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { Flags, init } from '../main'
import { TraceCompareRoute, urlToAppRoute } from '../route'
import * as TraceCompare from './trace-compare'
import { formatCost, formatDuration } from './trace/atif'
import {
  buildComparison,
  isMeasured,
  parseCompareIds,
} from './trace-compare/model'
import {
  SAMPLE_COMPARE_BASELINE_UUID,
  SAMPLE_COMPARE_MCP_ON_UUID,
  SAMPLE_COMPARE_NO_WAITFOR_UUID,
  SAMPLE_COMPARE_PATH_IDS,
  lookupTrajectoryForCompare,
  mcpOnTrajectory,
  noWaitForTrajectory,
} from './trace-compare/sample'

const appUrl = (pathname: string) => ({
  protocol: 'https:',
  host: 'openagents.com',
  port: Option.none(),
  pathname,
  search: Option.none(),
  hash: Option.none(),
})

type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string | null>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    class?: Record<string, boolean>
  }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === 'object' && value !== null

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, enabled]) => enabled)
    .map(([className]) => className)
    .join(' ')
  const pairs = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes.length === 0 ? [] : [['class', classes] as const]),
  ]
  return pairs
    .filter(
      ([, value]) => value !== false && value !== undefined && value !== null,
    )
    .map(([name, value]) =>
      value === true ? ` ${name}` : ` ${name}="${String(value)}"`,
    )
    .join('')
}

const renderHtml = (html: Html): string => {
  if (html === null || !isVNodeLike(html)) return ''
  const tag = html.sel ?? 'node'
  const children = (html.children ?? [])
    .map(child =>
      typeof child === 'string'
        ? child
        : child === null
          ? ''
          : renderHtml(child),
    )
    .join('')
  return `<${tag}${attrsToString(html)}>${html.text ?? ''}${children}</${tag}>`
}

const sampleRoute = TraceCompareRoute({ ids: SAMPLE_COMPARE_PATH_IDS })
const renderSample = (): string =>
  renderHtml(TraceCompare.view(sampleRoute, { _tag: 'LoggedOut' }))

describe('trace-compare route', () => {
  test('parses /trace/compare/{ids} and captures the ids', () => {
    const route = urlToAppRoute(appUrl(`/trace/compare/${SAMPLE_COMPARE_PATH_IDS}`))
    expect(route).toEqual(TraceCompareRoute({ ids: SAMPLE_COMPARE_PATH_IDS }))
  })

  test('is public: no auth bootstrap command is dispatched', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl(`/trace/compare/${SAMPLE_COMPARE_PATH_IDS}`),
    )
    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'TraceCompare', ids: SAMPLE_COMPARE_PATH_IDS },
    })
    expect(commands).toHaveLength(0)
  })
})

describe('trace-compare render (sample trace-set)', () => {
  test('renders the page shell + a comparison table', () => {
    const rendered = renderSample()
    expect(rendered).toContain('data-component="trace-compare-page"')
    expect(rendered).toContain('data-component="trace-compare-table"')
  })

  test('renders all three sample variants side by side', () => {
    const rendered = renderSample()
    // The baseline (PASS) and both variants are present as columns.
    expect(rendered).toContain('Baseline')
    expect(rendered).toContain('Variant 1')
    expect(rendered).toContain('Variant 2')
    // Verdicts from each variant's real final_metrics.
    expect(rendered).toContain('Verified') // baseline + mcp-on PASS
    expect(rendered).toContain('Refuted') // no-waitfor REFUTED
  })

  test('renders the per-trace metrics (latency, steps, cost)', () => {
    const rendered = renderSample()
    expect(rendered).toContain('Latency')
    expect(rendered).toContain('Steps')
    expect(rendered).toContain('Cost')
    expect(rendered).toContain('Video')
    // Baseline latency (11.5s) + mcp-on latency (13.9s) are both shown.
    expect(rendered).toContain(formatDuration(11459))
    expect(rendered).toContain(formatDuration(13900))
    expect(rendered).toContain(formatCost(0))
  })

  test('renders per-variant video links when traces carry video evidence', () => {
    const rendered = renderSample()
    expect(rendered).toContain('data-component="trace-compare-video"')
    expect(rendered).toContain('Watch video')
    expect(rendered).toContain('/pro-assets/sample-session.webm')
  })

  test('renders honest deltas vs the baseline', () => {
    const rendered = renderSample()
    expect(rendered).toContain('data-component="trace-compare-delta"')
    // mcp-on is one step MORE than the baseline (6 vs 5) -> +1.
    expect(rendered).toContain('+1')
    // no-waitfor regressed from PASS to REFUTED -> a pass regression marker.
    expect(rendered).toContain('−pass')
  })

  test('deep-links each variant to its full /trace/{uuid}', () => {
    const rendered = renderSample()
    expect(rendered).toContain('data-component="trace-compare-deeplink"')
    expect(rendered).toContain(`/trace/${SAMPLE_COMPARE_BASELINE_UUID}`)
    expect(rendered).toContain(`/trace/${SAMPLE_COMPARE_MCP_ON_UUID}`)
    expect(rendered).toContain(`/trace/${SAMPLE_COMPARE_NO_WAITFOR_UUID}`)
  })

  test('produces a shareable document title', () => {
    expect(TraceCompare.title(sampleRoute)).toBe(
      'Compare 3 agent traces - OpenAgents',
    )
  })
})

describe('trace-compare render (unknown id)', () => {
  const route = TraceCompareRoute({
    ids: `${SAMPLE_COMPARE_BASELINE_UUID},does-not-exist-0000`,
  })

  test('renders an explicit unknown-id column, not a fabricated trace', () => {
    const rendered = renderHtml(TraceCompare.view(route, { _tag: 'LoggedOut' }))
    expect(rendered).toContain('data-component="trace-compare-table"')
    expect(rendered).toContain('Unknown id')
    // The unknown column links nothing + invents no metrics (em dashes only).
    expect(rendered).toContain('1 unknown')
  })
})

describe('trace-compare render (empty id list)', () => {
  const route = TraceCompareRoute({ ids: ' , , ' })

  test('renders an honest empty state, not an empty table', () => {
    const rendered = renderHtml(TraceCompare.view(route, { _tag: 'LoggedOut' }))
    expect(rendered).toContain('data-component="trace-compare-empty"')
    expect(rendered).toContain('Nothing to compare')
    expect(rendered).not.toContain('data-component="trace-compare-table"')
  })

  test('uses a generic document title for an empty list', () => {
    expect(TraceCompare.title(route)).toBe('Trace comparison - OpenAgents')
  })
})

describe('comparison model (pure math)', () => {
  test('parseCompareIds de-dupes + preserves order + tolerates separators', () => {
    expect(parseCompareIds('a,b,c')).toEqual(['a', 'b', 'c'])
    expect(parseCompareIds('a, a , b')).toEqual(['a', 'b'])
    expect(parseCompareIds('a+b c')).toEqual(['a', 'b', 'c'])
    expect(parseCompareIds(' , , ')).toEqual([])
  })

  test('the two extra sample variants decode against the pinned contract', () => {
    expect(mcpOnTrajectory.schema_version).toBe('ATIF-v1.7')
    expect(noWaitForTrajectory.schema_version).toBe('ATIF-v1.7')
  })

  test('baseline is the first found trace; deltas are relative to it', () => {
    const c = buildComparison(
      [
        SAMPLE_COMPARE_BASELINE_UUID,
        SAMPLE_COMPARE_MCP_ON_UUID,
        SAMPLE_COMPARE_NO_WAITFOR_UUID,
      ],
      lookupTrajectoryForCompare,
    )
    expect(c.baselineUuid).toBe(SAMPLE_COMPARE_BASELINE_UUID)
    expect(c.foundCount).toBe(3)
    expect(c.unknownCount).toBe(0)

    const baseline = c.traces[0]
    expect(baseline?.found && baseline.isBaseline).toBe(true)

    const byUuid = new Map(c.deltas.map(d => [d.uuid, d]))
    // The baseline's own deltas are all 0.
    const baseDelta = byUuid.get(SAMPLE_COMPARE_BASELINE_UUID)
    expect(baseDelta?.durationDeltaMs).toBe(0)
    expect(baseDelta?.stepCountDelta).toBe(0)

    // mcp-on: one more agent step than the baseline (6 vs 5) + slower.
    const mcp = byUuid.get(SAMPLE_COMPARE_MCP_ON_UUID)
    expect(mcp?.stepCountDelta).toBe(1)
    expect(isMeasured(mcp!.durationDeltaMs) && mcp!.durationDeltaMs > 0).toBe(
      true,
    )
    // no-waitfor: pass regression (REFUTED vs PASS) -> passDelta < 0.
    const noWait = byUuid.get(SAMPLE_COMPARE_NO_WAITFOR_UUID)
    expect(isMeasured(noWait!.passDelta) && noWait!.passDelta < 0).toBe(true)
  })

  test('an unknown id resolves to a not-found cell with not_measured deltas', () => {
    const c = buildComparison(
      [SAMPLE_COMPARE_BASELINE_UUID, 'nope-0000'],
      lookupTrajectoryForCompare,
    )
    expect(c.foundCount).toBe(1)
    expect(c.unknownCount).toBe(1)
    const unknown = c.traces[1]
    expect(unknown?.found).toBe(false)
    const delta = c.deltas.find(d => d.uuid === 'nope-0000')
    expect(delta?.durationDeltaMs).toBe('not_measured')
    expect(delta?.passDelta).toBe('not_measured')
  })

  test('an all-unknown list has no baseline + no fabricated numbers', () => {
    const c = buildComparison(['x', 'y'], () => undefined)
    expect(c.baselineUuid).toBeUndefined()
    expect(c.foundCount).toBe(0)
    for (const d of c.deltas) expect(d.durationDeltaMs).toBe('not_measured')
  })
})
