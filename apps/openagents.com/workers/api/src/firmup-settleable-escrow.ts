import { type FirmupSettleableEscrowProjection } from './firmup-bitcoin-settlement-routes'
import { laborEscrowRef, readLaborEscrowById } from './labor-escrow'
import type { PaymentsLedgerDb } from './payments-ledger-db'

/**
 * Server-side SOURCE OF TRUTH for whether a firm-up escrow may settle to Bitcoin
 * (openagents #5459). It reads the escrow + accepted offer + work request rows
 * and is fail-closed: the operator cannot assert settleability through the
 * settle request body.
 *
 * A firm-up escrow is settleable only when:
 *   - the escrow exists and is still `reserved` (not already released/refunded),
 *   - the work request exists (so the declared verification command is known),
 *   - an accepted offer exists (so the provider/worker actor is known — without
 *     an accepted offer the job was never firmed up).
 *
 * Returns the public-safe projection the firm-up settlement route consumes, or
 * `undefined` when not settleable. The two supporting reads are Promise-native
 * D1 lookups for the two bounded fields the projection needs (the provider actor
 * and the declared verification command), so the worker entrypoint stays free of
 * `Effect.runPromise` bridges.
 */
export const readFirmupSettleableEscrow = async (
  deps: Readonly<{
    /** Credits/escrow authority (CFG-4 #8519: Postgres-only). */
    ledgerDb: PaymentsLedgerDb
    /** Forum work-request/acceptance rows (their own D1 domain). */
    db: D1Database
  }>,
  escrowRef: string,
): Promise<FirmupSettleableEscrowProjection | undefined> => {
  const { ledgerDb, db } = deps
  // The escrow public ref is `labor_escrow.public.<escrowId>`; recover the id.
  const prefix = laborEscrowRef('')
  const escrowId = escrowRef.startsWith(prefix)
    ? escrowRef.slice(prefix.length)
    : escrowRef
  const escrow = await readLaborEscrowById(ledgerDb, escrowId)

  // Fail-closed: only a real, still-reserved firm-up escrow is settleable.
  if (escrow === null || escrow.state !== 'reserved') {
    return undefined
  }

  const [workRequestRow, acceptanceRow] = await Promise.all([
    db
      .prepare(
        `SELECT verification_command_ref
           FROM forum_work_requests
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(escrow.workRequestId)
      .first<{ verification_command_ref: string }>(),
    db
      .prepare(
        `SELECT provider_actor_ref
           FROM forum_work_request_acceptances
          WHERE work_request_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(escrow.workRequestId)
      .first<{ provider_actor_ref: string }>(),
  ])

  // The accepted offer carries the provider (worker) actor; without an accepted
  // offer the job was never firmed up. Fail-closed.
  if (workRequestRow === null || acceptanceRow === null) {
    return undefined
  }

  return {
    amountSats: Math.trunc(escrow.amountMsat / 1000),
    escrowRef: laborEscrowRef(escrow.escrowId),
    providerActorRef: acceptanceRow.provider_actor_ref,
    verificationCommandRef: workRequestRow.verification_command_ref,
    workRequestRef: `work_request.public.${escrow.workRequestId}`,
  }
}
