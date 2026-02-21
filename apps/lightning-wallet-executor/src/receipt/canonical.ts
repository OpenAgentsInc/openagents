import crypto from "node:crypto"

const RECEIPT_VERSION = "openagents.lightning.wallet_receipt.v1" as const
const RECEIPT_RAIL = "lightning" as const
const RECEIPT_ASSET_ID = "BTC_LN" as const

export type WalletExecutionReceiptInput = Readonly<{
  requestId: string
  walletId: string
  host: string
  paymentId: string
  invoiceHash: string
  quotedAmountMsats: number
  settledAmountMsats: number
  preimageHex: string
  paidAtMs: number
}>

export type WalletExecutionReceipt = Readonly<{
  receiptVersion: typeof RECEIPT_VERSION
  receiptId: string
  requestId: string
  walletId: string
  host: string
  paymentId: string
  invoiceHash: string
  quotedAmountMsats: number
  settledAmountMsats: number
  preimageSha256: string
  paidAtMs: number
  rail: typeof RECEIPT_RAIL
  assetId: typeof RECEIPT_ASSET_ID
  canonicalJsonSha256: string
}>

type CanonicalPayload = Readonly<{
  receiptVersion: typeof RECEIPT_VERSION
  requestId: string
  walletId: string
  host: string
  paymentId: string
  invoiceHash: string
  quotedAmountMsats: number
  settledAmountMsats: number
  preimageSha256: string
  paidAtMs: number
  rail: typeof RECEIPT_RAIL
  assetId: typeof RECEIPT_ASSET_ID
}>

const normalizeHost = (host: string): string => host.trim().toLowerCase()

const sha256Hex = (value: string): string =>
  crypto.createHash("sha256").update(value, "utf8").digest("hex")

const toCanonicalPayload = (
  input: WalletExecutionReceiptInput,
): CanonicalPayload => ({
  receiptVersion: RECEIPT_VERSION,
  requestId: input.requestId.trim(),
  walletId: input.walletId.trim(),
  host: normalizeHost(input.host),
  paymentId: input.paymentId.trim(),
  invoiceHash: input.invoiceHash.trim().toLowerCase(),
  quotedAmountMsats: Math.max(0, Math.floor(input.quotedAmountMsats)),
  settledAmountMsats: Math.max(0, Math.floor(input.settledAmountMsats)),
  preimageSha256: sha256Hex(input.preimageHex.trim().toLowerCase()),
  paidAtMs: Math.max(0, Math.floor(input.paidAtMs)),
  rail: RECEIPT_RAIL,
  assetId: RECEIPT_ASSET_ID,
})

export const canonicalizeWalletExecutionReceipt = (
  input: WalletExecutionReceiptInput,
): string => JSON.stringify(toCanonicalPayload(input))

export const canonicalWalletExecutionReceiptHash = (
  input: WalletExecutionReceiptInput,
): string => sha256Hex(canonicalizeWalletExecutionReceipt(input))

export const buildWalletExecutionReceipt = (
  input: WalletExecutionReceiptInput,
): WalletExecutionReceipt => {
  const canonicalPayload = toCanonicalPayload(input)
  const canonicalJsonSha256 = sha256Hex(JSON.stringify(canonicalPayload))

  return {
    ...canonicalPayload,
    receiptId: `lwr_${canonicalJsonSha256.slice(0, 24)}`,
    canonicalJsonSha256,
  }
}
