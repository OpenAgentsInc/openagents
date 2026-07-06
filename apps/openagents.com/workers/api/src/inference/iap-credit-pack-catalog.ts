// Server-owned SKU -> credit-amount catalog for the IAP credit-pack rail
// (MM-E2, #8482). The webhook handler NEVER trusts a client- or
// RevenueCat-payload-supplied amount for the credit grant — it looks up the
// amount by SKU here. Store product prices (App Store Connect / Play
// Console) must be kept in sync with this table by whoever configures them
// (see NEEDS_OWNER.md's #8481 entry) — a SKU with no catalog entry is
// refused, never guessed.
//
// Pack pricing/SKU ids here are a REASONABLE DEFAULT that will very likely
// need adjustment once real store products exist (#8481); this table is the
// single place to update, per-SKU, once real prices are set.

export type IapCreditPack = Readonly<{
  sku: string
  amountUsdCents: number
}>

export const IAP_CREDIT_PACK_CATALOG: ReadonlyArray<IapCreditPack> = [
  { amountUsdCents: 499, sku: 'credits_499' },
  { amountUsdCents: 999, sku: 'credits_999' },
  { amountUsdCents: 1999, sku: 'credits_1999' },
]

const CATALOG_BY_SKU: ReadonlyMap<string, IapCreditPack> = new Map(
  IAP_CREDIT_PACK_CATALOG.map(pack => [pack.sku, pack]),
)

/** The ONLY place a SKU resolves to a dollar amount. Returns `undefined` for
 * any SKU not in the catalog (a subscription product id, a typo, or a store
 * product this rail doesn't recognize) — the caller must refuse the
 * fulfillment rather than fall back to a client/payload-supplied amount. */
export const iapCreditPackFromSku = (sku: string): IapCreditPack | undefined =>
  CATALOG_BY_SKU.get(sku)
