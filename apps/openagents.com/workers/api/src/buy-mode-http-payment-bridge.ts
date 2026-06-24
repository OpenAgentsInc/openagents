/**
 * Default-off HTTP payment bridge for buy-mode settlement.
 *
 * The Worker should not host wallet runtime directly. This bridge delegates
 * invoice payment to an operator-controlled service and accepts only
 * public-safe receipt refs back.
 */
import type {
  BuyModePaymentBridge,
  BuyModePaymentBridgeReceipt,
} from './buy-mode-dispatcher'
import { parseJsonUnknown } from './json-boundary'

type HttpPaymentBridgeOptions = Readonly<{
  endpoint: string
  fetch?: typeof fetch
  token: string
}>

class BuyModeHttpPaymentBridgeError extends Error {
  readonly _tag = 'BuyModeHttpPaymentBridgeError'
}

const stableReceiptRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const unsafeReceiptRefPattern =
  /(@|bolt11|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(invoice|payment|wallet)|secret|token|wallet)/i

const validPublicRef = (value: unknown): value is string =>
  typeof value === 'string' &&
  stableReceiptRefPattern.test(value) &&
  !unsafeReceiptRefPattern.test(value)

const decodeReceipt = (body: unknown): BuyModePaymentBridgeReceipt => {
  if (typeof body !== 'object' || body === null) {
    throw new BuyModeHttpPaymentBridgeError(
      'buy-mode payment bridge returned invalid receipt',
    )
  }
  const record = body as {
    receipt_ref?: unknown
    receiptRef?: unknown
    settlement_ref?: unknown
    settlementRef?: unknown
  }
  const receiptRef = record.receiptRef ?? record.receipt_ref
  const settlementRef = record.settlementRef ?? record.settlement_ref

  if (!validPublicRef(receiptRef) || !validPublicRef(settlementRef)) {
    throw new BuyModeHttpPaymentBridgeError(
      'buy-mode payment bridge returned unsafe receipt refs',
    )
  }

  return { receiptRef, settlementRef }
}

export const makeHttpBuyModePaymentBridge = (
  options: HttpPaymentBridgeOptions,
): BuyModePaymentBridge => {
  const fetchFn = options.fetch ?? fetch

  return {
    payBolt11: async input => {
      const response = await fetchFn(options.endpoint, {
        body: JSON.stringify({
          amount_msats: input.amountMsats,
          bolt11: input.bolt11,
          idempotency_ref: input.idempotencyRef,
          provider_pubkey: input.providerPubkey,
          result_event_id: input.resultEventId,
        }),
        headers: {
          authorization: `Bearer ${options.token}`,
          'content-type': 'application/json',
        },
        method: 'POST',
      })

      if (!response.ok) {
        throw new BuyModeHttpPaymentBridgeError(
          `buy-mode payment bridge failed with ${response.status}`,
        )
      }

      return decodeReceipt(parseJsonUnknown(await response.text()))
    },
  }
}

export const buyModePaymentBridgeForEnv = (
  environment: unknown,
): BuyModePaymentBridge | undefined => {
  const env = environment as {
    BUY_MODE_PAYMENT_BRIDGE_TOKEN?: string
    BUY_MODE_PAYMENT_BRIDGE_URL?: string
  }

  if (
    typeof env.BUY_MODE_PAYMENT_BRIDGE_URL !== 'string' ||
    env.BUY_MODE_PAYMENT_BRIDGE_URL.length === 0 ||
    typeof env.BUY_MODE_PAYMENT_BRIDGE_TOKEN !== 'string' ||
    env.BUY_MODE_PAYMENT_BRIDGE_TOKEN.length === 0
  ) {
    return undefined
  }

  return makeHttpBuyModePaymentBridge({
    endpoint: env.BUY_MODE_PAYMENT_BRIDGE_URL,
    token: env.BUY_MODE_PAYMENT_BRIDGE_TOKEN,
  })
}
