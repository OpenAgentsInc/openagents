import { Schema as S } from 'effect'

import { optionalString } from './json-boundary'

export type Nip90MarketStreamKind = 'compute' | 'data' | 'labor'

export type Nip90MarketSettlementReceiptRecord = Readonly<{
  amountMsats: number
  createdAt: string
  jobRef: string
  receiptRef: string
  requestEventRef: string
  resultEventRef: string
  settledAt: string
  state: 'issued' | 'settled' | 'settlement_blocked' | 'settlement_failed'
  streamKind: Nip90MarketStreamKind
}>

export type Nip90MarketReceiptStore = Readonly<{
  listSettledMarketReceipts: (
    limit: number,
  ) => Promise<ReadonlyArray<Nip90MarketSettlementReceiptRecord>>
  readSettledMarketReceiptByRef: (
    receiptRef: string,
  ) => Promise<Nip90MarketSettlementReceiptRecord | null>
}>

export class PublicNip90MarketSettlementReceipt extends S.Class<PublicNip90MarketSettlementReceipt>(
  'PublicNip90MarketSettlementReceipt',
)({
  amountSats: S.Number,
  caveatRefs: S.Array(S.String),
  jobRef: S.String,
  receiptRef: S.String,
  requestEventRef: S.String,
  resultEventRef: S.String,
  schemaVersion: S.Literal('openagents.nip90_market.receipt.v1'),
  settledAt: S.String,
  sourceRefs: S.Array(S.String),
  state: S.Literal('settled'),
  streamKind: S.Literals(['compute', 'data', 'labor']),
}) {}

const unsafePublicReceiptPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|bearer\s+|bolt11|cookie|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(invoice|payment|payload|prompt|runner|state)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

export const isPublicSafeNip90MarketReceiptProjection = (
  value: unknown,
): boolean => !unsafePublicReceiptPattern.test(JSON.stringify(value))

export const publicNip90MarketReceiptFromRecord = (
  record: Nip90MarketSettlementReceiptRecord,
): PublicNip90MarketSettlementReceipt | null => {
  if (
    record.state !== 'settled' ||
    record.amountMsats < 0 ||
    record.amountMsats % 1000 !== 0
  ) {
    return null
  }

  const receipt = new PublicNip90MarketSettlementReceipt({
    amountSats: record.amountMsats / 1000,
    caveatRefs: [
      'caveat.public.nip90_market.settled_receipt_only',
      'caveat.public.no_private_settlement_material',
      'caveat.public.counterparty_destination_details_excluded',
    ],
    jobRef: record.jobRef,
    receiptRef: record.receiptRef,
    requestEventRef: record.requestEventRef,
    resultEventRef: record.resultEventRef,
    schemaVersion: 'openagents.nip90_market.receipt.v1',
    settledAt: record.settledAt,
    sourceRefs: [
      `route:/api/public/nip90-market/receipts/${record.receiptRef}`,
      `nip90.market.${record.streamKind}.settlement`,
    ],
    state: 'settled',
    streamKind: record.streamKind,
  })

  return isPublicSafeNip90MarketReceiptProjection(receipt) ? receipt : null
}

type BuyModeMarketReceiptRow = Readonly<{
  amount_msats: number
  created_at: string
  job_id: string
  receipt_ref: string | null
  request_event_id: string
  result_event_id: string | null
  state: string
  updated_at: string
}>

const rowToBuyModeMarketReceipt = (
  row: BuyModeMarketReceiptRow,
): Nip90MarketSettlementReceiptRecord | null => {
  const receiptRef = optionalString(row.receipt_ref)
  const resultEventRef = optionalString(row.result_event_id)

  if (
    row.state !== 'settled' ||
    receiptRef === undefined ||
    resultEventRef === undefined
  ) {
    return null
  }

  return {
    amountMsats: row.amount_msats,
    createdAt: row.created_at,
    jobRef: row.job_id,
    receiptRef,
    requestEventRef: row.request_event_id,
    resultEventRef,
    settledAt: row.updated_at,
    state: 'settled',
    streamKind: 'compute',
  }
}

export const makeD1Nip90MarketReceiptStore = (
  db: D1Database,
): Nip90MarketReceiptStore => ({
  listSettledMarketReceipts: async limit => {
    const result = await db.prepare(
      `SELECT job_id, request_event_id, result_event_id, amount_msats,
              state, receipt_ref, created_at, updated_at
         FROM buy_mode_jobs
        WHERE state = 'settled'
          AND receipt_ref IS NOT NULL
        ORDER BY updated_at DESC
        LIMIT ?`,
    ).bind(limit).all<BuyModeMarketReceiptRow>()

    return (result.results ?? [])
      .map(rowToBuyModeMarketReceipt)
      .filter((record): record is Nip90MarketSettlementReceiptRecord =>
        record !== null
      )
  },
  readSettledMarketReceiptByRef: async receiptRef => {
    const row = await db.prepare(
      `SELECT job_id, request_event_id, result_event_id, amount_msats,
              state, receipt_ref, created_at, updated_at
         FROM buy_mode_jobs
        WHERE receipt_ref = ?
          AND state = 'settled'
        LIMIT 1`,
    ).bind(receiptRef).first<BuyModeMarketReceiptRow>()

    return row === null ? null : rowToBuyModeMarketReceipt(row)
  },
})
