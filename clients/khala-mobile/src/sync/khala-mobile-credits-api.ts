/**
 * MM-D3 (#8480): client for the mobile-bearer-authorized credits/balance and
 * transaction-history endpoints.
 *
 * SHIPPED (#8505 Part 1): both routes are now live on the server —
 * `apps/openagents.com/workers/api/src/mobile-credits-routes.ts`, backed
 * directly by the authoritative D1 `agent_balances` / `pay_ins` ledger
 * (`payments-ledger.ts`). The 404/unimplemented degrade path below is kept
 * intentionally (defense in depth against a future route regression, and so
 * an older client build against a rolled-back server still shows an honest
 * "not yet available" state instead of a fabricated balance) but is no
 * longer the expected steady-state response.
 *
 * Contract:
 *
 * `GET /api/mobile/credits/balance`
 *   200 `{ balanceUsdCents: number }`
 *   401 `{ error: "unauthorized" }`
 *
 * `GET /api/mobile/credits/transactions?limit=&cursor=`
 *   200 `{ transactions: KhalaMobileCreditsTransaction[], nextCursor: string | null }`
 *   401 `{ error: "unauthorized" }`
 *
 * Deliberately USD CENTS, never msat: the ledger's internal unit
 * (`agent_balances.balance_msat`) converts through a BTC/USD reference rate
 * that is explicitly a `!! BILLING TODO` fixed placeholder
 * (`apps/openagents.com/workers/api/src/inference/usd-msat-conversion.ts`,
 * `DEFAULT_BTC_USD`). The mobile client should never need to know that rate
 * or replicate its conversion; the server converts once, authoritatively,
 * and hands the client a currency the user actually understands.
 */

export type KhalaMobileCreditsTransactionKind = "grant" | "purchase" | "charge" | "other"

export type KhalaMobileCreditsTransaction = Readonly<{
  amountUsdCents: number
  description: string
  id: string
  kind: KhalaMobileCreditsTransactionKind
  occurredAt: string
}>

export type KhalaMobileCreditsFetchLike = (
  url: string,
  init: { headers: Record<string, string> },
) => Promise<{ json: () => Promise<unknown>; ok: boolean; status?: number }>

export type KhalaMobileCreditsResult<T> =
  | Readonly<{ ok: true; value: T }>
  | Readonly<{ kind: "unavailable" | "unauthorized" | "unknown"; ok: false }>

const requestCredits = async <T>(
  url: string,
  token: string,
  fetchImpl: KhalaMobileCreditsFetchLike,
  parse: (body: unknown) => T | null,
): Promise<KhalaMobileCreditsResult<T>> => {
  try {
    const response = await fetchImpl(url, { headers: { authorization: `Bearer ${token}` } })
    if (response.status === 404) return { kind: "unavailable", ok: false }
    const body = await response.json()
    if (!response.ok) {
      return { kind: response.status === 401 ? "unauthorized" : "unknown", ok: false }
    }
    const parsed = parse(body)
    if (parsed === null) return { kind: "unknown", ok: false }
    return { ok: true, value: parsed }
  } catch {
    return { kind: "unavailable", ok: false }
  }
}

const parseBalance = (body: unknown): number | null => {
  if (body === null || typeof body !== "object") return null
  const value = (body as Record<string, unknown>).balanceUsdCents
  return typeof value === "number" && Number.isFinite(value) ? value : null
}

export const fetchKhalaMobileCreditsBalance = async (
  apiBaseUrl: string,
  token: string,
  fetchImpl: KhalaMobileCreditsFetchLike = fetch,
): Promise<KhalaMobileCreditsResult<number>> =>
  requestCredits(`${apiBaseUrl.replace(/\/$/, "")}/api/mobile/credits/balance`, token, fetchImpl, parseBalance)

const parseTransaction = (value: unknown): KhalaMobileCreditsTransaction | null => {
  if (value === null || typeof value !== "object") return null
  const record = value as Record<string, unknown>
  if (
    typeof record.id !== "string" ||
    typeof record.description !== "string" ||
    typeof record.occurredAt !== "string" ||
    typeof record.amountUsdCents !== "number" ||
    (record.kind !== "grant" && record.kind !== "purchase" && record.kind !== "charge" && record.kind !== "other")
  ) {
    return null
  }
  return {
    amountUsdCents: record.amountUsdCents,
    description: record.description,
    id: record.id,
    kind: record.kind,
    occurredAt: record.occurredAt,
  }
}

export type KhalaMobileCreditsTransactionsPage = Readonly<{
  nextCursor: string | null
  transactions: ReadonlyArray<KhalaMobileCreditsTransaction>
}>

export const fetchKhalaMobileCreditsTransactions = async (
  apiBaseUrl: string,
  token: string,
  input: Readonly<{ cursor?: string; limit?: number }>,
  fetchImpl: KhalaMobileCreditsFetchLike = fetch,
): Promise<KhalaMobileCreditsResult<KhalaMobileCreditsTransactionsPage>> => {
  const url = new URL("/api/mobile/credits/transactions", apiBaseUrl)
  if (input.limit !== undefined) url.searchParams.set("limit", String(input.limit))
  if (input.cursor !== undefined) url.searchParams.set("cursor", input.cursor)
  return requestCredits(url.toString(), token, fetchImpl, body => {
    if (body === null || typeof body !== "object") return null
    const record = body as Record<string, unknown>
    if (!Array.isArray(record.transactions)) return null
    const transactions: Array<KhalaMobileCreditsTransaction> = []
    for (const entry of record.transactions) {
      const parsed = parseTransaction(entry)
      if (parsed === null) return null
      transactions.push(parsed)
    }
    const nextCursor = typeof record.nextCursor === "string" ? record.nextCursor : null
    return { nextCursor, transactions }
  })
}
