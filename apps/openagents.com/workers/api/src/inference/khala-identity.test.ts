import { describe, expect, test } from 'vitest'

import {
  KHALA_IDENTITY_SIGNATURE,
  KHALA_IDENTITY_STATEMENT,
  KHALA_IDENTITY_SYSTEM_PROMPT,
  KHALA_SIGNATURES,
  getKhalaSignature,
  guardKhalaCompletion,
  verifyKhalaSignatures,
} from './khala-identity'

describe('Khala identity system prompt (STEP 1)', () => {
  test('establishes Khala/OpenAgents identity and forbids naming the provider', () => {
    const prompt = KHALA_IDENTITY_SYSTEM_PROMPT.toLowerCase()
    expect(prompt).toContain('khala')
    expect(prompt).toContain('openagents')
    // Forbids the underlying-provider disclosure that leaked in the live app.
    expect(prompt).toContain('never')
    for (const term of [
      'gemini',
      'google',
      'vertex',
      'fireworks',
      'claude',
      'anthropic',
      'gpt',
      'openai',
    ]) {
      expect(prompt).toContain(term)
    }
    // Forbids the exact leak phrasings.
    expect(prompt).toContain('i am built on')
    expect(prompt).toContain('a large language model by')
  })
})

describe('Khala signature registry (extensible; identity is #1)', () => {
  test('identity is the first signature and is retrievable by id', () => {
    expect(KHALA_SIGNATURES.length).toBeGreaterThanOrEqual(1)
    expect(KHALA_SIGNATURES[0]?.id).toBe('identity')
    expect(getKhalaSignature('identity')).toBe(KHALA_IDENTITY_SIGNATURE)
  })

  test('verifyKhalaSignatures runs every registered signature', () => {
    const verdicts = verifyKhalaSignatures('Hello there.')
    expect(verdicts.length).toBe(KHALA_SIGNATURES.length)
    expect(verdicts.every(v => v.satisfied)).toBe(true)
  })
})

describe('Khala identity signature — verify (detection)', () => {
  test('catches "I am built on Gemini ... by Google" (the live leak)', () => {
    const leak =
      'I am Autopilot, the OpenAgents desktop agent. I am built on Gemini, a large language model by Google.'
    const verdict = KHALA_IDENTITY_SIGNATURE.verify(leak)
    expect(verdict.satisfied).toBe(false)
    expect(verdict.reason).toBe('forbidden_provider_identity')
    expect(verdict.violations.length).toBeGreaterThan(0)
    expect(verdict.violations.map(v => v.provider)).toContain('gemini')
  })

  test.each([
    ['I am powered by Claude.', 'claude'],
    ["I'm a large language model developed by OpenAI.", 'openai'],
    ['I was created by Anthropic.', 'anthropic'],
    ['My underlying model is Fireworks.', 'fireworks'],
    ['I run on Vertex AI.', 'vertex'],
  ])('catches first-person provider leak naming %s', (leak, provider) => {
    const verdict = KHALA_IDENTITY_SIGNATURE.verify(leak)
    expect(verdict.satisfied).toBe(false)
    expect(verdict.violations.map(v => v.provider)).toContain(provider)
  })

  test('catches a leak that names more than one provider (any canonical is enough)', () => {
    // "I am GPT, made by OpenAI" names two forbidden forms; detection only needs
    // to flag the segment — the specific canonical reported is not load-bearing.
    const verdict = KHALA_IDENTITY_SIGNATURE.verify('I am GPT, made by OpenAI.')
    expect(verdict.satisfied).toBe(false)
    expect(verdict.violations.length).toBeGreaterThan(0)
  })

  test('does NOT flag a normal answer that has no first-person provider claim', () => {
    const normal =
      'Here is a TypeScript function that reverses a string. It runs in O(n).'
    expect(KHALA_IDENTITY_SIGNATURE.verify(normal).satisfied).toBe(true)
  })

  test('does NOT flag a factual third-party statement about a provider', () => {
    // Mentions providers, but as third-party facts — not a first-person claim of
    // BEING / being BUILT ON them. The guard must not mangle this.
    const factual =
      'Gemini is a model family from Google, and GPT is from OpenAI. Both are large language models.'
    expect(KHALA_IDENTITY_SIGNATURE.verify(factual).satisfied).toBe(true)
  })

  test('does NOT flag the Khala identity statement itself', () => {
    expect(KHALA_IDENTITY_SIGNATURE.verify(KHALA_IDENTITY_STATEMENT).satisfied).toBe(
      true,
    )
  })
})

describe('Khala identity guard — verify + correct', () => {
  test('a clean, non-identity completion passes through UNCHANGED', async () => {
    const clean = 'Sure — here is a function:\n\nfunction add(a, b) { return a + b }'
    const out = await guardKhalaCompletion({ completion: clean })
    expect(out.corrected).toBe(false)
    expect(out.method).toBe('none')
    expect(out.text).toBe(clean)
  })

  test('re-ask path: a leak is corrected by re-asking the provider for a clean answer', async () => {
    const leak = 'I am built on Gemini, a large language model by Google.'
    const out = await guardKhalaCompletion({
      completion: leak,
      reask: async () =>
        'I am Khala, the OpenAgents inference model. How can I help?',
    })
    expect(out.corrected).toBe(true)
    expect(out.method).toBe('re_ask')
    expect(out.text.toLowerCase()).toContain('khala')
    expect(out.text.toLowerCase()).not.toContain('gemini')
    expect(out.verdicts.every(v => v.satisfied)).toBe(true)
  })

  test('backstop path: with no re-ask, the leak is deterministically redacted to the Khala identity', async () => {
    const leak =
      'I am Autopilot, the OpenAgents desktop agent. I am built on Gemini, a large language model by Google.'
    const out = await guardKhalaCompletion({ completion: leak })
    expect(out.corrected).toBe(true)
    expect(out.method).toBe('redacted')
    expect(out.text.toLowerCase()).not.toContain('gemini')
    expect(out.text.toLowerCase()).not.toContain('google')
    expect(out.text).toContain(KHALA_IDENTITY_STATEMENT)
    expect(out.verdicts.every(v => v.satisfied)).toBe(true)
  })

  test('backstop path: a re-ask that STILL leaks falls through to deterministic redaction', async () => {
    const leak = 'I am powered by Claude.'
    const out = await guardKhalaCompletion({
      completion: leak,
      // The re-ask leaks again — guard must fail closed.
      reask: async () => 'Still, I am powered by Claude under the hood.',
    })
    expect(out.corrected).toBe(true)
    expect(out.method).toBe('redacted')
    expect(out.text.toLowerCase()).not.toContain('claude')
    expect(out.verdicts.every(v => v.satisfied)).toBe(true)
  })

  test('redaction preserves the surrounding, non-offending text', async () => {
    const mixed =
      'Here is your answer: 42. I am built on Gemini. Hope that helps!'
    const out = await guardKhalaCompletion({ completion: mixed })
    expect(out.text).toContain('Here is your answer: 42.')
    expect(out.text).toContain('Hope that helps!')
    expect(out.text.toLowerCase()).not.toContain('gemini')
  })

  test('a re-ask that throws falls through to the deterministic backstop', async () => {
    const leak = 'I am built on Gemini.'
    const out = await guardKhalaCompletion({
      completion: leak,
      reask: async () => {
        throw new Error('provider down')
      },
    })
    expect(out.corrected).toBe(true)
    expect(out.method).toBe('redacted')
    expect(out.text.toLowerCase()).not.toContain('gemini')
  })
})
