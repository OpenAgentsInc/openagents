import { describe, expect, test } from 'vitest'

import {
  parseSarahRealtimeBridgeCreditRate,
  sarahEditorCommandRequiresConfirmation,
  usageFromInputTranscription,
  usageFromProviderResponse,
  validateSarahEditorCommandTarget,
} from './sarah-realtime-bridge'

describe('Sarah Realtime bridge metering', () => {
  test('prices exact provider response tokens with the operator credit rate', () => {
    expect(
      usageFromProviderResponse(
        {
          type: 'response.done',
          response: {
            id: 'resp-1',
            usage: {
              total_tokens: 1_250,
              input_tokens: 1_000,
              output_tokens: 250,
              input_token_details: {
                cached_tokens: 100,
                audio_tokens: 800,
              },
              output_token_details: { audio_tokens: 200 },
            },
          },
        },
        2_000_000,
        '2026-07-28T12:00:00.000Z',
      ),
    ).toEqual({
      providerResponseRef: 'resp-1',
      inputTokens: 1_000,
      outputTokens: 250,
      cachedInputTokens: 100,
      audioInputTokens: 800,
      audioOutputTokens: 200,
      chargeMsat: 2_500,
      observedAt: '2026-07-28T12:00:00.000Z',
    })
  })

  test('records the separate input transcription usage event', () => {
    expect(
      usageFromInputTranscription(
        {
          item_id: 'item-1',
          content_index: 0,
          usage: {
            total_tokens: 50,
            input_tokens: 40,
            output_tokens: 10,
            input_token_details: { audio_tokens: 35 },
          },
        },
        2_000_000,
        '2026-07-28T12:00:01.000Z',
      ),
    ).toMatchObject({
      providerResponseRef: 'transcription:item-1:0',
      inputTokens: 40,
      outputTokens: 10,
      audioInputTokens: 35,
      chargeMsat: 100,
    })
  })

  test('fails closed on a missing or fractional credit rate', () => {
    expect(parseSarahRealtimeBridgeCreditRate(undefined)).toBeUndefined()
    expect(parseSarahRealtimeBridgeCreditRate('1.5')).toBeUndefined()
    expect(parseSarahRealtimeBridgeCreditRate('2000000')).toBe(2_000_000)
  })

  test('requires confirmation for each admitted write command', () => {
    expect(
      sarahEditorCommandRequiresConfirmation({
        _tag: 'replace_selection',
        target: { workspaceRef: 'workspace-1', path: 'src/app.ts' },
        replacement: 'safe replacement',
      }),
    ).toBe(true)
    expect(
      sarahEditorCommandRequiresConfirmation({
        _tag: 'save_document',
        target: { workspaceRef: 'workspace-1', path: 'src/app.ts' },
      }),
    ).toBe(true)
    expect(
      sarahEditorCommandRequiresConfirmation({
        _tag: 'start_agent_thread',
        message: 'Inspect the current test failure.',
        presentation: 'foreground',
      }),
    ).toBe(true)
    expect(
      sarahEditorCommandRequiresConfirmation({
        _tag: 'context_read',
        target: { workspaceRef: 'workspace-1', path: 'src/app.ts' },
        startLine: 1,
        endLine: 20,
      }),
    ).toBe(false)
  })

  test('rejects traversal and excessive editor ranges', () => {
    expect(() =>
      validateSarahEditorCommandTarget({
        _tag: 'open_path',
        target: { workspaceRef: 'workspace-1', path: '../secret.txt' },
      }),
    ).toThrow('editor_path_not_allowed')
    expect(() =>
      validateSarahEditorCommandTarget({
        _tag: 'context_read',
        target: { workspaceRef: 'workspace-1', path: 'src/app.ts' },
        startLine: 1,
        endLine: 1_000,
      }),
    ).toThrow('editor_range_not_allowed')
  })

  test('enforces the start-agent-thread message limit in UTF-8 bytes', () => {
    expect(
      validateSarahEditorCommandTarget({
        _tag: 'start_agent_thread',
        message: 'Inspect the current test failure.',
        presentation: 'background',
      }),
    ).toMatchObject({ _tag: 'start_agent_thread' })
    expect(() =>
      validateSarahEditorCommandTarget({
        _tag: 'start_agent_thread',
        message: '😀'.repeat(4_097),
        presentation: 'foreground',
      }),
    ).toThrow('agent_thread_message_not_allowed')
  })
})
