// Lightning charge consume-once guard (EPIC #6049,
// draft-lightning-charge-00 §"Settlement Procedure" + §"Preimage
// Confidentiality"). Mirrors the SPT replay guard (`mpp-spt-replay.ts`).
//
// The Lightning spec requires ATOMIC consume-once: a challenge marked consumed
// MUST NOT be accepted again, and concurrent requests presenting the same valid
// preimage MUST result in exactly ONE success. We claim the paymentHash BEFORE
// serving: a second attempt collides on the PRIMARY KEY and is refused, so a
// preimage that has already paid for one served completion cannot be replayed
// for a second free completion.
//
// We key on the paymentHash (the public payment identifier), NOT the preimage
// (a bearer secret which must never be persisted — spec §"Preimage
// Confidentiality"). The challenge id is recorded alongside for audit binding.
//
// INERT NOTE: only the Lightning rail touches this table; it is never written
// unless the Lightning rail is armed and a real preimage verified.

import { Effect } from 'effect'

import { currentIsoTimestamp } from '../../runtime-primitives'
import {
  mirrorTreasuryRows,
  treasuryAuthorityDb,
  type TreasuryDatabase,
} from '../../treasury-domain-store'

export class MppLightningReplayError extends Error {
  override readonly name = 'MppLightningReplayError'
  override readonly cause: unknown
  constructor(cause: unknown) {
    super('mpp lightning replay store failure')
    this.cause = cause
  }
}

// Atomically claim a paymentHash for one challenge. Returns true when this is
// the FIRST use (the row was inserted) and false when the paymentHash was
// already consumed (replay rejected). `INSERT ... ON CONFLICT DO NOTHING` + a
// changes check makes the claim a single atomic D1 statement.
export const claimLightningPaymentHash = (
  database: TreasuryDatabase,
  input: Readonly<{ paymentHash: string; challengeId: string }>,
  nowIso: () => string = currentIsoTimestamp,
): Effect.Effect<boolean, MppLightningReplayError> =>
  Effect.tryPromise({
    catch: (cause: unknown) => new MppLightningReplayError(cause),
    try: async () => {
      const result = await treasuryAuthorityDb(database)
        .prepare(
          `INSERT INTO mpp_lightning_replay (payment_hash, challenge_id, consumed_at)
           VALUES (?, ?, ?)
           ON CONFLICT (payment_hash) DO NOTHING`,
        )
        .bind(input.paymentHash, input.challengeId, nowIso())
        .run()
      // D1 surfaces affected rows under meta.changes; 0 means the paymentHash
      // was already present (replay).
      const changes =
        (result as unknown as { meta?: { changes?: number } }).meta?.changes
      const claimed = changes === undefined ? true : changes > 0
      if (claimed) {
        // KS-8.8 (#8319): replay guards port KEY-EXACTLY — mirror the claim
        // fail-soft (D1 stays the enforcing store; diagnostics are redacted).
        await mirrorTreasuryRows(
          database,
          'mpp_lightning_replay',
          'payment_hash',
          [input.paymentHash],
        )
      }
      return claimed
    },
  })
