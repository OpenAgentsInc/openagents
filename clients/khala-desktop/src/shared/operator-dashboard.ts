import { Effect, Schema as S } from "effect"

export const KHALA_OPERATOR_DEFAULT_BASE_URL = "https://openagents.com"
export const KHALA_OPERATOR_POLL_INTERVAL_MS = 5_000

export type AccountProvider = "codex" | "claude"
export type AccountReadiness =
  | "ready"
  | "credentials_revoked"
  | "usage_limited"
  | "rate_limited"

export const RateLimitWindowSnapshot = S.Struct({
  limit_window_seconds: S.optional(S.Number),
  reset_after_seconds: S.optional(S.Number),
  reset_at: S.optional(S.Union([S.Number, S.String])),
  used_percent: S.optional(S.Number),
})
export type RateLimitWindowSnapshot = typeof RateLimitWindowSnapshot.Type

export const DashboardAccount = S.Struct({
  accountRef: S.String,
  email: S.NullOr(S.String),
  provider: S.Literals(["codex", "claude"]),
  rateLimit: S.NullOr(RateLimitWindowSnapshot),
  readiness: S.Literals([
    "ready",
    "credentials_revoked",
    "usage_limited",
    "rate_limited",
  ]),
  resetAt: S.NullOr(S.String),
  usedPercent: S.NullOr(S.Number),
})
export type DashboardAccount = typeof DashboardAccount.Type

export const DashboardPylon = S.Struct({
  busySlots: S.Number,
  codexCapable: S.Boolean,
  heartbeatFresh: S.Boolean,
  latestHeartbeatAt: S.NullOr(S.String),
  pylonRef: S.String,
  queuedSlots: S.Number,
  readySlots: S.Number,
  status: S.String,
})
export type DashboardPylon = typeof DashboardPylon.Type

export const DashboardSession = S.Struct({
  accountRef: S.NullOr(S.String),
  assignmentRef: S.String,
  elapsedMs: S.Number,
  jobKind: S.String,
  pylonRef: S.String,
  provider: S.NullOr(S.Literals(["codex", "claude"])),
  state: S.String,
  tokenCount: S.NullOr(S.Number),
  updatedAt: S.String,
})
export type DashboardSession = typeof DashboardSession.Type

export const KhalaDesktopDashboard = S.Struct({
  accounts: S.Array(DashboardAccount),
  generatedAt: S.String,
  pylons: S.Array(DashboardPylon),
  sessions: S.Array(DashboardSession),
  source: S.Struct({
    accountsStatusPath: S.String,
    baseUrl: S.String,
    fleetStatusPath: S.String,
  }),
  totals: S.Struct({
    activeAssignments: S.Number,
    busySlots: S.Number,
    queuedSlots: S.Number,
    readyAccounts: S.Number,
    readySlots: S.Number,
    tokensToday: S.Number,
  }),
})
export type KhalaDesktopDashboard = typeof KhalaDesktopDashboard.Type

export type KhalaDesktopDashboardResult =
  | {
      readonly ok: true
      readonly dashboard: KhalaDesktopDashboard
    }
  | {
      readonly ok: false
      readonly error: string
      readonly observedAt: string
    }

export type OperatorFetchOptions = {
  readonly baseUrl?: string
  readonly token?: string | null
  readonly fetch?: typeof fetch
}

const asRecord = (value: unknown): Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {}

const asArray = (value: unknown): ReadonlyArray<unknown> =>
  Array.isArray(value) ? value : []

const stringValue = (value: unknown, fallback = ""): string =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : fallback

const nullableString = (value: unknown): string | null =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : null

const numberValue = (value: unknown, fallback = 0): number =>
  typeof value === "number" && Number.isFinite(value) ? value : fallback

const nullableNumber = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null

const providerFrom = (value: unknown): AccountProvider =>
  stringValue(value).includes("claude") ? "claude" : "codex"

const resetAtFrom = (value: unknown, nowMs = Date.now()): string | null => {
  if (typeof value === "string" && value.trim() !== "") return value.trim()
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0) {
    return null
  }

  const millis = value < 10_000_000_000 ? value * 1000 : value
  if (!Number.isFinite(millis) || millis <= nowMs / 2) return null
  return new Date(millis).toISOString()
}

const windowFrom = (value: unknown): RateLimitWindowSnapshot | null => {
  const record = asRecord(value)
  if (Object.keys(record).length === 0) return null
  return {
    limit_window_seconds: nullableNumber(record.limit_window_seconds) ?? undefined,
    reset_after_seconds: nullableNumber(record.reset_after_seconds) ?? undefined,
    reset_at:
      typeof record.reset_at === "number" || typeof record.reset_at === "string"
        ? record.reset_at
        : undefined,
    used_percent: nullableNumber(record.used_percent) ?? undefined,
  }
}

const bestRateLimitWindow = (account: Record<string, unknown>): RateLimitWindowSnapshot | null => {
  const direct = windowFrom(account.rateLimit)
  if (direct !== null) return direct
  const rateLimits = asRecord(account.rateLimits ?? account.rate_limits)
  return windowFrom(rateLimits.primary ?? rateLimits.primary_window) ??
    windowFrom(rateLimits.secondary ?? rateLimits.secondary_window) ??
    null
}

const readinessFrom = (
  account: Record<string, unknown>,
  resetAt: string | null,
): AccountReadiness => {
  const raw =
    stringValue(account.readiness) ||
    stringValue(account.status) ||
    stringValue(account.recentFailureClass ?? account.recent_failure_class)

  if (raw === "credentials_revoked" || raw === "auth_error") {
    return "credentials_revoked"
  }
  if (raw === "usage_limited") return "usage_limited"
  if (raw === "rate_limited" || raw === "rateLimited") return "rate_limited"
  if (account.isRateLimited === true) return resetAt === null ? "rate_limited" : "usage_limited"
  return "ready"
}

const accountFrom = (value: unknown): DashboardAccount => {
  const account = asRecord(value)
  const rateLimit = bestRateLimitWindow(account)
  const resetAt =
    nullableString(account.resetAt ?? account.reset_at ?? account.cooldownExpiresAt) ??
    resetAtFrom(rateLimit?.reset_at)
  const windows = asArray(account.windows).map(asRecord)
  const highestWindowPercent = windows.reduce(
    (max, window) => Math.max(max, numberValue(window.percentUsed, 0)),
    0,
  )
  const usedPercent =
    nullableNumber(account.usedPercent ?? account.used_percent) ??
    nullableNumber(rateLimit?.used_percent) ??
    (highestWindowPercent > 0 ? highestWindowPercent : null)
  const readiness = readinessFrom(account, resetAt)
  const provider = providerFrom(account.provider)

  return {
    accountRef: stringValue(
      account.ref ?? account.accountRef ?? account.accountRefHash ?? account.provider_account_ref,
      "unknown-account",
    ),
    email: nullableString(account.email),
    provider,
    rateLimit,
    readiness,
    resetAt,
    usedPercent,
  }
}

const pylonFrom = (value: unknown): DashboardPylon => {
  const pylon = asRecord(value)
  return {
    busySlots: numberValue(pylon.busySlots),
    codexCapable: pylon.codexCapable === true,
    heartbeatFresh: pylon.heartbeatFresh === true,
    latestHeartbeatAt: nullableString(pylon.latestHeartbeatAt),
    pylonRef: stringValue(pylon.pylonRef, "unknown-pylon"),
    queuedSlots: numberValue(pylon.queuedSlots),
    readySlots: numberValue(pylon.readySlots),
    status: stringValue(pylon.status, "unknown"),
  }
}

const sessionFrom = (value: unknown): DashboardSession => {
  const session = asRecord(value)
  const rawAccountRef =
    session.accountRef ??
    session.accountRefHash ??
    session.providerAccountRef ??
    session.provider_account_ref
  const rawProvider = session.provider ?? session.agentKind ?? session.jobKind
  return {
    accountRef: nullableString(rawAccountRef),
    assignmentRef: stringValue(session.assignmentRef, "unknown-assignment"),
    elapsedMs: numberValue(session.elapsedMs),
    jobKind: stringValue(session.jobKind, "unknown"),
    pylonRef: stringValue(session.pylonRef, "unknown-pylon"),
    provider:
      rawProvider === undefined || rawProvider === null
        ? null
        : providerFrom(rawProvider),
    state: stringValue(session.state, "unknown"),
    tokenCount: nullableNumber(session.tokenCount ?? session.totalTokens ?? session.tokens),
    updatedAt: stringValue(session.updatedAt, ""),
  }
}

export const normalizeOperatorDashboard = (
  input: Readonly<{
    accountsStatus: unknown
    baseUrl?: string
    fleetStatus: unknown
    now?: string
  }>,
): KhalaDesktopDashboard => {
  const fleet = asRecord(input.fleetStatus)
  const fleetBlock = asRecord(fleet.fleet)
  const pace = asRecord(fleet.pace)
  const accounts = asArray(asRecord(input.accountsStatus).accounts).map(accountFrom)
  const pylons = asArray(fleetBlock.spread).map(pylonFrom)
  const sessions = asArray(fleetBlock.inFlightAssignments).map(sessionFrom)
  const baseUrl = (input.baseUrl ?? KHALA_OPERATOR_DEFAULT_BASE_URL).replace(/\/+$/, "")

  return {
    accounts,
    generatedAt: stringValue(fleet.generatedAt, input.now ?? new Date().toISOString()),
    pylons,
    sessions,
    source: {
      accountsStatusPath: "/api/operator/accounts/status",
      baseUrl,
      fleetStatusPath: "/api/operator/fleet/status",
    },
    totals: {
      activeAssignments: numberValue(fleetBlock.activeAssignmentCount, sessions.length),
      busySlots: numberValue(fleetBlock.busySlots),
      queuedSlots: numberValue(fleetBlock.queuedSlots),
      readyAccounts: accounts.filter(account => account.readiness === "ready").length,
      readySlots: numberValue(fleetBlock.readySlots),
      tokensToday: numberValue(pace.todayTokens),
    },
  }
}

const fetchJson = (options: {
  readonly baseUrl: string
  readonly fetch: typeof fetch
  readonly path: string
  readonly token: string
}): Effect.Effect<unknown, Error> =>
  Effect.tryPromise({
    catch: (error: unknown) => error instanceof Error ? error : new Error(String(error)),
    try: async () => {
      const response = await options.fetch(`${options.baseUrl}${options.path}`, {
        headers: {
          accept: "application/json",
          authorization: `Bearer ${options.token}`,
        },
        method: "GET",
      })
      if (!response.ok) {
        throw new Error(`${options.path} returned ${response.status}`)
      }
      return response.json()
    },
  })

export const fetchOperatorDashboard = (
  options: OperatorFetchOptions,
): Effect.Effect<KhalaDesktopDashboardResult> =>
  Effect.gen(function* () {
    const baseUrl = (options.baseUrl ?? KHALA_OPERATOR_DEFAULT_BASE_URL).replace(/\/+$/, "")
    const token = options.token?.trim()
    if (!token) {
      return {
        ok: false as const,
        error: "Set OPENAGENTS_AGENT_TOKEN to load the owner fleet dashboard.",
        observedAt: new Date().toISOString(),
      }
    }

    const fetchImpl = options.fetch ?? fetch
    const [fleetStatus, accountsStatus] = yield* Effect.all([
      fetchJson({ baseUrl, fetch: fetchImpl, path: "/api/operator/fleet/status", token }),
      fetchJson({ baseUrl, fetch: fetchImpl, path: "/api/operator/accounts/status", token }),
    ], { concurrency: 2 })

    return {
      ok: true as const,
      dashboard: normalizeOperatorDashboard({
        accountsStatus,
        baseUrl,
        fleetStatus,
      }),
    }
  }).pipe(
    Effect.catch((error: Error) =>
      Effect.succeed({
        ok: false as const,
        error: error.message,
        observedAt: new Date().toISOString(),
      }),
    ),
  )
