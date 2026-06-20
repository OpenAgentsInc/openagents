import { Effect, Schema as S } from 'effect'

import {
  type OmniAcceptedOutcomeEconomicsStorageError,
  readOmniAcceptedOutcomeEconomicsById,
} from './omni-accepted-outcome-economics'
import type { OmniContributorAccrualBundle } from './omni-contributor-accrual-bundle'
import { buildOmniContributorAccrualBundleFromRecord } from './omni-contributor-party-sourcing'

/**
 * Persisted dereference seam for the contributor accrual bundle.
 *
 * The blocker blocker.product_promises.contributor_ledger_missing calls for "a
 * persisted/queryable bundle record + read path to dereference accruals by
 * accepted-outcome id". The pure pipeline already exists end to end:
 * buildOmniContributorAccrualBundleFromRecord turns ONE stored economics record
 * into the reconciled gross-margin receipt + contributor accrual ledger, sourcing
 * the parties from the record itself. What was missing was the step that goes from
 * an accepted-outcome ID to that record: every prior caller had to already hold
 * the record in hand.
 *
 * This module is that step. Given a D1 handle and an accepted-outcome economics
 * id, it reads the persisted record (read-only, via
 * readOmniAcceptedOutcomeEconomicsById) and builds the reconciled bundle. It moves
 * no money and writes nothing; it is a query path only. The promise's no-collapse
 * discipline is preserved by the underlying builders: an accrual is still not a
 * payable balance, and a recorded gross margin is still not settlement evidence.
 */

/**
 * Raised when a record EXISTS for the id but cannot be attributed into a bundle --
 * e.g. it does not yet name its contributor parties, or a share/reconciliation
 * invariant rejects it. This is distinct from a storage fault and from "no such
 * outcome" (which yields null): it means the stored outcome's provenance is
 * incomplete, and that absence is surfaced honestly rather than papered over.
 */
export class OmniContributorAccrualBundleDereferenceError extends S.TaggedErrorClass<OmniContributorAccrualBundleDereferenceError>()(
  'OmniContributorAccrualBundleDereferenceError',
  {
    economicsId: S.String,
    reason: S.String,
  },
) {}

/**
 * Dereference one accepted outcome's contributor accrual bundle by economics id.
 *
 * - Returns the reconciled bundle when a non-archived record exists and carries
 *   the contributor provenance the attribution requires.
 * - Returns null when no such (non-archived) outcome exists, so callers can
 *   distinguish "unknown outcome" from a storage fault.
 * - Fails with OmniContributorAccrualBundleDereferenceError when a record exists
 *   but cannot be attributed (the bundle builders throw synchronously; this wraps
 *   that into the typed error channel).
 */
export const dereferenceOmniContributorAccrualBundle = (
  db: D1Database,
  economicsId: string,
): Effect.Effect<
  OmniContributorAccrualBundle | null,
  | OmniAcceptedOutcomeEconomicsStorageError
  | OmniContributorAccrualBundleDereferenceError
> =>
  Effect.gen(function* () {
    const record = yield* readOmniAcceptedOutcomeEconomicsById(db, economicsId)

    if (record === null) {
      return null
    }

    return yield* Effect.try({
      catch: error =>
        new OmniContributorAccrualBundleDereferenceError({
          economicsId,
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => buildOmniContributorAccrualBundleFromRecord(record),
    })
  })
