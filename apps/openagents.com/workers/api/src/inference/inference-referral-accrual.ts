// Ongoing referral accrual on ALL inference (EPIC #5474 / sub-EPIC #5475,
// children #5487 attribution + #5488 ongoing accrual + #5489 split feed).
//
// This is the wire between the LIVE metering decrement (`metering-hook.ts` — the
// single "a request was PAID" point) and the existing RL-1 referral payout
// ledger (`site-referral-payout-ledger.ts`). It does NOT build a parallel
// ledger: it resolves the paying account's referrer through the same attribution
// tables the site referral spine already maintains, then records one referral
// payout eligibility row per paid inference request, idempotently, ONGOING /
// indefinitely — every paid request by a referred account accrues, not a
// one-time bounty (business doc §3: "ongoing cut of all of their spend …
// indefinitely").
//
// SHAPE: a `MeteringHook` decorator (`withReferralAccrual`) wraps the live
// ledger hook. The inner hook runs first (the real balance decrement); accrual
// fires ONLY when the inner outcome is `metered: true` AND not `zeroCharge` —
// i.e. a real, non-zero paid request actually occurred. So accrual is INERT on
// the flag-off path (the route never reaches the hook), on the stub hook
// (`metered: false`), and on zero-charge requests. It also never fails the
// request: an accrual error is logged (public-safe) and swallowed — referral
// bookkeeping must not break the customer's inference call.
//
// #5487 attribution: the account is resolved to its referrer through the
// existing consume-once attribution tables. Inference principals are agents
// (`agent:<userId>`), so we read `agent_referral_attributions` first; a bare
// user principal reads `user_referral_attributions` (reusing the exact join the
// site feed uses). No new attribution path — we reuse the spine's link.
//
// #5489 split feed: the REFERRER share of the three-way margin split
// (`inference-referral-split.ts`) is the sat-denominated qualifying amount fed
// to the ledger. The serving-node share is computed by the same split but fed by
// the sibling serving-node payout work, not here.

import { Effect } from 'effect'

import type { InferenceEntitlementsMirror } from '../inference-entitlements-store'
import { workerLogEntry } from '../observability'
import {
  type CreateReferralPayoutEligibilityInput,
  type SiteReferralPayoutLedgerEntry,
  calculateReferralPayoutSats,
  createReferralPayoutEligibility,
} from '../site-referral-payout-ledger'
import {
  type SiteReferralRevenueAsset,
  referralRevenueAssetToBoundaryAsset,
} from '../site-referral-payout-feed'
import { validateAssetBoundary } from '../asset-bitcoin-boundary'
import { currentIsoTimestamp } from '../runtime-primitives'
import {
  type MeteringContext,
  type MeteringHook,
  type MeteringOutcome,
} from './metering-hook'
import { type FundingKind, priceRequest } from './pricing'
import {
  type InferenceSplit,
  type InferenceSplitWeights,
  computeInferenceSplit,
} from './inference-referral-split'

class InferenceReferralAccrualPersistenceError extends Error {
  readonly _tag = 'InferenceReferralAccrualPersistenceError'

  constructor(cause: unknown) {
    super(cause instanceof Error ? cause.message : String(cause))
    this.name = 'InferenceReferralAccrualPersistenceError'
  }
}

const inferenceReferralAccrualPersistenceError = (error: unknown) =>
  new InferenceReferralAccrualPersistenceError(error)

// Bounded, structural parse of a metering accountRef into the referred party.
// This is NOT intent routing — it is a fixed-shape principal-kind prefix on an
// already-authenticated account ref, so deterministic parsing is the correct
// tool (workspace guidance: deterministic parsing is acceptable for bounded
// fields after the route is already selected). `agent:<userId>` is the inference
// principal shape the gateway mints; a bare token is treated as a user id.
const SAFE_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/

export type ReferredParty =
  | Readonly<{ kind: 'agent'; userId: string }>
  | Readonly<{ kind: 'user'; userId: string }>
  | null

export const parseReferredParty = (accountRef: string): ReferredParty => {
  const trimmed = accountRef.trim()
  if (trimmed.startsWith('agent:')) {
    const userId = trimmed.slice('agent:'.length)
    return SAFE_ID_PATTERN.test(userId) ? { kind: 'agent', userId } : null
  }
  return SAFE_ID_PATTERN.test(trimmed)
    ? { kind: 'user', userId: trimmed }
    : null
}

type ConsumedAttributionRow = Readonly<{
  referral_attribution_id: string
  referral_invite_id: string | null
  referral_source_id: string
  referrer_user_id: string
}>

// Resolve the referrer behind a referred party. Reuses the SAME attribution
// tables + active-policy join the site feed uses; never infers a referrer from
// any other field. Agents read `agent_referral_attributions` (the agent itself
// is the referred entity — `agent_user_id` FKs `users(id)`); bare users read
// `user_referral_attributions`. Read-only and bounded.
const readReferrerForParty = async (
  db: D1Database,
  party: Exclude<ReferredParty, null>,
): Promise<ConsumedAttributionRow | null> => {
  if (party.kind === 'agent') {
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
      .bind(party.userId)
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
    .bind(party.userId)
    .first<ConsumedAttributionRow>()
}

// Map the funding kind onto the rev-share asset. Card/USD funding => `usd`
// revenue (credit revshare, never a Bitcoin liability); Bitcoin funding =>
// `bitcoin` revenue (withdrawable-Bitcoin-eligible revshare). This is what the
// asset-boundary guard (RL-3) checks both here (at accrual) and at dispatch.
export const fundingKindToRevenueAsset = (
  fundingKind: FundingKind,
): SiteReferralRevenueAsset => (fundingKind === 'bitcoin' ? 'bitcoin' : 'usd')

const revshareContributorAssetFor = (asset: SiteReferralRevenueAsset) =>
  asset === 'bitcoin' ? ('bitcoin' as const) : ('credit' as const)

// Public-safe, deterministic per-request qualifying-event ref + idempotency key.
// One referral accrual per served inference request id, so a retried/replayed
// settle for the SAME request hits the ledger's UNIQUE idempotency key and is a
// no-op (never double-accrues). Neutral; carries no payment material.
export const inferenceReferralQualifyingEventRef = (requestId: string): string =>
  `inference.referral.request.${requestId}`

export const inferenceReferralIdempotencyKey = (requestId: string): string =>
  `inference:referral:accrual:${requestId}`

// One PAYOUT per served inference request, so each request's referral cut is an
// independently dispatchable payout (and the dashboard's "latest per payout_ref"
// rollup resolves one row per request). The ledger defaults `payoutRef` to the
// attribution id, which would collapse every request from one referred account
// onto a SINGLE payout — wrong for ongoing per-request accrual — so we always
// pass an explicit per-request payout ref.
export const inferenceReferralPayoutRef = (requestId: string): string =>
  `inference.referral.payout.${requestId}`

export const INFERENCE_REFERRAL_QUALIFYING_EVENT_KIND =
  'inference_paid_request' as const

export const inferenceReferralMarginSplitRef = (requestId: string): string =>
  `inference.referral.margin_split.${requestId}`

export const inferenceReferralChargeReceiptRef = (requestId: string): string =>
  `receipt.inference.charge.${requestId}`

// Period key for the ledger's per-referrer-period caps. Inference accrual is
// ongoing, so a calendar-month bucket (YYYY-MM, UTC) is the period the existing
// per-referrer-period cap applies over. Derived from the request's ISO time.
export const inferenceReferralPeriodKey = (nowIso: string): string => {
  const month = nowIso.slice(0, 7)
  return /^\d{4}-\d{2}$/.test(month)
    ? `inference-${month}`
    : 'inference-unknown'
}

export type AccrueInferenceReferralInput = Readonly<{
  context: MeteringContext
  // The split weights to use (defaults applied in the split module).
  weights?: InferenceSplitWeights | undefined
  // ISO time source for the ledger row + period key. Defaults to the runtime
  // clock.
  nowIso?: (() => string) | undefined
}>

export type AccrueInferenceReferralResult =
  | Readonly<{ _tag: 'no_attribution' }>
  | Readonly<{ _tag: 'self_attribution' }>
  | Readonly<{ _tag: 'zero_referrer_share' }>
  | Readonly<{ _tag: 'boundary_refused'; reasonRef: string }>
  | Readonly<{
      _tag: 'recorded'
      entry: SiteReferralPayoutLedgerEntry
      marginSplitRef: string
    }>

const countServingNodes = (context: MeteringContext): number => {
  const stages = context.servingReceipt?.stages ?? []
  return new Set(stages.map(stage => stage.nodeRef)).size
}

const recordInferenceReferralMarginSplit = async (
  db: D1Database,
  input: Readonly<{
    attribution: ConsumedAttributionRow
    context: MeteringContext
    nowIso: string
    party: Exclude<ReferredParty, null>
    split: InferenceSplit
  }>,
  // KS-8.9 (#8320): fire-safe Postgres dual-write mirror.
  mirror?: InferenceEntitlementsMirror | undefined,
): Promise<string> => {
  const requestId = input.context.requestId
  const splitRef = inferenceReferralMarginSplitRef(requestId)
  const servingNodeCount = countServingNodes(input.context)

  await db
    .prepare(
      `INSERT OR IGNORE INTO inference_referral_margin_splits
         (id, request_id, account_ref, referred_user_id, referrer_user_id,
          referral_attribution_id, referral_source_id, referral_invite_id,
          payout_ref, qualifying_event_ref, charge_receipt_ref, funding_kind,
          adapter_id, requested_model, served_model, served_by_contributor,
          serving_node_count, charge_usd, cost_usd, margin_usd, margin_sats,
          openagents_usd, openagents_sats, serving_node_usd, serving_node_sats,
          referrer_usd, referrer_sats, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      splitRef,
      requestId,
      input.context.accountRef,
      input.party.userId,
      input.attribution.referrer_user_id,
      input.attribution.referral_attribution_id,
      input.attribution.referral_source_id,
      input.attribution.referral_invite_id,
      inferenceReferralPayoutRef(requestId),
      inferenceReferralQualifyingEventRef(requestId),
      inferenceReferralChargeReceiptRef(requestId),
      input.context.fundingKind,
      input.context.adapterId,
      input.context.requestedModel,
      input.context.servedModel,
      input.context.servingReceipt === undefined ? 0 : 1,
      servingNodeCount,
      input.split.chargeUsd,
      input.split.costUsd,
      input.split.marginUsd,
      input.split.marginSats,
      input.split.openagents.usd,
      input.split.openagents.sats,
      input.split.servingNode.usd,
      input.split.servingNode.sats,
      input.split.referrer.usd,
      input.split.referrer.sats,
      input.nowIso,
    )
    .run()

  mirror?.([
    {
      kind: 'write',
      row: {
        account_ref: input.context.accountRef,
        adapter_id: input.context.adapterId,
        archived_at: null,
        charge_receipt_ref: inferenceReferralChargeReceiptRef(requestId),
        charge_usd: input.split.chargeUsd,
        cost_usd: input.split.costUsd,
        created_at: input.nowIso,
        funding_kind: input.context.fundingKind,
        id: splitRef,
        margin_sats: input.split.marginSats,
        margin_usd: input.split.marginUsd,
        openagents_sats: input.split.openagents.sats,
        openagents_usd: input.split.openagents.usd,
        payout_ref: inferenceReferralPayoutRef(requestId),
        qualifying_event_ref: inferenceReferralQualifyingEventRef(requestId),
        referral_attribution_id: input.attribution.referral_attribution_id,
        referral_invite_id: input.attribution.referral_invite_id,
        referral_source_id: input.attribution.referral_source_id,
        referred_user_id: input.party.userId,
        referrer_sats: input.split.referrer.sats,
        referrer_usd: input.split.referrer.usd,
        referrer_user_id: input.attribution.referrer_user_id,
        request_id: requestId,
        requested_model: input.context.requestedModel,
        served_by_contributor: input.context.servingReceipt === undefined ? 0 : 1,
        served_model: input.context.servedModel,
        serving_node_count: servingNodeCount,
        serving_node_sats: input.split.servingNode.sats,
        serving_node_usd: input.split.servingNode.usd,
      },
      table: 'inference_referral_margin_splits',
    },
  ])

  return splitRef
}

/**
 * Accrue the referrer's ongoing cut for ONE paid inference request. Resolves the
 * paying account's referrer (agent or user attribution), computes the three-way
 * split, and records the REFERRER share as a single sat-denominated eligibility
 * row in the existing RL-1 ledger. Returns:
 *
 * - `no_attribution` when the paying account was not referred (the common case).
 * - `self_attribution` when the referrer is the paying account (short-circuited).
 * - `zero_referrer_share` when the priced referrer share rounded below 1 sat
 *   (tiny request) — nothing accrues this request; spend accrues as it crosses
 *   1 sat. NOT an error.
 * - `boundary_refused` when the RL-3 asset boundary blocks the revshare.
 * - `recorded` with the created ledger entry.
 *
 * Idempotent per served request id. PURE pricing + split; the only IO is the
 * read of the attribution + the ledger insert.
 */
export const accrueInferenceReferral = async (
  db: D1Database,
  input: AccrueInferenceReferralInput,
  // KS-8.9 (#8320): fire-safe Postgres dual-write mirror.
  mirror?: InferenceEntitlementsMirror | undefined,
): Promise<AccrueInferenceReferralResult> => {
  const context = input.context
  const party = parseReferredParty(context.accountRef)
  if (party === null) {
    return { _tag: 'no_attribution' }
  }

  const attribution = await readReferrerForParty(db, party)
  if (attribution === null) {
    return { _tag: 'no_attribution' }
  }

  // The referred party self-referred (its own user id is the source owner). The
  // ledger would refuse it anyway; short-circuit before computing/recording.
  if (attribution.referrer_user_id === party.userId) {
    return { _tag: 'self_attribution' }
  }

  // Re-price the request from the SAME inputs the live decrement used (pure,
  // cheap) and compute the three-way split. First-party quota / passthrough
  // adapters have no contributor; only network nodes do. The split module zeroes
  // the serving-node share when `servedByContributor` is false; the referrer
  // share is independent of who served it (referral is on ALL inference).
  const priced = priceRequest({
    batch: context.batch ?? false,
    fundingKind: context.fundingKind,
    model: context.servedModel,
    usage: context.usage,
  })

  const split = computeInferenceSplit({
    priced,
    // The referrer share is independent of who served it, but the receipt-first
    // split row must preserve the actual three-party posture for this request:
    // OpenAgents, serving node aggregate, and referrer.
    servedByContributor: context.servingReceipt !== undefined,
    ...(input.weights === undefined ? {} : { weights: input.weights }),
  })

  // The ledger's `createReferralPayoutEligibility` applies the STANDING RL-1
  // referral policy (5% of the qualifying amount, capped) to produce the payout.
  // We therefore feed it the request's MARGIN (gross profit) in sats as the
  // qualifying amount, and let the ledger compute the referrer's 5% cut with its
  // own caps — reusing the existing policy rather than pre-cutting a second
  // percentage on top. The split module's `referrer` share is the SAME 5% of
  // margin (its default referrer weight is aligned to the ledger's 500 bps), so
  // the dashboard's projected referrer earnings match what the ledger accrues.
  //
  // Gate on the LEDGER's own policy calc (not the split's rounded share) so the
  // guard exactly matches what the ledger would record: if 5% of the margin sats
  // rounds below 1 sat, nothing accrues this request (NOT an error). Spend
  // accrues ongoing as per-request margin crosses the 1-sat payout threshold.
  if (
    split.marginSats <= 0 ||
    calculateReferralPayoutSats(split.marginSats) <= 0
  ) {
    return { _tag: 'zero_referrer_share' }
  }

  const revenueAsset = fundingKindToRevenueAsset(context.fundingKind)

  // RL-3 (#5460): enforce the SHARED credit<->Bitcoin asset boundary at accrual,
  // exactly as the site feed does. Bitcoin funding => Bitcoin-eligible revshare;
  // card/USD funding => credit revshare (never a Bitcoin liability). Fail closed.
  const boundaryViolation = validateAssetBoundary({
    contributorAsset: revshareContributorAssetFor(revenueAsset),
    movement: 'revshare',
    revenueAsset: referralRevenueAssetToBoundaryAsset(revenueAsset),
  })
  if (boundaryViolation !== null) {
    return { _tag: 'boundary_refused', reasonRef: boundaryViolation.reasonRef }
  }

  const nowIso = (input.nowIso ?? currentIsoTimestamp)()

  const createInput: CreateReferralPayoutEligibilityInput = {
    idempotencyKey: inferenceReferralIdempotencyKey(context.requestId),
    nowIso,
    payoutRef: inferenceReferralPayoutRef(context.requestId),
    periodKey: inferenceReferralPeriodKey(nowIso),
    qualifyingAmountSats: split.marginSats,
    qualifyingEventKind: INFERENCE_REFERRAL_QUALIFYING_EVENT_KIND,
    qualifyingEventRef: inferenceReferralQualifyingEventRef(context.requestId),
    referredUserId: party.userId,
    referralAttributionId: attribution.referral_attribution_id,
    referralInviteId: attribution.referral_invite_id,
    referralSourceId: attribution.referral_source_id,
    referrerUserId: attribution.referrer_user_id,
  }

  const entry = await createReferralPayoutEligibility(db, createInput)
  const marginSplitRef = await recordInferenceReferralMarginSplit(
    db,
    {
      attribution,
      context,
      nowIso,
      party,
      split,
    },
    mirror,
  )
  return { _tag: 'recorded', entry, marginSplitRef }
}

export type ReferralAccrualDeps = Readonly<{
  db: D1Database
  weights?: InferenceSplitWeights | undefined
  nowIso?: (() => string) | undefined
  // KS-8.9 (#8320): fire-safe Postgres dual-write mirror.
  mirror?: InferenceEntitlementsMirror | undefined
}>

/**
 * Wrap a live metering hook so that, AFTER the inner hook records a real,
 * non-zero charge, the referrer's ongoing cut accrues into the RL-1 ledger.
 *
 * The inner hook is the authority on whether a request was paid: accrual fires
 * only on `metered: true` AND not `zeroCharge`. The customer's inference call is
 * never failed by referral bookkeeping — an accrual error is logged (public-
 * safe: refs + the result tag, never amounts/prompts/payment material) and
 * swallowed. The metering outcome is returned unchanged.
 */
export const withReferralAccrual = (
  inner: MeteringHook,
  deps: ReferralAccrualDeps,
): MeteringHook => {
  return (context: MeteringContext) =>
    Effect.gen(function* () {
      const outcome = yield* inner(context)

      // Only a real, non-zero paid request accrues (INERT otherwise).
      if (!outcome.metered || outcome.zeroCharge === true) {
        return outcome
      }

      const result = yield* Effect.tryPromise({
        catch: inferenceReferralAccrualPersistenceError,
        try: () =>
          accrueInferenceReferral(
            deps.db,
            {
              context,
              ...(deps.nowIso === undefined ? {} : { nowIso: deps.nowIso }),
              ...(deps.weights === undefined
                ? {}
                : { weights: deps.weights }),
            },
            deps.mirror,
          ),
      }).pipe(
        Effect.catch(error =>
          Effect.gen(function* () {
            // Public-safe diagnostic only; never break the inference response.
            yield* Effect.logInfo(
              workerLogEntry('inference.referral.accrual.failed', {
                accountRef: context.accountRef,
                adapterId: context.adapterId,
                requestId: context.requestId,
                servedModel: context.servedModel,
                // error message is provider/SQL text, not payment material
                reason: error.message,
              }),
            )
            return { _tag: 'error' as const }
          }),
        ),
      )

      yield* Effect.logInfo(
        workerLogEntry('inference.referral.accrual', {
          accountRef: context.accountRef,
          adapterId: context.adapterId,
          outcome: result._tag,
          requestId: context.requestId,
          servedModel: context.servedModel,
        }),
      )

      return outcome satisfies MeteringOutcome
    })
}
