import { isDemoToken } from "../demo/demo-fixtures"

import {
  codexReadinessForAccount,
  type KhalaMobileCodexAccountFailure,
  type KhalaMobileCodexAccountView,
  type KhalaMobileCodexQuotaState,
} from "./khala-mobile-codex-accounts-core"

export type KhalaMobileCodexAttemptStatus = "connected" | "denied" | "expired" | "failed" | "pending"

export type KhalaMobileCodexAttempt = Readonly<{
  expiresAt: string
  id: string
  providerAccountRef: string
  status: KhalaMobileCodexAttemptStatus
  userCode: string | null
  verificationUrl: string | null
}>

export type KhalaMobileCodexAccountsBundle = Readonly<{
  accounts: ReadonlyArray<KhalaMobileCodexAccountView>
  attempts: ReadonlyArray<KhalaMobileCodexAttempt>
}>

export type KhalaMobileCodexDeviceLoginStart = Readonly<{
  account: KhalaMobileCodexAccountView
  attempt: KhalaMobileCodexAttempt
  expiresAt: string
  intervalSeconds: number
  providerAccountRef: string
  userCode: string
  verificationUrl: string
}>

export type KhalaMobileCodexAttemptResult = Readonly<{
  account: KhalaMobileCodexAccountView
  attempt: KhalaMobileCodexAttempt
}>

export type KhalaMobileCodexFetchLike = (
  url: string,
  init: { body?: string; headers: Record<string, string>; method: string },
) => Promise<{ json: () => Promise<unknown>; ok: boolean; status?: number }>

export type KhalaMobileCodexResult<T> =
  | Readonly<{ ok: true; value: T }>
  | Readonly<{ kind: "not_found" | "unavailable" | "unauthorized" | "unknown"; ok: false }>

const demoAccount: KhalaMobileCodexAccountView = {
  accountLabel: "Demo Codex",
  health: "healthy",
  lastStatusAt: "2026-07-08T12:00:00.000Z",
  planType: "Plus",
  providerAccountRef: "provider-account_demo-codex",
  quotaState: "available",
  readiness: "ready",
  status: "connected",
}

const headers = (token: string): Record<string, string> => ({
  authorization: `Bearer ${token}`,
  "content-type": "application/json",
})

const requestCodex = async <T>(
  url: string,
  token: string,
  init: Readonly<{ body?: unknown; method: "GET" | "POST" }>,
  fetchImpl: KhalaMobileCodexFetchLike,
  parse: (body: unknown) => T | null,
): Promise<KhalaMobileCodexResult<T>> => {
  try {
    const response = await fetchImpl(url, {
      ...(init.body === undefined ? {} : { body: JSON.stringify(init.body) }),
      headers: headers(token),
      method: init.method,
    })
    if (response.status === 401) return { kind: "unauthorized", ok: false }
    if (response.status === 404) return { kind: "not_found", ok: false }
    const body = await response.json()
    if (!response.ok) return { kind: "unknown", ok: false }
    const parsed = parse(body)
    return parsed === null ? { kind: "unknown", ok: false } : { ok: true, value: parsed }
  } catch {
    return { kind: "unavailable", ok: false }
  }
}

const stringOrNull = (value: unknown): string | null => (typeof value === "string" ? value : null)

const parseQuotaState = (value: unknown): KhalaMobileCodexQuotaState =>
  value === "available" || value === "exhausted" || value === "rate_limited" || value === "unknown"
    ? value
    : "unknown"

const parseFailure = (value: unknown): KhalaMobileCodexAccountFailure | null =>
  value === "account_exhausted" || value === "account_rate_limited" ? value : null

const parseAccount = (value: unknown): KhalaMobileCodexAccountView | null => {
  if (value === null || typeof value !== "object") return null
  const record = value as Record<string, unknown>
  const status = record.publicStatus ?? record.status
  const health = record.health
  if (
    typeof record.providerAccountRef !== "string" ||
    typeof record.lastStatusAt !== "string" ||
    (status !== "connected" &&
      status !== "denied" &&
      status !== "disconnected" &&
      status !== "expired" &&
      status !== "pending" &&
      status !== "unhealthy") ||
    (health !== "healthy" && health !== "requires_reauth" && health !== "unhealthy" && health !== "unknown")
  ) {
    return null
  }
  const quotaState = parseQuotaState(record.quotaState)
  const failure = parseFailure(record.failureClass)
  return {
    accountLabel: stringOrNull(record.accountLabel),
    health,
    lastStatusAt: record.lastStatusAt,
    planType: stringOrNull(record.planType),
    providerAccountRef: record.providerAccountRef,
    quotaState,
    readiness: codexReadinessForAccount({ failure, health, quotaState, status }),
    status,
  }
}

const parseAttempt = (value: unknown): KhalaMobileCodexAttempt | null => {
  if (value === null || typeof value !== "object") return null
  const record = value as Record<string, unknown>
  if (
    typeof record.id !== "string" ||
    typeof record.expiresAt !== "string" ||
    typeof record.providerAccountRef !== "string" ||
    (record.status !== "connected" &&
      record.status !== "denied" &&
      record.status !== "expired" &&
      record.status !== "failed" &&
      record.status !== "pending")
  ) {
    return null
  }
  return {
    expiresAt: record.expiresAt,
    id: record.id,
    providerAccountRef: record.providerAccountRef,
    status: record.status,
    userCode: stringOrNull(record.userCode),
    verificationUrl: stringOrNull(record.verificationUrl),
  }
}

const parseBundle = (body: unknown): KhalaMobileCodexAccountsBundle | null => {
  if (body === null || typeof body !== "object") return null
  const record = body as Record<string, unknown>
  if (!Array.isArray(record.accounts) || !Array.isArray(record.attempts)) return null
  const accounts = record.accounts.map(parseAccount)
  const attempts = record.attempts.map(parseAttempt)
  if (accounts.some(account => account === null) || attempts.some(attempt => attempt === null)) return null
  return {
    accounts: accounts as ReadonlyArray<KhalaMobileCodexAccountView>,
    attempts: attempts as ReadonlyArray<KhalaMobileCodexAttempt>,
  }
}

const parseStart = (body: unknown): KhalaMobileCodexDeviceLoginStart | null => {
  if (body === null || typeof body !== "object") return null
  const record = body as Record<string, unknown>
  const account = parseAccount(record.account)
  const attempt = parseAttempt(record.attempt)
  if (
    account === null ||
    attempt === null ||
    typeof record.expiresAt !== "string" ||
    typeof record.intervalSeconds !== "number" ||
    typeof record.providerAccountRef !== "string" ||
    typeof record.userCode !== "string" ||
    typeof record.verificationUrl !== "string"
  ) {
    return null
  }
  return {
    account,
    attempt,
    expiresAt: record.expiresAt,
    intervalSeconds: record.intervalSeconds,
    providerAccountRef: record.providerAccountRef,
    userCode: record.userCode,
    verificationUrl: record.verificationUrl,
  }
}

const parseAttemptResult = (body: unknown): KhalaMobileCodexAttemptResult | null => {
  if (body === null || typeof body !== "object") return null
  const record = body as Record<string, unknown>
  const account = parseAccount(record.account)
  const attempt = parseAttempt(record.attempt)
  return account === null || attempt === null ? null : { account, attempt }
}

const parseDisconnect = (body: unknown): KhalaMobileCodexAccountView | null => {
  if (body === null || typeof body !== "object") return null
  return parseAccount((body as Record<string, unknown>).account)
}

export const fetchKhalaMobileCodexAccounts = async (
  apiBaseUrl: string,
  token: string,
  fetchImpl: KhalaMobileCodexFetchLike = fetch,
): Promise<KhalaMobileCodexResult<KhalaMobileCodexAccountsBundle>> =>
  isDemoToken(token)
    ? { ok: true, value: { accounts: [demoAccount], attempts: [] } }
    : requestCodex(
        `${apiBaseUrl.replace(/\/$/, "")}/api/mobile/codex-accounts`,
        token,
        { method: "GET" },
        fetchImpl,
        parseBundle,
      )

export const startKhalaMobileCodexDeviceLogin = async (
  apiBaseUrl: string,
  token: string,
  fetchImpl: KhalaMobileCodexFetchLike = fetch,
): Promise<KhalaMobileCodexResult<KhalaMobileCodexDeviceLoginStart>> =>
  requestCodex(
    `${apiBaseUrl.replace(/\/$/, "")}/api/mobile/codex-accounts/device-login/start`,
    token,
    { body: { createNew: true }, method: "POST" },
    fetchImpl,
    parseStart,
  )

export const pollKhalaMobileCodexDeviceLogin = async (
  apiBaseUrl: string,
  token: string,
  attemptId: string,
  fetchImpl: KhalaMobileCodexFetchLike = fetch,
): Promise<KhalaMobileCodexResult<KhalaMobileCodexAttemptResult>> =>
  requestCodex(
    `${apiBaseUrl.replace(/\/$/, "")}/api/mobile/codex-accounts/device-login/${encodeURIComponent(attemptId)}`,
    token,
    { method: "GET" },
    fetchImpl,
    parseAttemptResult,
  )

export const disconnectKhalaMobileCodexAccount = async (
  apiBaseUrl: string,
  token: string,
  providerAccountRef: string,
  fetchImpl: KhalaMobileCodexFetchLike = fetch,
): Promise<KhalaMobileCodexResult<KhalaMobileCodexAccountView>> =>
  requestCodex(
    `${apiBaseUrl.replace(/\/$/, "")}/api/mobile/codex-accounts/${encodeURIComponent(providerAccountRef)}/disconnect`,
    token,
    { method: "POST" },
    fetchImpl,
    parseDisconnect,
  )
