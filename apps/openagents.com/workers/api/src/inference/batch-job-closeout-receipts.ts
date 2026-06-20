import { Schema as S } from 'effect'

import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from '../public-projection-staleness'

export const BatchJobCloseoutReceiptSchema = S.Struct({
  schemaVersion: S.Literal('openagents.inference.batch_job.closeout.v1'),
  receiptRef: S.String,
  jobId: S.String,
  chargeReceiptRef: S.String,
  totalItems: S.Number,
  successfulItems: S.Number,
  failedItems: S.Number,
  totalCostMsat: S.Number,
  completedAtIso: S.String,
  resultsR2Key: S.String,
})

export type BatchJobCloseoutReceipt = S.Schema.Type<
  typeof BatchJobCloseoutReceiptSchema
>

export const projectBatchJobCloseoutReceipt = (
  receipt: BatchJobCloseoutReceipt,
  generatedAt: string,
): Readonly<{
  authorityBoundary: string
  caveatRefs: ReadonlyArray<string>
  generatedAt: string
  receipt: BatchJobCloseoutReceipt
  staleness: PublicProjectionStalenessContract
}> => ({
  authorityBoundary:
    'Public proof only. This receipt read grants no spend, refund, payout, checkout, settlement, provider, or registry authority.',
  caveatRefs: [
    'caveat.public.no_private_payment_material',
    'caveat.public.no_account_or_amount_projection',
  ],
  generatedAt,
  receipt,
  staleness: liveAtReadStaleness([]),
})
