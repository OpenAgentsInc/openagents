/**
 * Platform-wide public-projection staleness contract (epic #4751).
 *
 * The defect class this encodes produced eight confirmed instances in
 * ~24 hours (#4744, #4745, #4746, #4747, #4735, #4752, #4753, #4754):
 * a state-changing write commits to a source-of-truth table while a
 * public read surface either never rebuilds or never existed, so the
 * platform briefly asserts the opposite of its own ledger on exactly
 * the surfaces it tells agents to trust.
 *
 * The adopted invariant: every public projection carries `generatedAt`
 * (or a numeric `generatedAtUnixMs` where raw ISO timestamps are banned
 * from the payload) plus a declared staleness contract, and either
 * rebuilds on the state transitions that matter or composes live at
 * read. A projection that cannot meet its own declared staleness must
 * say so in the payload rather than serve stale data as current.
 *
 * This module is the single shared vocabulary for that declaration. It
 * deliberately reuses the shape frozen by the Tassadar trace factory's
 * day-0 `projection_rebuild.v0.1` contract (#4748,
 * `tassadar-trace-factory/projection-rebuild.ts`) and first applied
 * platform-wide by the x_claim_reward eligibility read path (#4754)
 * rather than inventing a second vocabulary.
 *
 * The zero-debt architecture check
 * (`scripts/check-zero-debt-architecture.mjs`) greps public projection
 * modules for `maxStalenessSeconds`; declaring the contract through
 * this module is what keeps a projection module compliant.
 */
import { Schema as S } from 'effect'

export const PUBLIC_PROJECTION_STALENESS_CONTRACT_VERSION =
  'projection_staleness.v1'

/**
 * How a public projection relates to its source of truth:
 * - `live_at_read`: composed from source tables at request time; the
 *   payload can never be older than the request.
 * - `rebuilt_on_transition`: a stored projection refreshed by
 *   event-driven invalidation at the write site, on the transitions
 *   named in `rebuildsOn`.
 * - `stored_snapshot`: an explicitly recorded snapshot series; each
 *   record carries its own capture time and the declared max staleness
 *   bounds how old the newest record may be.
 */
export const PublicProjectionComposition = S.Literals([
  'live_at_read',
  'rebuilt_on_transition',
  'stored_snapshot',
])
export type PublicProjectionComposition =
  typeof PublicProjectionComposition.Type

export const PublicProjectionStalenessContract = S.Struct({
  composition: PublicProjectionComposition,
  contractVersion: S.Literal(PUBLIC_PROJECTION_STALENESS_CONTRACT_VERSION),
  /**
   * The oldest the served data may be, in seconds, before the
   * projection must flag itself stale in the payload. `0` means
   * "as fresh as the request" and is only honest for `live_at_read`.
   */
  maxStalenessSeconds: S.Number,
  /** The state transitions that matter — the write-site invalidation set. */
  rebuildsOn: S.Array(S.String),
})
export type PublicProjectionStalenessContract =
  typeof PublicProjectionStalenessContract.Type

export const liveAtReadStaleness = (
  rebuildsOn: ReadonlyArray<string>,
): PublicProjectionStalenessContract => ({
  composition: 'live_at_read',
  contractVersion: PUBLIC_PROJECTION_STALENESS_CONTRACT_VERSION,
  maxStalenessSeconds: 0,
  rebuildsOn,
})

export const rebuiltOnTransitionStaleness = (
  maxStalenessSeconds: number,
  rebuildsOn: ReadonlyArray<string>,
): PublicProjectionStalenessContract => ({
  composition: 'rebuilt_on_transition',
  contractVersion: PUBLIC_PROJECTION_STALENESS_CONTRACT_VERSION,
  maxStalenessSeconds,
  rebuildsOn,
})

export const storedSnapshotStaleness = (
  maxStalenessSeconds: number,
  rebuildsOn: ReadonlyArray<string>,
): PublicProjectionStalenessContract => ({
  composition: 'stored_snapshot',
  contractVersion: PUBLIC_PROJECTION_STALENESS_CONTRACT_VERSION,
  maxStalenessSeconds,
  rebuildsOn,
})

/**
 * Whether served data older than the declared bound must be flagged.
 * `live_at_read` projections (bound 0) are never stale by construction;
 * for the other compositions a positive age beyond the bound exceeds
 * the contract.
 */
export const projectionStalenessExceeded = (
  contract: PublicProjectionStalenessContract,
  dataAgeSeconds: number | null,
): boolean =>
  contract.maxStalenessSeconds > 0 &&
  dataAgeSeconds !== null &&
  dataAgeSeconds > contract.maxStalenessSeconds

/** Whole seconds between a data timestamp and now; null when unknown. */
export const projectionDataAgeSeconds = (
  dataUpdatedAtIso: string | null,
  nowIso: string,
): number | null => {
  if (dataUpdatedAtIso === null) {
    return null
  }

  const updatedAtMs = Date.parse(dataUpdatedAtIso)
  const nowMs = Date.parse(nowIso)

  return Number.isNaN(updatedAtMs) || Number.isNaN(nowMs)
    ? null
    : Math.max(0, Math.floor((nowMs - updatedAtMs) / 1000))
}
