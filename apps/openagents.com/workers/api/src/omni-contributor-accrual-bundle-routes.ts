import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { publicOmniContributorAccrualBundleProjection } from './omni-contributor-accrual-bundle'
import { dereferenceOmniContributorAccrualBundle } from './omni-contributor-accrual-bundle-store'
import {
  liveAtReadStaleness,
  type PublicProjectionStalenessContract,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

export type OmniContributorAccrualBundleEnvelope = Readonly<{
  bundle: ReturnType<typeof publicOmniContributorAccrualBundleProjection>
  economicsId: string
  generatedAt: string
  staleness: PublicProjectionStalenessContract
}>

/**
 * Staleness contract (epic #4751). This surface is composed live from the
 * persisted economics record at request time — it stores no snapshot — so it can
 * never be older than the request (maxStalenessSeconds 0). The transitions that
 * change what it serves are the recording of an accepted-outcome economics row
 * and the recording of its contributor provenance.
 */
export const OmniContributorAccrualBundleStaleness = liveAtReadStaleness([
  'omni_accepted_outcome_economics_recorded',
  'omni_contributor_provenance_recorded',
])

/**
 * Public read route for the contributor accrual bundle, addressed by
 * accepted-outcome economics id (blocker.product_promises.contributor_ledger_missing).
 *
 * The pure pipeline and the persisted dereference seam already existed:
 * dereferenceOmniContributorAccrualBundle turns an accepted-outcome id into the
 * reconciled gross-margin receipt + contributor accrual ledger. What was missing
 * was the wire: nothing exposed that seam over HTTP, so a reviewer could not
 * dereference an outcome's accruals end to end. This is that wire.
 *
 * It is read-only and money-free: it writes nothing, moves nothing, and serves
 * the PUBLIC projection only -- lifecycle and evidence labels stay visible while
 * internal monetary figures are dropped. The promise's no-collapse discipline is
 * carried through unchanged: an accrual is still not a payable balance, and a
 * recorded gross margin is still not settlement evidence. Every contributor entry
 * the route returns keeps its payable/settlement states honestly
 * not_yet_evidenced while the source record disclaims settlement.
 */
export const handleOmniContributorAccrualBundleApi = (
  request: Request,
  db: D1Database,
): Effect.Effect<HttpResponse> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  const economicsId =
    new URL(request.url).searchParams.get('economicsId')?.trim() ?? ''

  if (economicsId === '') {
    return Effect.succeed(
      noStoreJsonResponse({ error: 'economics_id_required' }, { status: 400 }),
    )
  }

  return dereferenceOmniContributorAccrualBundle(db, economicsId).pipe(
    Effect.map(bundle =>
      bundle === null
        ? noStoreJsonResponse(
            { economicsId, error: 'accepted_outcome_not_found' },
            { status: 404 },
          )
        : noStoreJsonResponse({
            bundle: publicOmniContributorAccrualBundleProjection(bundle),
            economicsId,
            generatedAt: currentIsoTimestamp(),
            staleness: OmniContributorAccrualBundleStaleness,
          }),
    ),
    Effect.catchTags({
      // A record exists but cannot be attributed (e.g. it names no contributor
      // parties). Surface the absence of provenance honestly rather than papering
      // over it with a fabricated bundle.
      OmniContributorAccrualBundleDereferenceError: error =>
        Effect.succeed(
          noStoreJsonResponse(
            {
              economicsId,
              error: 'contributor_provenance_incomplete',
              reason: error.reason,
            },
            { status: 422 },
          ),
        ),
      OmniAcceptedOutcomeEconomicsStorageError: () =>
        Effect.succeed(
          noStoreJsonResponse({ error: 'storage_error' }, { status: 500 }),
        ),
    }),
  )
}
