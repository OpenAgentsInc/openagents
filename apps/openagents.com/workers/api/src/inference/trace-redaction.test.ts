// Tests for the default-on capture trace redactor (#6219, #6293).
//
// The safety bar: a battery of secret / PII / path / email / URL / wallet
// fixtures must each be scrubbed (nothing leaks), the allowlist must be
// preserved, the engine must be deterministic, and CRUCIALLY a trajectory
// containing every one of these MUST pass `atifTraceTripwire` AFTER redaction
// (redact-before-tripwire; the tripwire is only the fail-closed backstop).

import { describe, expect, test } from 'vitest'
import {
  ATIF_PINNED_SCHEMA_VERSION,
  AtifStep,
  AtifTrajectory,
  atifTraceTripwire,
  validateAtifTrajectory,
} from '../atif-trace-schema'
import {
  redactTraceString,
  redactTraceValue,
  type TraceRedactionResult,
} from './trace-redaction'

const red = (s: string): TraceRedactionResult<string> => redactTraceString(s)

// Concrete, secret-SHAPED fixtures (none real). Each entry: a label, the raw
// string, the leak substring that must NOT survive, and the expected category.
const SECRET_FIXTURES: ReadonlyArray<{
  label: string
  raw: string
  leak: string
  category: string
}> = [
  {
    label: 'OpenAI sk- key',
    raw: 'use sk-abcdefghijklmnop0123456789ABCD now',
    leak: 'sk-abcdefghijklmnop',
    category: 'provider_key',
  },
  {
    label: 'OpenRouter sk-or- key',
    raw: 'sk-or-v1-0011223344556677889900aabbccddeeff00112233',
    leak: '0011223344556677',
    category: 'provider_key',
  },
  {
    label: 'Anthropic sk-ant- key',
    raw: 'key sk-ant-api03-AbCdEf0123456789AbCdEf done',
    leak: 'AbCdEf0123456789',
    category: 'provider_key',
  },
  {
    label: 'Stripe sk_live_ key',
    raw: 'STRIPE=sk_live_0123456789abcdefABCDEF rest',
    leak: 'sk_live_0123456789',
    category: 'provider_key',
  },
  {
    label: 'oa_agent_ token',
    raw: 'bearer creds oa_agent_AbCdEf123456789xyz end',
    leak: 'oa_agent_AbCdEf',
    category: 'oa_agent_token',
  },
  {
    label: 'generic oa_ token',
    raw: 'auth oa_live_abcdef0123456789abcdef next',
    leak: 'oa_live_abcdef0123456789',
    category: 'oa_token',
  },
  {
    label: 'AWS access key',
    raw: 'AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE here',
    leak: 'AKIAIOSFODNN7EXAMPLE',
    category: 'aws_key',
  },
  {
    label: 'Google API key',
    raw: 'gkey AIzaSyA1234567890abcdefghijklmnopqrstuv ok',
    leak: 'AIzaSyA1234567890',
    category: 'google_key',
  },
  {
    label: 'Slack token',
    raw: 'slack xoxb-1234567890-abcdefghijkl set',
    leak: 'xoxb-1234567890',
    category: 'slack_token',
  },
  {
    label: 'GitHub token',
    raw: 'ghp_0123456789abcdefABCDEF0123456789abcd token',
    leak: 'ghp_0123456789',
    category: 'github_token',
  },
  {
    label: 'JWT',
    raw: 'token=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c',
    leak: 'SflKxwRJSMeKKF2QT4',
    category: 'jwt',
  },
  {
    label: 'Bearer credential',
    raw: 'Authorization: Bearer abcdef0123456789ABCDEFxyz',
    leak: 'abcdef0123456789ABCDEFxyz',
    category: 'bearer',
  },
  {
    label: 'env secret line',
    raw: 'DATABASE_PASSWORD=hunter2supersecretvalue more',
    leak: 'hunter2supersecretvalue',
    category: 'env_secret',
  },
  {
    label: 'email PII',
    raw: 'contact me at jane.doe@example.com please',
    leak: 'jane.doe@example.com',
    category: 'email',
  },
  {
    label: 'home path',
    raw: 'open /Users/alice/work/secret.txt then',
    leak: '/Users/alice',
    category: 'home_path',
  },
  {
    label: 'linux home path',
    raw: 'cat /home/bob/.ssh/id_rsa fails',
    leak: '/home/bob',
    category: 'home_path',
  },
  {
    label: 'file url',
    raw: 'see file:///Users/carol/private/doc.md now',
    leak: 'carol',
    category: 'file_url',
  },
  {
    label: '.secrets path',
    raw: 'read .secrets/tailnet.env carefully',
    leak: '.secrets/tailnet.env',
    category: 'secrets_path',
  },
  {
    label: 'lightning invoice',
    raw: 'pay lnbc2500u1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypq more',
    leak: 'lnbc2500u1',
    category: 'wallet_or_payment',
  },
  {
    label: 'bolt12 offer',
    raw: 'offer lno1pqpsgq0123456789abcdefghijklmnop here',
    leak: 'lno1pqsgq',
    category: 'wallet_or_payment',
  },
  {
    label: 'on-chain bc1 address',
    raw: 'send to bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq please',
    leak: 'bc1qar0srrr7xfkvy',
    category: 'wallet_or_payment',
  },
  {
    label: 'xpub',
    raw: 'xpub6CUGRUonZSQ4TWtTMmzXdrXDtypWKiKrhko4egpiMZbpiaQL2jkwSB1icqYh2cfDfVxdx4df189oLKnC5fSwqPfgyP3hooxujYzAu3fDVmz key',
    leak: 'xpub6CUGRUonZSQ4T',
    category: 'wallet_or_payment',
  },
  {
    label: 'private internal IP',
    raw: 'host 10.0.0.42 and 100.96.1.2 internal',
    leak: '10.0.0.42',
    category: 'ip',
  },
  {
    label: 'PEM private key',
    raw: 'k=-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEABBBBBBBB\nmoredata==\n-----END OPENSSH PRIVATE KEY-----\nend',
    leak: 'b3BlbnNzaC1rZXkt',
    category: 'private_key',
  },
  {
    label: 'mnemonic seed phrase',
    raw: 'seed legal winner thank year wave sausage worth useful legal winner thank yellow done',
    leak: 'legal winner thank year wave sausage worth useful legal winner thank yellow',
    category: 'mnemonic',
  },
]

describe('redactTraceString — every secret/PII fixture is scrubbed', () => {
  for (const fx of SECRET_FIXTURES) {
    test(fx.label, () => {
      const r = red(fx.raw)
      expect(r.value).not.toContain(fx.leak)
      expect(r.report.counts[fx.category] ?? 0).toBeGreaterThanOrEqual(1)
      expect(r.report.total).toBeGreaterThanOrEqual(1)
    })
  }
})

describe('redactTraceString — allowlist is preserved', () => {
  test('the public model id survives', () => {
    const r = red('this ran on openagents/khala the public model')
    expect(r.value).toContain('openagents/khala')
  })
  test('public openagents.com URL survives', () => {
    const r = red('see https://openagents.com/trace/abc-123 for the trace')
    expect(r.value).toContain('https://openagents.com/trace/abc-123')
  })
  test('public OpenAgentsInc GitHub URL survives', () => {
    const r = red('issue at https://github.com/OpenAgentsInc/openagents/issues/6293')
    expect(r.value).toContain(
      'https://github.com/OpenAgentsInc/openagents/issues/6293',
    )
  })
  test('issue ref survives', () => {
    const r = red('per #6293 we capture free traffic')
    expect(r.value).toContain('#6293')
  })
  test('benign prose is unchanged (no false positives)', () => {
    const benign = 'The user asked Khala to write a function and it did.'
    const r = red(benign)
    expect(r.value).toBe(benign)
    expect(r.report.total).toBe(0)
  })
})

describe('redactTraceString — deterministic', () => {
  test('same input -> same output, twice', () => {
    const raw = SECRET_FIXTURES.map(f => f.raw).join(' | ')
    const a = redactTraceString(raw)
    const b = redactTraceString(raw)
    expect(a.value).toBe(b.value)
    expect(a.report.total).toBe(b.report.total)
  })
})

describe('redactTraceValue — deep walk, numbers preserved', () => {
  test('redacts string leaves, never numeric metric fields', () => {
    const trajectory = {
      schema_version: ATIF_PINNED_SCHEMA_VERSION,
      message: 'token oa_agent_AbCdEf123456789xyz mail bob@x.io',
      metrics: { prompt_tokens: 12, completion_tokens: 34, cost_usd: 0.001 },
      nested: ['sk-abcdefghijklmnop0123456789ABCD', 99],
    }
    const r = redactTraceValue(trajectory)
    expect(JSON.stringify(r.value)).not.toContain('oa_agent_AbCdEf')
    expect(JSON.stringify(r.value)).not.toContain('bob@x.io')
    expect(JSON.stringify(r.value)).not.toContain('sk-abcdefghijklmnop')
    // numeric metrics untouched
    expect(r.value.metrics.prompt_tokens).toBe(12)
    expect(r.value.metrics.completion_tokens).toBe(34)
    expect(r.value.metrics.cost_usd).toBe(0.001)
    expect(r.value.nested[1]).toBe(99)
    expect(r.report.total).toBeGreaterThanOrEqual(3)
  })
})

describe('SAFETY BAR: a redacted trajectory passes the tripwire', () => {
  // Build a real ATIF trajectory whose CONTENT is a leak of EVERY fixture, then
  // confirm: (1) the un-redacted trajectory TRIPS the tripwire, and (2) the
  // redacted trajectory passes both the structural validator and the tripwire.
  const leakBlob = SECRET_FIXTURES.map(f => f.raw).join('\n')

  const buildTrajectory = (userMessage: string): AtifTrajectory =>
    new AtifTrajectory({
      schema_version: ATIF_PINNED_SCHEMA_VERSION,
      trajectory_id: 'redaction-test-1',
      session_id: 'chatcmpl-redaction-test-1',
      visibility: 'owner_only',
      agent: { name: 'Khala', version: 'gateway-1', model_name: 'openagents/khala' },
      steps: [
        new AtifStep({ step_id: 1, source: 'user', message: userMessage }),
        new AtifStep({
          step_id: 2,
          source: 'agent',
          message: 'Here is the result, redacted appropriately.',
          model_name: 'openagents/khala',
          metrics: { prompt_tokens: 10, completion_tokens: 5 },
        }),
      ],
    })

  test('the leaky trajectory trips the tripwire (control)', () => {
    const trips = atifTraceTripwire(buildTrajectory(leakBlob))
    expect(trips.length).toBeGreaterThan(0)
  })

  test('after redactTraceValue, the trajectory passes the tripwire', () => {
    const raw = buildTrajectory(leakBlob)
    const { value: redacted } = redactTraceValue(raw)
    // Structural validity is preserved.
    expect(validateAtifTrajectory(redacted as AtifTrajectory)).toEqual([])
    // The safety bar: NOTHING the tripwire rejects survives.
    const findings = atifTraceTripwire(redacted as AtifTrajectory)
    expect(findings).toEqual([])
  })

  test('the public model id is still present after redaction', () => {
    const raw = buildTrajectory(leakBlob)
    const { value: redacted } = redactTraceValue(raw)
    expect(JSON.stringify(redacted)).toContain('openagents/khala')
  })
})
