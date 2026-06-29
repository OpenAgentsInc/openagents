import { Effect, Schema as S } from 'effect'

import {
  type OmniAcceptedOutcomeEconomicsStorageError,
  readOmniAcceptedOutcomeEconomicsById,
} from './omni-accepted-outcome-economics'
import {
  buildOmniAcceptedOutcomeSettlementBundle,
  type OmniAcceptedOutcomeSettlementBundle,
} from './omni-accepted-outcome-settlement-bundle'

/**
 * Persisted dereference seam for the accepted-outcome settlement bundle.
 *
 * Mirrors omni-contributor-accrual-bundle-store.ts: given a D1 handle and an
 * accepted-outcome economics id, read the persisted (non-archived) record and
 * build the INERT settlement bundle (eight-state machine + reconciled accrual
 * bundle). Read-only: it moves no money and writes nothing. It always builds the
 * disarmed (intent-only) bundle, so dereferencing an outcome can never imply a
 * settlement that did not happen.
 */

/**
 * Raised when a record EXISTS for the id but cannot be composed into a settlement
 * bundle -- e.g. it does not yet name its contributor parties, or a
 * reconciliation invariant rejects it. Distinct from a storage fault and from "no
 * such outcome" (which yields null).
 */
export class OmniAcceptedOutcomeSettlementBundleDereferenceError extends S.TaggedErrorClass<OmniAcceptedOutcomeSettlementBundleDereferenceError>()(
  'OmniAcceptedOutcomeSettlementBundleDereferenceError',
  {
    economicsId: S.String,
    reason: S.String,
  },
) {}

/**
 * Dereference one accepted outcome's INERT settlement bundle by economics id.
 *
 * - Returns the bundle when a non-archived record exists and carries the
 *   contributor provenance the composition requires.
 * - Returns null when no such (non-archived) outcome exists.
 * - Fails with OmniAcceptedOutcomeSettlementBundleDereferenceError when a record
 *   exists but cannot be composed (the builders throw synchronously; this wraps
 *   that into the typed error channel).
 */
export const dereferenceOmniAcceptedOutcomeSettlementBundle = (
  db: D1Database,
  economicsId: string,
): Effect.Effect<
  OmniAcceptedOutcomeSettlementBundle | null,
  | OmniAcceptedOutcomeEconomicsStorageError
  | OmniAcceptedOutcomeSettlementBundleDereferenceError
> =>
  Effect.gen(function* () {
    const record = yield* readOmniAcceptedOutcomeEconomicsById(db, economicsId)

    if (record === null) {
      return null
    }

    return yield* Effect.try({
      catch: error =>
        new OmniAcceptedOutcomeSettlementBundleDereferenceError({
          economicsId,
          reason: error instanceof Error ? error.message : String(error),
        }),
      // Always disarmed: a dereference is a read, never an arming/dispatch path.
      try: () => buildOmniAcceptedOutcomeSettlementBundle(record),
    })
  })
