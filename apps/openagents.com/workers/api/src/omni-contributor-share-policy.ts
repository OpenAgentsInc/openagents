import { Schema as S } from 'effect'

import {
  OmniContributorAccrualRole,
  type OmniContributorAccrualShare,
} from './omni-contributor-accrual-ledger'

/**
 * Contributor share policy for a single accepted outcome.
 *
 * The contributor accrual ledger (omni-contributor-accrual-ledger.ts) attributes
 * an outcome's derived gross margin to contributors by basis-points share, but it
 * requires the caller to hand the split in already. The registry blocker
 * blocker.product_promises.contributor_ledger_missing calls out the missing
 * upstream step: deciding WHO the contributors are and at WHAT split, rather than
 * inventing a split per call site.
 *
 * This module is that step. Given the identified parties for one outcome (the
 * runner is always present; reviewer, originator, and referrer are optional; the
 * platform always retains a share), it produces a canonical, deterministic share
 * set that sums to EXACTLY 10000 basis points and plugs straight into
 * buildOmniContributorAccrualLedger.
 *
 * It is a SPLIT policy only: it never reads or implies money movement. Funding
 * mode and gross-margin sign are deliberately not consulted here -- a loss or a
 * free_beta outcome still has a canonical split; the ledger builder is what turns
 * a non-positive margin into zero accruals. Keeping the two concerns separate
 * preserves the promise's no-collapse discipline.
 */

export class OmniContributorSharePolicyError extends S.TaggedErrorClass<OmniContributorSharePolicyError>()(
  'OmniContributorSharePolicyError',
  { reason: S.String },
) {}

const TOTAL_BASIS_POINTS = 10_000

const CONTRIBUTOR_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,128}$/

const DEFAULT_PLATFORM_ID = 'platform'

/**
 * Canonical relative weights per role. Only roles with an identified party (and
 * the always-present platform) participate; the weights of the participating
 * roles are normalized to exactly 10000 basis points, so adding or omitting an
 * optional party reshuffles the split deterministically rather than leaking or
 * inventing basis points.
 */
const ROLE_WEIGHT: Readonly<Record<OmniContributorAccrualRole, number>> = {
  runner: 60,
  reviewer: 10,
  originator: 10,
  referrer: 5,
  platform: 15,
}

// Canonical emit order; the ledger's largest-remainder distribution is stable by
// input order, so a fixed order keeps the whole pipeline deterministic.
const ROLE_ORDER: ReadonlyArray<OmniContributorAccrualRole> = [
  'runner',
  'reviewer',
  'originator',
  'referrer',
  'platform',
]

export type OmniContributorSharePolicyInput = Readonly<{
  // The agent/operator that executed the work. Always present.
  runnerId: string
  // The human or agent that reviewed/accepted the outcome, if any.
  reviewerId?: string | undefined
  // The party that supplied the source data, prompt, or brief, if any.
  originatorId?: string | undefined
  // The party that referred the buyer or a contributor, if any.
  referrerId?: string | undefined
  // The platform's retained-share id. Defaults to 'platform'.
  platformId?: string | undefined
}>

type RoleParty = Readonly<{
  contributorId: string
  role: OmniContributorAccrualRole
}>

const assertSafeId = (
  role: OmniContributorAccrualRole,
  contributorId: string,
): void => {
  if (!CONTRIBUTOR_ID_PATTERN.test(contributorId)) {
    throw new OmniContributorSharePolicyError({
      reason: `${role} id ${JSON.stringify(contributorId)} must be a safe ref.`,
    })
  }
}

/**
 * Normalize integer relative weights to basis points summing EXACTLY to 10000,
 * using the largest-remainder method with a stable tie-break by input order.
 */
const normalizeToBasisPoints = (
  weights: ReadonlyArray<number>,
): ReadonlyArray<number> => {
  const totalWeight = weights.reduce((sum, weight) => sum + weight, 0)
  if (totalWeight <= 0) {
    throw new OmniContributorSharePolicyError({
      reason: 'participating role weights must sum to a positive value.',
    })
  }

  const exact = weights.map(weight => (weight * TOTAL_BASIS_POINTS) / totalWeight)
  const floors = exact.map(value => Math.floor(value))
  const assigned = floors.reduce((sum, value) => sum + value, 0)
  let remainder = TOTAL_BASIS_POINTS - assigned

  const order = weights
    .map((_, index) => index)
    .sort((a, b) => {
      const fractionalA = exact[a]! - floors[a]!
      const fractionalB = exact[b]! - floors[b]!
      if (fractionalB !== fractionalA) {
        return fractionalB - fractionalA
      }
      return a - b
    })

  const result = [...floors]
  let cursor = 0
  while (remainder > 0 && cursor < order.length) {
    const index = order[cursor]!
    result[index] = result[index]! + 1
    remainder -= 1
    cursor += 1
  }

  return result
}

/**
 * Resolve a canonical contributor share set from the identified parties for one
 * accepted outcome.
 *
 * Deterministic and pure: the same parties always yield the same split.
 *
 * Honesty rules enforced by construction:
 * - The runner and platform always participate; reviewer/originator/referrer
 *   participate only when an id is supplied.
 * - Every supplied id must be a safe ref and unique across roles.
 * - The returned shares sum to exactly 10000 basis points, so feeding them to
 *   buildOmniContributorAccrualLedger never trips its share-sum invariant.
 */
export const resolveOmniContributorShares = (
  input: OmniContributorSharePolicyInput,
): ReadonlyArray<OmniContributorAccrualShare> => {
  const platformId = input.platformId ?? DEFAULT_PLATFORM_ID

  const candidates: ReadonlyArray<RoleParty | null> = [
    { contributorId: input.runnerId, role: 'runner' },
    input.reviewerId === undefined
      ? null
      : { contributorId: input.reviewerId, role: 'reviewer' },
    input.originatorId === undefined
      ? null
      : { contributorId: input.originatorId, role: 'originator' },
    input.referrerId === undefined
      ? null
      : { contributorId: input.referrerId, role: 'referrer' },
    { contributorId: platformId, role: 'platform' },
  ]

  const parties = candidates.filter(
    (candidate): candidate is RoleParty => candidate !== null,
  )

  const seen = new Set<string>()
  for (const party of parties) {
    assertSafeId(party.role, party.contributorId)
    if (seen.has(party.contributorId)) {
      throw new OmniContributorSharePolicyError({
        reason: `contributor id ${party.contributorId} is used for more than one role.`,
      })
    }
    seen.add(party.contributorId)
  }

  // Emit in canonical role order so the downstream distribution is stable.
  const ordered = ROLE_ORDER.flatMap(role =>
    parties.filter(party => party.role === role),
  )

  const basisPoints = normalizeToBasisPoints(
    ordered.map(party => ROLE_WEIGHT[party.role]),
  )

  return ordered.map((party, index) => ({
    contributorId: party.contributorId,
    role: party.role,
    shareBasisPoints: basisPoints[index]!,
  }))
}
