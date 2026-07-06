import { Schema as S } from 'effect'
import { assertLaborEscrowPublicSafe } from './labor-escrow'
import type { PaymentsLedgerDb } from './payments-ledger-db'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

export const LaborEarningsSummary = S.Struct({
  releasedEscrowCount: S.Number,
  totalReleasedMsat: S.Number,
})

export type LaborEarningsSummary = typeof LaborEarningsSummary.Type

export const LaborEarningRow = S.Struct({
  amountMsat: S.Number,
  escrowRef: S.String,
  jobEventRef: S.String,
  receiptRef: S.String,
  requesterActorRef: S.String,
  workRequestRef: S.String,
  releasedAtIso: S.String,
})

export type LaborEarningRow = typeof LaborEarningRow.Type

export const LaborEarningsResponse = S.Struct({
  schemaVersion: S.Literal('openagents.labor_earnings.v1'),
  providerActorRef: S.String,
  publicSafe: S.Literal(true),
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
  summary: LaborEarningsSummary,
  rows: S.Array(LaborEarningRow),
  authorityBoundary: S.String,
})

export type LaborEarningsResponse = typeof LaborEarningsResponse.Type

export const buildLaborEarningsProjection = (
  providerActorRef: string,
  rows: ReadonlyArray<LaborEarningRow>,
  generatedAt: string,
): LaborEarningsResponse => {
  assertLaborEscrowPublicSafe(providerActorRef, 'provider actor ref')
  
  let totalReleasedMsat = 0
  for (const row of rows) {
    assertLaborEscrowPublicSafe(row)
    totalReleasedMsat += row.amountMsat
  }

  return {
    schemaVersion: 'openagents.labor_earnings.v1',
    providerActorRef,
    publicSafe: true,
    generatedAt,
    staleness: liveAtReadStaleness(['labor_escrow_receipts']),
    summary: {
      releasedEscrowCount: rows.length,
      totalReleasedMsat,
    },
    rows: [...rows],
    authorityBoundary: 'This projection exposes public-safe labor earnings only and grants no spend, settlement, or payout authority.',
  }
}

/** CFG-4 (#8519): `labor_escrows` is Cloud SQL Postgres-authoritative — this
 * public earnings read runs on the credits-domain `PaymentsLedgerDb`, never
 * on D1. */
export const readLaborEarnings = async (
  ledgerDb: PaymentsLedgerDb,
  providerActorRef: string,
  generatedAt: string,
  limit: number = 50,
): Promise<LaborEarningsResponse> => {
  const results = await ledgerDb.query(
    `SELECT e.amount_msat, e.id AS escrow_id, e.job_event_id,
            e.release_receipt_ref, e.requester_actor_ref, e.work_request_id,
            e.released_at
       FROM labor_escrows e
      WHERE e.provider_actor_ref = ?
        AND e.state = 'released_to_provider'
      ORDER BY e.released_at DESC
      LIMIT ?`,
    [providerActorRef, limit],
  )

  const rows = results.map(
    (row) => ({
      amountMsat: Number(row.amount_msat),
      escrowRef: `labor_escrow.public.${String(row.escrow_id)}`,
      jobEventRef: `nostr.event.${String(row.job_event_id)}`,
      receiptRef: String(row.release_receipt_ref),
      requesterActorRef: String(row.requester_actor_ref),
      workRequestRef: `work_request.public.${String(row.work_request_id)}`,
      releasedAtIso: String(row.released_at),
    })
  )

  return buildLaborEarningsProjection(providerActorRef, rows, generatedAt)
}
