import { Effect, Exit, Schema } from "effect"

import type { DesktopSessionCredential } from "./desktop-session-vault.ts"
import type { DesktopCodexUsageOutbox } from "./desktop-codex-usage-outbox.ts"

export const DESKTOP_CODEX_USAGE_ADMISSION_PATH = "/api/desktop/codex/turn-admission"
export const DESKTOP_CODEX_USAGE_INGEST_PATH = "/api/desktop/codex/turn-usage"
export const DESKTOP_CODEX_USAGE_SCHEMA_VERSION = "openagents.desktop.codex_turn_usage.v1"
export const DESKTOP_CODEX_USAGE_ADMISSION_SCHEMA_VERSION =
  "openagents.desktop.codex_turn_admission.v1"

const TokenCount = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)
const BoundedRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(256))
const DesktopCodexUsageBody = Schema.Struct({
  schemaVersion: Schema.Literal(DESKTOP_CODEX_USAGE_SCHEMA_VERSION),
  admissionRef: BoundedRef,
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
const AdmissionResponse = Schema.Struct({
  schemaVersion: Schema.Literal(DESKTOP_CODEX_USAGE_ADMISSION_SCHEMA_VERSION),
  admissionRef: BoundedRef,
  admittedAt: Schema.String,
  expiresAt: Schema.String,
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
  admit: (input: Readonly<{ turnRef: string; model: string }>) => Promise<void>
  report: (input: DesktopCodexUsageReport) => Effect.Effect<void>
  flush: () => Effect.Effect<void>
}>

export const makeDesktopCodexUsageReporter = (input: Readonly<{
  consentEnabled: () => boolean
  sessionReady: () => boolean
  credential: () => DesktopSessionCredential | null
  outbox: DesktopCodexUsageOutbox
  baseUrl: string
  fetch?: typeof fetch
}>): DesktopCodexUsageReporter => {
  const fetcher = input.fetch ?? fetch
  let draining: Promise<void> | null = null

  const readyCredential = (): DesktopSessionCredential | null =>
    input.consentEnabled() && input.sessionReady() ? input.credential() : null

  const drain = async (): Promise<void> => {
    const credential = readyCredential()
    if (credential === null) return
    for (const entry of input.outbox.due()) {
      if (entry.report === null || !input.consentEnabled()) return
      const decoded = Schema.decodeUnknownExit(DesktopCodexUsageBody)({
        schemaVersion: DESKTOP_CODEX_USAGE_SCHEMA_VERSION,
        admissionRef: entry.admissionRef,
        turnRef: entry.turnRef,
        model: entry.model,
        ...entry.report,
      })
      if (!Exit.isSuccess(decoded)) {
        input.outbox.drop(entry.admissionRef)
        continue
      }
      try {
        const response = await fetcher(new URL(DESKTOP_CODEX_USAGE_INGEST_PATH, input.baseUrl), {
          method: "POST",
          headers: {
            authorization: `Bearer ${credential.accessToken}`,
            "content-type": "application/json",
            "idempotency-key": `desktop.codex.turn.${entry.turnRef}`,
          },
          body: JSON.stringify(decoded.value),
        })
        if (response.ok) {
          input.outbox.success(entry.admissionRef)
        } else if (response.status >= 400 && response.status < 500 && ![401, 408, 429].includes(response.status)) {
          input.outbox.drop(entry.admissionRef)
        } else {
          input.outbox.retry(entry.admissionRef)
        }
      } catch {
        input.outbox.retry(entry.admissionRef)
      }
    }
  }
  const flushPromise = (): Promise<void> => {
    if (draining !== null) return draining
    draining = drain().finally(() => { draining = null })
    return draining
  }

  const admit = async (
    value: Readonly<{ turnRef: string; model: string }>,
  ): Promise<void> => {
    const credential = readyCredential()
    if (credential === null) return
    try {
      const response = await fetcher(new URL(DESKTOP_CODEX_USAGE_ADMISSION_PATH, input.baseUrl), {
          method: "POST",
          headers: {
            authorization: `Bearer ${credential.accessToken}`,
            "content-type": "application/json",
          },
          body: JSON.stringify({
            schemaVersion: DESKTOP_CODEX_USAGE_ADMISSION_SCHEMA_VERSION,
            turnRef: value.turnRef,
            model: value.model,
          }),
        })
      if (!response.ok) return
      const decoded = Schema.decodeUnknownExit(AdmissionResponse)(await response.json())
      if (!Exit.isSuccess(decoded)) return
      input.outbox.recordAdmission({ ...value, ...decoded.value })
    } catch {
      // Telemetry admission can never block the local turn.
    }
  }

  const report = Effect.fn("DesktopCodexUsageReporter.report")(function* (
    usageReport: DesktopCodexUsageReport,
  ) {
    if (!input.consentEnabled() || !input.sessionReady()) return
    if (
      usageReport.usage.totalTokens !==
        usageReport.usage.inputTokens + usageReport.usage.outputTokens + usageReport.usage.reasoningTokens
    ) return
    if (!input.outbox.complete(usageReport.turnRef, {
      observedAt: usageReport.observedAt,
      usage: usageReport.usage,
    })) return
    yield* Effect.promise(flushPromise)
  })

  return {
    admit,
    report: value => report(value).pipe(Effect.catch(() => Effect.void)),
    flush: () => Effect.promise(flushPromise).pipe(Effect.catch(() => Effect.void)),
  }
}
