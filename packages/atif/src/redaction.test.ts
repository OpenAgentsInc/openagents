import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  ATIF_PINNED_SCHEMA_VERSION,
  AtifStep,
  AtifTrajectory,
  atifTraceTripwire,
  validateAtifTrajectory,
} from "./trace-schema.ts"
import {
  TraceRedactor,
  TraceRedactorLive,
  redactString,
  redactTraceString,
  redactTraceValue,
  redactValue,
  type RedactionResult,
} from "./redaction.ts"

const red = (s: string): RedactionResult<string> => redactString(s)

const SECRET_FIXTURES: ReadonlyArray<{
  label: string
  raw: string
  leak: string
  category: string
}> = [
  {
    label: "OpenAI sk- key",
    raw: "use sk-abcdefghijklmnop0123456789ABCD now",
    leak: "sk-abcdefghijklmnop",
    category: "provider_key",
  },
  {
    label: "OpenRouter sk-or- key",
    raw: "sk-or-v1-0011223344556677889900aabbccddeeff00112233",
    leak: "0011223344556677",
    category: "provider_key",
  },
  {
    label: "Anthropic sk-ant- key",
    raw: "key sk-ant-api03-AbCdEf0123456789AbCdEf done",
    leak: "AbCdEf0123456789",
    category: "provider_key",
  },
  {
    label: "Stripe sk_live_ key",
    raw: "STRIPE=sk_live_0123456789abcdefABCDEF rest",
    leak: "sk_live_0123456789",
    category: "provider_key",
  },
  {
    label: "oa_agent_ token",
    raw: "bearer creds oa_agent_AbCdEf123456789xyz end",
    leak: "oa_agent_AbCdEf",
    category: "oa_agent_token",
  },
  {
    label: "generic oa_ token",
    raw: "auth oa_live_abcdef0123456789abcdef next",
    leak: "oa_live_abcdef0123456789",
    category: "oa_token",
  },
  {
    label: "X verification code",
    raw: "Code: oa-x-9f2bc-defG",
    leak: "9f2bc-defG",
    category: "x_code",
  },
  {
    label: "owner identifier",
    raw: "owner github:12345678 claimed it",
    leak: "12345678",
    category: "owner_id",
  },
  {
    label: "AWS access key",
    raw: "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE here",
    leak: "AKIAIOSFODNN7EXAMPLE",
    category: "aws_key",
  },
  {
    label: "Google API key",
    raw: "gkey AIzaSyA1234567890abcdefghijklmnopqrstuv ok",
    leak: "AIzaSyA1234567890",
    category: "google_key",
  },
  {
    label: "Slack token",
    raw: "slack xoxb-1234567890-abcdefghijkl set",
    leak: "xoxb-1234567890",
    category: "slack_token",
  },
  {
    label: "GitHub token",
    raw: "ghp_0123456789abcdefABCDEF0123456789abcd token",
    leak: "ghp_0123456789",
    category: "github_token",
  },
  {
    label: "JWT",
    raw: "token=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
    leak: "SflKxwRJSMeKKF2QT4",
    category: "jwt",
  },
  {
    label: "Bearer credential",
    raw: "Authorization: Bearer abcdef0123456789ABCDEFxyz",
    leak: "abcdef0123456789ABCDEFxyz",
    category: "bearer",
  },
  {
    label: "env secret line",
    raw: "DATABASE_PASSWORD=hunter2supersecretvalue more",
    leak: "hunter2supersecretvalue",
    category: "env_secret",
  },
  {
    label: "email PII",
    raw: "contact me at jane.doe@example.com please",
    leak: "jane.doe@example.com",
    category: "email",
  },
  {
    label: "home path",
    raw: "open /Users/alice/work/secret.txt then",
    leak: "/Users/alice",
    category: "home_path",
  },
  {
    label: "linux home path",
    raw: "cat /home/bob/.ssh/id_rsa fails",
    leak: "/home/bob",
    category: "home_path",
  },
  {
    label: "file URL",
    raw: "see file:///Users/carol/private/doc.md now",
    leak: "carol",
    category: "file_url",
  },
  {
    label: ".secrets path",
    raw: "read .secrets/tailnet.env carefully",
    leak: ".secrets/tailnet.env",
    category: "secrets_path",
  },
  {
    label: "lightning invoice",
    raw: "pay lnbc2500u1pvjluezpp5qqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqqqsyqcyq5rqwzqfqypq more",
    leak: "lnbc2500u1",
    category: "wallet_or_payment",
  },
  {
    label: "bolt12 offer",
    raw: "offer lno1pqpsgq0123456789abcdefghijklmnop here",
    leak: "lno1pqpsgq",
    category: "wallet_or_payment",
  },
  {
    label: "on-chain bc1 address",
    raw: "send to bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq please",
    leak: "bc1qar0srrr7xfkvy",
    category: "wallet_or_payment",
  },
  {
    label: "xpub",
    raw: "xpub6CUGRUonZSQ4TWtTMmzXdrXDtypWKiKrhko4egpiMZbpiaQL2jkwSB1icqYh2cfDfVxdx4df189oLKnC5fSwqPfgyP3hooxujYzAu3fDVmz key",
    leak: "xpub6CUGRUonZSQ4T",
    category: "wallet_or_payment",
  },
  {
    label: "private internal IP",
    raw: "host 10.0.0.42 and 100.96.1.2 internal",
    leak: "10.0.0.42",
    category: "ip",
  },
  {
    label: "PEM private key",
    raw: "k=-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEABBBBBBBB\nmoredata==\n-----END OPENSSH PRIVATE KEY-----\nend",
    leak: "b3BlbnNzaC1rZXkt",
    category: "private_key",
  },
  {
    label: "mnemonic seed phrase",
    raw: "seed legal winner thank year wave sausage worth useful legal winner thank yellow done",
    leak: "legal winner thank year wave sausage worth useful legal winner thank yellow",
    category: "mnemonic",
  },
]

describe("redactString", () => {
  for (const fx of SECRET_FIXTURES) {
    test(`${fx.label} is scrubbed`, () => {
      const r = red(fx.raw)
      expect(r.value).not.toContain(fx.leak)
      expect(r.report.counts[fx.category] ?? 0).toBeGreaterThanOrEqual(1)
      expect(r.report.total).toBeGreaterThanOrEqual(1)
    })
  }

  test("known public false positives are preserved", () => {
    const r = red(
      "See https://openagents.com/trace/abc-123 and https://github.com/OpenAgentsInc/openagents/issues/6219 on openagents/khala for #6219.",
    )
    expect(r.value).toContain("https://openagents.com/trace/abc-123")
    expect(r.value).toContain(
      "https://github.com/OpenAgentsInc/openagents/issues/6219",
    )
    expect(r.value).toContain("openagents/khala")
    expect(r.value).toContain("#6219")
    expect(r.report.total).toBe(0)
  })

  test("is deterministic", () => {
    const input = SECRET_FIXTURES.map(f => f.raw).join(" | ")
    const a = redactTraceString(input)
    const b = redactTraceString(input)
    expect(a.value).toBe(b.value)
    expect(a.report).toEqual(b.report)
  })
})

describe("redactValue", () => {
  test("walks deeply, preserves numeric metrics, and redacts usernames", () => {
    const value = {
      metrics: { prompt_tokens: 12, completion_tokens: 34, cached_tokens: 7 },
      path: "wrote /Users/alice/work/x.ts",
      listing: "drwxr-xr-x@ 3 alice staff 96 file",
      slug: "/private/tmp/-Users-alice-work/log",
      token: "oa_agent_AbCdEf123456789xyz",
    }
    const r = redactValue(value)
    const json = JSON.stringify(r.value)

    expect(r.value.metrics).toEqual(value.metrics)
    expect(json).not.toContain("/Users/alice")
    expect(json).not.toContain(" alice ")
    expect(json).not.toContain("oa_agent_AbCdEf")
    expect(r.report.counts.home_path).toBeGreaterThanOrEqual(1)
    expect(r.report.counts.username).toBeGreaterThanOrEqual(1)
  })
})

describe("redact-before-tripwire safety bar", () => {
  const leakBlob = SECRET_FIXTURES.map(f => f.raw).join("\n")

  const buildTrajectory = (userMessage: string): AtifTrajectory =>
    new AtifTrajectory({
      schema_version: ATIF_PINNED_SCHEMA_VERSION,
      trajectory_id: "redaction-test-1",
      session_id: "chatcmpl-redaction-test-1",
      visibility: "owner_only",
      agent: {
        name: "Khala",
        version: "gateway-1",
        model_name: "openagents/khala",
      },
      steps: [
        new AtifStep({ step_id: 1, source: "user", message: userMessage }),
        new AtifStep({
          step_id: 2,
          source: "agent",
          message: "Here is the result, redacted appropriately.",
          model_name: "openagents/khala",
          metrics: { prompt_tokens: 10, completion_tokens: 5 },
        }),
      ],
    })

  test("the leaky control trips the backstop", () => {
    expect(atifTraceTripwire(buildTrajectory(leakBlob)).length).toBeGreaterThan(0)
  })

  test("the scrubbed trajectory passes validation and tripwire", () => {
    const { value } = redactTraceValue(buildTrajectory(leakBlob))
    expect(validateAtifTrajectory(value as AtifTrajectory)).toEqual([])
    expect(atifTraceTripwire(value as AtifTrajectory)).toEqual([])
    expect(JSON.stringify(value)).toContain("openagents/khala")
  })
})

describe("TraceRedactor Effect service", () => {
  test("redacts through the Default layer", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const redactor = yield* TraceRedactor
        return yield* redactor.redact({
          message: "Bearer abcdef0123456789ABCDEF in /Users/carol/x",
        })
      }).pipe(Effect.provide(TraceRedactor.Default)),
    )

    expect(JSON.stringify(result.value)).not.toContain("abcdef0123456789ABCDEF")
    expect(JSON.stringify(result.value)).not.toContain("/Users/carol")
    expect(result.report.counts.bearer).toBe(1)
    expect(result.report.counts.home_path).toBe(1)
  })

  test("redacts through the legacy live layer alias", async () => {
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const redactor = yield* TraceRedactor
        return yield* redactor.redactText("email d@example.com")
      }).pipe(Effect.provide(TraceRedactorLive)),
    )

    expect(result.value).toContain("[REDACTED:email]")
  })
})
