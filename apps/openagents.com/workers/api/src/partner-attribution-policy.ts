/**
 * Partner-attribution policy (#4986 / #5524 follow-up).
 *
 * Advances `blocker.product_promises.partner_attribution_policy_missing`.
 *
 * The partner payout *ledger* (`partner-payout-ledger.ts`) already knows how to
 * compute a payout amount and walk a payout through its lifecycle once an
 * eligibility row is created. What was missing is the decision that sits BEFORE
 * eligibility: given a real paid customer event, WHICH partner (if any) should
 * be credited, under WHICH role, and is that attribution allowed at all.
 *
 * This is deliberately DISTINCT from the referral feed
 * (`site-referral-payout-feed.ts`). The referral rail *infers* its earner from
 * last-touch click attribution (`user_referral_attributions` ->
 * `site_referral_sources.referrer_user_id`). Inference is appropriate for a
 * low-percentage, self-serve referral reward, but it is NOT an acceptable basis
 * for the larger design-partner / affiliate payouts this lane carries. So the
 * partner-attribution policy here requires an EXPLICIT, currently-active partner
 * agreement that covers the customer at the time of the qualifying event — never
 * an inferred click.
 *
 * The policy is pure: it takes the qualifying event plus the candidate partner
 * agreements that the (deferred) feed will read from storage, and returns a
 * single attribution DECISION. It never reads a database and never moves money.
 * The feed that wires this to `createPartnerPayoutEligibility` is a documented
 * remaining step (see the promise note); keeping the decision pure makes the
 * product rules — which still need owner sign-off — independently testable.
 *
 * PRODUCT DECISIONS ENCODED HERE (conservative defaults, OWNER SIGN-OFF PENDING)
 * 1. Explicit-agreement-only: no partner is attributed without an active,
 *    customer-covering partner agreement. No last-touch inference.
 * 2. Referral exclusion: the `referral` role is owned by the referral rail and
 *    is refused here, so the same revenue is never double-paid across rails.
 * 3. Role precedence: if multiple active agreements match, the highest-precedence
 *    role wins (design_partner > affiliate). Exactly one partner is credited.
 * 4. Active window: the agreement must have started at/before the event and not
 *    have ended/expired at the event time.
 * 5. Self-payout exclusion: the partner cannot be the paying customer.
 *
 * The thresholds/percentages themselves live in `PARTNER_PAYOUT_ROLE_POLICY`
 * and remain owner-gated; this module only decides attribution, not amount.
 */
import { type PartnerPayoutRole } from './partner-payout-ledger'

export const PARTNER_ATTRIBUTION_POLICY_REF = 'policy.partner_attribution.v1'

/**
 * Roles this policy is allowed to attribute. `referral` is intentionally absent:
 * it is owned by the referral feed and excluded to prevent cross-rail double-pay.
 */
export const PARTNER_ATTRIBUTION_ELIGIBLE_ROLES: ReadonlyArray<PartnerPayoutRole> =
  ['design_partner', 'affiliate']

/**
 * Role precedence (highest first). When more than one active agreement covers a
 * customer, exactly one partner is credited: the highest-precedence role, and
 * within a role the agreement that started earliest (first-committed wins, a
 * deterministic tie-break that does not reward last-minute agreement churn).
 */
const ROLE_PRECEDENCE: ReadonlyArray<PartnerPayoutRole> = [
  'design_partner',
  'affiliate',
]

const rolePrecedenceIndex = (role: PartnerPayoutRole): number => {
  const index = ROLE_PRECEDENCE.indexOf(role)

  return index === -1 ? Number.MAX_SAFE_INTEGER : index
}

/**
 * A candidate partner agreement, as the deferred feed would read it from
 * storage. Refs are public-safe identifiers; this module does not validate the
 * ref shape (the ledger does that on eligibility insert) — it decides only
 * attribution.
 */
export type PartnerAgreement = Readonly<{
  /** Public-safe agreement ref recorded as the attribution's policy linkage. */
  agreementRef: string
  /** ISO timestamp the agreement became effective. */
  effectiveFromIso: string
  /**
   * ISO timestamp the agreement stops covering events, or null if open-ended.
   * The event is covered when `effectiveFromIso <= eventIso < effectiveUntilIso`.
   */
  effectiveUntilIso: string | null
  /** Public-safe partner ref (org/account), distinct from the user id. */
  partnerRef: string
  /** The partner's own user id, used for self-payout exclusion. */
  partnerUserId: string
  role: PartnerPayoutRole
}>

export type PartnerAttributionEvent = Readonly<{
  /** The paying customer's user id. */
  customerUserId: string
  /** ISO timestamp of the qualifying paid event. */
  eventIso: string
}>

export type PartnerAttributionDecision =
  | Readonly<{ _tag: 'no_active_agreement' }>
  | Readonly<{ _tag: 'self_attribution'; partnerRef: string }>
  | Readonly<{
      _tag: 'attributed'
      agreementRef: string
      partnerRef: string
      partnerRole: PartnerPayoutRole
      partnerUserId: string
      policyRef: string
    }>

const isRoleAttributable = (role: PartnerPayoutRole): boolean =>
  PARTNER_ATTRIBUTION_ELIGIBLE_ROLES.includes(role)

const isWithinWindow = (
  agreement: PartnerAgreement,
  eventIso: string,
): boolean => {
  const eventMillis = Date.parse(eventIso)
  const fromMillis = Date.parse(agreement.effectiveFromIso)

  if (!Number.isFinite(eventMillis) || !Number.isFinite(fromMillis)) {
    return false
  }

  if (eventMillis < fromMillis) {
    return false
  }

  if (agreement.effectiveUntilIso === null) {
    return true
  }

  const untilMillis = Date.parse(agreement.effectiveUntilIso)

  return Number.isFinite(untilMillis) && eventMillis < untilMillis
}

/**
 * Order two active, attributable agreements deterministically: higher role
 * precedence first, then earliest effective date, then agreement ref as a final
 * stable tie-break.
 */
const compareAgreements = (a: PartnerAgreement, b: PartnerAgreement): number => {
  const byRole = rolePrecedenceIndex(a.role) - rolePrecedenceIndex(b.role)

  if (byRole !== 0) {
    return byRole
  }

  const byEffective =
    Date.parse(a.effectiveFromIso) - Date.parse(b.effectiveFromIso)

  if (byEffective !== 0) {
    return byEffective
  }

  return a.agreementRef < b.agreementRef
    ? -1
    : a.agreementRef > b.agreementRef
      ? 1
      : 0
}

/**
 * Result of validating an agreement that is about to be SEEDED into storage.
 */
export type PartnerAgreementSeedDecision =
  | Readonly<{ _tag: 'seedable' }>
  | Readonly<{ _tag: 'rejected'; reason: string }>

/**
 * The policy-bearing fields of an agreement about to be persisted. Ref/id SHAPE
 * (public-safety) is intentionally NOT checked here — that is the storage
 * writer's job, mirroring the ledger which validates refs on eligibility insert.
 * This validator owns the *attribution* invariants only.
 */
export type PartnerAgreementSeed = Readonly<{
  customerUserId: string
  effectiveFromIso: string
  effectiveUntilIso: string | null
  partnerUserId: string
  role: PartnerPayoutRole
}>

/**
 * Enforce the attribution policy's invariants at the WRITE boundary, so a
 * policy-violating agreement can never land in `partner_agreements` (and so can
 * never be read back by the feed and credited). Pure; throws nothing.
 *
 * This is the same rule set `decidePartnerAttribution` applies at read time, but
 * applied BEFORE storage so the violation is rejected once, at the source,
 * rather than silently filtered on every read:
 *  - `referral` (and any non-attributable role) is refused — the referral rail
 *    owns referral payouts; storing one here would risk cross-rail double-pay.
 *  - a partner may not hold an agreement that credits them on their own
 *    purchases (self-attribution exclusion, defense-in-depth for the ledger's
 *    own self-payout guard).
 *  - the effective window must be internally consistent: a parseable start, and
 *    an end that is either open-ended or strictly after the start.
 */
export const assessPartnerAgreementSeed = (
  seed: PartnerAgreementSeed,
): PartnerAgreementSeedDecision => {
  if (!isRoleAttributable(seed.role)) {
    return {
      _tag: 'rejected',
      reason: `role ${seed.role} is not attributable; the referral rail owns referral payouts.`,
    }
  }

  if (seed.partnerUserId === seed.customerUserId) {
    return {
      _tag: 'rejected',
      reason: 'partnerUserId must differ from customerUserId (no self-agreement).',
    }
  }

  const fromMillis = Date.parse(seed.effectiveFromIso)

  if (!Number.isFinite(fromMillis)) {
    return {
      _tag: 'rejected',
      reason: 'effectiveFromIso must be a valid ISO timestamp.',
    }
  }

  if (seed.effectiveUntilIso !== null) {
    const untilMillis = Date.parse(seed.effectiveUntilIso)

    if (!Number.isFinite(untilMillis)) {
      return {
        _tag: 'rejected',
        reason: 'effectiveUntilIso must be a valid ISO timestamp or null.',
      }
    }

    if (untilMillis <= fromMillis) {
      return {
        _tag: 'rejected',
        reason: 'effectiveUntilIso must be strictly after effectiveFromIso.',
      }
    }
  }

  return { _tag: 'seedable' }
}

/**
 * Decide which single partner, if any, a qualifying paid event is attributed to.
 *
 * Pure and side-effect free. The caller (the deferred eligibility feed) maps an
 * `attributed` decision onto `createPartnerPayoutEligibility`, carrying the
 * agreement ref and this policy ref as `policyRefs` so the ledger row records
 * the explicit basis for the payout. A non-`attributed` decision records NO
 * eligibility — there is no inferred fallback.
 */
export const decidePartnerAttribution = (
  event: PartnerAttributionEvent,
  candidateAgreements: ReadonlyArray<PartnerAgreement>,
): PartnerAttributionDecision => {
  const active = candidateAgreements.filter(
    agreement =>
      isRoleAttributable(agreement.role) &&
      isWithinWindow(agreement, event.eventIso),
  )

  const [winner] = [...active].sort(compareAgreements)

  if (winner === undefined) {
    return { _tag: 'no_active_agreement' }
  }

  if (winner.partnerUserId === event.customerUserId) {
    return { _tag: 'self_attribution', partnerRef: winner.partnerRef }
  }

  return {
    _tag: 'attributed',
    agreementRef: winner.agreementRef,
    partnerRef: winner.partnerRef,
    partnerRole: winner.role,
    partnerUserId: winner.partnerUserId,
    policyRef: PARTNER_ATTRIBUTION_POLICY_REF,
  }
}
