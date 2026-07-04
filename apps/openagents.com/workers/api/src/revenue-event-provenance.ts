import { Schema as S } from 'effect'

import { parseJsonStringArray } from './json-boundary'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import {
  mirrorTreasuryRows,
  treasuryAuthorityDb,
  type TreasuryDatabase,
} from './treasury-domain-store'

export const REVENUE_EVENT_PROVENANCE_TABLE =
  'revenue_event_provenance' as const
export const FIRST_DOLLAR_EVIDENCE_PUBLIC_ENDPOINT =
  '/api/public/revenue-loop/first-dollar-evidence' as const
export const FIRST_DOLLAR_EVIDENCE_PUBLIC_PATH =
  '/api/public/revenue-loop/first-dollar-evidence/:bundleRef' as const

export const RevenueEventProductRef = S.Literals(['khala_code', 'qa_swarm'])
export type RevenueEventProductRef = typeof RevenueEventProductRef.Type

export const RevenueEventDemandProvenance = S.Literals([
  'internal',
  'external',
])
export type RevenueEventDemandProvenance =
  typeof RevenueEventDemandProvenance.Type

export const RevenueEventPaymentState = S.Literals([
  'requires_payment',
  'payment_evidence_recorded',
  'fulfilled',
  'settled',
])
export type RevenueEventPaymentState = typeof RevenueEventPaymentState.Type

export const RevenueEventLedgerTable = S.Literals([
  'khala_code_paid_plan_payment_intents',
  'qa_swarm_first_engagements',
])
export type RevenueEventLedgerTable = typeof RevenueEventLedgerTable.Type

export const PublicFirstDollarEvidenceBundle = S.Struct({
  schemaVersion: S.Literal(
    'openagents.revenue_loop.first_dollar_evidence_bundle.v1',
  ),
  bundleRef: S.String,
  bundleUrl: S.String,
  generatedAt: S.String,
  recordedAt: S.String,
  revenueEvent: S.Struct({
    eventRef: S.String,
    productRef: RevenueEventProductRef,
    revenueSurfaceRef: S.String,
    receiptRef: S.String,
    ledgerTable: RevenueEventLedgerTable,
    ledgerRowRef: S.String,
    demandProvenance: RevenueEventDemandProvenance,
    paymentState: RevenueEventPaymentState,
    amountCents: S.NullOr(S.Number),
    amountSats: S.NullOr(S.Number),
  }),
  registryEvidenceRefs: S.Array(S.String),
  provenance: S.Struct({
    label: RevenueEventDemandProvenance,
    rule: S.Literal('no_external_dollar_no_demand_claim'),
    labelSource: S.Literal('ledger.revenue_event_provenance.demand_provenance'),
  }),
  publicSafety: S.Struct({
    noCustomerIdentity: S.Literal(true),
    noRawPaymentMaterial: S.Literal(true),
    noRawInvoice: S.Literal(true),
    noPaymentHashOrPreimage: S.Literal(true),
    noCheckoutUrl: S.Literal(true),
    noProviderPayloads: S.Literal(true),
    noWalletMaterial: S.Literal(true),
  }),
  caveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
})
export type PublicFirstDollarEvidenceBundle =
  typeof PublicFirstDollarEvidenceBundle.Type

export const PublicFirstDollarEvidenceEnvelope = S.Struct({
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
  bundle: PublicFirstDollarEvidenceBundle,
})
export type PublicFirstDollarEvidenceEnvelope =
  typeof PublicFirstDollarEvidenceEnvelope.Type

export type RevenueEventProvenanceRecord = Readonly<{
  eventRef: string
  evidenceBundleRef: string
  productRef: RevenueEventProductRef
  revenueSurfaceRef: string
  receiptRef: string
  ledgerTable: RevenueEventLedgerTable
  ledgerRowRef: string
  demandProvenance: RevenueEventDemandProvenance
  paymentState: RevenueEventPaymentState
  amountCents: number | null
  amountSats: number | null
  publicEvidenceRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  recordedAt: string
}>

export type RevenueEventProvenanceDraft =
  RevenueEventProvenanceRecord &
    Readonly<{
      idempotencyKey: string
    }>

type RevenueEventProvenanceRow = Readonly<{
  event_ref: string
  evidence_bundle_ref: string
  product_ref: string
  revenue_surface_ref: string
  receipt_ref: string
  ledger_table: string
  ledger_row_ref: string
  demand_provenance: string
  payment_state: string
  amount_cents: number | null
  amount_sats: number | null
  public_evidence_refs_json: string
  caveat_refs_json: string
  source_refs_json: string
  recorded_at: string
}>

const firstDollarStaleness = liveAtReadStaleness([
  REVENUE_EVENT_PROVENANCE_TABLE,
])

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,300}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|checkout[_-]?(raw|url)|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(raw|payload|body|url)|lnbc|lntb|lnbcrt|lno1|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage|proof|raw|secret|payload)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(customer|key|repo|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(auth|customer|fixture|invoice|log|package|payment|payload|prompt|provider|receipt|runner|run[_-]?log|schema|source|target|trace|usage|webhook)|secret|seed[_-]?phrase|sk-[a-z0-9]|spark[_-]?(address|invoice|request|secret)|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const refIsSafe = (ref: string): boolean =>
  safeRefPattern.test(ref) &&
  !unsafeRefPattern.test(ref) &&
  !rawTimestampPattern.test(ref)

const assertSafeRefs = (refs: ReadonlyArray<string>): void => {
  for (const ref of refs) {
    if (!refIsSafe(ref)) {
      throw Error(`unsafe_revenue_evidence_ref:${ref}`)
    }
  }
}

const eventSuffix = (value: string): string => {
  const parts = value.split(/[:.]/)
  return parts.at(-1) ?? compactRandomId('revenue_event')
}

export const firstDollarEvidenceBundleRef = (
  productRef: RevenueEventProductRef,
  eventRef: string,
): string =>
  `evidence.revenue.first_dollar.${productRef}.${eventSuffix(eventRef)}`

const rowToRecord = (
  row: RevenueEventProvenanceRow | null | undefined,
): RevenueEventProvenanceRecord | null => {
  if (row === null || row === undefined) {
    return null
  }

  const productRef =
    row.product_ref === 'qa_swarm' ? 'qa_swarm' : 'khala_code'
  const demandProvenance =
    row.demand_provenance === 'internal' ? 'internal' : 'external'
  const paymentState =
    row.payment_state === 'requires_payment' ||
    row.payment_state === 'fulfilled' ||
    row.payment_state === 'settled'
      ? row.payment_state
      : 'payment_evidence_recorded'
  const ledgerTable =
    row.ledger_table === 'qa_swarm_first_engagements'
      ? 'qa_swarm_first_engagements'
      : 'khala_code_paid_plan_payment_intents'

  return {
    eventRef: row.event_ref,
    evidenceBundleRef: row.evidence_bundle_ref,
    productRef,
    revenueSurfaceRef: row.revenue_surface_ref,
    receiptRef: row.receipt_ref,
    ledgerTable,
    ledgerRowRef: row.ledger_row_ref,
    demandProvenance,
    paymentState,
    amountCents: row.amount_cents,
    amountSats: row.amount_sats,
    publicEvidenceRefs: parseJsonStringArray(row.public_evidence_refs_json),
    caveatRefs: parseJsonStringArray(row.caveat_refs_json),
    sourceRefs: parseJsonStringArray(row.source_refs_json),
    recordedAt: row.recorded_at,
  }
}

export const recordRevenueEventProvenance = async (
  database: TreasuryDatabase,
  draft: RevenueEventProvenanceDraft,
): Promise<RevenueEventProvenanceRecord> => {
  assertSafeRefs([
    draft.eventRef,
    draft.evidenceBundleRef,
    draft.revenueSurfaceRef,
    draft.receiptRef,
    draft.ledgerTable,
    draft.ledgerRowRef,
    ...draft.publicEvidenceRefs,
    ...draft.caveatRefs,
    ...draft.sourceRefs,
  ])

  if (draft.amountCents === null && draft.amountSats === null) {
    throw Error('revenue_event_amount_required')
  }

  const nowIso = currentIsoTimestamp()
  const db = treasuryAuthorityDb(database)
  await db
    .prepare(
      `INSERT OR IGNORE INTO revenue_event_provenance (
        event_ref,
        evidence_bundle_ref,
        idempotency_key,
        product_ref,
        revenue_surface_ref,
        receipt_ref,
        ledger_table,
        ledger_row_ref,
        demand_provenance,
        payment_state,
        amount_cents,
        amount_sats,
        public_evidence_refs_json,
        caveat_refs_json,
        source_refs_json,
        recorded_at,
        created_at,
        updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      draft.eventRef,
      draft.evidenceBundleRef,
      draft.idempotencyKey,
      draft.productRef,
      draft.revenueSurfaceRef,
      draft.receiptRef,
      draft.ledgerTable,
      draft.ledgerRowRef,
      draft.demandProvenance,
      draft.paymentState,
      draft.amountCents,
      draft.amountSats,
      JSON.stringify(draft.publicEvidenceRefs),
      JSON.stringify(draft.caveatRefs),
      JSON.stringify(draft.sourceRefs),
      draft.recordedAt,
      nowIso,
      nowIso,
    )
    .run()

  const row = await db
    .prepare(
      `SELECT event_ref,
              evidence_bundle_ref,
              product_ref,
              revenue_surface_ref,
              receipt_ref,
              ledger_table,
              ledger_row_ref,
              demand_provenance,
              payment_state,
              amount_cents,
              amount_sats,
              public_evidence_refs_json,
              caveat_refs_json,
              source_refs_json,
              recorded_at
         FROM revenue_event_provenance
        WHERE idempotency_key = ?
        LIMIT 1`,
    )
    .bind(draft.idempotencyKey)
    .first<RevenueEventProvenanceRow>()

  const record = rowToRecord(row)
  if (record === null) {
    throw Error('revenue_event_provenance_not_persisted')
  }

  // KS-8.8 (#8319): fail-soft Postgres mirror of the persisted evidence row.
  await mirrorTreasuryRows(database, 'revenue_event_provenance', 'event_ref', [
    record.eventRef,
  ])

  return record
}

export const firstDollarEvidenceBundleFromRecord = (
  record: RevenueEventProvenanceRecord,
  generatedAt: string,
): PublicFirstDollarEvidenceBundle => {
  const bundleUrl = `${FIRST_DOLLAR_EVIDENCE_PUBLIC_ENDPOINT}/${encodeURIComponent(
    record.evidenceBundleRef,
  )}`
  const registryEvidenceRefs = [
    `route:${bundleUrl}`,
    `receipt:${record.receiptRef}`,
    `ledger:${record.ledgerTable}:${record.ledgerRowRef}`,
    `ledger:${REVENUE_EVENT_PROVENANCE_TABLE}:${record.eventRef}`,
    ...record.publicEvidenceRefs,
  ]

  return {
    schemaVersion:
      'openagents.revenue_loop.first_dollar_evidence_bundle.v1',
    bundleRef: record.evidenceBundleRef,
    bundleUrl,
    generatedAt,
    recordedAt: record.recordedAt,
    revenueEvent: {
      eventRef: record.eventRef,
      productRef: record.productRef,
      revenueSurfaceRef: record.revenueSurfaceRef,
      receiptRef: record.receiptRef,
      ledgerTable: record.ledgerTable,
      ledgerRowRef: record.ledgerRowRef,
      demandProvenance: record.demandProvenance,
      paymentState: record.paymentState,
      amountCents: record.amountCents,
      amountSats: record.amountSats,
    },
    registryEvidenceRefs,
    provenance: {
      label: record.demandProvenance,
      rule: 'no_external_dollar_no_demand_claim',
      labelSource: 'ledger.revenue_event_provenance.demand_provenance',
    },
    publicSafety: {
      noCustomerIdentity: true,
      noRawPaymentMaterial: true,
      noRawInvoice: true,
      noPaymentHashOrPreimage: true,
      noCheckoutUrl: true,
      noProviderPayloads: true,
      noWalletMaterial: true,
    },
    caveatRefs: record.caveatRefs,
    sourceRefs: [
      `table:${REVENUE_EVENT_PROVENANCE_TABLE}`,
      ...record.sourceRefs,
    ],
    staleness: firstDollarStaleness,
  }
}

export const readFirstDollarEvidenceBundle = async (
  db: D1Database,
  bundleRef: string,
  generatedAt: string,
): Promise<PublicFirstDollarEvidenceBundle | null> => {
  const row = await db
    .prepare(
      `SELECT event_ref,
              evidence_bundle_ref,
              product_ref,
              revenue_surface_ref,
              receipt_ref,
              ledger_table,
              ledger_row_ref,
              demand_provenance,
              payment_state,
              amount_cents,
              amount_sats,
              public_evidence_refs_json,
              caveat_refs_json,
              source_refs_json,
              recorded_at
         FROM revenue_event_provenance
        WHERE evidence_bundle_ref = ?
        LIMIT 1`,
    )
    .bind(bundleRef)
    .first<RevenueEventProvenanceRow>()
  const record = rowToRecord(row)

  return record === null
    ? null
    : firstDollarEvidenceBundleFromRecord(record, generatedAt)
}
