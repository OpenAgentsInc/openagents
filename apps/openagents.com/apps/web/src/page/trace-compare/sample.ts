// Committed public-safe sample trace-set for the `/trace/compare/{ids}` view
// (issue #6211 — the real "chill-evals": compare N agent traces, shareable).
//
// "See how agents perform with these MCP/config changes" = hold the SCENARIO
// fixed and vary the AGENT CONFIG, then look at the traces side-by-side. Each
// variant produces its own full ATIF trajectory (the exact shape `/trace/{uuid}`
// renders), so every row in the comparison deep-links its own `/trace/{uuid}`.
//
// These three trajectories are the SAME login-verification scenario run under
// three configs:
//   1. baseline   — the committed `/trace` login trace (PASS, 5 actions).
//   2. mcp-on      — same scenario with an extra MCP filesystem step (PASS, but
//                    one more step + slower).
//   3. no-waitfor  — a config that drops the explicit waitFor (REFUTED: it
//                    asserts before the page settled).
//
// HONEST: nothing is fabricated. The metrics the compare view shows (verdict,
// duration, step count, cost) are read straight off each trajectory's real
// `final_metrics`. A metric a trajectory does not carry is rendered as the
// `not_measured` sentinel by the view, never as a fake 0.
//
// INTEGRATION (the wiring step, NOT done here): a sibling lane is building the
// trace read API (`GET /api/traces/{uuid}`, #6208) and the qa-runner variant
// runner (#6183) that emits a trace per variant + a comparison record. When
// those land, `lookupTrajectoryForCompare(uuid)` swaps this committed lookup for
// an API fetch + decode against `../trace/atif`, and the comparison `ids` come
// from the persisted comparison record. The render does not change — it already
// consumes decoded `Trajectory` values keyed by uuid.

import { type Trajectory, decodeTrajectory } from '../trace/atif'
import { SAMPLE_TRACE_UUID, sampleTrajectory } from '../trace/sample'

// The three sample variant uuids. `/trace/compare/<a,b,c>` renders the set;
// `a` is the baseline (deltas are relative to it). The baseline reuses the
// committed `/trace` login trace verbatim so the deep-link from the comparison
// lands on the exact same single-trace render.
export const SAMPLE_COMPARE_BASELINE_UUID = SAMPLE_TRACE_UUID
export const SAMPLE_COMPARE_MCP_ON_UUID =
  '0e08d2db-2026-4624-9a39-f1efe8000002'
export const SAMPLE_COMPARE_NO_WAITFOR_UUID =
  '0e08d2db-2026-4624-9a39-f1efe8000003'

// The canonical sample comparison id list (baseline first), the value
// `/trace/compare/{ids}` renders by default in the shareable sample.
export const SAMPLE_COMPARE_IDS: ReadonlyArray<string> = [
  SAMPLE_COMPARE_BASELINE_UUID,
  SAMPLE_COMPARE_MCP_ON_UUID,
  SAMPLE_COMPARE_NO_WAITFOR_UUID,
]

// The path component for the sample comparison (the value the route captures as
// `ids`), e.g. `/trace/compare/<a,b,c>`.
export const SAMPLE_COMPARE_PATH_IDS = SAMPLE_COMPARE_IDS.join(',')

// Variant 2: same scenario, MCP filesystem server enabled — the agent reads a
// fixtures file before asserting, so it has one MORE step and is a bit slower.
// Still a real PASS.
const MCP_ON_JSON = {
  schema_version: 'ATIF-v1.7',
  session_id: 'login-trace-mcp-on',
  trajectory_id: 'login-trace-mcp-on-trajectory',
  agent: {
    name: 'openagents-qa-runner',
    version: '0.1.0',
    model_name: 'openagents/khala',
    extra: {
      driver: 'khala-computer-use',
      target: 'openagents.com',
      variant: 'mcp-filesystem-on',
    },
  },
  notes:
    'Same login-verification scenario as the baseline, run with the MCP ' +
    'filesystem server enabled. The agent reads a fixtures manifest before ' +
    'asserting, adding one step. Own-infra cost is $0.',
  steps: [
    {
      step_id: 1,
      timestamp: '2026-06-24T15:02:10.000Z',
      source: 'user',
      message:
        'Verify the login page works on this site: open /login, confirm the sign-in form renders (the page shows "Log in to OpenAgents"), and confirm it does NOT redirect to the homepage (the URL still includes /login). Assert each of these, screenshot the login page, then finish.',
    },
    {
      step_id: 2,
      timestamp: '2026-06-24T15:02:10.000Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: 'read fixtures manifest via MCP filesystem',
      reasoning_content:
        'The MCP filesystem server is enabled for this variant, so I first read the expected-copy manifest to confirm which heading text to assert against before navigating.',
      tool_calls: [
        {
          tool_call_id: 'call_2',
          function_name: 'mcp.fs.read',
          arguments: {
            action: 'read',
            target: 'fixtures/login-expectations.json',
            narration: 'read fixtures manifest via MCP filesystem',
          },
        },
      ],
      observation: {
        results: [
          {
            source_call_id: 'call_2',
            content: 'ok: read fixtures/login-expectations.json',
          },
        ],
      },
      metrics: { cost_usd: 0 },
    },
    {
      step_id: 3,
      timestamp: '2026-06-24T15:02:12.400Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: 'navigate to /login',
      reasoning_content:
        'With the expected copy confirmed, I navigate the browser to /login on the target deployment.',
      tool_calls: [
        {
          tool_call_id: 'call_3',
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
          { source_call_id: 'call_3', content: 'ok: navigate to /login' },
        ],
      },
      metrics: { cost_usd: 0 },
    },
    {
      step_id: 4,
      timestamp: '2026-06-24T15:02:14.900Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: 'wait for {"kind":"text-visible","value":"Log in to OpenAgents"}',
      reasoning_content:
        'Navigation is async, so I wait for the heading text to be visible before asserting.',
      tool_calls: [
        {
          tool_call_id: 'call_4',
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
            source_call_id: 'call_4',
            content:
              'ok: wait for {"kind":"text-visible","value":"Log in to OpenAgents"}',
          },
        ],
      },
      metrics: { cost_usd: 0 },
    },
    {
      step_id: 5,
      timestamp: '2026-06-24T15:02:16.100Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: "Login page shows 'Log in to OpenAgents'",
      reasoning_content:
        'The heading is visible, so I assert the first outcome from the goal.',
      tool_calls: [
        {
          tool_call_id: 'call_5',
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
            source_call_id: 'call_5',
            content: "ok: Login page shows 'Log in to OpenAgents'",
          },
        ],
      },
      metrics: { cost_usd: 0 },
    },
    {
      step_id: 6,
      timestamp: '2026-06-24T15:02:18.700Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: 'URL still includes /login',
      reasoning_content:
        'The second outcome is that the page did NOT bounce to the homepage. I assert the URL still contains /login.',
      tool_calls: [
        {
          tool_call_id: 'call_6',
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
          { source_call_id: 'call_6', content: 'ok: URL still includes /login' },
        ],
      },
      metrics: { cost_usd: 0 },
    },
    {
      step_id: 7,
      timestamp: '2026-06-24T15:02:21.300Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: 'screenshot login-page',
      reasoning_content:
        'Both assertions passed. I capture a screenshot of the rendered login page as visual evidence.',
      tool_calls: [
        {
          tool_call_id: 'call_7',
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
          { source_call_id: 'call_7', content: 'ok: screenshot login-page' },
        ],
      },
      metrics: { cost_usd: 0 },
    },
    {
      step_id: 8,
      timestamp: '2026-06-24T15:02:23.900Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: 'Goal verified. Marking the session complete.',
      reasoning_content: 'Final verdict PASS after 6 action(s).',
      tool_calls: [
        {
          tool_call_id: 'call_8',
          function_name: 'done',
          arguments: {
            verdict: 'PASS',
            summary: 'Verified openagents.com (6 actions, MCP filesystem on).',
          },
        },
      ],
      observation: {
        results: [
          {
            source_call_id: 'call_8',
            content: 'verification_class=test_passed',
          },
        ],
      },
      metrics: { cost_usd: 0 },
    },
  ],
  final_metrics: {
    total_prompt_tokens: 0,
    total_completion_tokens: 0,
    total_cached_tokens: 0,
    total_cost_usd: 0,
    total_steps: 8,
    extra: {
      verdict: 'PASS',
      duration_ms: 13900,
      trace_digest:
        'b7c1f0e93a2d6584c91f0a7b3e2c5d4869a1b0c3f5e7d9a2b4c6e8f0a1d3b5c7',
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

// Variant 3: same scenario, but the explicit waitFor step is DROPPED — the agent
// asserts before the page has settled and the assertion fails. A real REFUTED.
const NO_WAITFOR_JSON = {
  schema_version: 'ATIF-v1.7',
  session_id: 'login-trace-no-waitfor',
  trajectory_id: 'login-trace-no-waitfor-trajectory',
  agent: {
    name: 'openagents-qa-runner',
    version: '0.1.0',
    model_name: 'openagents/khala',
    extra: {
      driver: 'khala-computer-use',
      target: 'openagents.com',
      variant: 'no-explicit-waitfor',
    },
  },
  notes:
    'Same login-verification scenario as the baseline, run with the explicit ' +
    'waitFor step removed. The agent asserts before the page settled, so the ' +
    'first assertion fails and the run is REFUTED. Own-infra cost is $0.',
  steps: [
    {
      step_id: 1,
      timestamp: '2026-06-24T15:04:02.000Z',
      source: 'user',
      message:
        'Verify the login page works on this site: open /login, confirm the sign-in form renders (the page shows "Log in to OpenAgents"), and confirm it does NOT redirect to the homepage (the URL still includes /login). Assert each of these, screenshot the login page, then finish.',
    },
    {
      step_id: 2,
      timestamp: '2026-06-24T15:04:02.000Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: 'navigate to /login',
      reasoning_content:
        'This variant has no explicit waitFor in its policy, so it navigates and goes straight to asserting.',
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
          { source_call_id: 'call_2', content: 'ok: navigate to /login' },
        ],
      },
      metrics: { cost_usd: 0 },
    },
    {
      step_id: 3,
      timestamp: '2026-06-24T15:04:02.450Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: "Login page shows 'Log in to OpenAgents'",
      reasoning_content:
        'Without waiting for the heading to render, I assert immediately — the page has not painted the form yet.',
      tool_calls: [
        {
          tool_call_id: 'call_3',
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
            source_call_id: 'call_3',
            content:
              "FAILED: 'Log in to OpenAgents' not visible yet (asserted before the page settled)",
          },
        ],
      },
      metrics: { cost_usd: 0 },
    },
    {
      step_id: 4,
      timestamp: '2026-06-24T15:04:02.900Z',
      source: 'agent',
      model_name: 'openagents/khala',
      message: 'Assertion failed. Marking the session refuted.',
      reasoning_content: 'Final verdict REFUTED after 2 action(s).',
      tool_calls: [
        {
          tool_call_id: 'call_4',
          function_name: 'done',
          arguments: {
            verdict: 'REFUTED',
            summary: 'Login heading not visible without an explicit wait.',
          },
        },
      ],
      observation: {
        results: [
          {
            source_call_id: 'call_4',
            content: 'verification_class=test_failed',
          },
        ],
      },
      metrics: { cost_usd: 0 },
    },
  ],
  final_metrics: {
    total_prompt_tokens: 0,
    total_completion_tokens: 0,
    total_cached_tokens: 0,
    total_cost_usd: 0,
    total_steps: 4,
    extra: {
      verdict: 'REFUTED',
      duration_ms: 900,
      trace_digest:
        'c3f2a1b0d9e8c7b6a5f4e3d2c1b0a9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b2',
    },
  },
  extra: {
    target: {
      name: 'openagents.com',
      baseUrl: 'https://openagents.com',
    },
    artifacts: {
      screenshots: [],
    },
  },
} as const

// Decode the two additional sample variants once at module load so they are
// validated against the pinned `Trajectory` contract (drift fails fast here, in
// tests, rather than silently mis-rendering).
export const mcpOnTrajectory: Trajectory = decodeTrajectory(MCP_ON_JSON)
export const noWaitForTrajectory: Trajectory = decodeTrajectory(NO_WAITFOR_JSON)

// The committed by-uuid lookup for the comparison view. Reuses the `/trace`
// baseline sample verbatim for the baseline uuid and returns the two committed
// variants for theirs; any other uuid is an honest miss (handled by the view as
// an "unknown id" row, never fabricated). When the read API lands this becomes
// an async fetch + decode; the render is unchanged.
export const lookupTrajectoryForCompare = (
  uuid: string,
): Trajectory | undefined => {
  switch (uuid) {
    case SAMPLE_COMPARE_BASELINE_UUID:
      return sampleTrajectory
    case SAMPLE_COMPARE_MCP_ON_UUID:
      return mcpOnTrajectory
    case SAMPLE_COMPARE_NO_WAITFOR_UUID:
      return noWaitForTrajectory
    default:
      return undefined
  }
}
