// Committed public-safe sample trajectory for the `/trace/{uuid}` page.
//
// This is the REAL ATIF-v1.7 trajectory emitted by the qa-runner from a live
// Khala `/login` computer-use run, lifted verbatim from
// `apps/qa-runner/samples/login-trace/trajectory.json` (the sibling ATIF-emitter
// lane). It is committed here so the page renders a real, beautiful trace today,
// against the exact shape the worker read API will serve.
//
// INTEGRATION (the wiring step, NOT done here): a sibling lane is building the
// read API (`GET` a stored trajectory by uuid). When that lands, the page's
// `lookupTrajectory(uuid)` swaps this committed lookup for an API fetch + decode
// against `./atif`. The render does not change — it already consumes a decoded
// `Trajectory`.

import { type Trajectory, decodeTrajectory } from './atif'

// The canonical sample uuid. `/trace/<this>` renders the login trace; any other
// uuid 404s honestly (no real ingest store wired yet).
export const SAMPLE_TRACE_UUID = '0e08d2db-2026-4624-9a39-f1efe8000001'

const SAMPLE_TRAJECTORY_JSON = {
  schema_version: 'ATIF-v1.7',
  session_id: 'login-trace',
  trajectory_id: 'login-trace-trajectory',
  agent: {
    name: 'openagents-qa-runner',
    version: '0.1.0',
    model_name: 'openagents/khala',
    extra: {
      driver: 'khala-computer-use',
      target: 'openagents.com',
    },
  },
  notes:
    'Emitted by the OpenAgents qa-runner ATIF mapper from a real Khala ' +
    'computer-use run (result.json + session-trace.json). Token counts are ' +
    'withheld for public safety; own-infra cost is $0.',
  steps: [
    {
      step_id: 1,
      timestamp: '2026-06-24T14:39:44.732Z',
      source: 'user',
      message:
        'Verify the login page works on this site: open /login, confirm the sign-in form renders (the page shows "Log in to OpenAgents"), and confirm it does NOT redirect to the homepage (the URL still includes /login). Assert each of these, screenshot the login page, then finish.',
    },
    {
      step_id: 2,
      timestamp: '2026-06-24T14:39:44.732Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: 'navigate to /login',
      reasoning_content:
        'The goal starts at the login page, so the first action is to navigate the browser to /login on the target deployment before asserting anything.',
      tool_calls: [
        {
          tool_call_id: 'call_2',
          function_name: 'navigate',
          arguments: {
            action: 'navigate',
            target: '/login',
            narration: 'navigate to /login',
          },
        },
      ],
      observation: {
        results: [
          {
            source_call_id: 'call_2',
            content: 'ok: navigate to /login',
          },
        ],
      },
      metrics: {
        cost_usd: 0,
      },
    },
    {
      step_id: 3,
      timestamp: '2026-06-24T14:39:47.118Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: 'wait for {"kind":"text-visible","value":"Log in to OpenAgents"}',
      reasoning_content:
        'Navigation is async, so before asserting I wait for the heading text "Log in to OpenAgents" to be visible — that confirms the sign-in form actually rendered rather than a blank or redirected page.',
      tool_calls: [
        {
          tool_call_id: 'call_3',
          function_name: 'waitFor',
          arguments: {
            action: 'wait',
            target: 'text-visible:Log in to OpenAgents',
            narration:
              'wait for {"kind":"text-visible","value":"Log in to OpenAgents"}',
          },
        },
      ],
      observation: {
        results: [
          {
            source_call_id: 'call_3',
            content: 'ok: wait for {"kind":"text-visible","value":"Log in to OpenAgents"}',
          },
        ],
      },
      metrics: {
        cost_usd: 0,
      },
    },
    {
      step_id: 4,
      timestamp: '2026-06-24T14:39:48.402Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: "Login page shows 'Log in to OpenAgents'",
      reasoning_content:
        'The heading is visible, so I assert the first outcome from the goal: the login page shows the expected "Log in to OpenAgents" copy.',
      tool_calls: [
        {
          tool_call_id: 'call_4',
          function_name: 'assert',
          arguments: {
            action: 'assert',
            target: "Login page shows 'Log in to OpenAgents'",
            narration: "Login page shows 'Log in to OpenAgents'",
          },
        },
      ],
      observation: {
        results: [
          {
            source_call_id: 'call_4',
            content: "ok: Login page shows 'Log in to OpenAgents'",
          },
        ],
      },
      metrics: {
        cost_usd: 0,
      },
    },
    {
      step_id: 5,
      timestamp: '2026-06-24T14:39:50.913Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: 'URL still includes /login',
      reasoning_content:
        'The second outcome is that the page did NOT bounce to the homepage. I assert the current URL still contains /login, which rules out the redirect regression.',
      tool_calls: [
        {
          tool_call_id: 'call_5',
          function_name: 'assert',
          arguments: {
            action: 'assert',
            target: 'URL still includes /login',
            narration: 'URL still includes /login',
          },
        },
      ],
      observation: {
        results: [
          {
            source_call_id: 'call_5',
            content: 'ok: URL still includes /login',
          },
        ],
      },
      metrics: {
        cost_usd: 0,
      },
    },
    {
      step_id: 6,
      timestamp: '2026-06-24T14:39:53.640Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: 'screenshot login-page',
      reasoning_content:
        'Both assertions passed. I capture a screenshot of the rendered login page as visual evidence to attach to the trace before finishing.',
      tool_calls: [
        {
          tool_call_id: 'call_6',
          function_name: 'screenshot',
          arguments: {
            action: 'screenshot',
            target: 'login-page',
            narration: 'screenshot login-page',
          },
        },
      ],
      observation: {
        results: [
          {
            source_call_id: 'call_6',
            content: 'ok: screenshot login-page',
          },
        ],
      },
      metrics: {
        cost_usd: 0,
      },
    },
    {
      step_id: 7,
      timestamp: '2026-06-24T14:39:56.191Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: 'Goal verified. Marking the session complete.',
      reasoning_content: 'Final verdict PASS after 5 action(s).',
      tool_calls: [
        {
          tool_call_id: 'call_7',
          function_name: 'done',
          arguments: {
            verdict: 'PASS',
            summary: 'Verified openagents.com (5 actions).',
          },
        },
      ],
      observation: {
        results: [
          {
            source_call_id: 'call_7',
            content: 'verification_class=test_passed',
          },
        ],
      },
      metrics: {
        cost_usd: 0,
      },
    },
  ],
  final_metrics: {
    total_prompt_tokens: 0,
    total_completion_tokens: 0,
    total_cached_tokens: 0,
    total_cost_usd: 0,
    total_steps: 7,
    extra: {
      verdict: 'PASS',
      duration_ms: 11459,
      trace_digest:
        '84953e17b368f7e05288c0001c60cafb6b1ca92419f223219cbd7a5d63629fa2',
    },
  },
  extra: {
    target: {
      name: 'openagents.com',
      baseUrl: 'https://openagents.com',
    },
    artifacts: {
      video: 'session.mp4',
      screenshots: ['00-login-page.png'],
    },
  },
} as const

// Decode once at module load so the committed sample is validated against the
// pinned `Trajectory` contract (a drift between the emitter shape and the render
// contract fails fast here, in tests, rather than silently mis-rendering).
export const sampleTrajectory: Trajectory =
  decodeTrajectory(SAMPLE_TRAJECTORY_JSON)

// The committed lookup. The ONLY known uuid is the sample; everything else is an
// honest miss (404). When the worker read API lands, this becomes an async
// fetch + decode; the page render is unchanged.
export const lookupTrajectory = (uuid: string): Trajectory | undefined =>
  uuid === SAMPLE_TRACE_UUID ? sampleTrajectory : undefined
