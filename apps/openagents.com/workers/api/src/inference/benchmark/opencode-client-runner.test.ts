import { describe, expect, test } from 'vitest'

import { expandMatrix } from './matrix'
import {
  OpenCodeUsageExtractionError,
  extractOpenCodeUsage,
  openCodeSampleFromObservation,
  provisionOpenCodeConfigForLane,
} from './opencode-client-runner'
import { OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG } from './fixtures'

describe('OpenCode client runner config provisioning', () => {
  test('provisions the Khala OpenCode config with the clean selector and upstream model id', () => {
    const config = provisionOpenCodeConfigForLane('khala')

    expect(config.providerId).toBe('openagents')
    expect(config.modelKey).toBe('khala')
    expect(config.modelId).toBe('openagents/khala')
    expect(config.modelSelector).toBe('openagents/khala')
    expect(JSON.stringify(config.opencodeJson)).toContain(
      '"id":"openagents/khala"',
    )
    expect(JSON.stringify(config.opencodeJson)).not.toContain(
      'openagents/openagents/khala',
    )
  })

  test('provisions BigPickle as a fixture-only OpenCode lane', () => {
    const config = provisionOpenCodeConfigForLane('bigpickle')

    expect(config.availabilityNote).toBe('fixture_only')
    expect(config.providerId).toBe('opencode')
    expect(config.modelSelector).toBe('opencode/bigpickle')
    expect(config.configRef).toBe('opencode.config.fixture.bigpickle.v1')
  })
})

describe('OpenCode usage extraction', () => {
  test('uses provider usage fields directly', () => {
    expect(
      extractOpenCodeUsage({
        prompt_tokens: 10,
        completion_tokens: 5,
        total_tokens: 15,
      }),
    ).toEqual({
      promptTokens: 10,
      completionTokens: 5,
      totalTokens: 15,
    })
  })

  test('rejects missing usage instead of estimating tokens', () => {
    expect(() =>
      extractOpenCodeUsage({
        prompt_tokens: 10,
        completion_tokens: 5,
      }),
    ).toThrow(OpenCodeUsageExtractionError)
  })

  test('turns an observed OpenCode run into a canonical benchmark sample', () => {
    const cell = expandMatrix(OPENCODE_KHALA_VS_BIGPICKLE_FIXTURE_CONFIG)[0]!
    const sample = openCodeSampleFromObservation(cell, {
      usage: {
        prompt_tokens: 100,
        completion_tokens: 40,
        total_tokens: 140,
      },
      ttftMs: 250,
      totalWallClockMs: 1800,
      generationWallClockMs: 1200,
      providerTimeMs: 1700,
      gatewayOverheadMs: 100,
      verifierTimeMs: 400,
      costBasisMsat: 50,
      region: 'fixture',
      verifierVerdict: 'passed',
      scalarReward: 1,
      toolCallsAttempted: 3,
      toolCallsSucceeded: 3,
    })

    expect(sample.promptTokens).toBe(100)
    expect(sample.completionTokens).toBe(40)
    expect(sample.totalTokens).toBe(140)
    expect(sample.executedVerdict).toBe('passed')
    expect(sample.clientSurface?.client).toBe('opencode')
    expect(sample.clientSurface?.toolCallsAttempted).toBe(3)
    expect(sample.clientSurface?.toolCallsSucceeded).toBe(3)
  })
})
