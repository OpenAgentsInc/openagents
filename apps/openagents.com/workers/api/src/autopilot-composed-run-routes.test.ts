import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  buildComposedRunPlan,
  makeInMemoryComposedRunStore,
} from './autopilot-composed-run'
import {
  AutopilotComposedRunEndpoint,
  handleAutopilotComposedRunApi,
  isAutopilotComposedRunEnabled,
} from './autopilot-composed-run-routes'

const runStore = () => {
  const result = buildComposedRunPlan({
    runId: 'run-1',
    businessRef: 'agent:raynor',
    title: 'All-in-one run',
    summary: 'inference + sandbox on one balance',
    balance: { balanceRef: 'balance:agent:raynor', asset: 'credit' },
    components: [
      {
        primitive: 'inference',
        capabilityRef: 'promise:inference.gateway_credits_business.v1',
        componentRunId: 'req-1',
      },
      {
        primitive: 'sandbox',
        capabilityRef: 'promise:cloud.sandbox_compute_service.v1',
        componentRunId: 'sbx-1',
      },
    ],
  })
  if (!result.ok) {
    throw new Error(result.error.reason)
  }
  return makeInMemoryComposedRunStore([result.plan])
}

const request = (suffix = '') =>
  new Request(`https://openagents.com${AutopilotComposedRunEndpoint}${suffix}`)

describe('autopilot composed-run flag (#5519)', () => {
  test('flag defaults OFF', () => {
    expect(isAutopilotComposedRunEnabled(undefined)).toBe(false)
    expect(isAutopilotComposedRunEnabled('false')).toBe(false)
    expect(isAutopilotComposedRunEnabled('0')).toBe(false)
    expect(isAutopilotComposedRunEnabled('on')).toBe(true)
    expect(isAutopilotComposedRunEnabled('TRUE')).toBe(true)
  })
})

describe('autopilot composed-run route (#5519)', () => {
  test('is INERT (empty list) when disabled, even with a populated store', async () => {
    const response = await Effect.runPromise(
      handleAutopilotComposedRunApi(request(), {
        enabled: false,
        store: runStore(),
      }),
    )
    const body = (await response.json()) as {
      inert: boolean
      promiseState: string
      runs: ReadonlyArray<unknown>
    }
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.inert).toBe(true)
    expect(body.promiseState).toBe('planned')
    expect(body.runs).toHaveLength(0)
  })

  test('lists runs when armed, still reporting inert/planned', async () => {
    const response = await Effect.runPromise(
      handleAutopilotComposedRunApi(request(), {
        enabled: true,
        store: runStore(),
      }),
    )
    const body = (await response.json()) as {
      inert: boolean
      promiseState: string
      promiseIds: ReadonlyArray<string>
      runs: ReadonlyArray<{ runId: string }>
    }
    expect(body.inert).toBe(true)
    expect(body.promiseState).toBe('planned')
    expect(body.promiseIds).toEqual([
      'autopilot.all_in_one_business_system.v1',
      'cloud.primitives_suite.v1',
    ])
    expect(body.runs.map(r => r.runId)).toEqual(['run-1'])
  })

  test('reads a single run by id', async () => {
    const response = await Effect.runPromise(
      handleAutopilotComposedRunApi(request('?runId=run-1'), {
        enabled: true,
        store: runStore(),
      }),
    )
    const body = (await response.json()) as {
      inert: boolean
      run: { runId: string } | null
    }
    expect(body.inert).toBe(true)
    expect(body.run?.runId).toBe('run-1')
  })

  test('returns null run for a missing id', async () => {
    const response = await Effect.runPromise(
      handleAutopilotComposedRunApi(request('?runId=missing'), {
        enabled: true,
        store: runStore(),
      }),
    )
    const body = (await response.json()) as { run: unknown }
    expect(body.run).toBeNull()
  })

  test('rejects non-GET', async () => {
    const response = await Effect.runPromise(
      handleAutopilotComposedRunApi(
        new Request(`https://openagents.com${AutopilotComposedRunEndpoint}`, {
          method: 'POST',
        }),
        { enabled: false },
      ),
    )
    expect(response.status).toBe(405)
  })
})
