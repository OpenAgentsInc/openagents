import {
  EcommerceCampaignDeliveryReceiptDocument,
  decodeEcommerceCampaignDeliveryReceiptDocument,
  serializeEcommerceCampaignDeliveryReceiptDocument,
} from './ecommerce-campaign-delivery-receipt'

export type EcommerceCampaignSealedReceipt = Readonly<{
  document: EcommerceCampaignDeliveryReceiptDocument
  receiptRef: string
  serialized: string
}>

export class EcommerceCampaignReceiptStoreError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'EcommerceCampaignReceiptStoreError'
  }
}

export type EcommerceCampaignReceiptPutResult =
  | Readonly<{ kind: 'stored'; sealed: EcommerceCampaignSealedReceipt }>
  | Readonly<{ kind: 'already_stored'; sealed: EcommerceCampaignSealedReceipt }>

export type EcommerceCampaignReceiptStore = Readonly<{
  put: (
    document: EcommerceCampaignDeliveryReceiptDocument,
    receiptRef: string,
  ) => Promise<EcommerceCampaignReceiptPutResult>
  get: (
    receiptRef: string,
  ) => Promise<EcommerceCampaignSealedReceipt | undefined>
  list: () => Promise<ReadonlyArray<EcommerceCampaignSealedReceipt>>
}>

export const makeInMemoryEcommerceCampaignReceiptStore =
  (): EcommerceCampaignReceiptStore & {
    readonly rows: ReadonlyMap<string, EcommerceCampaignSealedReceipt>
  } => {
    const rows = new Map<string, EcommerceCampaignSealedReceipt>()

    const assertConsistent = (
      document: EcommerceCampaignDeliveryReceiptDocument,
      serialized: string,
    ): EcommerceCampaignDeliveryReceiptDocument => {
      const decoded = decodeEcommerceCampaignDeliveryReceiptDocument(serialized)
      if (serializeEcommerceCampaignDeliveryReceiptDocument(decoded) !== serialized) {
        throw new EcommerceCampaignReceiptStoreError(
          'Document does not match serialized bytes',
        )
      }
      return decoded
    }

    return {
      get: async receiptRef => {
        const existing = rows.get(receiptRef)
        if (existing === undefined) {
          return undefined
        }
        assertConsistent(existing.document, existing.serialized)
        return existing
      },
      list: async () => [...rows.values()],
      put: async (document, receiptRef) => {
        const serialized = serializeEcommerceCampaignDeliveryReceiptDocument(document)
        assertConsistent(document, serialized)

        const existing = rows.get(receiptRef)
        if (existing !== undefined) {
          return { kind: 'already_stored', sealed: existing }
        }

        const sealed: EcommerceCampaignSealedReceipt = {
          document,
          receiptRef,
          serialized,
        }
        rows.set(receiptRef, sealed)
        return { kind: 'stored', sealed }
      },
      rows,
    }
  }


export const makeD1EcommerceCampaignReceiptStore = (
  db: D1Database,
  nowIso: () => string,
): EcommerceCampaignReceiptStore => ({
  get: async receiptRef => {
    const row = await db
      .prepare(
        `SELECT receipt_ref, serialized_json
           FROM ecommerce_campaign_receipts
          WHERE receipt_ref = ?
          LIMIT 1`,
      )
      .bind(receiptRef)
      .first<{ receipt_ref: string; serialized_json: string }>()

    if (row === null) return undefined

    return {
      document: decodeEcommerceCampaignDeliveryReceiptDocument(row.serialized_json),
      receiptRef: row.receipt_ref,
      serialized: row.serialized_json,
    }
  },
  list: async () => {
    const result = await db
      .prepare(
        `SELECT receipt_ref, serialized_json
           FROM ecommerce_campaign_receipts
          ORDER BY created_at ASC, rowid ASC`,
      )
      .all<{ receipt_ref: string; serialized_json: string }>()

    return (result.results ?? []).map(row => ({
      document: decodeEcommerceCampaignDeliveryReceiptDocument(row.serialized_json),
      receiptRef: row.receipt_ref,
      serialized: row.serialized_json,
    }))
  },
  put: async (document, receiptRef) => {
    const serialized = serializeEcommerceCampaignDeliveryReceiptDocument(document)

    const inserted = await db
      .prepare(
        `INSERT OR IGNORE INTO ecommerce_campaign_receipts
           (receipt_ref, serialized_json, created_at)
         VALUES (?, ?, ?)`,
      )
      .bind(receiptRef, serialized, nowIso())
      .run()

    const sealed: EcommerceCampaignSealedReceipt = { document, receiptRef, serialized }
    return (inserted.meta?.changes ?? 0) > 0
      ? { kind: 'stored', sealed }
      : { kind: 'already_stored', sealed }
  },
})
