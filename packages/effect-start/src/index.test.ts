import { describe, expect, test } from 'bun:test'
import { Effect, Schema as S } from 'effect'

import {
  StartHttpError,
  currentStartEnv,
  currentStartRequest,
  currentStartRequestContext,
  decodeStartInput,
  effectStartRuntime,
  getStartRequestContext,
  scheduleStartTask,
  withStartRequestContext,
} from './index.js'

const Input = S.Struct({
  name: S.String,
})

type TestEnv = Readonly<{
  flag: string
}>

const testContext = () => ({
  request: new Request('https://start.test/hello'),
  env: {
    flag: 'enabled',
  } satisfies TestEnv,
  executionCtx: {
    waitUntil(promise: Promise<unknown>) {
      void promise
    },
  },
})

describe('effect-start request context', () => {
  test('preserves env and request through async work', async () => {
    const context = testContext()
    const value = await withStartRequestContext(context, async () => {
      await Promise.resolve()
      return getStartRequestContext<TestEnv>()?.env.flag
    })

    expect(value).toBe('enabled')
  })

  test('exposes request/env through Effect helpers', async () => {
    const context = testContext()
    const result = await effectStartRuntime.runWithContext(
      context,
      Effect.gen(function* () {
        const env = yield* currentStartEnv<TestEnv>()
        const request = yield* currentStartRequest()
        return `${env.flag}:${new URL(request.url).pathname}`
      }),
    )

    expect(result).toBe('enabled:/hello')
  })

  test('fails with a typed missing-context error outside the boundary', async () => {
    const error = await Effect.runPromise(
      currentStartRequestContext('test.context').pipe(Effect.flip),
    )

    expect(error._tag).toBe('StartRequestContextMissing')
    expect(error.reasonRef).toBe('start.request_context.missing')
    expect(error.operation).toBe('test.context')
  })

  test('schedules background promises through the execution context', async () => {
    const scheduled: Array<Promise<unknown>> = []
    const context = {
      ...testContext(),
      executionCtx: {
        waitUntil(promise: Promise<unknown>) {
          scheduled.push(promise)
        },
      },
    }

    const didSchedule = await effectStartRuntime.runWithContext(
      context,
      scheduleStartTask(() => Promise.resolve('done')),
    )

    expect(didSchedule).toBe(true)
    expect(scheduled).toHaveLength(1)
  })
})

describe('effect-start handler boundary', () => {
  test('decodes schema input and returns JSON from an Effect program', async () => {
    const response = await effectStartRuntime.handleJson({
      context: testContext(),
      effect: input =>
        Effect.gen(function* () {
          const env = yield* currentStartEnv<TestEnv>()
          return {
            greeting: `hello ${input.name}`,
            flag: env.flag,
          }
        }),
      input: { name: 'Artanis' },
      inputSchema: Input,
      operation: 'test.hello',
    })

    expect(response.status).toBe(200)
    expect(response.headers.get('Content-Type')).toContain('application/json')
    expect(await response.json()).toEqual({
      flag: 'enabled',
      greeting: 'hello Artanis',
    })
  })

  test('maps schema failures to a public-safe 400', async () => {
    const response = await effectStartRuntime.handleJson({
      context: testContext(),
      effect: input => Effect.succeed({ greeting: input.name }),
      input: { name: 123 },
      inputSchema: Input,
      operation: 'test.hello',
    })

    expect(response.status).toBe(400)
    expect(await response.json()).toEqual({
      error: 'start.input.schema_invalid',
      operation: 'test.hello',
    })
  })

  test('maps typed HTTP errors without leaking internal detail', async () => {
    const response = await effectStartRuntime.handleJson({
      context: testContext(),
      effect: () =>
        Effect.fail(
          new StartHttpError({
            messageSafe: 'Conflict',
            reasonRef: 'test.conflict',
            status: 409,
          }),
        ),
      input: { name: 'Artanis' },
      inputSchema: Input,
      operation: 'test.conflict',
    })

    expect(response.status).toBe(409)
    expect(await response.json()).toEqual({
      error: 'test.conflict',
      message: 'Conflict',
    })
  })

  test('decodes inputs as a standalone Effect for loaders/server functions', async () => {
    const decoded = await Effect.runPromise(
      decodeStartInput(Input, { name: 'Raynor' }, 'test.decode'),
    )

    expect(decoded.name).toBe('Raynor')
  })
})
