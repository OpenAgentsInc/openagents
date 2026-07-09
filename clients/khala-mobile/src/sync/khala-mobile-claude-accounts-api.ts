import { isDemoToken } from "../demo/demo-fixtures"

import {
  claudeReadinessForAccount,
  isVisibleClaudeAccount,
  type KhalaMobileClaudeAccountFailure,
  type KhalaMobileClaudeAccountView,
  type KhalaMobileClaudeQuotaState,
} from "./khala-mobile-claude-accounts-core"

export type KhalaMobileClaudeAccountsBundle = Readonly<{
  accounts: ReadonlyArray<KhalaMobileClaudeAccountView>
  attempts: ReadonlyArray<unknown>
}>

export type KhalaMobileClaudeImportResult = Readonly<{
  account: KhalaMobileClaudeAccountView
  providerAccountRef: string
}>

export type KhalaMobileClaudeFetchLike = (
  url: string,
  init: { body?: string; headers: Record<string, string>; method: string },
) => Promise<{ json: () => Promise<unknown>; ok: boolean; status?: number }>

export type KhalaMobileClaudeResult<T> =
  | Readonly<{ ok: true; value: T }>
  | Readonly<{ kind: "not_found" | "unavailable" | "unauthorized" | "unknown"; ok: false }>

const demoAccount: KhalaMobileClaudeAccountView = {
  accountLabel: "Demo Claude",
  health: "healthy",
  lastStatusAt: "2026-07-08T12:00:00.000Z",
  planType: "Pro",
  providerAccountRef: "provider-account_demo-claude",
  quotaState: "available",
  readiness: "ready",
  status: "connected",
}

const headers = (token: string): Record<string, string> => ({
  authorization: `Bearer ${token}`,
  "content-type": "application/json",
})

const requestClaude = async <T>(
  url: string,
  token: string,
  init: Readonly<{ body?: unknown; method: "GET" | "POST" }>,
  fetchImpl: KhalaMobileClaudeFetchLike,
  parse: (body: unknown) => T | null,
): Promise<KhalaMobileClaudeResult<T>> => {
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

const parseQuotaState = (value: unknown): KhalaMobileClaudeQuotaState =>
  value === "available" || value === "exhausted" || value === "rate_limited" || value === "unknown"
    ? value
    : "unknown"

const parseFailure = (value: unknown): KhalaMobileClaudeAccountFailure | null =>
  value === "account_exhausted" || value === "account_rate_limited" ? value : null

const parseAccount = (value: unknown): KhalaMobileClaudeAccountView | null => {
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
    readiness: claudeReadinessForAccount({ failure, health, quotaState, status }),
    status,
  }
}

const parseBundle = (body: unknown): KhalaMobileClaudeAccountsBundle | null => {
  if (body === null || typeof body !== "object") return null
  const record = body as Record<string, unknown>
  if (!Array.isArray(record.accounts)) return null
  const accounts = record.accounts.map(parseAccount)
  if (accounts.some(account => account === null)) return null
  return {
    accounts: (accounts as ReadonlyArray<KhalaMobileClaudeAccountView>).filter(isVisibleClaudeAccount),
    attempts: Array.isArray(record.attempts) ? record.attempts : [],
  }
}

const parseImport = (body: unknown): KhalaMobileClaudeImportResult | null => {
  if (body === null || typeof body !== "object") return null
  const record = body as Record<string, unknown>
  const account = parseAccount(record.account)
  const providerAccountRef =
    typeof record.providerAccountRef === "string"
      ? record.providerAccountRef
      : account?.providerAccountRef
  if (account === null || providerAccountRef === undefined) return null
  return { account, providerAccountRef }
}

const parseDisconnect = (body: unknown): KhalaMobileClaudeAccountView | null => {
  if (body === null || typeof body !== "object") return null
  return parseAccount((body as Record<string, unknown>).account)
}

export const fetchKhalaMobileClaudeAccounts = async (
  apiBaseUrl: string,
  token: string,
  fetchImpl: KhalaMobileClaudeFetchLike = fetch,
): Promise<KhalaMobileClaudeResult<KhalaMobileClaudeAccountsBundle>> =>
  isDemoToken(token)
    ? { ok: true, value: { accounts: [demoAccount], attempts: [] } }
    : requestClaude(
        `${apiBaseUrl.replace(/\/$/, "")}/api/mobile/claude-accounts`,
        token,
        { method: "GET" },
        fetchImpl,
        parseBundle,
      )

/**
 * Import a CLAUDE_CODE_OAUTH_TOKEN the owner obtained via `claude setup-token`.
 * The token travels only in this request body over the mobile bearer session;
 * it is never written to local storage or returned in the response parser.
 */
export const importKhalaMobileClaudeLocalAuth = async (
  apiBaseUrl: string,
  token: string,
  input: Readonly<{ accountLabel?: string; authContentValue: string }>,
  fetchImpl: KhalaMobileClaudeFetchLike = fetch,
): Promise<KhalaMobileClaudeResult<KhalaMobileClaudeImportResult>> =>
  requestClaude(
    `${apiBaseUrl.replace(/\/$/, "")}/api/mobile/claude-accounts/local-auth/import`,
    token,
    {
      body: {
        authContentValue: input.authContentValue,
        createNew: true,
        ...(input.accountLabel === undefined || input.accountLabel.trim() === ""
          ? {}
          : { accountLabel: input.accountLabel.trim() }),
      },
      method: "POST",
    },
    fetchImpl,
    parseImport,
  )

export const disconnectKhalaMobileClaudeAccount = async (
  apiBaseUrl: string,
  token: string,
  providerAccountRef: string,
  fetchImpl: KhalaMobileClaudeFetchLike = fetch,
): Promise<KhalaMobileClaudeResult<KhalaMobileClaudeAccountView>> =>
  requestClaude(
    `${apiBaseUrl.replace(/\/$/, "")}/api/mobile/claude-accounts/${encodeURIComponent(providerAccountRef)}/disconnect`,
    token,
    { method: "POST" },
    fetchImpl,
    parseDisconnect,
  )
