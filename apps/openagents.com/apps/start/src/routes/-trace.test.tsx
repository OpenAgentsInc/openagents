import { decodeAtifTrajectorySync } from '@openagentsinc/atif/trace'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import {
  TraceBlobRef,
  TraceProjection,
  traceBlobUrl,
  traceProjectionUrl,
  traceReadToken,
} from './-trace-fetch'
import {
  TraceFailedView,
  TraceLoadedView,
  TracePage,
  traceSurfaceView,
  traceVerdict,
  trajectoryToMarkdown,
} from './-trace-page'

const trajectory = decodeAtifTrajectorySync({
  schema_version: 'ATIF-v1.7',
  trajectory_id: 'trajectory.public.fixture',
  session_id: 'session.public.fixture',
  agent: {
    name: 'openagents-qa-runner',
    version: '0.1.0',
    model_name: 'openagents/khala',
  },
  steps: [
    {
      step_id: 1,
      timestamp: '2026-07-18T12:00:00.000Z',
      source: 'user',
      message: 'Verify the login page renders.',
    },
    {
      step_id: 2,
      timestamp: '2026-07-18T12:00:02.000Z',
      source: 'agent',
      message: 'Open the login page.',
      reasoning_content: 'The route must stay visible after navigation.',
      model_name: 'openagents/khala',
      tool_calls: [{
        tool_call_id: 'call.navigate',
        function_name: 'navigate',
        arguments: { target: '/login' },
      }],
      observation: {
        results: [{ source_call_id: 'call.navigate', content: 'ok: /login' }],
      },
      metrics: { prompt_tokens: 12, completion_tokens: 8, cost_usd: 0 },
    },
    {
      step_id: 3,
      timestamp: '2026-07-18T12:00:04.000Z',
      source: 'agent',
      message: 'The goal is verified.',
      tool_calls: [{
        tool_call_id: 'call.done',
        function_name: 'done',
        arguments: { verdict: 'PASS', summary: 'Verified the login route.' },
      }],
      observation: {
        results: [{ source_call_id: 'call.done', content: 'verification_class=test_passed' }],
      },
    },
  ],
  final_metrics: {
    total_prompt_tokens: 12,
    total_completion_tokens: 8,
    total_cost_usd: 0,
    total_steps: 3,
  },
})

const projection = new TraceProjection({
  uuid: '448644bd-f2ce-4ad4-bfad-e4e898ed12ef',
  schemaVersion: 'ATIF-v1.7',
  trajectoryId: trajectory.trajectory_id,
  ...(trajectory.session_id === undefined ? {} : { sessionId: trajectory.session_id }),
  visibility: 'public',
  agentRef: 'agent:qa-fixture',
  stepCount: trajectory.steps.length,
  trajectory,
  blobRefs: [
    new TraceBlobRef({
      kind: 'video',
      r2Key: 'session.mp4',
      contentType: 'video/mp4',
      caption: 'Session recording',
    }),
    new TraceBlobRef({
      kind: 'screenshot',
      r2Key: 'shots/login page.png',
      contentType: 'image/png',
      caption: 'Login page',
    }),
  ],
  createdAt: '2026-07-18T12:00:04.000Z',
  dataMarket: {
    trainingConsent: false,
    uploadSource: 'agent',
    reward: { eligible: false, amountSats: null, status: 'tbd' },
  },
  authority: {
    acceptedWorkAuthority: false,
    payoutAuthority: false,
    publicClaimAuthority: false,
  },
})

describe('Start /trace/$traceUuid route', () => {
  test('server-renders a bounded loading state before the live read', () => {
    const html = renderToStaticMarkup(<TracePage traceUuid={projection.uuid} />)
    expect(html).toContain('data-route="trace"')
    expect(html).toContain('data-component="trace-skeleton"')
    expect(html).toContain('aria-busy="true"')
  })

  test('renders the former evidence hierarchy: verdict, goal, timeline, tools, and media', () => {
    const view = traceSurfaceView({
      tag: 'loaded',
      projection,
      origin: 'https://openagents.com',
      copied: false,
      scrollToKey: 'step-2',
    })
    const serialized = JSON.stringify(view)
    expect(serialized).toContain('"_tag":"Transcript"')
    expect(serialized).toContain('"key":"trace-timeline"')
    expect(serialized).toContain('"scrollToKey":"step-2"')
    expect(serialized).toContain('Verified the login route.')
    expect(serialized).toContain('Verified')
    expect(serialized).toContain('Verify the login page renders.')
    expect(serialized).toContain('"key":"step-2"')
    expect(serialized).toContain('"_tag":"CodeBlock"')
    expect(serialized).toContain('navigate')
    expect(serialized).toContain('ok: /login')
    expect(serialized).toContain('"_tag":"Image"')
    expect(serialized).toContain('/blob/shots/login%20page.png')

    const html = renderToStaticMarkup(<TraceLoadedView projection={projection} />)
    expect(html).toContain('data-component="trace-page"')
    expect(html).toContain('data-trace-effect-native-root')
    expect(html).toContain('data-component="trace-recording"')
    expect(html).toContain('/blob/session.mp4')
  })

  test('keeps the trace explicitly evidence-only', () => {
    const serialized = JSON.stringify(traceSurfaceView({
      tag: 'loaded',
      projection,
      origin: 'https://openagents.com',
      copied: false,
    }))
    expect(serialized).toContain('grants no accepted-work, payout, or public-claim authority')
  })

  test('renders an honest not-found state without leaking private existence', () => {
    const html = renderToStaticMarkup(<TraceFailedView status={404} />)
    expect(html).toContain('data-component="trace-not-found"')
    const serialized = JSON.stringify(traceSurfaceView({ tag: 'failed', status: 404 }))
    expect(serialized).toContain('No trace at this link')
    expect(serialized).toContain('does not exist, is private, or is no longer available')
  })
})

describe('trace projection helpers', () => {
  test('derives the terminal verdict and copyable Markdown', () => {
    expect(traceVerdict(trajectory)).toBe('PASS')
    const markdown = trajectoryToMarkdown(trajectory)
    expect(markdown).toContain('# Agent session trace')
    expect(markdown).toContain('## Goal')
    expect(markdown).toContain('## Step 2')
    expect(markdown).toContain('"tool": "navigate"')
  })

  test('preserves only the explicit owner read token across JSON and media reads', () => {
    expect(traceReadToken('?token=oa_agent_fixture&utm_source=ignored')).toBe('oa_agent_fixture')
    expect(traceReadToken('?utm_source=ignored')).toBeUndefined()
    expect(traceProjectionUrl('trace/unsafe', 'owner token')).toBe(
      '/api/traces/trace%2Funsafe?token=owner%20token',
    )
    expect(traceBlobUrl('trace.fixture', 'shots/a b.png', 'owner token')).toBe(
      '/api/traces/trace.fixture/blob/shots/a%20b.png?token=owner%20token',
    )
  })
})
