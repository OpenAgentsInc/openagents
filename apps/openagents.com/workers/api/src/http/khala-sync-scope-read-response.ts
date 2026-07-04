// Khala Sync scope-read decision → HTTP response mapping (KS-7.1, #8305).
//
// The resolver seam (`./khala-sync-scope-auth` in the parent directory)
// returns typed {@link ScopeReadDecision} values only; mapping a decision
// to a wire `SyncError` response is HTTP concern and lives here, shared by
// the three read routes (log / bootstrap / connect) — same placement rule
// as `routeAccessResponse` in ./route-access-response.
import { Schema as S } from 'effect'

import { SyncError } from '@openagentsinc/khala-sync'
import type { ScopeReadDecision } from '@openagentsinc/khala-sync-server'

import { noStoreJsonResponse } from './responses'

const encodeSyncError = S.encodeSync(SyncError)

/**
 * Map a non-allowed {@link ScopeReadDecision} to its typed `SyncError`
 * response; `undefined` when the read may proceed. Status map:
 * 403 `unauthorized_scope` (denied), 403 `unknown_scope` (taxonomy member
 * with no read policy — gated closed), 503 `storage_unavailable`
 * (capability failure — fail-closed, retryable).
 */
export const scopeReadDecisionResponse = (
  decision: ScopeReadDecision,
): Response | undefined => {
  if (decision.kind === 'allowed') return undefined
  if (decision.kind === 'unavailable') {
    return noStoreJsonResponse(
      encodeSyncError(
        new SyncError({
          code: 'storage_unavailable',
          messageSafe: decision.messageSafe,
          retryable: true,
        }),
      ),
      { status: 503 },
    )
  }
  return noStoreJsonResponse(
    encodeSyncError(
      new SyncError({
        code: decision.reason,
        messageSafe:
          decision.reason === 'unknown_scope'
            ? 'This scope kind has no read policy and is denied (fail-closed).'
            : 'This user cannot read the requested scope.',
        retryable: false,
      }),
    ),
    { status: 403 },
  )
}
