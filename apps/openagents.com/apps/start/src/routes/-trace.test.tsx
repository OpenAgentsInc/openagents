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

  test('renders the evidence hierarchy through the shared desktop-workbench cards', () => {
    const html = renderToStaticMarkup(<TraceLoadedView projection={projection} />)
    // Parity (#9061): the timeline renders through the SAME desktop-workbench
    // components desktop uses — `oa-react-*` classes, not generic JSON boxes.
    expect(html).toContain('data-component="trace-page"')
    expect(html).toContain('data-component="trace-timeline"')
    expect(html).toContain('oa-react-')
    // Verdict, goal, and the terminal summary title.
    expect(html).toContain('Verified')
    expect(html).toContain('Verify the login page renders.')
    expect(html).toContain('Verified the login route.')
    // The `navigate` tool call and its observation both render.
    expect(html).toContain('navigate')
    expect(html).toContain('ok: /login')
    // Media: screenshot + recording.
    expect(html).toContain('/blob/shots/login%20page.png')
    expect(html).toContain('data-component="trace-recording"')
    expect(html).toContain('/blob/session.mp4')
  })

  test('keeps the trace explicitly evidence-only', () => {
    const html = renderToStaticMarkup(<TraceLoadedView projection={projection} />)
    expect(html).toContain('grants no accepted-work, payout, or public-claim authority')
  })

  test('renders an honest not-found state without leaking private existence', () => {
    const html = renderToStaticMarkup(<TraceFailedView status={404} />)
    expect(html).toContain('data-component="trace-not-found"')
    expect(html).toContain('No trace at this link')
    expect(html).toContain('does not exist, is private, or is no longer available')
  })
})

const toolTrajectory = decodeAtifTrajectorySync({
  schema_version: 'ATIF-v1.7',
  trajectory_id: 'trajectory.tools.fixture',
  agent: { name: 'claude-code', version: '2.1.0', model_name: 'openagents/khala' },
  steps: [
    { step_id: 1, source: 'user', message: 'Do the work.' },
    {
      step_id: 2,
      source: 'agent',
      message: '',
      tool_calls: [{
        tool_call_id: 'c-bash',
        function_name: 'Bash',
        arguments: { command: 'ls -la', description: 'list files' },
      }],
      observation: { results: [{ source_call_id: 'c-bash', content: 'total 0' }] },
    },
    {
      step_id: 3,
      source: 'agent',
      message: '',
      tool_calls: [{
        tool_call_id: 'c-read',
        function_name: 'Read',
        arguments: { file_path: 'src/x.ts' },
      }],
      observation: { results: [{ source_call_id: 'c-read', content: 'export const x = 1' }] },
    },
    {
      step_id: 4,
      source: 'agent',
      message: '',
      tool_calls: [{
        tool_call_id: 'c-edit',
        function_name: 'Edit',
        arguments: { file_path: 'src/x.ts', old_string: 'const x = 1', new_string: 'const x = 2' },
      }],
    },
    {
      step_id: 5,
      source: 'agent',
      message: '',
      tool_calls: [{
        tool_call_id: 'c-write',
        function_name: 'Write',
        arguments: { file_path: 'src/y.ts', content: 'line one\nline two' },
      }],
    },
    {
      step_id: 6,
      source: 'agent',
      message: '',
      tool_calls: [{
        tool_call_id: 'c-agent',
        function_name: 'Agent',
        arguments: { subagent_type: 'Explore', description: 'find the config', prompt: 'search the repo' },
      }],
      observation: { results: [{ source_call_id: 'c-agent', content: 'Found: config at root' }] },
    },
  ],
})

const toolProjection = new TraceProjection({
  uuid: 'tool-fixture',
  schemaVersion: 'ATIF-v1.7',
  trajectoryId: toolTrajectory.trajectory_id,
  visibility: 'public',
  agentRef: 'agent:tools',
  stepCount: toolTrajectory.steps.length,
  trajectory: toolTrajectory,
  blobRefs: [],
  createdAt: '2026-07-19T00:00:00.000Z',
  dataMarket: { trainingConsent: false, uploadSource: 'agent', reward: { eligible: false, amountSats: null, status: 'tbd' } },
  authority: { acceptedWorkAuthority: false, payoutAuthority: false, publicClaimAuthority: false },
})

describe('per-tool desktop-workbench parity (#9061)', () => {
  const html = renderToStaticMarkup(<TraceLoadedView projection={toolProjection} />)

  test('Bash renders the command card with its output', () => {
    expect(html).toContain('oa-react-command-output')
    expect(html).toContain('ls -la')
    expect(html).toContain('total 0')
  })

  test('Read renders a tool-call card with args and result', () => {
    expect(html).toContain('oa-react-tool-args')
    expect(html).toContain('oa-react-tool-result')
    expect(html).toContain('src/x.ts')
    expect(html).toContain('export const x = 1')
  })

  test('Edit and Write render colorized file-change diffs', () => {
    expect(html).toContain('data-diff-line')
    expect(html).toContain('const x = 2')
    expect(html).toContain('line one')
  })

  test('Agent renders the sub-agent card and its returned report', () => {
    expect(html).toContain('oa-react-agent-group')
    expect(html).toContain('Explore')
    expect(html).toContain('search the repo')
    expect(html).toContain('Found: config at root')
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
