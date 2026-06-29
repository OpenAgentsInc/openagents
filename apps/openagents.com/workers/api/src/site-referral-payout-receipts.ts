import { parseJsonStringArray } from './json-boundary'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import type { SiteReferralPayoutState } from './site-referral-payout-ledger'

export const SITE_REFERRAL_PAYOUT_RECEIPT_SCHEMA_VERSION =
  'openagents.site_referral_payout_receipt.v1'

export type SiteReferralPayoutReceiptSettlementRail =
  | 'hosted_mdk'
  | 'staging_test'
  | 'public_safe_adapter'

export type PublicSiteReferralPayoutReceiptProjection = Readonly<{
  amountSats: number
  attributionLinked: true
  authorityBoundary: string
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  evidenceRefs: ReadonlyArray<string>
  generatedAt: string
  liveRailReceipt: boolean
  policyRefs: ReadonlyArray<string>
  qualifyingEventKind: string
  receiptRef: string
  resolution: Readonly<{
    settlementRail: SiteReferralPayoutReceiptSettlementRail
    state: Extract<SiteReferralPayoutState, 'settled'>
    status: 'ok'
  }>
  schemaVersion: typeof SITE_REFERRAL_PAYOUT_RECEIPT_SCHEMA_VERSION
  sourceRefs: ReadonlyArray<string>
  staleness: PublicProjectionStalenessContract
}>

export type SiteReferralPayoutReceiptStore = Readonly<{
  readSiteReferralPayoutReceipt: (
    receiptRef: string,
    generatedAt: string,
  ) => Promise<PublicSiteReferralPayoutReceiptProjection | null>
}>

type SettledReceiptRow = Readonly<{
  amount_sats: number
  caveat_refs_json: string
  evidence_refs_json: string
  policy_refs_json: string
  qualifying_event_kind: string
}>

const receiptPrefix = 'receipt.site_referral_payout.'

const safeReceiptPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,300}$/
const prohibitedRefPattern =
  /\b(lnbc|lntb|lnbcrt|lno1|mnemonic|xprv|payment_preimage|payment_secret|payment_hash|wallet_secret|private_key|webhook_secret|mdk_access_token|access_token|refresh_token|device_auth_id|code_verifier|gho_[a-z0-9_]+)/i

const isPublicSafeRef = (value: string): boolean =>
  safeReceiptPattern.test(value) && !prohibitedRefPattern.test(value)

const settlementRailForReceipt = (
  receiptRef: string,
): SiteReferralPayoutReceiptSettlementRail =>
  receiptRef.startsWith(`${receiptPrefix}hosted_mdk.`)
    ? 'hosted_mdk'
    : receiptRef.startsWith(`${receiptPrefix}staging_test.`)
      ? 'staging_test'
      : 'public_safe_adapter'

const isLiveRailReceipt = (receiptRef: string): boolean =>
  settlementRailForReceipt(receiptRef) === 'hosted_mdk'

const blockerRefsForReceipt = (receiptRef: string): ReadonlyArray<string> =>
  isLiveRailReceipt(receiptRef)
    ? []
    : ['blocker.product_promises.referral_first_real_live_payout_pending']

const publicProjection = (
  row: SettledReceiptRow,
  receiptRef: string,
  generatedAt: string,
): PublicSiteReferralPayoutReceiptProjection => {
  const evidenceRefs = parseJsonStringArray(row.evidence_refs_json).filter(ref =>
    isPublicSafeRef(ref),
  )

  return {
    amountSats: Number(row.amount_sats),
    attributionLinked: true,
    authorityBoundary:
      'Public proof only. This referral payout receipt read grants no attribution, invite, checkout, spend, refund, payout, settlement, wallet, provider, or registry authority.',
    blockerRefs: blockerRefsForReceipt(receiptRef),
    caveatRefs: parseJsonStringArray(row.caveat_refs_json),
    evidenceRefs,
    generatedAt,
    liveRailReceipt: isLiveRailReceipt(receiptRef),
    policyRefs: parseJsonStringArray(row.policy_refs_json),
    qualifyingEventKind: row.qualifying_event_kind,
    receiptRef,
    resolution: {
      settlementRail: settlementRailForReceipt(receiptRef),
      state: 'settled',
      status: 'ok',
    },
    schemaVersion: SITE_REFERRAL_PAYOUT_RECEIPT_SCHEMA_VERSION,
    sourceRefs: [
      `route:/api/public/site-referral-payout-receipts/${receiptRef}`,
      'ledger.site_referral_payout_ledger_entries.state',
      'ledger.site_referral_payout_ledger_entries.evidence_refs_json',
    ],
    staleness: liveAtReadStaleness([
      'site_referral_payout_eligibility_recorded',
      'site_referral_payout_state_transition_recorded',
    ]),
  }
}

export const makeD1SiteReferralPayoutReceiptStore = (
  db: D1Database,
): SiteReferralPayoutReceiptStore => ({
  readSiteReferralPayoutReceipt: async (receiptRef, generatedAt) => {
    if (!receiptRef.startsWith(receiptPrefix) || !isPublicSafeRef(receiptRef)) {
      return null
    }

    const result = await db
      .prepare(
        `SELECT amount_sats, qualifying_event_kind, evidence_refs_json,
                policy_refs_json, caveat_refs_json
           FROM site_referral_payout_ledger_entries
          WHERE state = 'settled'
            AND archived_at IS NULL
            AND evidence_refs_json LIKE ?
          ORDER BY created_at DESC, id DESC
          LIMIT 10`,
      )
      .bind(`%${receiptRef}%`)
      .all<SettledReceiptRow>()

    const matchingRow =
      (result.results ?? []).find(row =>
        parseJsonStringArray(row.evidence_refs_json).includes(receiptRef),
      ) ?? null

    return matchingRow === null
      ? null
      : publicProjection(matchingRow, receiptRef, generatedAt)
  },
})
