import { Effect } from 'effect'
import { describe, expect, test, vi } from 'vitest'

import type { HostedGeminiInferenceCallerInput } from './autopilot-hosted-gemini-inference-bridge'
import {
  buildHostedGeminiInferenceRequest,
  createHostedGeminiRequestRunner,
  DEFAULT_HOSTED_GEMINI_MAX_OUTPUT_TOKENS,
  DEFAULT_HOSTED_GEMINI_MODEL,
} from './autopilot-hosted-gemini-request-runner'
import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  type InferenceRequest,
  type InferenceResult,
} from './inference/provider-adapter'

const callerInput: HostedGeminiInferenceCallerInput = {
  assignmentRef: 'assignment.work-1.a1',
  objectiveRefs: ['acceptance.work-1.a1.criteria', 'acceptance.work-1.a1.tests'],
  taskRef: 'task.work-1.a1',
  workOrderRef: 'work_order.work-1',
}

const geminiResult = (
  overrides: Partial<InferenceResult> = {},
): InferenceResult => ({
  content: 'closeout summary',
  finishReason: 'stop',
  servedModel: 'gemini-3.5-flash',
  usage: { completionTokens: 4, promptTokens: 9, totalTokens: 13 },
  ...overrides,
})

// A spy adapter whose `complete` succeeds with a fixed result and records the
// request it was handed; `stream` is unused by the runner.
const succeedingAdapter = (
  result: InferenceResult,
): {
  adapter: InferenceProviderAdapter
  complete: ReturnType<typeof vi.fn>
} => {
  const complete = vi.fn((request: InferenceRequest) => {
    void request
    return Effect.succeed(result)
  })
  return {
    adapter: {
      complete,
      id: 'vertex-gemini',
      stream: () => Effect.succeed([]),
    },
    complete,
  }
}

const failingAdapter = (error: InferenceAdapterError): InferenceProviderAdapter => ({
  complete: () => Effect.fail(error),
  id: 'vertex-gemini',
  stream: () => Effect.fail(error),
})

describe('buildHostedGeminiInferenceRequest', () => {
  test('builds a non-streaming refs-only request with model + max_tokens', () => {
    const request = buildHostedGeminiInferenceRequest(callerInput, {
      maxOutputTokens: 256,
      model: 'gemini-3.5-pro',
    })
    expect(request).toBeDefined()
    expect(request?.stream).toBe(false)
    expect(request?.model).toBe('gemini-3.5-pro')
    expect(request?.passthroughParams['max_tokens']).toBe(256)
    // System frame + user frame.
    expect(request?.messages).toHaveLength(2)
    expect(request?.messages[0]?.role).toBe('system')
    expect(request?.messages[1]?.role).toBe('user')
  })

  test('user content carries only the work-order refs, no raw content', () => {
    const request = buildHostedGeminiInferenceRequest(callerInput, {
      maxOutputTokens: 256,
      model: 'gemini-3.5-flash',
    })
    const user = request?.messages[1]?.content ?? ''
    expect(user).toContain('work_order=work_order.work-1')
    expect(user).toContain('assignment=assignment.work-1.a1')
    expect(user).toContain('task=task.work-1.a1')
    expect(user).toContain(
      'objectives=acceptance.work-1.a1.criteria,acceptance.work-1.a1.tests',
    )
  })

  test('omits the objectives line when no non-empty objective refs survive', () => {
    const request = buildHostedGeminiInferenceRequest(
      { ...callerInput, objectiveRefs: ['', '   '] },
      { maxOutputTokens: 256, model: 'gemini-3.5-flash' },
    )
    expect(request?.messages[1]?.content).not.toContain('objectives=')
  })

  test('declines (undefined) when the work order ref is empty', () => {
    expect(
      buildHostedGeminiInferenceRequest(
        { ...callerInput, workOrderRef: '   ' },
        { maxOutputTokens: 256, model: 'gemini-3.5-flash' },
      ),
    ).toBeUndefined()
  })

  test('declines (undefined) when the task ref is empty', () => {
    expect(
      buildHostedGeminiInferenceRequest(
        { ...callerInput, taskRef: '' },
        { maxOutputTokens: 256, model: 'gemini-3.5-flash' },
      ),
    ).toBeUndefined()
  })
})

describe('createHostedGeminiRequestRunner', () => {
  test('is INERT when disabled: returns undefined and never touches the adapter', async () => {
    const { adapter, complete } = succeedingAdapter(geminiResult())
    const runner = createHostedGeminiRequestRunner({ adapter, enabled: false })
    expect(await runner(callerInput)).toBeUndefined()
    expect(complete).not.toHaveBeenCalled()
  })

  test('drives the adapter when armed and returns the provider result', async () => {
    const result = geminiResult()
    const { adapter, complete } = succeedingAdapter(result)
    const runner = createHostedGeminiRequestRunner({ adapter, enabled: true })
    expect(await runner(callerInput)).toEqual(result)
    expect(complete).toHaveBeenCalledTimes(1)
  })

  test('defaults the requested model + max_tokens when unconfigured', async () => {
    const { adapter, complete } = succeedingAdapter(geminiResult())
    const runner = createHostedGeminiRequestRunner({ adapter, enabled: true })
    await runner(callerInput)
    const request = complete.mock.calls[0]?.[0] as InferenceRequest
    expect(request.model).toBe(DEFAULT_HOSTED_GEMINI_MODEL)
    expect(request.passthroughParams['max_tokens']).toBe(
      DEFAULT_HOSTED_GEMINI_MAX_OUTPUT_TOKENS,
    )
  })

  test('declines (undefined) without calling the adapter on an unframeable work order', async () => {
    const { adapter, complete } = succeedingAdapter(geminiResult())
    const runner = createHostedGeminiRequestRunner({ adapter, enabled: true })
    expect(await runner({ ...callerInput, workOrderRef: '' })).toBeUndefined()
    expect(complete).not.toHaveBeenCalled()
  })

  test('embeds resolver-supplied public-safe content in the request when armed', async () => {
    const { adapter, complete } = succeedingAdapter(geminiResult())
    const runner = createHostedGeminiRequestRunner({
      adapter,
      enabled: true,
      resolveRefContent: async (ref: string) => `resolved:${ref}`,
    })
    await runner(callerInput)
    const request = complete.mock.calls[0]?.[0] as InferenceRequest
    const user = request.messages[1]?.content ?? ''
    // Refs stay for provenance; resolved content is appended.
    expect(user).toContain('task=task.work-1.a1')
    expect(user).toContain('task_content: resolved:task.work-1.a1')
    expect(user).toContain(
      'objective_content[0]: resolved:acceptance.work-1.a1.criteria',
    )
  })

  test('keeps the refs-only frame when the resolver returns nothing safe', async () => {
    const { adapter, complete } = succeedingAdapter(geminiResult())
    const runner = createHostedGeminiRequestRunner({
      adapter,
      enabled: true,
      resolveRefContent: async () => undefined,
    })
    await runner(callerInput)
    const request = complete.mock.calls[0]?.[0] as InferenceRequest
    const user = request.messages[1]?.content ?? ''
    expect(user).toContain('task=task.work-1.a1')
    expect(user).not.toContain('resolved task content')
  })

  test('folds a typed adapter failure into undefined (never throws)', async () => {
    const runner = createHostedGeminiRequestRunner({
      adapter: failingAdapter(
        new InferenceAdapterError({
          adapterId: 'vertex-gemini',
          reason: 'quota exhausted',
          retryable: true,
        }),
      ),
      enabled: true,
    })
    await expect(runner(callerInput)).resolves.toBeUndefined()
  })
})
