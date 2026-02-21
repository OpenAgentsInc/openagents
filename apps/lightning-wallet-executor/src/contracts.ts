import { Schema } from "effect"

export const Msats = Schema.Int.pipe(Schema.nonNegative())
export type Msats = typeof Msats.Type

export const InvoicePaymentRequest = Schema.Struct({
  invoice: Schema.NonEmptyString,
  maxAmountMsats: Msats.pipe(Schema.positive()),
  host: Schema.NonEmptyString,
})
export type InvoicePaymentRequest = typeof InvoicePaymentRequest.Type

export const InvoicePaymentResult = Schema.Struct({
  paymentId: Schema.NonEmptyString,
  amountMsats: Msats.pipe(Schema.positive()),
  preimageHex: Schema.NonEmptyString,
  paidAtMs: Schema.Int.pipe(Schema.nonNegative()),
})
export type InvoicePaymentResult = typeof InvoicePaymentResult.Type

export const ExecutorLifecycle = Schema.Literal("disconnected", "connecting", "connected", "error")
export type ExecutorLifecycle = typeof ExecutorLifecycle.Type

export const ExecutorMode = Schema.Literal("mock", "spark")
export type ExecutorMode = typeof ExecutorMode.Type
export const WalletExecutorAuthMode = Schema.Literal("disabled", "bearer_static")
export type WalletExecutorAuthMode = typeof WalletExecutorAuthMode.Type

export const SparkNetwork = Schema.Literal("mainnet", "regtest")
export type SparkNetwork = typeof SparkNetwork.Type

export const WalletExecutionRail = Schema.Literal("lightning")
export type WalletExecutionRail = typeof WalletExecutionRail.Type

export const WalletExecutionReceipt = Schema.Struct({
  receiptVersion: Schema.Literal("openagents.lightning.wallet_receipt.v1"),
  receiptId: Schema.NonEmptyString,
  requestId: Schema.NonEmptyString,
  walletId: Schema.NonEmptyString,
  host: Schema.NonEmptyString,
  paymentId: Schema.NonEmptyString,
  invoiceHash: Schema.NonEmptyString,
  quotedAmountMsats: Msats,
  settledAmountMsats: Msats,
  preimageSha256: Schema.NonEmptyString,
  paidAtMs: Schema.Int.pipe(Schema.nonNegative()),
  rail: WalletExecutionRail,
  assetId: Schema.Literal("BTC_LN"),
  canonicalJsonSha256: Schema.NonEmptyString,
})
export type WalletExecutionReceipt = typeof WalletExecutionReceipt.Type

export const WalletStatus = Schema.Struct({
  walletId: Schema.NonEmptyString,
  mode: ExecutorMode,
  authMode: WalletExecutorAuthMode,
  authEnforced: Schema.Boolean,
  authTokenVersion: Schema.Int.pipe(Schema.positive()),
  lifecycle: ExecutorLifecycle,
  network: SparkNetwork,
  identityPubkey: Schema.NullOr(Schema.String),
  balanceSats: Schema.NullOr(Schema.Int.pipe(Schema.nonNegative())),
  apiKeyConfigured: Schema.Boolean,
  ready: Schema.Boolean,
  allowedHostCount: Schema.Int.pipe(Schema.nonNegative()),
  requestCapMsats: Msats,
  windowCapMsats: Msats,
  windowMs: Schema.Int.pipe(Schema.positive()),
  recentPaymentsCount: Schema.Int.pipe(Schema.nonNegative()),
  lastPaymentId: Schema.NullOr(Schema.String),
  lastPaymentAtMs: Schema.NullOr(Schema.Int.pipe(Schema.nonNegative())),
  lastErrorCode: Schema.NullOr(Schema.String),
  lastErrorMessage: Schema.NullOr(Schema.String),
  updatedAtMs: Schema.Int.pipe(Schema.nonNegative()),
})
export type WalletStatus = typeof WalletStatus.Type

export const PayBolt11HttpRequest = Schema.Struct({
  requestId: Schema.optional(Schema.NonEmptyString),
  payment: InvoicePaymentRequest,
})
export type PayBolt11HttpRequest = typeof PayBolt11HttpRequest.Type
export const decodePayBolt11HttpRequest = (input: unknown): PayBolt11HttpRequest =>
  Schema.decodeUnknownSync(PayBolt11HttpRequest)(input)

export const PayBolt11Response = Schema.Struct({
  requestId: Schema.NonEmptyString,
  walletId: Schema.NonEmptyString,
  payment: InvoicePaymentResult,
  quotedAmountMsats: Msats,
  windowSpendMsatsAfterPayment: Msats,
  receipt: WalletExecutionReceipt,
})
export type PayBolt11Response = typeof PayBolt11Response.Type

export const ErrorResponse = Schema.Struct({
  requestId: Schema.NonEmptyString,
  code: Schema.NonEmptyString,
  message: Schema.NonEmptyString,
  details: Schema.optional(Schema.Record({ key: Schema.String, value: Schema.Unknown })),
})
export type ErrorResponse = typeof ErrorResponse.Type
