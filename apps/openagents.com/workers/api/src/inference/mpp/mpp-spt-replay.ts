// MPP Stripe SPT single-use replay guard (EPIC #6049, defect B).
//
// The Payment Auth core spec requires single-use payment proofs (§"Replay
// Protection"), and the Stripe charge intent makes the SERVER reject a reused
// SPT (draft-stripe-charge-00 §"Verification Procedure" step 4). This is the
// local defense-in-depth replay cache the spec says servers SHOULD/MAY keep, on
// top of Stripe's own SPT single-use enforcement and the PaymentIntent
// idempotency key. We claim the SPT BEFORE creating the charge: a second attempt
// collides on the PRIMARY KEY and is refused before any second charge.
//
// INERT NOTE: only the card/SPT rail touches this; the crypto rail never does.

import { Effect } from 'effect'

import { currentIsoTimestamp } from '../../runtime-primitives'
import {
  mirrorTreasuryRows,
  treasuryAuthorityDb,
  type TreasuryDatabase,
} from '../../treasury-domain-store'

export class MppSptReplayError extends Error {
  override readonly name = 'MppSptReplayError'
  override readonly cause: unknown
  constructor(cause: unknown) {
    super('mpp spt replay store failure')
    this.cause = cause
  }
}

// Atomically claim an SPT for one challenge. Returns true when this is the FIRST
// use (the row was inserted) and false when the SPT was already consumed (replay
// rejected). Uses `INSERT ... ON CONFLICT DO NOTHING` + a changes check so the
// claim is a single atomic D1 statement.
export const claimSpt = (
  database: TreasuryDatabase,
  input: Readonly<{ spt: string; challengeId: string }>,
  nowIso: () => string = currentIsoTimestamp,
): Effect.Effect<boolean, MppSptReplayError> =>
  Effect.tryPromise({
    catch: (cause: unknown) => new MppSptReplayError(cause),
    try: async () => {
      const result = await treasuryAuthorityDb(database)
        .prepare(
          `INSERT INTO mpp_spt_replay (spt, challenge_id, payment_intent_id, consumed_at)
           VALUES (?, ?, NULL, ?)
           ON CONFLICT (spt) DO NOTHING`,
        )
        .bind(input.spt, input.challengeId, nowIso())
        .run()
      // D1 surfaces affected rows under meta.changes; a 0 means the SPT was
      // already present (replay).
      const changes =
        (result as unknown as { meta?: { changes?: number } }).meta?.changes
      const claimed = changes === undefined ? true : changes > 0
      if (claimed) {
        // KS-8.8 (#8319): replay guards port KEY-EXACTLY — mirror the claim
        // fail-soft (D1 stays the enforcing store; diagnostics are redacted).
        await mirrorTreasuryRows(database, 'mpp_spt_replay', 'spt', [
          input.spt,
        ])
      }
      return claimed
    },
  })

// Record the resulting PaymentIntent id against a consumed SPT (best-effort
// dereference; not load-bearing for the replay guard itself).
export const recordSptPaymentIntent = (
  database: TreasuryDatabase,
  input: Readonly<{ spt: string; paymentIntentId: string }>,
): Effect.Effect<void, MppSptReplayError> =>
  Effect.tryPromise({
    catch: (cause: unknown) => new MppSptReplayError(cause),
    try: async () => {
      await treasuryAuthorityDb(database)
        .prepare(
          `UPDATE mpp_spt_replay SET payment_intent_id = ? WHERE spt = ?`,
        )
        .bind(input.paymentIntentId, input.spt)
        .run()
      await mirrorTreasuryRows(database, 'mpp_spt_replay', 'spt', [input.spt])
    },
  })
