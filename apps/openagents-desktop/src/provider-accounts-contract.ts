/**
 * Provider-neutral accounts bridge contract (#8712 Fleet overview).
 *
 * The renderer may only ever see: account refs, provider names, the signed-in
 * email the pylon projection already exposes, a closed readiness set, and
 * bounded public-safe usage totals. No tokens, credential paths, raw child
 * output, or local filesystem paths cross this line. Failures are typed
 * `{ ok: false, reason }` values — never a thrown error across IPC.
 */
import { Exit, Schema } from "@effect-native/core/effect"

export const ProviderAccountsListChannel = "openagents:provider-accounts:list" as const
export const ProviderAccountsUsageChannel = "openagents:provider-accounts:usage" as const

/** Pylon account-ref grammar (mirrors apps/pylon auth's accountRefPattern). */
export const providerAccountRefPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/

export const providerAccountReadinessStates = ["ready", "credentials-missing", "unknown"] as const
export type ProviderAccountReadiness = (typeof providerAccountReadinessStates)[number]

export const ProviderAccountEntrySchema = Schema.Struct({
  ref: Schema.String,
  provider: Schema.String,
  email: Schema.NullOr(Schema.String),
  readiness: Schema.Literals(providerAccountReadinessStates),
})

export type ProviderAccountEntry = typeof ProviderAccountEntrySchema.Type

export const ProviderAccountsListResultSchema = Schema.Union([
  Schema.Struct({
    ok: Schema.Literal(true),
    /** ISO timestamp from the host clock at decode time — the "as of" caption. */
    generatedAt: Schema.String,
    accounts: Schema.Array(ProviderAccountEntrySchema),
  }),
  Schema.Struct({
    ok: Schema.Literal(false),
    reason: Schema.String,
  }),
])

export type ProviderAccountsListResult = typeof ProviderAccountsListResultSchema.Type

export const ProviderAccountUsageRequestSchema = Schema.Struct({
  ref: Schema.String,
})

export type ProviderAccountUsageRequest = typeof ProviderAccountUsageRequestSchema.Type

export const ProviderAccountUsageSummarySchema = Schema.Struct({
  inputTokens: Schema.NullOr(Schema.Number),
  outputTokens: Schema.NullOr(Schema.Number),
  totalTokens: Schema.NullOr(Schema.Number),
})

/**
 * One provider rate-limit window (EP250 sidebar accounts box): the bounded
 * public-safe projection of pylon's `truth.provider.snapshots[*].primary/
 * secondary` windows (codex-rs RateLimitSnapshot lineage: used_percent,
 * window_minutes, resets_at). `remainingPercent` is what the sidebar bar
 * renders; `label` is pylon's own window label ("5h", "weekly", "hourly", …).
 * The field is ADDITIVE and optional — providers without window truth simply
 * omit it, and the renderer shows an honest grayed bar.
 */
export const ProviderAccountUsageWindowSchema = Schema.Struct({
  label: Schema.String,
  usedPercent: Schema.Number,
  remainingPercent: Schema.Number,
  windowMinutes: Schema.NullOr(Schema.Number),
  resetsAt: Schema.NullOr(Schema.String),
})

export type ProviderAccountUsageWindow = typeof ProviderAccountUsageWindowSchema.Type

/** Bounded window count per account across the bridge (primary+secondary x2). */
export const providerAccountUsageWindowCap = 4

export const ProviderAccountUsageResultSchema = Schema.Union([
  Schema.Struct({
    ok: Schema.Literal(true),
    ref: Schema.String,
    refreshedAt: Schema.String,
    summary: ProviderAccountUsageSummarySchema,
    windows: Schema.Array(ProviderAccountUsageWindowSchema).pipe(Schema.optionalKey),
  }),
  Schema.Struct({
    ok: Schema.Literal(false),
    ref: Schema.String,
    reason: Schema.String,
  }),
])

export type ProviderAccountUsageResult = typeof ProviderAccountUsageResultSchema.Type

export const unavailableProviderAccountsListResult = (
  reason = "pylon_runtime_unavailable",
): ProviderAccountsListResult => ({ ok: false, reason })

export const unavailableProviderAccountUsageResult = (
  ref: string,
  reason = "pylon_runtime_unavailable",
): ProviderAccountUsageResult => ({ ok: false, ref, reason })

export const decodeProviderAccountUsageRequest = (
  value: unknown,
): ProviderAccountUsageRequest | null => {
  const decoded = Schema.decodeUnknownExit(ProviderAccountUsageRequestSchema)(value)
  if (!Exit.isSuccess(decoded)) return null
  return providerAccountRefPattern.test(decoded.value.ref) ? { ref: decoded.value.ref } : null
}

export const decodeProviderAccountsListResult = (value: unknown): ProviderAccountsListResult => {
  const decoded = Schema.decodeUnknownExit(ProviderAccountsListResultSchema)(value)
  if (!Exit.isSuccess(decoded)) return unavailableProviderAccountsListResult("invalid_bridge_payload")
  if (!decoded.value.ok) return { ok: false, reason: decoded.value.reason.slice(0, 120) }
  // Defense-in-depth: drop entries whose ref does not fit the pylon grammar
  // and bound every projected string.
  return {
    ok: true,
    generatedAt: decoded.value.generatedAt.slice(0, 40),
    accounts: decoded.value.accounts
      .filter((account) => providerAccountRefPattern.test(account.ref))
      .map((account) => ({
        ref: account.ref,
        provider: account.provider.slice(0, 40),
        email: account.email === null || account.email.length > 120 ? null : account.email,
        readiness: account.readiness,
      })),
  }
}

export const decodeProviderAccountUsageResult = (
  value: unknown,
  ref: string,
): ProviderAccountUsageResult => {
  const decoded = Schema.decodeUnknownExit(ProviderAccountUsageResultSchema)(value)
  if (!Exit.isSuccess(decoded) || decoded.value.ref !== ref) {
    return unavailableProviderAccountUsageResult(ref, "invalid_bridge_payload")
  }
  if (!decoded.value.ok) return { ok: false, ref, reason: decoded.value.reason.slice(0, 120) }
  const windows = boundProviderAccountUsageWindows(decoded.value.windows)
  return {
    ok: true,
    ref,
    refreshedAt: decoded.value.refreshedAt.slice(0, 40),
    summary: decoded.value.summary,
    ...(windows.length > 0 ? { windows } : {}),
  }
}

const clampPercent = (value: number): number => Math.min(100, Math.max(0, value))

/**
 * Defense-in-depth window bounding shared by both bridge sides: cap the
 * count, clamp percents into [0, 100], and bound every projected string.
 */
export const boundProviderAccountUsageWindows = (
  windows: ReadonlyArray<ProviderAccountUsageWindow> | undefined,
): ReadonlyArray<ProviderAccountUsageWindow> =>
  (windows ?? [])
    .filter((window) => Number.isFinite(window.usedPercent) && Number.isFinite(window.remainingPercent))
    .slice(0, providerAccountUsageWindowCap)
    .map((window) => ({
      label: window.label.slice(0, 20),
      usedPercent: clampPercent(window.usedPercent),
      remainingPercent: clampPercent(window.remainingPercent),
      windowMinutes:
        window.windowMinutes !== null && Number.isFinite(window.windowMinutes) && window.windowMinutes >= 0
          ? Math.floor(window.windowMinutes)
          : null,
      resetsAt: window.resetsAt === null ? null : window.resetsAt.slice(0, 40),
    }))
