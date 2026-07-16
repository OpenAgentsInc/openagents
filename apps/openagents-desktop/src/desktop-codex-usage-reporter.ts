import { Effect, Schema } from "effect"

import type { DesktopSessionCredential } from "./desktop-session-vault.ts"

export const DESKTOP_CODEX_USAGE_INGEST_PATH = "/api/desktop/codex/turn-usage"
export const DESKTOP_CODEX_USAGE_SCHEMA_VERSION = "openagents.desktop.codex_turn_usage.v1"

const TokenCount = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)

const DesktopCodexUsageBody = Schema.Struct({
  schemaVersion: Schema.Literal(DESKTOP_CODEX_USAGE_SCHEMA_VERSION),
  turnRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180)),
  model: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120)),
  observedAt: Schema.String.check(
    Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/),
  ),
  usage: Schema.Struct({
    inputTokens: TokenCount,
    cachedInputTokens: TokenCount,
    outputTokens: TokenCount,
    reasoningTokens: TokenCount,
    totalTokens: TokenCount.pipe(Schema.check(Schema.isGreaterThan(0))),
  }),
})

export type DesktopCodexUsageReport = Readonly<{
  turnRef: string
  model: string
  observedAt: string
  usage: Readonly<{
    inputTokens: number
    cachedInputTokens: number
    outputTokens: number
    reasoningTokens: number
    totalTokens: number
  }>
}>

export type DesktopCodexUsageReporter = Readonly<{
  report: (input: DesktopCodexUsageReport) => Effect.Effect<void>
}>

export const makeDesktopCodexUsageReporter = (input: Readonly<{
  consentEnabled: () => boolean
  sessionReady: () => boolean
  credential: () => DesktopSessionCredential | null
  baseUrl: string
  fetch?: typeof fetch
}>): DesktopCodexUsageReporter => {
  const fetcher = input.fetch ?? fetch

  const report = Effect.fn("DesktopCodexUsageReporter.report")(function* (
    usageReport: DesktopCodexUsageReport,
  ) {
    if (!input.consentEnabled() || !input.sessionReady()) {
      return
    }
    const credential = input.credential()
    if (credential === null) {
      return
    }
    if (
      usageReport.usage.totalTokens !==
        usageReport.usage.inputTokens + usageReport.usage.outputTokens + usageReport.usage.reasoningTokens
    ) {
      return
    }
    const body = yield* Schema.decodeUnknownEffect(DesktopCodexUsageBody)({
      schemaVersion: DESKTOP_CODEX_USAGE_SCHEMA_VERSION,
      ...usageReport,
    })
    yield* Effect.tryPromise({
      try: signal => fetcher(new URL(DESKTOP_CODEX_USAGE_INGEST_PATH, input.baseUrl), {
        method: "POST",
        signal,
        headers: {
          authorization: `Bearer ${credential.accessToken}`,
          "content-type": "application/json",
          "idempotency-key": `desktop.codex.turn.${usageReport.turnRef}`,
        },
        body: JSON.stringify(body),
      }),
      catch: cause => cause,
    })
  })

  return {
    report: usageReport => report(usageReport).pipe(Effect.catch(() => Effect.void)),
  }
}
