import { describe, expect, test, vi } from 'vitest'

import type { HostedGeminiInferenceCallerInput } from './autopilot-hosted-gemini-inference-bridge'
import {
  MAX_HOSTED_GEMINI_SNIPPET_CHARS,
  resolveHostedGeminiPromptContext,
  sanitizeResolvedSnippet,
} from './autopilot-hosted-gemini-content-resolver'

const callerInput: HostedGeminiInferenceCallerInput = {
  assignmentRef: 'assignment.work-1.a1',
  objectiveRefs: ['acceptance.work-1.a1.criteria', 'acceptance.work-1.a1.tests'],
  taskRef: 'task.work-1.a1',
  workOrderRef: 'work_order.work-1',
}

describe('sanitizeResolvedSnippet', () => {
  test('collapses whitespace and control characters to single spaces', () => {
    expect(sanitizeResolvedSnippet('  hello\t\n  world \r\n ')).toBe('hello world')
  })

  test('returns undefined for empty or whitespace-only content', () => {
    expect(sanitizeResolvedSnippet('')).toBeUndefined()
    expect(sanitizeResolvedSnippet('   \n\t  ')).toBeUndefined()
  })

  test('bounds output to MAX_HOSTED_GEMINI_SNIPPET_CHARS', () => {
    const cleaned = sanitizeResolvedSnippet('a'.repeat(MAX_HOSTED_GEMINI_SNIPPET_CHARS + 500))
    expect(cleaned?.length).toBe(MAX_HOSTED_GEMINI_SNIPPET_CHARS)
  })

  test('drops a snippet bearing a PEM private key fingerprint', () => {
    const leaky = 'context -----BEGIN RSA PRIVATE KEY----- MIIB more text'
    expect(sanitizeResolvedSnippet(leaky)).toBeUndefined()
  })

  test('drops snippets bearing token-shaped secrets', () => {
    expect(
      sanitizeResolvedSnippet('use key sk-abcdef012345678901234 now'),
    ).toBeUndefined()
    expect(
      sanitizeResolvedSnippet('aws AKIAABCDEFGHIJKLMNOP rotate'),
    ).toBeUndefined()
    expect(
      sanitizeResolvedSnippet('token ghp_abcdefghijklmnopqrstuvwxyz0123 ok'),
    ).toBeUndefined()
  })
})

describe('resolveHostedGeminiPromptContext', () => {
  test('dereferences task + objective refs into sanitized content', async () => {
    const resolver = vi.fn(async (ref: string) => `content for ${ref}`)
    const context = await resolveHostedGeminiPromptContext(callerInput, resolver)
    expect(context).toBeDefined()
    expect(context?.taskContent).toBe('content for task.work-1.a1')
    expect(context?.objectiveContents).toEqual([
      'content for acceptance.work-1.a1.criteria',
      'content for acceptance.work-1.a1.tests',
    ])
    expect(resolver).toHaveBeenCalledTimes(3)
  })

  test('declines (undefined) when the task ref cannot be resolved', async () => {
    const resolver = vi.fn(async (ref: string) =>
      ref === callerInput.taskRef ? undefined : `content for ${ref}`,
    )
    expect(
      await resolveHostedGeminiPromptContext(callerInput, resolver),
    ).toBeUndefined()
  })

  test('declines (undefined) when the task ref resolves to only-secret content', async () => {
    const resolver = vi.fn(async () => 'sk-abcdef012345678901234567')
    expect(
      await resolveHostedGeminiPromptContext(callerInput, resolver),
    ).toBeUndefined()
  })

  test('skips empty, unresolvable, and unsafe objective refs', async () => {
    const input: HostedGeminiInferenceCallerInput = {
      ...callerInput,
      objectiveRefs: [
        'acceptance.ok',
        '   ',
        'acceptance.missing',
        'acceptance.leaky',
      ],
    }
    const resolver = vi.fn(async (ref: string) => {
      if (ref === input.taskRef) return 'real task'
      if (ref === 'acceptance.ok') return 'first criterion'
      if (ref === 'acceptance.missing') return undefined
      if (ref === 'acceptance.leaky') return 'ghp_abcdefghijklmnopqrstuvwxyz0123'
      return undefined
    })
    const context = await resolveHostedGeminiPromptContext(input, resolver)
    expect(context?.taskContent).toBe('real task')
    expect(context?.objectiveContents).toEqual(['first criterion'])
    // The blank ref is never dereferenced.
    expect(resolver).not.toHaveBeenCalledWith('   ')
  })

  test('declines (undefined) when the task ref is blank without calling resolver', async () => {
    const resolver = vi.fn(async (ref: string) => `content for ${ref}`)
    expect(
      await resolveHostedGeminiPromptContext(
        { ...callerInput, taskRef: '   ' },
        resolver,
      ),
    ).toBeUndefined()
    expect(resolver).not.toHaveBeenCalled()
  })
})
