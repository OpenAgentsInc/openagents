import { parseJsonStringArray } from './json-boundary'
import type { PartnerPayoutAsset, PartnerPayoutState } from './partner-payout-ledger'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

export const PARTNER_PAYOUT_RECEIPT_SCHEMA_VERSION =
  'openagents.partner_payout_receipt.v1'

export type PartnerPayoutReceiptSettlementRail =
  | 'hosted_mdk'
  | 'staging_test'
  | 'public_safe_adapter'

export type PublicPartnerPayoutReceiptProjection = Readonly<{
  amount: number
  asset: PartnerPayoutAsset
  authorityBoundary: string
  caveatRefs: ReadonlyArray<string>
  evidenceRefs: ReadonlyArray<string>
  generatedAt: string
  policyRefs: ReadonlyArray<string>
  qualifyingEventKind: string
  receiptRef: string
  resolution: Readonly<{
    settlementRail: PartnerPayoutReceiptSettlementRail
    state: Extract<PartnerPayoutState, 'settled'>
    status: 'ok'
  }>
  schemaVersion: typeof PARTNER_PAYOUT_RECEIPT_SCHEMA_VERSION
  sourceRefs: ReadonlyArray<string>
  staleness: PublicProjectionStalenessContract
}>

export type PartnerPayoutReceiptStore = Readonly<{
  readPartnerPayoutReceipt: (
    receiptRef: string,
    generatedAt: string,
  ) => Promise<PublicPartnerPayoutReceiptProjection | null>
}>

type SettledPartnerPayoutReceiptRow = Readonly<{
  amount: number
  asset: PartnerPayoutAsset
  caveat_refs_json: string
  evidence_refs_json: string
  policy_refs_json: string
  qualifying_event_kind: string
}>

const receiptPrefix = 'receipt.partner_payout.'

const safeReceiptPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,300}$/
const prohibitedRefPattern =
  /\b(lnbc|lntb|lnbcrt|lno1|mnemonic|xprv|payment_preimage|payment_secret|payment_hash|wallet_secret|private_key|webhook_secret|mdk_access_token|access_token|refresh_token|device_auth_id|code_verifier|gho_[a-z0-9_]+)/i

const isPublicSafeRef = (value: string): boolean =>
  safeReceiptPattern.test(value) && !prohibitedRefPattern.test(value)

const settlementRailForReceipt = (
  receiptRef: string,
): PartnerPayoutReceiptSettlementRail =>
  receiptRef.startsWith(`${receiptPrefix}hosted_mdk.`)
    ? 'hosted_mdk'
    : receiptRef.startsWith(`${receiptPrefix}staging_test.`)
      ? 'staging_test'
      : 'public_safe_adapter'

const publicProjection = (
  row: SettledPartnerPayoutReceiptRow,
  receiptRef: string,
  generatedAt: string,
): PublicPartnerPayoutReceiptProjection => {
  const evidenceRefs = parseJsonStringArray(row.evidence_refs_json).filter(ref =>
    isPublicSafeRef(ref),
  )

  return {
    amount: Number(row.amount),
    asset: row.asset,
    authorityBoundary:
      'Public proof only. This partner payout receipt read grants no partner attribution, eligibility, payout, settlement, withdrawal, wallet, provider, spend, revenue, registry, or public-claim authority.',
    caveatRefs: parseJsonStringArray(row.caveat_refs_json),
    evidenceRefs,
    generatedAt,
    policyRefs: parseJsonStringArray(row.policy_refs_json),
    qualifyingEventKind: row.qualifying_event_kind,
    receiptRef,
    resolution: {
      settlementRail: settlementRailForReceipt(receiptRef),
      state: 'settled',
      status: 'ok',
    },
    schemaVersion: PARTNER_PAYOUT_RECEIPT_SCHEMA_VERSION,
    sourceRefs: [
      `route:/api/public/partner-payout-receipts/${receiptRef}`,
      'ledger.partner_payout_ledger_entries.state',
      'ledger.partner_payout_ledger_entries.evidence_refs_json',
    ],
    staleness: liveAtReadStaleness([
      'partner_payout_eligibility_recorded',
      'partner_payout_state_transition_recorded',
    ]),
  }
}

export const makeD1PartnerPayoutReceiptStore = (
  db: D1Database,
): PartnerPayoutReceiptStore => ({
  readPartnerPayoutReceipt: async (receiptRef, generatedAt) => {
    if (!receiptRef.startsWith(receiptPrefix) || !isPublicSafeRef(receiptRef)) {
      return null
    }

    const result = await db
      .prepare(
        `SELECT amount, asset, qualifying_event_kind, evidence_refs_json,
                policy_refs_json, caveat_refs_json
           FROM partner_payout_ledger_entries
          WHERE state = 'settled'
            AND archived_at IS NULL
            AND evidence_refs_json LIKE ?
          ORDER BY created_at DESC, id DESC
          LIMIT 10`,
      )
      .bind(`%${receiptRef}%`)
      .all<SettledPartnerPayoutReceiptRow>()

    const matchingRow =
      (result.results ?? []).find(row =>
        parseJsonStringArray(row.evidence_refs_json).includes(receiptRef),
      ) ?? null

    return matchingRow === null
      ? null
      : publicProjection(matchingRow, receiptRef, generatedAt)
  },
})
