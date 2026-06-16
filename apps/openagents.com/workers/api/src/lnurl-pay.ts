// LNURL-pay resolution for paying a lud16 Lightning Address (#5078).
//
// Our treasury is an MDK/Lightning wallet that pays BOLT11/BOLT12 — it does not
// resolve a Lightning Address (name@domain) on its own. A Spark-hosted Lightning
// Address (the offline-receive rail) is exactly such an address. This module
// runs the standard LNURL-pay flow to turn an address + amount into a payable
// BOLT11, so the existing Lightning send path can pay it (and the recipient's
// LSP can hold it while they're offline).
//
// Result-typed (never throws) so callers stay inside the worker error budget.

// lud16: name@domain.tld. Lowercased before matching.
const lightningAddressPattern =
  /^[a-z0-9._+-]{1,64}@[a-z0-9.-]{1,190}\.[a-z]{2,63}$/u

export const isLightningAddress = (value: string): boolean =>
  lightningAddressPattern.test(value.trim().toLowerCase())

export type LightningAddressResolution =
  | { readonly ok: true; readonly bolt11: string }
  | { readonly ok: false; readonly reason: string }

const asRecord = (value: unknown): Record<string, unknown> | null =>
  typeof value === 'object' && value !== null
    ? (value as Record<string, unknown>)
    : null

/**
 * Resolve a Lightning Address to a BOLT11 invoice for `amountSat` via LNURL-pay:
 * GET https://<domain>/.well-known/lnurlp/<name> (payRequest metadata) then the
 * `callback?amount=<msat>` to mint the invoice. No raw invoice/address is logged;
 * the caller treats the returned bolt11 as payment material.
 */
export const resolveLightningAddressInvoice = async (
  address: string,
  amountSat: number,
  fetchFn: typeof fetch = fetch,
): Promise<LightningAddressResolution> => {
  const trimmed = address.trim()
  const at = trimmed.indexOf('@')
  if (at <= 0 || at === trimmed.length - 1) {
    return { ok: false, reason: 'not_a_lightning_address' }
  }
  if (!Number.isInteger(amountSat) || amountSat <= 0) {
    return { ok: false, reason: 'invalid_amount' }
  }
  const name = trimmed.slice(0, at)
  const domain = trimmed.slice(at + 1)
  const metaUrl = `https://${domain}/.well-known/lnurlp/${encodeURIComponent(name)}`

  let meta: Record<string, unknown> | null
  try {
    const response = await fetchFn(metaUrl, {
      headers: { accept: 'application/json' },
    })
    if (!response.ok) {
      return { ok: false, reason: `lnurlp_meta_http_${response.status}` }
    }
    meta = asRecord(await response.json())
  } catch {
    return { ok: false, reason: 'lnurlp_meta_fetch_failed' }
  }
  if (meta === null || meta.tag !== 'payRequest') {
    return { ok: false, reason: 'lnurlp_meta_not_pay_request' }
  }
  const callback = typeof meta.callback === 'string' ? meta.callback : ''
  if (callback === '' || !/^https:\/\//u.test(callback)) {
    return { ok: false, reason: 'lnurlp_meta_callback_invalid' }
  }

  const amountMsat = amountSat * 1000
  const minSendable = Number(meta.minSendable ?? 0)
  const maxSendable = Number(meta.maxSendable ?? Number.MAX_SAFE_INTEGER)
  if (
    Number.isFinite(minSendable) &&
    Number.isFinite(maxSendable) &&
    (amountMsat < minSendable || amountMsat > maxSendable)
  ) {
    return {
      ok: false,
      reason: `amount_out_of_range_${minSendable}_${maxSendable}_msat`,
    }
  }

  let callbackUrl: URL
  try {
    callbackUrl = new URL(callback)
  } catch {
    return { ok: false, reason: 'lnurlp_callback_unparseable' }
  }
  callbackUrl.searchParams.set('amount', String(amountMsat))

  let invoice: Record<string, unknown> | null
  try {
    const response = await fetchFn(callbackUrl.toString(), {
      headers: { accept: 'application/json' },
    })
    if (!response.ok) {
      return { ok: false, reason: `lnurlp_callback_http_${response.status}` }
    }
    invoice = asRecord(await response.json())
  } catch {
    return { ok: false, reason: 'lnurlp_callback_fetch_failed' }
  }
  if (invoice === null) {
    return { ok: false, reason: 'lnurlp_callback_not_json' }
  }
  if (invoice.status === 'ERROR') {
    return { ok: false, reason: 'lnurlp_callback_error' }
  }
  const pr = typeof invoice.pr === 'string' ? invoice.pr.trim() : ''
  if (pr === '' || !/^ln[a-z0-9]/iu.test(pr)) {
    return { ok: false, reason: 'lnurlp_callback_no_invoice' }
  }
  return { ok: true, bolt11: pr }
}
