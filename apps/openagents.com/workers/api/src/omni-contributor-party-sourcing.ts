import { Schema as S } from 'effect'

import type { OmniAcceptedOutcomeEconomicsRecord } from './omni-accepted-outcome-economics'
import {
  buildOmniContributorAccrualBundle,
  type OmniContributorAccrualBundle,
} from './omni-contributor-accrual-bundle'
import type { OmniContributorSharePolicyInput } from './omni-contributor-share-policy'

/**
 * Contributor party sourcing for a single accepted outcome.
 *
 * The share policy (omni-contributor-share-policy.ts) decides WHAT split each
 * contributor gets, but it still requires the caller to hand in WHO the parties
 * are (runnerId, and optional reviewer/originator/referrer/platform ids). Every
 * call site so far has had to invent those ids, which is exactly the gap the
 * registry blocker blocker.product_promises.contributor_ledger_missing names as
 * "real per-outcome party sourcing": a persisted accepted-outcome record must be
 * able to say, on its own, which identities produced it.
 *
 * This module is that step. It reads the parties from a CANONICAL location on a
 * stored economics record -- `metadata.contributors` -- rather than from an
 * ad-hoc per-call-site convention, validates them, and returns the exact
 * OmniContributorSharePolicyInput the share policy consumes. It performs no money
 * movement and invents no identities: if a record does not yet name its runner,
 * sourcing FAILS honestly instead of fabricating a contributor, so the absence of
 * party provenance can never be silently papered over.
 */

export class OmniContributorPartySourcingError extends S.TaggedErrorClass<OmniContributorPartySourcingError>()(
  'OmniContributorPartySourcingError',
  { reason: S.String },
) {}

/**
 * Canonical key, under an economics record's `metadata`, of the object that names
 * the outcome's contributor parties.
 */
export const OMNI_CONTRIBUTOR_PARTIES_METADATA_KEY = 'contributors'

type ContributorPartyField =
  | 'runnerId'
  | 'reviewerId'
  | 'originatorId'
  | 'referrerId'
  | 'platformId'

type OptionalContributorPartyField = Exclude<ContributorPartyField, 'runnerId'>

const OPTIONAL_PARTY_FIELDS: ReadonlyArray<OptionalContributorPartyField> = [
  'reviewerId',
  'originatorId',
  'referrerId',
  'platformId',
]

const isPlainObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const readStringField = (
  source: Record<string, unknown>,
  field: ContributorPartyField,
): string | undefined => {
  const value = source[field]
  if (value === undefined) {
    return undefined
  }
  if (typeof value !== 'string' || value.length === 0) {
    throw new OmniContributorPartySourcingError({
      reason: `metadata.${OMNI_CONTRIBUTOR_PARTIES_METADATA_KEY}.${field} must be a non-empty string when present.`,
    })
  }
  return value
}

/**
 * Resolve the contributor parties for one accepted outcome from its stored
 * economics record.
 *
 * Deterministic and pure: the same record always yields the same parties.
 *
 * Honesty rules enforced by construction:
 * - The record must carry a `metadata.contributors` OBJECT; a missing or
 *   non-object value fails rather than defaulting to an invented party set.
 * - The runner must be named; an outcome with no identified runner cannot be
 *   attributed, so sourcing fails instead of fabricating one.
 * - Every present id must be a non-empty string. Safe-ref shape and
 *   cross-role-uniqueness are enforced downstream by the share policy, so a
 *   sourced input plugs straight into resolveOmniContributorShares.
 */
export const resolveOmniContributorPartiesFromRecord = (
  record: OmniAcceptedOutcomeEconomicsRecord,
): OmniContributorSharePolicyInput => {
  const raw = record.metadata[OMNI_CONTRIBUTOR_PARTIES_METADATA_KEY]
  if (raw === undefined) {
    throw new OmniContributorPartySourcingError({
      reason: `economics record ${record.id} has no metadata.${OMNI_CONTRIBUTOR_PARTIES_METADATA_KEY}; contributor parties cannot be sourced.`,
    })
  }
  if (!isPlainObject(raw)) {
    throw new OmniContributorPartySourcingError({
      reason: `economics record ${record.id} metadata.${OMNI_CONTRIBUTOR_PARTIES_METADATA_KEY} must be an object.`,
    })
  }

  const runnerId = readStringField(raw, 'runnerId')
  if (runnerId === undefined) {
    throw new OmniContributorPartySourcingError({
      reason: `economics record ${record.id} metadata.${OMNI_CONTRIBUTOR_PARTIES_METADATA_KEY}.runnerId is required; an outcome with no identified runner cannot be attributed.`,
    })
  }

  const optional: { -readonly [K in OptionalContributorPartyField]?: string } =
    {}
  for (const field of OPTIONAL_PARTY_FIELDS) {
    const value = readStringField(raw, field)
    if (value !== undefined) {
      optional[field] = value
    }
  }

  return { runnerId, ...optional }
}

/**
 * Build a contributor accrual bundle directly from one persisted economics
 * record, sourcing the parties from the record itself.
 *
 * This is the single dereference point the blocker calls for: given a stored
 * accepted-outcome economics record, produce the reconciled receipt + accrual
 * bundle without any call site re-stating who the contributors are. Determinism,
 * the share-sum invariant, and the no-collapse / settlement-disclaimed
 * discipline are all preserved by the underlying builders.
 */
export const buildOmniContributorAccrualBundleFromRecord = (
  record: OmniAcceptedOutcomeEconomicsRecord,
): OmniContributorAccrualBundle =>
  buildOmniContributorAccrualBundle(
    record,
    resolveOmniContributorPartiesFromRecord(record),
  )
