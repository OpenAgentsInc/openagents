import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  type InferenceResult,
} from './inference/provider-adapter'
import {
  type SarahAgentToolActivity,
  runSarahAgentTurn,
} from './sarah-agent-runtime'

const usage = {
  completionTokens: 2,
  promptTokens: 5,
  totalTokens: 7,
}

const adapterFor = (
  results: ReadonlyArray<InferenceResult>,
  requests: Array<unknown>,
): InferenceProviderAdapter => {
  let index = 0
  return {
    complete: request => {
      requests.push(request)
      const result = results[index]
      index += 1
      return result === undefined
        ? Effect.die('missing fixture result')
        : Effect.succeed(result)
    },
    id: 'fixture.sarah',
    stream: () => Effect.succeed([]),
  }
}

describe('Sarah bounded agent runtime', () => {
  test('retries one retryable inference failure before any tool can run', async () => {
    let attempts = 0
    const adapter: InferenceProviderAdapter = {
      complete: () =>
        Effect.suspend(() => {
          attempts += 1
          return attempts === 1
            ? Effect.fail(
                new InferenceAdapterError({
                  adapterId: 'fixture.sarah',
                  reason: 'retryable: transient provider failure',
                  retryable: true,
                }),
              )
            : Effect.succeed({
                content: 'The provider recovered.',
                finishReason: 'STOP',
                servedModel: 'gemma-4-31b-it',
                usage,
              })
        }),
      id: 'fixture.sarah',
      stream: () => Effect.succeed([]),
    }

    const result = await Effect.runPromise(
      runSarahAgentTurn({
        adapter,
        model: 'gemma-4-31b-it',
        prompt: 'Check current state.',
        system: 'You are Sarah.',
        tools: [],
      }),
    )

    expect(attempts).toBe(2)
    expect(result.text).toBe('The provider recovered.')
  })

  test('executes a tool, emits ordered activity, and composes a final answer', async () => {
    const requests: Array<unknown> = []
    const activity: Array<SarahAgentToolActivity> = []
    const adapter = adapterFor(
      [
        {
          content: '',
          finishReason: 'tool_calls',
          servedModel: 'gemma-4-31b-it',
          toolCalls: [
            {
              function: {
                arguments: '{"runRef":"run.fixture"}',
                name: 'full_auto_status',
              },
              id: 'call.fixture',
              type: 'function',
            },
          ],
          usage,
        },
        {
          content: 'The run is paused.',
          finishReason: 'STOP',
          servedModel: 'gemma-4-31b-it',
          usage,
        },
      ],
      requests,
    )

    const result = await Effect.runPromise(
      runSarahAgentTurn({
        adapter,
        model: 'gemma-4-31b-it',
        onToolActivity: event =>
          Effect.sync(() => {
            activity.push(event)
          }),
        prompt: 'What is happening?',
        system: 'You are Sarah.',
        tools: [
          {
            definition: {
              description: 'Read Full Auto status.',
              name: 'full_auto_status',
              parameters: { type: 'object' },
            },
            execute: () =>
              Effect.succeed({
                authorityReceiptRef: 'receipt.authority.fixture',
                content: '{"state":"paused"}',
                resultRefs: ['run.fixture'],
                summary: 'Full Auto is paused.',
              }),
          },
        ],
      }),
    )

    expect(result).toMatchObject({
      text: 'The run is paused.',
      toolCallCount: 1,
      usage: { completionTokens: 4, promptTokens: 10, totalTokens: 14 },
    })
    expect(activity.map(event => event.phase)).toEqual(['started', 'succeeded'])
    expect(requests).toHaveLength(2)
    const second = requests[1] as {
      messages: Array<{ role: string; name?: string }>
    }
    expect(second.messages.at(-1)).toMatchObject({
      role: 'tool',
      name: 'full_auto_status',
    })
  })

  test('returns honest failed activity when a tool executor refuses', async () => {
    const activity: Array<SarahAgentToolActivity> = []
    const adapter = adapterFor(
      [
        {
          content: '',
          finishReason: 'tool_calls',
          servedModel: 'gemma-4-31b-it',
          toolCalls: [
            {
              function: { arguments: '{}', name: 'dispatch_workers' },
              id: 'call.refused',
              type: 'function',
            },
          ],
          usage,
        },
        {
          content:
            'I could not dispatch workers because no owner capacity is available.',
          finishReason: 'STOP',
          servedModel: 'gemma-4-31b-it',
          usage,
        },
      ],
      [],
    )
    const result = await Effect.runPromise(
      runSarahAgentTurn({
        adapter,
        model: 'gemma-4-31b-it',
        onToolActivity: event =>
          Effect.sync(() => {
            activity.push(event)
          }),
        prompt: 'Start workers.',
        system: 'You are Sarah.',
        tools: [
          {
            definition: {
              description: 'Dispatch workers.',
              name: 'dispatch_workers',
              parameters: { type: 'object' },
            },
            execute: () =>
              Effect.succeed({
                authorityReceiptRef: 'receipt.authority.fixture',
                content: '{"ok":false,"error":"no_capacity"}',
                isError: true,
                resultRefs: ['blocker.no_capacity'],
                summary: 'No owner capacity is available.',
              }),
          },
        ],
      }),
    )
    expect(result.text).toContain('could not dispatch')
    expect(activity.map(event => event.phase)).toEqual(['started', 'failed'])
  })

  test('replays an identical model tool request without executing the target twice', async () => {
    let executions = 0
    const repeatedCall = {
      function: { arguments: '{"count":2}', name: 'dispatch_workers' },
      id: 'call.repeated',
      type: 'function' as const,
    }
    const adapter = adapterFor(
      [
        {
          content: '',
          finishReason: 'tool_calls',
          servedModel: 'gemma-4-31b-it',
          toolCalls: [repeatedCall],
          usage,
        },
        {
          content: '',
          finishReason: 'tool_calls',
          servedModel: 'gemma-4-31b-it',
          toolCalls: [{ ...repeatedCall, id: 'call.repeated.again' }],
          usage,
        },
        {
          content: 'The same two workers remain dispatched.',
          finishReason: 'STOP',
          servedModel: 'gemma-4-31b-it',
          usage,
        },
      ],
      [],
    )
    const result = await Effect.runPromise(
      runSarahAgentTurn({
        adapter,
        model: 'gemma-4-31b-it',
        prompt: 'Start two workers.',
        system: 'You are Sarah.',
        tools: [
          {
            definition: {
              description: 'Dispatch workers.',
              name: 'dispatch_workers',
              parameters: { type: 'object' },
            },
            execute: () => {
              executions += 1
              return Effect.succeed({
                authorityReceiptRef: 'receipt.authority.fixture',
                content: '{"assigned":2}',
                resultRefs: ['spawn.fixture'],
                summary: 'Two workers started.',
              })
            },
          },
        ],
      }),
    )
    expect(result.toolCallCount).toBe(2)
    expect(executions).toBe(1)
  })
})
