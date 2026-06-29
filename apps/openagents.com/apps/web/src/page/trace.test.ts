// Public `/trace/{uuid}` render tests (issue #6209). Pure: we parse the route,
// confirm the public (no-auth-bootstrap) startup posture via `init`, and walk
// the foldkit VNode tree to a string to assert the rendered markup — the header,
// the step timeline (tool calls + observations + deep-link anchors), the
// embedded video, the final metrics, and the honest unknown-uuid 404 + skeleton.

import { Option } from 'effect'
import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { Flags, init } from '../main'
import { TraceRoute, urlToAppRoute } from '../route'
import * as Trace from './trace'
import {
  decodeTrajectory,
  formatCost,
  formatDuration,
  trajectoryToMarkdown,
  traceVerdict,
} from './trace/atif'
import { SAMPLE_TRACE_UUID, sampleTrajectory } from './trace/sample'

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

const sampleRoute = TraceRoute({ uuid: SAMPLE_TRACE_UUID })
const renderSample = (): string =>
  renderHtml(Trace.view(sampleRoute, { _tag: 'LoggedOut' }))

describe('trace route', () => {
  test('parses /trace/{uuid} and captures the uuid', () => {
    const route = urlToAppRoute(appUrl(`/trace/${SAMPLE_TRACE_UUID}`))
    expect(route).toEqual(TraceRoute({ uuid: SAMPLE_TRACE_UUID }))
  })

  test('is public: no auth bootstrap command is dispatched', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl(`/trace/${SAMPLE_TRACE_UUID}`),
    )
    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Trace', uuid: SAMPLE_TRACE_UUID },
    })
    expect(commands).toHaveLength(0)
  })
})

describe('trace render (sample trajectory)', () => {
  test('renders the page shell with the public header', () => {
    const rendered = renderSample()
    expect(rendered).toContain('data-component="trace-page"')
    // The shared public header is present (auth-aware shell).
    expect(rendered).toContain('header')
  })

  test('renders the header: agent, model, verdict, duration, cost', () => {
    const rendered = renderSample()
    expect(rendered).toContain('openagents-qa-runner') // agent
    expect(rendered).toContain('openagents/khala') // model
    expect(rendered).toContain('Verified') // PASS verdict label
    expect(rendered).toContain(formatDuration(11459)) // 11.5s
    expect(rendered).toContain(formatCost(0)) // $0.00
  })

  test('renders the goal node and a step timeline', () => {
    const rendered = renderSample()
    expect(rendered).toContain('data-component="trace-timeline"')
    expect(rendered).toContain('Goal')
    expect(rendered).toContain('Verify the login page works on this site')
    // Each step is its own timeline node.
    expect(rendered).toContain('Step 2')
    expect(rendered).toContain('Step 7')
  })

  test('renders tool calls with their function name + arguments', () => {
    const rendered = renderSample()
    expect(rendered).toContain('data-component="trace-tool-call"')
    expect(rendered).toContain('navigate()')
    expect(rendered).toContain('waitFor()')
    expect(rendered).toContain('assert()')
    expect(rendered).toContain('done()')
    // Public-safe argument values are rendered.
    expect(rendered).toContain('/login')
  })

  test('renders observation results', () => {
    const rendered = renderSample()
    expect(rendered).toContain('data-component="trace-observation"')
    expect(rendered).toContain('ok: navigate to /login')
    expect(rendered).toContain('verification_class=test_passed')
    // The observation is correlated back to its tool call.
    expect(rendered).toContain('call_2')
  })

  test('renders collapsible reasoning', () => {
    const rendered = renderSample()
    expect(rendered).toContain('data-component="trace-reasoning"')
    expect(rendered).toContain('Reasoning')
    expect(rendered).toContain('the first action is to navigate')
  })

  test('renders the embedded video', () => {
    const rendered = renderSample()
    expect(rendered).toContain('data-component="pro-video-pane"')
    expect(rendered).toContain('/pro-assets/sample-session.webm')
  })

  test('renders the final metrics', () => {
    const rendered = renderSample()
    expect(rendered).toContain('data-component="trace-final-metrics"')
    expect(rendered).toContain('Verdict')
    expect(rendered).toContain('Steps')
    expect(rendered).toContain('Duration')
  })

  test('gives each step a stable deep-link anchor + copy affordance', () => {
    const rendered = renderSample()
    expect(rendered).toContain('id="goal"')
    expect(rendered).toContain('id="step-2"')
    expect(rendered).toContain('id="step-7"')
    expect(rendered).toContain('Copy link to this step')
    // On-mount hash scroll is wired so a `#step-N` deep link lands on the step.
    expect(rendered).toContain('location.hash')
  })

  test('produces a shareable document title', () => {
    expect(Trace.title(sampleRoute)).toBe(
      'Trace: openagents.com (Verified) - OpenAgents',
    )
  })

  test('renders a "Copy Markdown" button carrying the serialized transcript (#6223)', () => {
    const rendered = renderSample()
    expect(rendered).toContain('data-component="trace-copy-markdown"')
    expect(rendered).toContain('Copy Markdown')
    // The serialized markdown is embedded as a data attribute (no fetch/state).
    expect(rendered).toContain('data-markdown=')
    expect(rendered).toContain('# Agent session trace')
  })

  test('lays the header meta out as a full-width strip (#6223)', () => {
    const rendered = renderSample()
    expect(rendered).toContain('data-component="trace-meta"')
    // A flex strip (full-width row), not the old narrow stacked grid column.
    expect(rendered).toContain('flex flex-wrap items-stretch')
  })
})

describe('trajectory markdown serialization (#6223)', () => {
  test('serializes goal, steps, tool calls, and observations to clean Markdown', () => {
    const md = trajectoryToMarkdown(sampleTrajectory)
    expect(md).toContain('# Agent session trace')
    expect(md).toContain('## Goal')
    expect(md).toContain('Verify the login page works on this site')
    expect(md).toContain('## Step 2')
    // Tool calls render as fenced function(args) blocks.
    expect(md).toContain('```')
    expect(md).toContain('navigate(')
    expect(md).toContain('target: /login')
    // Observations render as quoted evidence.
    expect(md).toContain('> ok: navigate to /login')
    // No raw object dumps for string args.
    expect(md).not.toContain('[object Object]')
  })
})

describe('trace media via envelope blobRefs (#6223)', () => {
  const realRoute = TraceRoute({ uuid: '24c6fea6-b271-46c6-a9a9-bc614440e9ef' })

  test('renders the recording from a blobRef at the gated blob URL', () => {
    const rendered = renderHtml(
      Trace.view(realRoute, { _tag: 'LoggedOut' }, {
        _tag: 'TraceLoaded',
        uuid: realRoute.uuid,
        trajectory: sampleTrajectory,
        blobRefs: [{ kind: 'video', r2Key: 'session.mp4' }],
      }),
    )
    expect(rendered).toContain('data-component="pro-video-pane"')
    expect(rendered).toContain(
      `/api/traces/${realRoute.uuid}/blob/session.mp4`,
    )
  })

  test('renders screenshots inline from blobRefs', () => {
    const rendered = renderHtml(
      Trace.view(realRoute, { _tag: 'LoggedOut' }, {
        _tag: 'TraceLoaded',
        uuid: realRoute.uuid,
        trajectory: sampleTrajectory,
        blobRefs: [
          { kind: 'screenshot', r2Key: 'shots/00-login.png', caption: 'login' },
        ],
      }),
    )
    expect(rendered).toContain('data-component="trace-screenshots"')
    expect(rendered).toContain(
      `/api/traces/${realRoute.uuid}/blob/shots/00-login.png`,
    )
    expect(rendered).toContain('login')
  })

  test('omits the screenshots section when there are no image blobRefs', () => {
    const rendered = renderHtml(
      Trace.view(realRoute, { _tag: 'LoggedOut' }, {
        _tag: 'TraceLoaded',
        uuid: realRoute.uuid,
        trajectory: sampleTrajectory,
        blobRefs: [{ kind: 'video', r2Key: 'session.mp4' }],
      }),
    )
    expect(rendered).not.toContain('data-component="trace-screenshots"')
  })
})

describe('trace render (unknown uuid, no live state)', () => {
  const unknownRoute = TraceRoute({ uuid: 'does-not-exist-0000' })

  test('falls back to the committed sample lookup (only the sample matches) → not-found body', () => {
    const rendered = renderHtml(
      Trace.view(unknownRoute, { _tag: 'LoggedOut' }),
    )
    expect(rendered).toContain('data-component="trace-not-found"')
    expect(rendered).toContain('No trace at this link')
    expect(rendered).not.toContain('data-component="trace-timeline"')
  })

  test('a real uuid with no live state yet gets the neutral title (it may still be loading)', () => {
    expect(Trace.title(unknownRoute)).toBe('Trace - OpenAgents')
  })
})

describe('trace render (live read-API load state)', () => {
  const realRoute = TraceRoute({ uuid: '24c6fea6-b271-46c6-a9a9-bc614440e9ef' })
  const render = (loadState: Trace.TraceLoadState): string =>
    renderHtml(Trace.view(realRoute, { _tag: 'LoggedOut' }, loadState))

  test('loading state renders the bounded skeleton (not the not-found body)', () => {
    const rendered = render({
      _tag: 'TraceLoading',
      uuid: realRoute.uuid,
    })
    expect(rendered).toContain('data-component="trace-skeleton"')
    expect(rendered).toContain('aria-busy="true"')
    expect(rendered).not.toContain('data-component="trace-not-found"')
    expect(rendered).not.toContain('data-component="trace-timeline"')
  })

  test('a loaded trajectory renders the full timeline (not the not-found body)', () => {
    const rendered = render({
      _tag: 'TraceLoaded',
      uuid: realRoute.uuid,
      trajectory: sampleTrajectory,
    })
    expect(rendered).toContain('data-component="trace-page"')
    expect(rendered).toContain('data-component="trace-timeline"')
    expect(rendered).toContain('Step 2')
    expect(rendered).toContain('Verify the login page works on this site')
    expect(rendered).not.toContain('data-component="trace-not-found"')
  })

  test('a 404 not-found state renders the honest not-found body', () => {
    const rendered = render({
      _tag: 'TraceNotFound',
      uuid: realRoute.uuid,
    })
    expect(rendered).toContain('data-component="trace-not-found"')
    expect(rendered).toContain('No trace at this link')
    expect(rendered).not.toContain('data-component="trace-timeline"')
  })

  test('a non-404 failure renders the same honest not-found body', () => {
    const rendered = render({
      _tag: 'TraceFailed',
      uuid: realRoute.uuid,
      error: 'network down',
    })
    expect(rendered).toContain('data-component="trace-not-found"')
    expect(rendered).not.toContain('data-component="trace-timeline"')
  })

  test('a stale load state for a different uuid is ignored (falls back to sample lookup → not-found)', () => {
    const rendered = renderHtml(
      Trace.view(realRoute, { _tag: 'LoggedOut' }, {
        _tag: 'TraceLoaded',
        uuid: 'a-different-uuid',
        trajectory: sampleTrajectory,
      }),
    )
    expect(rendered).toContain('data-component="trace-not-found"')
    expect(rendered).not.toContain('data-component="trace-timeline"')
  })

  test('a loaded trajectory drives a shareable, honest document title', () => {
    expect(
      Trace.title(realRoute, {
        _tag: 'TraceLoaded',
        uuid: realRoute.uuid,
        trajectory: sampleTrajectory,
      }),
    ).toBe('Trace: openagents.com (Verified) - OpenAgents')
  })

  test('a not-found load state drives the not-found document title', () => {
    expect(
      Trace.title(realRoute, { _tag: 'TraceNotFound', uuid: realRoute.uuid }),
    ).toBe('Trace not found - OpenAgents')
  })

  test('the committed sample uuid always renders the sample, ignoring any load state', () => {
    const rendered = renderHtml(
      Trace.view(sampleRoute, { _tag: 'LoggedOut' }, {
        _tag: 'TraceNotFound',
        uuid: SAMPLE_TRACE_UUID,
      }),
    )
    expect(rendered).toContain('data-component="trace-timeline"')
    expect(rendered).not.toContain('data-component="trace-not-found"')
  })
})

describe('trace loading skeleton', () => {
  test('renders a bounded skeleton state', () => {
    const rendered = renderHtml(Trace.skeletonArticle())
    expect(rendered).toContain('data-component="trace-skeleton"')
    expect(rendered).toContain('animate-pulse')
    expect(rendered).toContain('aria-busy="true"')
  })
})

describe('clamped text (long reasoning / observations)', () => {
  test('short content renders inline with no show-more affordance', () => {
    const short = 'ok: navigate to /login'
    const rendered = renderHtml(
      Trace.clampedText<unknown>(short, 'text-sm text-white/75'),
    )
    expect(rendered).toContain(short)
    expect(rendered).not.toContain('Show more')
    expect(rendered).not.toContain('data-component="trace-clamp"')
  })

  test('long content clamps with a stateless show-more / show-less toggle', () => {
    const long = 'FAILED '.padEnd(Trace.CLAMP_THRESHOLD + 50, 'x')
    const rendered = renderHtml(
      Trace.clampedText<unknown>(long, 'text-sm text-[#d32f2f]'),
    )
    // The full content is still present (clamp is visual, not truncation).
    expect(rendered).toContain(long)
    expect(rendered).toContain('data-component="trace-clamp"')
    // The checkbox-hack toggle: a peer checkbox + line-clamp + both labels.
    expect(rendered).toContain('type="checkbox"')
    expect(rendered).toContain('line-clamp-4')
    expect(rendered).toContain('peer-checked:line-clamp-none')
    expect(rendered).toContain('Show more')
    expect(rendered).toContain('Show less')
  })

  test('the toggle label is wired to the checkbox by a stable id', () => {
    const long = 'reasoning '.padEnd(Trace.CLAMP_THRESHOLD + 80, 'y')
    const rendered = renderHtml(
      Trace.clampedText<unknown>(long, 'text-sm text-white/65'),
    )
    const idMatch = /id="(clamp-[a-z0-9]+)"/.exec(rendered)
    expect(idMatch).not.toBeNull()
    if (idMatch !== null) {
      expect(rendered).toContain(`for="${idMatch[1]}"`)
    }
  })
})

describe('atif contract + committed sample', () => {
  test('the committed sample decodes against the pinned Trajectory contract', () => {
    expect(sampleTrajectory.schema_version).toBe('ATIF-v1.7')
    expect(sampleTrajectory.steps.length).toBe(7)
    expect(sampleTrajectory.steps[0]?.source).toBe('user')
  })

  test('derives the verdict from final_metrics.extra', () => {
    expect(traceVerdict(sampleTrajectory)).toBe('PASS')
  })

  test('falls back to the terminal done tool call when final_metrics.extra is stripped', () => {
    // The ingest can drop `final_metrics.extra` on storage; the verdict still
    // survives as the terminal `done` tool call's argument. The badge must read
    // "Verified", not "In progress".
    const stripped = decodeTrajectory(
      JSON.parse(
        JSON.stringify({
          ...sampleTrajectory,
          final_metrics: {
            ...sampleTrajectory.final_metrics,
            extra: {},
          },
          steps: [
            ...sampleTrajectory.steps,
            {
              step_id: sampleTrajectory.steps.length + 1,
              timestamp: '2026-06-24T20:18:21.285Z',
              source: 'agent',
              message: 'Goal verified. Marking the session complete.',
              model_name: 'openagents/khala',
              tool_calls: [
                {
                  tool_call_id: 'call_done',
                  function_name: 'done',
                  arguments: { verdict: 'PASS', summary: 'Verified (9 actions).' },
                },
              ],
              observation: {
                results: [
                  { source_call_id: 'call_done', content: 'verification_class=test_passed' },
                ],
              },
              metrics: { cost_usd: 0 },
            },
          ],
        }),
      ),
    )
    expect(traceVerdict(stripped)).toBe('PASS')
  })

  test('re-decoding a serialized sample is stable (read-API parity)', () => {
    // A round-trip through JSON mirrors what the worker read API will serve:
    // the page consumes a decoded `Trajectory`, regardless of the source.
    const roundTripped = decodeTrajectory(
      JSON.parse(JSON.stringify(sampleTrajectory)),
    )
    expect(roundTripped.trajectory_id).toBe(sampleTrajectory.trajectory_id)
    expect(roundTripped.steps.length).toBe(sampleTrajectory.steps.length)
  })

  test('formats durations and cost honestly', () => {
    expect(formatDuration(11459)).toBe('11.5s')
    expect(formatDuration(500)).toBe('500ms')
    expect(formatDuration(64000)).toBe('1m 4s')
    expect(formatCost(0)).toBe('$0.00')
    expect(formatCost(0.0012)).toBe('$0.0012')
    expect(formatCost(1.5)).toBe('$1.50')
  })
})
