import { describe, expect, test, vi } from 'vitest'

import { publicSafeExecutionCloseoutRef } from './autopilot-work-routes'
import type { InferenceResult } from './inference/provider-adapter'
import {
  createVertexGeminiHostedCaller,
  hostedGeminiResponseDigestHex,
  projectGeminiResultToPublicSafeRefs,
  type HostedGeminiInferenceCallerInput,
} from './autopilot-hosted-gemini-inference-bridge'

const callerInput: HostedGeminiInferenceCallerInput = {
  assignmentRef: 'assignment.work-1.a1',
  objectiveRefs: ['acceptance.work-1.a1.criteria'],
  taskRef: 'task.work-1.a1',
  workOrderRef: 'work_order.work-1',
}

const geminiResult = (
  overrides: Partial<InferenceResult> = {},
): InferenceResult => ({
  content: 'the raw model completion that must never leak',
  finishReason: 'stop',
  servedModel: 'gemini-3.5-flash',
  usage: {
    completionTokens: 64,
    promptTokens: 128,
    totalTokens: 192,
  },
  ...overrides,
})

describe('hosted Gemini inference bridge — public-safe projection', () => {
  test('projects a real Gemini result into refs-only, all public-safe', () => {
    const projected = projectGeminiResultToPublicSafeRefs(
      geminiResult(),
      'a'.repeat(64),
    )

    expect(projected).toBeDefined()
    expect(projected).toMatchObject({
      modelRef: 'model.hosted_gemini.gemini-3.5-flash',
      responseDigestRef: `proof.hosted_gemini.response_digest.sha256.${'a'.repeat(64)}`,
      usageRef: 'usage.hosted_gemini.prompt_128.completion_64.total_192',
    })
    for (const ref of Object.values(projected!)) {
      expect(publicSafeExecutionCloseoutRef(ref)).toBe(true)
    }
  })

  test('never leaks the raw completion content into any emitted ref', () => {
    const secretContent =
      'ghp_abcdefSECRETtoken sk-deadbeef Bearer abc /Users/op/.ssh'
    const projected = projectGeminiResultToPublicSafeRefs(
      geminiResult({ content: secretContent }),
      'b'.repeat(64),
    )

    expect(projected).toBeDefined()
    const joined = Object.values(projected!).join(' ')
    expect(joined).not.toContain('ghp_')
    expect(joined).not.toContain('sk-')
    expect(joined).not.toContain('Bearer')
    expect(joined).not.toContain('/Users/')
  })

  test('surfaces a reported cached-prompt split in the usage ref', () => {
    const projected = projectGeminiResultToPublicSafeRefs(
      geminiResult({
        usage: {
          cachedPromptTokens: 32,
          completionTokens: 10,
          promptTokens: 100,
          totalTokens: 110,
        },
      }),
      'c'.repeat(64),
    )

    expect(projected?.usageRef).toBe(
      'usage.hosted_gemini.prompt_100.completion_10.total_110.cached_32',
    )
    expect(publicSafeExecutionCloseoutRef(projected!.usageRef!)).toBe(true)
  })

  test('aborts (undefined) when the served model sanitizes to empty', () => {
    expect(
      projectGeminiResultToPublicSafeRefs(
        geminiResult({ servedModel: '   ' }),
        'd'.repeat(64),
      ),
    ).toBeUndefined()
  })

  test('aborts (undefined) when the digest hex is empty', () => {
    expect(
      projectGeminiResultToPublicSafeRefs(geminiResult(), ''),
    ).toBeUndefined()
  })

  test('clamps negative/non-integer token counts to zero in the usage ref', () => {
    const projected = projectGeminiResultToPublicSafeRefs(
      geminiResult({
        usage: { completionTokens: -1, promptTokens: 1.5, totalTokens: NaN },
      }),
      'e'.repeat(64),
    )

    expect(projected?.usageRef).toBe(
      'usage.hosted_gemini.prompt_0.completion_0.total_0',
    )
  })
})

describe('hosted Gemini inference bridge — caller factory', () => {
  test('is INERT when disabled: never calls the runner, returns undefined', async () => {
    const runInference = vi.fn(async () => geminiResult())
    const caller = createVertexGeminiHostedCaller({
      digest: async () => 'f'.repeat(64),
      enabled: false,
      runInference,
    })

    expect(await caller(callerInput)).toBeUndefined()
    expect(runInference).not.toHaveBeenCalled()
  })

  test('armed: drives the runner and returns public-safe projected refs', async () => {
    const runInference = vi.fn(async () => geminiResult())
    const caller = createVertexGeminiHostedCaller({
      digest: async () => '0'.repeat(64),
      enabled: true,
      runInference,
    })

    const result = await caller(callerInput)

    expect(runInference).toHaveBeenCalledWith(callerInput)
    expect(result).toMatchObject({
      modelRef: 'model.hosted_gemini.gemini-3.5-flash',
      responseDigestRef: `proof.hosted_gemini.response_digest.sha256.${'0'.repeat(64)}`,
    })
    for (const ref of Object.values(result!)) {
      expect(publicSafeExecutionCloseoutRef(ref)).toBe(true)
    }
  })

  test('armed but runner declines: returns undefined (no leak, no partial)', async () => {
    const caller = createVertexGeminiHostedCaller({
      digest: async () => '1'.repeat(64),
      enabled: true,
      runInference: async () => undefined,
    })

    expect(await caller(callerInput)).toBeUndefined()
  })
})

describe('hosted Gemini response digest', () => {
  test('produces a stable 64-char lowercase hex SHA-256 of the content', async () => {
    const hex = await hostedGeminiResponseDigestHex('hello world')

    expect(hex).toMatch(/^[0-9a-f]{64}$/u)
    // Known SHA-256("hello world").
    expect(hex).toBe(
      'b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9',
    )
  })
})
