import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  handleDispatchFailureTelemetryReadout,
  publicDispatchFailureTelemetryReadout,
} from './dispatch-failure-telemetry-routes'
import { makeBoundedDispatchFailureTelemetry } from './model-router'

const run = <A>(effect: Effect.Effect<A>): Promise<A> =>
  Effect.runPromise(effect)

describe('dispatch failure telemetry readout', () => {
  test('returns bounded aggregate counts and redacted event shapes only', async () => {
    const telemetry = makeBoundedDispatchFailureTelemetry({
      nowMs: () => 1_000,
      windowMs: 60_000,
    })
    telemetry.record({
      adapterId: 'raw-internal-adapter-id',
      classifier: 'empty_content',
      kind: 'empty_assistant_content',
      retryable: true,
      stage: 'validation_failure',
    })
    telemetry.record({
      adapterId: 'raw-rate-limit-adapter-id',
      classifier: 'rate_limited_429',
      httpStatus: 429,
      kind: 'rate_limited',
      retryable: true,
      stage: 'adapter_error',
    })

    const readout = publicDispatchFailureTelemetryReadout(telemetry, 1_000)

    expect(readout).toMatchObject({
      counts: {
        empty_content: 1,
        fallback: 0,
        invalid_tool: 0,
        provider_error: 0,
        rate_limited_429: 1,
      },
      generatedAtMs: 1_000,
      schemaVersion: 'openagents.dispatch_failure_telemetry.v1',
      staleness: 'live_at_read',
      windowMs: 60_000,
    })
    expect(readout.recentEvents).toEqual([
      {
        classifier: 'empty_content',
        retryable: true,
        stage: 'validation_failure',
        statusClass: 'none',
      },
      {
        classifier: 'rate_limited_429',
        retryable: true,
        stage: 'adapter_error',
        statusClass: 'http_429',
      },
    ])
    expect(JSON.stringify(readout)).not.toContain('raw-internal-adapter-id')
    expect(JSON.stringify(readout)).not.toContain('raw-rate-limit-adapter-id')
    expect(JSON.stringify(readout)).not.toContain('empty_assistant_content')
    expect(JSON.stringify(readout)).not.toContain('rate_limited"')
  })

  test('serves the public readout live at read', async () => {
    const telemetry = makeBoundedDispatchFailureTelemetry({
      nowMs: () => 2_000,
      windowMs: 30_000,
    })
    telemetry.record({
      adapterId: 'provider-private-lane',
      classifier: 'provider_error',
      httpStatus: 503,
      kind: 'provider_error',
      retryable: true,
      stage: 'adapter_error',
    })

    const response = await run(
      handleDispatchFailureTelemetryReadout(
        new Request('https://openagents.com/v1/gateway/dispatch-failures'),
        {
          enabled: true,
          nowMs: () => 2_000,
          telemetry,
        },
      ),
    )

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      counts: Record<string, number>
      recentEvents: ReadonlyArray<Record<string, unknown>>
    }
    expect(body.counts.provider_error).toBe(1)
    expect(body.recentEvents[0]).toEqual({
      classifier: 'provider_error',
      retryable: true,
      stage: 'adapter_error',
      statusClass: 'http_5xx',
    })
  })
})
