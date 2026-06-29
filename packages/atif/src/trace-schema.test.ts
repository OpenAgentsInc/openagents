import { describe, expect, it } from 'bun:test'

import {
  ATIF_PINNED_SCHEMA_VERSION,
  type AtifTrajectory,
  atifTraceTripwire,
  decodeAtifTrajectorySync,
  validateAtifTrajectory,
} from './trace-schema.ts'

const cleanTrajectory = (): AtifTrajectory => ({
  schema_version: ATIF_PINNED_SCHEMA_VERSION,
  trajectory_id: 'traj-1',
  session_id: 'sess-1',
  agent: { name: 'Raynor', version: '1.0.0', model_name: 'openagents/khala' },
  steps: [
    { step_id: 1, source: 'user', message: 'Log into the app.' },
    {
      step_id: 2,
      source: 'agent',
      message: 'Filling the form.',
      reasoning_content: 'I should click submit.',
      model_name: 'openagents/khala',
      tool_calls: [
        {
          tool_call_id: 'call-1',
          function_name: 'browser_click',
          arguments: { selector: '#submit' },
        },
      ],
      observation: {
        results: [{ source_call_id: 'call-1', content: 'clicked' }],
      },
      metrics: { prompt_tokens: 10, completion_tokens: 5, cost_usd: 0.001 },
    },
    { step_id: 3, source: 'system', message: 'Session ended.' },
  ],
  final_metrics: {
    total_prompt_tokens: 10,
    total_completion_tokens: 5,
    total_cost_usd: 0.001,
    total_steps: 3,
  },
})

describe('ATIF trace schema decode', () => {
  it('decodes a clean public-safe trajectory', () => {
    const decoded = decodeAtifTrajectorySync(cleanTrajectory())
    expect(decoded.schema_version).toBe('ATIF-v1.7')
    expect(decoded.steps).toHaveLength(3)
  })

  it('rejects a non-pinned schema_version', () => {
    expect(() =>
      decodeAtifTrajectorySync({ ...cleanTrajectory(), schema_version: 'ATIF-v1.6' }),
    ).toThrow()
  })

  it('rejects an unknown step source', () => {
    const bad = cleanTrajectory()
    expect(() =>
      decodeAtifTrajectorySync({
        ...bad,
        steps: [{ step_id: 1, source: 'robot', message: 'x' }],
      }),
    ).toThrow()
  })
})

describe('ATIF structural validator', () => {
  it('passes a well-formed trajectory', () => {
    expect(validateAtifTrajectory(cleanTrajectory())).toEqual([])
  })

  it('flags non-sequential step ids', () => {
    const t = cleanTrajectory()
    const issues = validateAtifTrajectory({
      ...t,
      steps: [
        { step_id: 1, source: 'user', message: 'a' },
        { step_id: 3, source: 'agent', message: 'b' },
      ],
    })
    expect(issues.map(i => i.code)).toContain('step_id_not_sequential')
  })

  it('flags an empty step list', () => {
    const issues = validateAtifTrajectory({ ...cleanTrajectory(), steps: [] })
    expect(issues.map(i => i.code)).toContain('empty_steps')
  })

  it('flags an observation that references no tool_call', () => {
    const issues = validateAtifTrajectory({
      ...cleanTrajectory(),
      steps: [
        {
          step_id: 1,
          source: 'agent',
          message: 'x',
          observation: {
            results: [{ source_call_id: 'missing', content: 'y' }],
          },
        },
      ],
    })
    expect(issues.map(i => i.code)).toContain('observation_without_tool_call')
  })

  it('flags agent-only fields on a non-agent step', () => {
    const issues = validateAtifTrajectory({
      ...cleanTrajectory(),
      steps: [
        {
          step_id: 1,
          source: 'user',
          message: 'x',
          reasoning_content: 'should not be here',
        },
      ],
    })
    expect(issues.map(i => i.code)).toContain('agent_field_on_non_agent_step')
  })
})

describe('ATIF public-safety tripwire', () => {
  it('passes a clean public-safe trajectory', () => {
    expect(atifTraceTripwire(cleanTrajectory())).toEqual([])
  })

  it('ALLOWS a model id on the agent (a trace records the model that ran)', () => {
    // A shareable trace — esp. a user-uploaded Claude Code / Codex session — runs
    // on a real model; its id is session content, not a leak. The
    // "openagents/khala only" rule is a Khala GATEWAY invariant, not a trace one.
    const findings = atifTraceTripwire({
      ...cleanTrajectory(),
      agent: { name: 'a', version: '1', model_name: 'claude-opus-4' },
    })
    expect(findings).toEqual([])
  })

  it('ALLOWS a provider-namespaced model id in the body (content, not a leak)', () => {
    const t = cleanTrajectory()
    const findings = atifTraceTripwire({
      ...t,
      steps: [
        {
          step_id: 1,
          source: 'agent',
          message: 'routed via anthropic/claude-3-5-sonnet',
        },
      ],
    })
    expect(findings).toEqual([])
  })

  it('rejects an API key / bearer secret', () => {
    const findings = atifTraceTripwire({
      ...cleanTrajectory(),
      steps: [
        {
          step_id: 1,
          source: 'agent',
          message: 'used key sk-abcdef0123456789abcdef',
        },
      ],
    })
    expect(findings.map(f => f.code)).toContain('secret_material')
  })

  it('rejects wallet / payment material', () => {
    const findings = atifTraceTripwire({
      ...cleanTrajectory(),
      steps: [
        {
          step_id: 1,
          source: 'agent',
          message:
            'paid lnbc2500u1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypq',
        },
      ],
    })
    expect(findings.map(f => f.code)).toContain('wallet_or_payment_material')
  })

  it('rejects a local filesystem path', () => {
    const findings = atifTraceTripwire({
      ...cleanTrajectory(),
      steps: [
        {
          step_id: 1,
          source: 'agent',
          message: 'opened /Users/chris/secret.txt',
        },
      ],
    })
    expect(findings.map(f => f.code)).toContain('local_path')
  })

  it('rejects an email (PII)', () => {
    const findings = atifTraceTripwire({
      ...cleanTrajectory(),
      steps: [
        { step_id: 1, source: 'agent', message: 'emailed alice@example.com' },
      ],
    })
    expect(findings.map(f => f.code)).toContain('pii_email')
  })

  it('ALLOWS JSON-escaped text that looks like a drive path (kind:\\| , key:\\n)', () => {
    // `<letter>:\` appears constantly in serialized JSON (escaped quotes/newlines,
    // grep patterns); it is NOT a Windows path and must not false-trip local_path.
    const findings = atifTraceTripwire({
      ...cleanTrajectory(),
      steps: [
        {
          step_id: 1,
          source: 'agent',
          message: 'ran grep -n "kind:\\|wait" with config:\\nnext and target:\\"prod\\"',
        },
      ],
    })
    expect(findings).toEqual([])
  })
})
