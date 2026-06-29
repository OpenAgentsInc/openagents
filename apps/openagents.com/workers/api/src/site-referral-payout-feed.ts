import {
  type CreateReferralPayoutEligibilityInput,
  type SiteReferralPayoutLedgerEntry,
  type SiteReferralPayoutLedgerStorageError,
  type SiteReferralPayoutLedgerValidationError,
  createReferralPayoutEligibility,
} from './site-referral-payout-ledger'
import {
  type AssetBoundaryAsset,
  validateAssetBoundary,
} from './asset-bitcoin-boundary'

/**
 * RL-1 (openagents #5458): FEED the referral payout ledger from a real paid
 * event.
 *
 * The referral *attribution* spine is already built and live: a paid customer's
 * consumed attribution row sits in `user_referral_attributions`, last-touch and
 * consume-once. This module is the missing wire between that captured
 * attribution and the 5% payout ledger's `createReferralPayoutEligibility`,
 * which previously had zero production callers.
 *
 * Given a paid `userId` and a qualifying paid event, it resolves the owning
 * referrer (the `site_referral_sources.referrer_user_id` behind the attribution)
 * and creates exactly one eligibility row, keyed idempotently so a webhook retry
 * never double-feeds. The rev-share asset of the qualifying event is carried
 * through to the dispatch path, which enforces the Bitcoin/credit boundary
 * (`site-referral-payout-dispatch.ts`): credit/USD revenue records a non-
 * withdrawable-Bitcoin eligibility; only Bitcoin revenue may later move Bitcoin.
 *
 * This module never moves money. It only records eligibility.
 */

/**
 * Rev-share asset of the qualifying paid event. Per the invariant
 * (`INVARIANTS.md` "Site Referral Bitcoin Withdrawal Gate"):
 *
 * - `bitcoin`  -- Bitcoin revenue -> Bitcoin revshare is withdrawable-eligible.
 * - `credit`   -- credit-balance spend -> credit revshare; never a Bitcoin
 *   liability (the dispatcher refuses Bitcoin movement for it).
 * - `usd`      -- fiat credit purchase (Stripe) -> credit revshare; never a
 *   Bitcoin liability either. Stripe credit top-up is this asset.
 */
export type SiteReferralRevenueAsset = 'bitcoin' | 'credit' | 'usd'

/**
 * Map a referral rev-share asset onto the shared credit<->Bitcoin boundary asset
 * (RL-3 #5460). The referral path has no `free` source (the ledger refuses a
 * zero/no-qualifying-amount event before it reaches dispatch), so the three
 * referral assets map 1:1: `bitcoin` -> `bitcoin`, `credit` -> `credit`, `usd`
 * -> `usd`. Centralizing the mapping keeps the boundary the single source of
 * truth on both the feed (eligibility) and dispatch (payout) sides.
 */
export const referralRevenueAssetToBoundaryAsset = (
  asset: SiteReferralRevenueAsset,
): AssetBoundaryAsset => asset

/**
 * A real paid event for an attributed customer. The caller supplies the public-
 * safe qualifying event ref (e.g. a Stripe checkout-session-derived ref), the
 * sat-denominated qualifying amount used for the 5% calculation, and the
 * rev-share asset. `idempotencyKey` MUST be deterministic per paid event so the
 * same Stripe webhook delivered twice creates the eligibility row at most once.
 */
export type ReferralPaidEventInput = Readonly<{
  idempotencyKey: string
  nowIso: string
  periodKey: string
  qualifyingAmountSats: number
  qualifyingEventKind: string
  qualifyingEventRef: string
  revenueAsset: SiteReferralRevenueAsset
  /** The paid customer whose consumed attribution decides the referrer. */
  userId: string
}>

export type ReferralPaidEventResult =
  | Readonly<{ _tag: 'no_attribution' }>
  | Readonly<{ _tag: 'self_attribution' }>
  | Readonly<{
      _tag: 'boundary_refused'
      reasonRef: string
    }>
  | Readonly<{
      _tag: 'recorded'
      entry: SiteReferralPayoutLedgerEntry
    }>

/**
 * The asset the recorded revshare eligibility is denominated in. Bitcoin revenue
 * creates a Bitcoin-withdrawable-eligible revshare; credit/USD revenue creates a
 * credit revshare (never a Bitcoin liability). This is the contributor-side asset
 * the shared boundary guard checks against the revenue asset.
 */
const revshareContributorAssetFor = (
  revenueAsset: SiteReferralRevenueAsset,
): AssetBoundaryAsset => (revenueAsset === 'bitcoin' ? 'bitcoin' : 'credit')

export type ReferralPaidEventError =
  | SiteReferralPayoutLedgerStorageError
  | SiteReferralPayoutLedgerValidationError

type ConsumedAttributionRow = Readonly<{
  referral_attribution_id: string
  referral_invite_id: string | null
  referral_source_id: string
  referrer_user_id: string
}>

const SAFE_USER_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/

/**
 * Read the consumed referral attribution for a paid user and join it to the
 * owning referrer. `user_referral_attributions` holds exactly one consumed
 * attribution per user (PRIMARY KEY user_id, consume-once), and
 * `site_referral_sources.referrer_user_id` is the source owner who earns the
 * 5% reward. The join is read-only and bounded; the referrer is never inferred
 * from any other field.
 */
const readConsumedAttributionForUser = async (
  db: D1Database,
  userId: string,
): Promise<ConsumedAttributionRow | null> => {
  if (!SAFE_USER_ID_PATTERN.test(userId)) {
    return null
  }

  return db
    .prepare(
      `SELECT ura.referral_attribution_id AS referral_attribution_id,
              ura.referral_invite_id AS referral_invite_id,
              ura.referral_source_id AS referral_source_id,
              src.referrer_user_id AS referrer_user_id
         FROM user_referral_attributions AS ura
         JOIN site_referral_sources AS src
           ON src.id = ura.referral_source_id
        WHERE ura.user_id = ?
          AND ura.archived_at IS NULL
          AND ura.policy_state = 'active'
          AND src.archived_at IS NULL
          AND src.policy_state = 'active'
        LIMIT 1`,
    )
    .bind(userId)
    .first<ConsumedAttributionRow>()
}

/**
 * Wire a real paid event into the referral payout ledger. Resolves the paid
 * user's consumed attribution + referrer and creates a single eligibility row
 * (idempotent). Returns `no_attribution` when the paid user was not referred
 * (the common case -- most paid users have no consumed referral). Returns
 * `self_attribution` when the referrer and the paid customer are the same user
 * (the ledger would refuse it anyway; we short-circuit before recording).
 *
 * The qualifying amount is sat-denominated; the ledger calculates the 5% reward
 * and applies the per-event and per-referrer-period caps. The `revenueAsset` is
 * stamped onto the qualifying-event ref family so the dispatch path can enforce
 * the rev-share asset boundary.
 */
export const recordReferralPayoutForPaidEvent = async (
  db: D1Database,
  input: ReferralPaidEventInput,
): Promise<ReferralPaidEventResult> => {
  const attribution = await readConsumedAttributionForUser(db, input.userId)

  if (attribution === null) {
    return { _tag: 'no_attribution' }
  }

  if (attribution.referrer_user_id === input.userId) {
    return { _tag: 'self_attribution' }
  }

  // RL-3 (#5460): enforce the SHARED credit<->Bitcoin asset boundary at
  // eligibility (revshare) creation, not only at dispatch. The recorded revshare
  // is denominated to match the revenue source (Bitcoin -> Bitcoin; credit/USD
  // -> credit), so this never silently records a credit-funded Bitcoin
  // liability. Fail closed (record NO eligibility) if a mapping ever violated
  // the boundary.
  const boundaryViolation = validateAssetBoundary({
    contributorAsset: revshareContributorAssetFor(input.revenueAsset),
    movement: 'revshare',
    revenueAsset: referralRevenueAssetToBoundaryAsset(input.revenueAsset),
  })

  if (boundaryViolation !== null) {
    return { _tag: 'boundary_refused', reasonRef: boundaryViolation.reasonRef }
  }

  const createInput: CreateReferralPayoutEligibilityInput = {
    idempotencyKey: input.idempotencyKey,
    nowIso: input.nowIso,
    periodKey: input.periodKey,
    qualifyingAmountSats: input.qualifyingAmountSats,
    qualifyingEventKind: input.qualifyingEventKind,
    qualifyingEventRef: input.qualifyingEventRef,
    referredUserId: input.userId,
    referralAttributionId: attribution.referral_attribution_id,
    referralInviteId: attribution.referral_invite_id,
    referralSourceId: attribution.referral_source_id,
    referrerUserId: attribution.referrer_user_id,
  }

  const entry = await createReferralPayoutEligibility(db, createInput)

  return { _tag: 'recorded', entry }
}
