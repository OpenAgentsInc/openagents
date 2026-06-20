import { describe, expect, test } from 'vitest'

const smoke = await import('./ep239-staging-smoke.mjs')

describe('Ep239 staging funded-loop smoke', () => {
  test('redact scrubs agent tokens, bearer headers, and stripe-style secrets', () => {
    const dirty = {
      authorization: 'Bearer oa_agent_super_secret_value_123',
      note: 'token oa_agent_abcDEF-_123 must not appear',
      stripe: 'sk-test_abcdEFGH1234',
    }
    const cleaned = smoke.redact(dirty)
    expect(cleaned).not.toContain('oa_agent_super_secret_value_123')
    expect(cleaned).not.toContain('oa_agent_abcDEF-_123')
    expect(cleaned).not.toContain('sk-test_abcdEFGH1234')
    expect(cleaned).toContain('[REDACTED]')
  })

  test('redact leaves non-secret refs (chatcmpl, receipt) intact for dereference', () => {
    const ref = {
      chatcmplId: 'chatcmpl_abc123',
      receiptRef: 'receipt.inference.charge.chatcmpl_abc123',
    }
    const cleaned = smoke.redact(ref)
    expect(cleaned).toContain('chatcmpl_abc123')
    expect(cleaned).toContain('receipt.inference.charge.chatcmpl_abc123')
  })

  test('presenceTag never echoes the value, only a length fingerprint', () => {
    const tag = smoke.presenceTag('oa_agent_secret')
    expect(tag).not.toContain('oa_agent_secret')
    expect(tag).toMatch(/^present\(len=\d+\)$/)
    expect(smoke.presenceTag(undefined)).toBe('absent')
    expect(smoke.presenceTag('')).toBe('absent')
  })

  test('assertStagingHost rejects production hosts', () => {
    expect(() =>
      smoke.assertStagingHost('https://openagents.com'),
    ).toThrowError(/production/i)
    expect(() =>
      smoke.assertStagingHost('https://auth.openagents.com/callback'),
    ).toThrowError(/production/i)
    expect(() =>
      smoke.assertStagingHost('https://www.openagents.com'),
    ).toThrowError(/production/i)
  })

  test('assertStagingHost accepts the isolated staging Worker host', () => {
    expect(
      smoke.assertStagingHost(
        'https://openagents-staging.openagents.workers.dev',
      ),
    ).toBe('openagents-staging.openagents.workers.dev')
  })

  test('parseArgs defaults to the staging Worker base url', () => {
    const options = smoke.parseArgs([])
    expect(options.baseUrl).toContain('openagents-staging')
    expect(options.json).toBe(false)
    expect(options.help).toBe(false)
  })

  test('parseArgs honors --base-url, --json, and --help; rejects unknown flags', () => {
    const options = smoke.parseArgs(['--base-url', 'https://x.example', '--json'])
    expect(options.baseUrl).toBe('https://x.example')
    expect(options.json).toBe(true)
    expect(smoke.parseArgs(['--help']).help).toBe(true)
    expect(() => smoke.parseArgs(['--nope'])).toThrowError(/Unknown argument/)
  })
})
