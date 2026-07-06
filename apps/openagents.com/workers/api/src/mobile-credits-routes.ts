// `GET /api/mobile/credits/balance` and `GET /api/mobile/credits/transactions`
// — mobile-bearer-authorized credit balance + transaction-history reads
// (issue #8505, Part 1: fixes #8480's shipped-but-dead REST routes; see
// `khala-mobile-credits-api.ts`'s header comment for the CONTRACT this file
// implements).
//
// Reuses the SAME mobile user bearer-session boundary as
// `GET /api/mobile/auth/session` (`auth/mobile-session.ts`'s
// `makeUserBearerSessionBoundary`) — never a browser session or agent token.
//
// CFG-4 (#8519): backed directly by the Postgres-authoritative
// `agent_balances` / `pay_ins` credits ledger (`PaymentsLedgerDb`,
// `payments-ledger-db.ts`) — the SAME authoritative tables every other
// credits surface reads (the Aiur admin console's `admin-credits-routes.ts`,
// #8500; the registered-agent self-view in `agent-balance-routes.ts`). No new
// ledger table, no invented read path, no ledger write here.

import { Effect } from 'effect'

import type { VerifiedSession } from './auth/session'
import { methodNotAllowed, noStoreJsonResponse, unauthorized } from './http/responses'
import { agentRefForUser } from './inference/usd-credit-bridge'
import { msatToUsdCentsRound } from './inference/usd-msat-conversion'
import { readAgentBalance } from './payments-ledger'
import type { PaymentsLedgerDb } from './payments-ledger-db'

type HttpResponse = globalThis.Response

export const MOBILE_CREDITS_BALANCE_PATH = '/api/mobile/credits/balance'
export const MOBILE_CREDITS_TRANSACTIONS_PATH = '/api/mobile/credits/transactions'

export type MobileCreditsRouteDependencies<Bindings, User = unknown> = Readonly<{
  /** CFG-4 (#8519): the Postgres-authoritative credits ledger handle. Both
   * routes here are credits-domain reads only, so this is the only store. */
  ledgerDb: (env: Bindings) => PaymentsLedgerDb
  requireUserBearerSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<VerifiedSession<User> | undefined>
  /** Resolve the caller's stable OpenAgents user id from the verified
   * session's decoded subject shape (same shape used by the push-token and
   * push-preference mobile-bearer routes). */
  userIdFromSession: (session: VerifiedSession<User>) => string
}>

const DEFAULT_TRANSACTIONS_LIMIT = 50
const MAX_TRANSACTIONS_LIMIT = 200

export type MobileCreditsTransactionKind = 'grant' | 'purchase' | 'charge' | 'other'

type PayInHistoryRow = Readonly<{
  id: string
  pay_in_type: string
  cost_msat: number
  context_ref: string | null
  created_at: string
}>

// `pay_in_type` -> mobile transaction kind. `usd_credit_grant` covers every
// credit-granting write this ledger has today (the $10 GitHub signup grant,
// #8478; the Aiur admin console grant, #8500) — no purchase flow is wired yet
// (RevenueCat/IAP, #8481, is postponed), so `purchase` is reachable in the type
// but not yet produced. `adjustment` covers every debit (inference charges,
// `metering-hook.ts`; cloud/agent-computer charges, `cloud-metering.ts`; admin
// clawbacks, `inference-abuse-controls.ts`) — from the user's point of view a
// clawback and a usage charge are both "credit left", so both map to `charge`.
export const mobileCreditsTransactionKind = (
  payInType: string,
): MobileCreditsTransactionKind => {
  if (payInType === 'usd_credit_grant') return 'grant'
  if (payInType === 'adjustment') return 'charge'
  return 'other'
}

// Best-effort human description from the stable `context_ref` prefixes the
// ledger already writes (see `admin-credit-grant.ts`, `github-signup-credit-grant.ts`,
// `inference/inference-charge-context.ts`, `cloud/cloud-metering.ts`). An empty
// description is a safe, honest fallback: the mobile client already renders
// `transactionKindLabel(kind)` when `description` is blank
// (`credits-history-screen.tsx`), so an unrecognized context_ref never shows a
// wrong or fabricated label.
export const mobileCreditsTransactionDescription = (
  payInType: string,
  contextRef: string | null,
): string => {
  const ref = contextRef ?? ''
  if (payInType === 'usd_credit_grant') {
    if (ref.startsWith('admin-credit-grant:')) return 'Admin credit grant'
    if (ref.startsWith('github-signup:')) return 'GitHub signup credit'
    return 'Credit grant'
  }
  if (payInType === 'adjustment') {
    if (ref.startsWith('admin-credit-clawback:')) return 'Credit clawback'
    if (ref.startsWith('inference:')) return 'Inference usage'
    if (ref.startsWith('cloud.')) return 'Cloud compute usage'
  }
  return ''
}

// Opaque keyset-pagination cursor: base64url of `createdAt|id`. Reversible on
// purpose — nothing sensitive, since the same created_at/id pair is already
// visible on the row it came from. Keyset (not OFFSET) pagination so a
// concurrent new charge/grant landing between pages never shifts or
// duplicates rows in the client's "Load more" scan.
const encodeTransactionsCursor = (createdAt: string, id: string): string =>
  btoa(`${createdAt}|${id}`)
    .replaceAll('+', '-')
    .replaceAll('/', '_')
    .replaceAll('=', '')

const decodeTransactionsCursor = (
  cursor: string,
): Readonly<{ createdAt: string; id: string }> | undefined => {
  try {
    const padded = cursor
      .replaceAll('-', '+')
      .replaceAll('_', '/')
      .padEnd(Math.ceil(cursor.length / 4) * 4, '=')
    const decoded = atob(padded)
    const separatorIndex = decoded.indexOf('|')
    if (separatorIndex <= 0) return undefined
    const createdAt = decoded.slice(0, separatorIndex)
    const id = decoded.slice(separatorIndex + 1)
    if (createdAt.length === 0 || id.length === 0) return undefined
    return { createdAt, id }
  } catch {
    return undefined
  }
}

const routeBalance = async <Bindings, User>(
  dependencies: MobileCreditsRouteDependencies<Bindings, User>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<HttpResponse> => {
  if (request.method !== 'GET') return methodNotAllowed(['GET'])

  const session = await dependencies.requireUserBearerSession(request, env, ctx)
  if (session === undefined) return unauthorized()

  const userId = dependencies.userIdFromSession(session)
  const balance = await readAgentBalance(dependencies.ledgerDb(env), agentRefForUser(userId))

  return noStoreJsonResponse({
    // `availableMsat` (balance minus any escrow-held amount, `readAgentBalance`
    // in `payments-ledger.ts`) is the same figure the coding-admission gate
    // reads (`decideCloudCodingAdmission`) — the honest "what can I actually
    // spend right now" number, converted through the single USD/msat rate
    // (`usd-msat-conversion.ts`) every other credits surface uses.
    balanceUsdCents: msatToUsdCentsRound(balance?.availableMsat ?? 0),
  })
}

const routeTransactions = async <Bindings, User>(
  dependencies: MobileCreditsRouteDependencies<Bindings, User>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Promise<HttpResponse> => {
  if (request.method !== 'GET') return methodNotAllowed(['GET'])

  const session = await dependencies.requireUserBearerSession(request, env, ctx)
  if (session === undefined) return unauthorized()

  const userId = dependencies.userIdFromSession(session)
  const actorRef = agentRefForUser(userId)
  const url = new URL(request.url)

  const rawLimit = Number(url.searchParams.get('limit') ?? '')
  const limit =
    Number.isFinite(rawLimit) && rawLimit > 0
      ? Math.min(MAX_TRANSACTIONS_LIMIT, Math.trunc(rawLimit))
      : DEFAULT_TRANSACTIONS_LIMIT

  const rawCursor = url.searchParams.get('cursor')
  const hasCursorParam = rawCursor !== null && rawCursor.trim().length > 0
  const cursor = hasCursorParam ? decodeTransactionsCursor(rawCursor as string) : undefined

  if (hasCursorParam && cursor === undefined) {
    return noStoreJsonResponse(
      { error: 'invalid_request', reason: 'cursor is malformed' },
      { status: 400 },
    )
  }

  const ledgerDb = dependencies.ledgerDb(env)
  // Same shape as `agent-balance-routes.ts`'s registered-agent self-view: an
  // actor's history is every pay-in it funded (payer_ref) OR received a
  // payout leg on (direction='out') — the OR branch matters for agent-to-agent
  // rails but is harmless (and correct) here too, so this reuses the exact
  // existing read rather than inventing a narrower one. Keyset pagination on
  // (created_at, id) DESC, both indexed/unique, so a concurrent write never
  // shifts a page.
  //
  // CFG-4 (#8519): two explicit SQL variants instead of the old single query
  // with a `? IS NULL OR ...` cursor branch — a bare `? IS NULL` is not
  // Postgres-safe (parameter type inference fails), so the cursor either
  // exists (three typed comparisons) or the predicate is omitted entirely.
  const baseSelect = `SELECT id, pay_in_type, cost_msat, context_ref, created_at
         FROM pay_ins
        WHERE (
          payer_ref = ?
          OR id IN (SELECT pay_in_id FROM pay_in_legs WHERE party_ref = ? AND direction = 'out')
        )`
  const orderAndLimit = `
        ORDER BY created_at DESC, id DESC
        LIMIT ?`

  const rowsRaw =
    cursor === undefined
      ? await ledgerDb.query(`${baseSelect}${orderAndLimit}`, [
          actorRef,
          actorRef,
          limit + 1,
        ])
      : await ledgerDb.query(
          `${baseSelect}
        AND (
          created_at < ?
          OR (created_at = ? AND id < ?)
        )${orderAndLimit}`,
          [actorRef, actorRef, cursor.createdAt, cursor.createdAt, cursor.id, limit + 1],
        )

  // Postgres returns bigint columns (`cost_msat`) as STRINGS; normalize every
  // field through the same explicit decode the response mapping needs.
  const rows: Array<PayInHistoryRow> = rowsRaw.map(row => ({
    context_ref: row.context_ref === null ? null : String(row.context_ref),
    cost_msat: Number(row.cost_msat),
    created_at: String(row.created_at),
    id: String(row.id),
    pay_in_type: String(row.pay_in_type),
  }))
  const hasMore = rows.length > limit
  const page = hasMore ? rows.slice(0, limit) : rows
  const last = page[page.length - 1]
  const nextCursor = hasMore && last !== undefined ? encodeTransactionsCursor(last.created_at, last.id) : null

  return noStoreJsonResponse({
    nextCursor,
    transactions: page.map(row => ({
      amountUsdCents: msatToUsdCentsRound(row.cost_msat),
      description: mobileCreditsTransactionDescription(row.pay_in_type, row.context_ref),
      id: row.id,
      kind: mobileCreditsTransactionKind(row.pay_in_type),
      occurredAt: row.created_at,
    })),
  })
}

export const handleMobileCreditsBalanceRequest = <Bindings, User>(
  dependencies: MobileCreditsRouteDependencies<Bindings, User>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> =>
  Effect.promise(() => routeBalance(dependencies, request, env, ctx))

export const handleMobileCreditsTransactionsRequest = <Bindings, User>(
  dependencies: MobileCreditsRouteDependencies<Bindings, User>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
): Effect.Effect<HttpResponse> =>
  Effect.promise(() => routeTransactions(dependencies, request, env, ctx))
