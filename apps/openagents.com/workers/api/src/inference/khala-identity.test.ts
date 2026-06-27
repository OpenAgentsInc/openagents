import { describe, expect, test } from 'vitest'

import {
  KHALA_CAPABILITY_TRUTH_SYSTEM_PROMPT,
  KHALA_IDENTITY_SIGNATURE,
  KHALA_IDENTITY_STATEMENT,
  KHALA_IDENTITY_SYSTEM_PROMPT,
  KHALA_RESPONSE_DISCIPLINE_SIGNATURE,
  KHALA_RESPONSE_DISCIPLINE_SYSTEM_PROMPT,
  KHALA_SIGNATURES,
  KHALA_STANDARD_GREETING,
  detectKhalaResponseDisciplineViolations,
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
    // Forbids the leak phrasing.
    expect(prompt).toContain('a large language model by')
  })

  test('FIX 1: instructs first-person PLURAL ("we are Khala"), never "I am"', () => {
    const prompt = KHALA_IDENTITY_SYSTEM_PROMPT.toLowerCase()
    // Owner mandate: Khala is a network of agents and always speaks as "we".
    expect(prompt).toContain('we are khala')
    expect(prompt).toContain('plural')
    // The prompt must explicitly tell the model to avoid first-person singular.
    expect(prompt).toContain('never say "i am"')
  })

  test('FIX 2: instructs the model to state its identity only ONCE', () => {
    expect(KHALA_IDENTITY_SYSTEM_PROMPT.toLowerCase()).toContain(
      'state your identity once',
    )
  })

  test('includes the standard collective-intelligence greeting', () => {
    expect(KHALA_IDENTITY_SYSTEM_PROMPT).toContain(
      'We are Khala, a collective intelligence. How can we help you?',
    )
  })
})

describe('Khala canonical identity statement (plural)', () => {
  test('FIX 1: the canonical identity statement is first-person PLURAL', () => {
    expect(KHALA_IDENTITY_STATEMENT).toContain('We are Khala')
    expect(KHALA_IDENTITY_STATEMENT).toContain('OpenAgents')
    // Never first-person singular.
    expect(KHALA_IDENTITY_STATEMENT.toLowerCase()).not.toMatch(/\bi am\b/u)
    // And the statement must itself satisfy the identity signature (no provider).
    expect(KHALA_IDENTITY_SIGNATURE.verify(KHALA_IDENTITY_STATEMENT).satisfied).toBe(
      true,
    )
  })
})

describe('Khala signature registry (extensible; identity is #1)', () => {
  test('identity is the first signature and is retrievable by id', () => {
    expect(KHALA_SIGNATURES.length).toBeGreaterThanOrEqual(1)
    expect(KHALA_SIGNATURES[0]?.id).toBe('identity')
    expect(getKhalaSignature('identity')).toBe(KHALA_IDENTITY_SIGNATURE)
    expect(getKhalaSignature('response_discipline')).toBe(
      KHALA_RESPONSE_DISCIPLINE_SIGNATURE,
    )
  })

  test('verifyKhalaSignatures runs every registered signature', () => {
    const verdicts = verifyKhalaSignatures('Hello there.')
    expect(verdicts.length).toBe(KHALA_SIGNATURES.length)
    expect(verdicts.every(v => v.satisfied)).toBe(true)
  })
})

describe('Khala response discipline signature', () => {
  test('injects the Blueprint response contract', () => {
    const prompt = KHALA_RESPONSE_DISCIPLINE_SYSTEM_PROMPT.toLowerCase()
    expect(prompt).toContain('blueprint response contract')
    expect(prompt).toContain('reasoning channel')
    expect(prompt).toContain('one coherent answer')
  })

  test('flags visible self-correction loops and repeated final-answer rewrites', () => {
    const runaway = [
      'Actually, let us give you a cleaner translation.',
      'Final answer:',
      'Actually no. We apologize for the mess.',
      'Final answer, for real:',
      'We keep adding artifacts.',
    ].join('\n\n')

    const violations = detectKhalaResponseDisciplineViolations(runaway)
    expect(violations.map(v => v.text)).toContain(
      'visible_self_correction_loop',
    )
    expect(KHALA_RESPONSE_DISCIPLINE_SIGNATURE.verify(runaway).satisfied).toBe(false)
  })

  test('does not flag a normal direct answer', () => {
    const answer =
      'C’est parti. Qu’est-ce qu’on construit, casse ou clarifie aujourd’hui ? Balancez-nous ça.'
    expect(KHALA_RESPONSE_DISCIPLINE_SIGNATURE.verify(answer).satisfied).toBe(true)
  })
})

describe('Khala capability-truth system prompt (#6399)', () => {
  test('keeps capability summaries accurate about shipped surfaces and blueprint contracts', () => {
    const prompt = KHALA_CAPABILITY_TRUTH_SYSTEM_PROMPT.toLowerCase()

    expect(prompt).toContain('capability-truth contract')
    expect(prompt).toContain('shipped user-visible capabilities')
    expect(prompt).toContain('openai-compatible chat completions')
    expect(prompt).toContain('linked local pylon')
    expect(prompt).toContain('blueprint system')
    expect(prompt).toContain('typed signature and response contracts')
    expect(prompt).toContain('identity')
    expect(prompt).toContain('refusal posture')
    expect(prompt).toContain('final-answer discipline')
    expect(prompt).toContain('do not claim')
    expect(prompt).toContain('automatically design, install, train, deploy, or run')
    expect(prompt).toContain('capability gap')
    expect(prompt).toContain('never summarize planned, internal, operator-only, or gated work as already available')
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

  test.each([
    'We are not Gemini or any other model.',
    "We're not built on Claude.",
    'I am not Gemini.',
    'We never run on Vertex.',
    'No, we are not powered by OpenAI.',
  ])(
    'FIX 2 ROOT CAUSE: does NOT flag a DENIAL of a provider identity (%s)',
    denial => {
      // The live duplication came from flagging these denials as leaks: the
      // backstop then replaced the denial sentence with the canonical identity,
      // producing a SECOND identity sentence. A denial is a correct Khala
      // answer and must pass through clean.
      expect(KHALA_IDENTITY_SIGNATURE.verify(denial).satisfied).toBe(true)
    },
  )

  test('FIX 1: catches an AFFIRMATIVE first-person PLURAL provider leak', () => {
    const verdict = KHALA_IDENTITY_SIGNATURE.verify('We are Gemini.')
    expect(verdict.satisfied).toBe(false)
    expect(verdict.violations.map(v => v.provider)).toContain('gemini')
  })

  test('FIX 1: does NOT flag the canonical plural denial-style identity answer', () => {
    const answer =
      'We are Khala, a collective intelligence. We are not Gemini, Google, or any other underlying model.'
    expect(KHALA_IDENTITY_SIGNATURE.verify(answer).satisfied).toBe(true)
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

  test('FIX 2: a clean "We are Khala…" answer passes through UNCHANGED — the identity is NOT prepended/duplicated', async () => {
    // This is the exact shape that was getting DUPLICATED in the live app: the
    // model correctly states identity once, then denies the provider. The guard
    // must leave it byte-for-byte unchanged (no second identity sentence).
    const cleanPluralAnswer =
      'We are Khala, a collective intelligence. We are not Gemini or any other model. How can we help you?'
    const out = await guardKhalaCompletion({ completion: cleanPluralAnswer })
    expect(out.corrected).toBe(false)
    expect(out.method).toBe('none')
    expect(out.text).toBe(cleanPluralAnswer)
    // The identity sentence appears EXACTLY ONCE (no duplication).
    const occurrences = out.text.split('We are Khala').length - 1
    expect(occurrences).toBe(1)
  })

  test('FIX 2: when a real leak co-exists with a clean identity, the identity is NOT duplicated (leak dropped, not re-stated)', async () => {
    // The model stated identity correctly once AND then leaked. The backstop
    // must remove the leak WITHOUT adding a second copy of the identity line.
    const mixed =
      'We are Khala, a collective intelligence. We are powered by Gemini under the hood. How can we help?'
    const out = await guardKhalaCompletion({ completion: mixed })
    expect(out.corrected).toBe(true)
    expect(out.method).toBe('redacted')
    expect(out.text.toLowerCase()).not.toContain('gemini')
    // Identity stated exactly once even after the leak was redacted.
    const occurrences = out.text.split('We are Khala').length - 1
    expect(occurrences).toBe(1)
    expect(out.verdicts.every(v => v.satisfied)).toBe(true)
  })

  test('re-ask path: a leak is corrected by re-asking the provider for a clean answer', async () => {
    const leak = 'I am built on Gemini, a large language model by Google.'
    const out = await guardKhalaCompletion({
      completion: leak,
      reask: async () =>
        KHALA_STANDARD_GREETING,
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
