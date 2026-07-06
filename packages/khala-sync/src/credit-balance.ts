import { Schema as S } from "effect"

/**
 * Per-user credit-balance entity contract (issue #8505, Part 2; see
 * docs/khala-code/2026-07-06-credits-ledger-vs-khala-sync-architecture-audit.md
 * for why the D1 `agent_balances` ledger stays the authoritative write/decision
 * path and this is an additive, best-effort projection on top of it).
 *
 * A user's credit balance is projected from the D1 ledger into
 * `scope.user.<userId>` (the SAME personal scope `personalScope`/`chat.ts`
 * already replicates thread metadata into, so the mobile app's existing
 * subscription just gains one more entity type). The ingest path applies a
 * signed delta (a grant is positive, a charge/clawback is negative) and
 * appends the post-image to the scope in the SAME Postgres transaction as the
 * exact-once idempotency guard — see
 * `khala-sync-server/src/user-credit-balance-projection.ts` for the
 * increment/backfill/repair logic, modeled directly on
 * `public-counter.ts` + `public-counter-projection.ts`.
 *
 * PUBLIC-SAFE-SHAPED BUT OWNER-PRIVATE BY SCOPE: the entity itself carries no
 * payment material (no card, no destination, no raw msat ledger internals) —
 * just a bounded user id and a non-negative USD-cents total — but it is only
 * ever appended into the OWNING user's personal scope, never a public one.
 *
 * This module is deliberately self-contained (imports only `effect`) so it
 * can be re-exported from ./index without a module cycle — same rule as
 * ./public-counter and ./fleet.
 */

// ---------------------------------------------------------------------------
// Entity type name (changelog `entityType` value)
// ---------------------------------------------------------------------------

export const CREDIT_BALANCE_ENTITY_TYPE = "credit_balance"

// ---------------------------------------------------------------------------
// Bounded field primitives
// ---------------------------------------------------------------------------

/** Stable OpenAgents user id — the `entityId` and the personal-scope owner. */
export const CreditBalanceUserId = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(128),
)
export type CreditBalanceUserId = typeof CreditBalanceUserId.Type

/** ISO-8601 UTC timestamp string (same shape the other sync entities use). */
export const CreditBalanceIsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
)

/**
 * The projected balance in USD cents: a non-negative safe integer, converted
 * from D1 `agent_balances.balance_msat` at the single shared BTC/USD
 * reference rate (`usd-msat-conversion.ts`'s `msatToUsdCentsRound`) every
 * other credits surface (the Aiur admin console, the mobile REST balance
 * route) already uses — this projection can never disagree with those on the
 * conversion, only (transiently, until the next reconcile) on freshness.
 */
export const CreditBalanceUsdCents = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
)

// ---------------------------------------------------------------------------
// credit_balance entity
// ---------------------------------------------------------------------------

/**
 * One user's projected balance post-image. `entityId` is the user id (the
 * same id that names the scope: `scope.user.<userId>`).
 */
export class CreditBalanceEntity extends S.Class<CreditBalanceEntity>(
  "CreditBalanceEntity",
)({
  userId: CreditBalanceUserId,
  balanceUsdCents: CreditBalanceUsdCents,
  /** Observed-at of the newest source ledger event applied; null before any
   * event (immediately after backfill, this is the backfill instant). */
  lastEventAt: S.NullOr(CreditBalanceIsoTimestamp),
}) {}

// ---------------------------------------------------------------------------
// Boundary codecs
// ---------------------------------------------------------------------------

export const decodeCreditBalanceEntity = S.decodeUnknownSync(CreditBalanceEntity)
export const encodeCreditBalanceEntity = S.encodeSync(CreditBalanceEntity)
