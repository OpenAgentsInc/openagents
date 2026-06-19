// Refer-once-earn-forever: PERMANENT, CROSS-CATEGORY referral binding + accrual
// (openagents #5513).
//
// "Refer once, earn forever" means the referrer<->referee binding is permanent
// and spans ALL purchase categories, not just Sites. The attribution spine
// already makes the binding permanent and category-independent: a consumed
// `user_referral_attributions` / `agent_referral_attributions` row (consume-once,
// last-touch, active-policy) names the referrer for that account FOREVER. The
// missing piece for #5513 is a SINGLE category-agnostic accrual entry point so
// any category's "a referred party just paid for something" event feeds the ONE
// RL-1 payout ledger the same way -- inference already has its own wrapper
// (`inference/inference-referral-accrual.ts`); this is the generalization every
// other category (Sites firm-up, marketplace, fine-tuning, sandbox, web
// services, ...) routes through, so there is never a parallel ledger or a
// per-category attribution path.
//
// HONEST SCOPE (the invariants this primitive makes literally true):
// - Usage-funded only: accrual fires on a real, metered, non-zero PAID event.
//   Never on signups, never on free/promo grants. The caller passes the
//   qualifying spend (margin/revenue) in sats; a zero/negative amount accrues
//   nothing (NOT an error).
// - Asset boundary (RL-3, the shared `validateAssetBoundary` guard): Bitcoin
//   revenue -> Bitcoin-eligible revshare; credit/USD revenue -> credit revshare,
//   never a withdrawable Bitcoin liability. Fail closed on any violation.
// - One ledger, one policy: the 5% cut + per-event/per-referrer-period caps are
//   the existing RL-1 ledger policy. This primitive computes NO second
//   percentage; it feeds the qualifying amount and lets the ledger apply policy.
// - Idempotent per (category, event): the caller supplies a deterministic event
//   id; a replayed paid event hits the ledger's UNIQUE idempotency key and is a
//   no-op (never double-accrues), so an at-least-once webhook is safe.
//
// This module moves NO money. It records eligibility only. Settlement stays on
// the readiness-gated, owner-armed dispatch rail (`site-referral-payout-
// dispatch.ts`), unchanged.

import {
  type AssetBoundaryAsset,
  validateAssetBoundary,
} from './asset-bitcoin-boundary'
import { currentIsoTimestamp } from './runtime-primitives'
import {
  type SiteReferralRevenueAsset,
  referralRevenueAssetToBoundaryAsset,
} from './site-referral-payout-feed'
import {
  type CreateReferralPayoutEligibilityInput,
  type SiteReferralPayoutLedgerEntry,
  calculateReferralPayoutSats,
  createReferralPayoutEligibility,
} from './site-referral-payout-ledger'

// The principal whose consumed attribution decides the referrer. Inference
// principals are agents (`agent:<userId>`); most other categories spend as a
// bare user. Both resolve through the SAME attribution tables the site feed and
// inference accrual already use -- no new attribution path.
export type ReferredPrincipal =
  | Readonly<{ kind: 'agent'; userId: string }>
  | Readonly<{ kind: 'user'; userId: string }>

const SAFE_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/
// Category labels are bounded enum-like fields used to namespace refs, so a
// deterministic shape check is the correct tool (not intent routing).
const SAFE_CATEGORY_PATTERN = /^[a-z][a-z0-9_]{0,40}$/
const SAFE_EVENT_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/

type ConsumedAttributionRow = Readonly<{
  referral_attribution_id: string
  referral_invite_id: string | null
  referral_source_id: string
  referrer_user_id: string
}>

/**
 * Resolve the permanent referrer behind a referred principal. Reuses the SAME
 * consume-once attribution tables + active-policy join the site feed and
 * inference accrual use; agents read `agent_referral_attributions`, bare users
 * read `user_referral_attributions`. Read-only, bounded, never infers a referrer
 * from any other field. This is the "earn forever, across categories" binding:
 * the link is the same regardless of which category the spend came from.
 */
const readReferrerForPrincipal = async (
  db: D1Database,
  principal: ReferredPrincipal,
): Promise<ConsumedAttributionRow | null> => {
  if (!SAFE_ID_PATTERN.test(principal.userId)) {
    return null
  }

  if (principal.kind === 'agent') {
    return db
      .prepare(
        `SELECT ara.referral_attribution_id AS referral_attribution_id,
                ara.referral_invite_id AS referral_invite_id,
                ara.referral_source_id AS referral_source_id,
                src.referrer_user_id AS referrer_user_id
           FROM agent_referral_attributions AS ara
           JOIN site_referral_sources AS src
             ON src.id = ara.referral_source_id
          WHERE ara.agent_user_id = ?
            AND ara.archived_at IS NULL
            AND ara.policy_state = 'active'
            AND src.archived_at IS NULL
            AND src.policy_state = 'active'
          LIMIT 1`,
      )
      .bind(principal.userId)
      .first<ConsumedAttributionRow>()
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
    .bind(principal.userId)
    .first<ConsumedAttributionRow>()
}

const revshareContributorAssetFor = (
  asset: SiteReferralRevenueAsset,
): AssetBoundaryAsset => (asset === 'bitcoin' ? 'bitcoin' : 'credit')

// Deterministic, public-safe per-(category,event) refs. One accrual per category
// purchase event, so a replay hits the ledger's UNIQUE idempotency key and is a
// no-op. Category namespacing keeps a sites event and an inference event for the
// SAME underlying id from colliding.
export const crossCategoryQualifyingEventRef = (
  category: string,
  eventId: string,
): string => `referral.${category}.event.${eventId}`

export const crossCategoryIdempotencyKey = (
  category: string,
  eventId: string,
): string => `referral:${category}:accrual:${eventId}`

// One PAYOUT per category event so each purchase's referral cut is an
// independently dispatchable payout (mirrors inference per-request payouts).
export const crossCategoryPayoutRef = (
  category: string,
  eventId: string,
): string => `referral.${category}.payout.${eventId}`

// Per-referrer-period cap bucket: a per-category calendar-month bucket (YYYY-MM,
// UTC), so the existing per-referrer-period caps apply per category per month.
export const crossCategoryPeriodKey = (
  category: string,
  nowIso: string,
): string => {
  const month = nowIso.slice(0, 7)
  return /^\d{4}-\d{2}$/.test(month)
    ? `${category}-${month}`
    : `${category}-unknown`
}

export type AccrueCrossCategoryReferralInput = Readonly<{
  // Bounded category label, e.g. `sites`, `marketplace`, `fine_tuning`,
  // `sandbox`, `web_services`. Namespaces the ledger refs.
  category: string
  // Deterministic per-event id (e.g. a settled-order ref). Idempotency anchor.
  eventId: string
  // Human/agent-facing kind recorded on the ledger row.
  qualifyingEventKind: string
  // The qualifying spend (margin/revenue) in sats. The ledger applies the 5%
  // policy + caps. <= 0 accrues nothing.
  qualifyingAmountSats: number
  // Rev-share asset of the qualifying revenue. Decides the asset boundary.
  revenueAsset: SiteReferralRevenueAsset
  // The paying principal whose permanent attribution decides the referrer.
  principal: ReferredPrincipal
  // ISO clock override (tests). Defaults to the runtime clock.
  nowIso?: (() => string) | undefined
}>

export type AccrueCrossCategoryReferralResult =
  | Readonly<{ _tag: 'invalid_input'; reason: string }>
  | Readonly<{ _tag: 'no_attribution' }>
  | Readonly<{ _tag: 'self_attribution' }>
  | Readonly<{ _tag: 'zero_referrer_share' }>
  | Readonly<{ _tag: 'boundary_refused'; reasonRef: string }>
  | Readonly<{ _tag: 'recorded'; entry: SiteReferralPayoutLedgerEntry }>

/**
 * Accrue the referrer's ongoing cut for ONE paid event in ANY category. Resolves
 * the paying principal's permanent referrer (agent or user attribution),
 * enforces the asset boundary, and records the referrer's 5% (ledger policy) cut
 * as a single sat-denominated eligibility row in the existing RL-1 ledger.
 *
 * Returns:
 * - `invalid_input` when the category/event id is malformed (bounded fields).
 * - `no_attribution` when the principal was not referred (the common case).
 * - `self_attribution` when the referrer is the principal (short-circuited).
 * - `zero_referrer_share` when 5% of the qualifying amount rounds below 1 sat.
 * - `boundary_refused` when the RL-3 asset boundary blocks the revshare.
 * - `recorded` with the created ledger entry.
 *
 * Idempotent per (category, event). Records eligibility only -- moves NO money.
 */
export const accrueCrossCategoryReferral = async (
  db: D1Database,
  input: AccrueCrossCategoryReferralInput,
): Promise<AccrueCrossCategoryReferralResult> => {
  if (!SAFE_CATEGORY_PATTERN.test(input.category)) {
    return { _tag: 'invalid_input', reason: 'category must be a bounded label' }
  }

  if (!SAFE_EVENT_ID_PATTERN.test(input.eventId)) {
    return { _tag: 'invalid_input', reason: 'eventId must be a bounded id' }
  }

  // Usage-funded only: a zero/negative or below-1-sat-cut event accrues nothing.
  // Never on signups; the caller only invokes this on a real metered paid event.
  if (
    !Number.isFinite(input.qualifyingAmountSats) ||
    input.qualifyingAmountSats <= 0 ||
    calculateReferralPayoutSats(input.qualifyingAmountSats) <= 0
  ) {
    return { _tag: 'zero_referrer_share' }
  }

  const attribution = await readReferrerForPrincipal(db, input.principal)

  if (attribution === null) {
    return { _tag: 'no_attribution' }
  }

  if (attribution.referrer_user_id === input.principal.userId) {
    return { _tag: 'self_attribution' }
  }

  // RL-3: enforce the SHARED credit<->Bitcoin asset boundary at accrual, exactly
  // as the site feed and inference accrual do. Bitcoin revenue -> Bitcoin-
  // eligible revshare; credit/USD -> credit revshare (never a Bitcoin liability).
  const boundaryViolation = validateAssetBoundary({
    contributorAsset: revshareContributorAssetFor(input.revenueAsset),
    movement: 'revshare',
    revenueAsset: referralRevenueAssetToBoundaryAsset(input.revenueAsset),
  })

  if (boundaryViolation !== null) {
    return { _tag: 'boundary_refused', reasonRef: boundaryViolation.reasonRef }
  }

  const nowIso = (input.nowIso ?? currentIsoTimestamp)()

  const createInput: CreateReferralPayoutEligibilityInput = {
    idempotencyKey: crossCategoryIdempotencyKey(input.category, input.eventId),
    nowIso,
    payoutRef: crossCategoryPayoutRef(input.category, input.eventId),
    periodKey: crossCategoryPeriodKey(input.category, nowIso),
    qualifyingAmountSats: input.qualifyingAmountSats,
    qualifyingEventKind: input.qualifyingEventKind,
    qualifyingEventRef: crossCategoryQualifyingEventRef(
      input.category,
      input.eventId,
    ),
    referredUserId: input.principal.userId,
    referralAttributionId: attribution.referral_attribution_id,
    referralInviteId: attribution.referral_invite_id,
    referralSourceId: attribution.referral_source_id,
    referrerUserId: attribution.referrer_user_id,
  }

  const entry = await createReferralPayoutEligibility(db, createInput)

  return { _tag: 'recorded', entry }
}
