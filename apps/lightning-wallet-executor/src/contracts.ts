import { Schema } from "effect"

import {
  InvoicePaymentRequest as InvoicePaymentRequestSchema,
  InvoicePaymentResult as InvoicePaymentResultSchema,
  Msats as MsatsSchema,
} from "@openagentsinc/lightning-effect/contracts"

export const InvoicePaymentRequest = InvoicePaymentRequestSchema
export type InvoicePaymentRequest = typeof InvoicePaymentRequest.Type

export const InvoicePaymentResult = InvoicePaymentResultSchema
export type InvoicePaymentResult = typeof InvoicePaymentResult.Type

export const Msats = MsatsSchema
export type Msats = typeof Msats.Type

export const ExecutorLifecycle = Schema.Literal("disconnected", "connecting", "connected", "error")
export type ExecutorLifecycle = typeof ExecutorLifecycle.Type

export const ExecutorMode = Schema.Literal("mock", "spark")
export type ExecutorMode = typeof ExecutorMode.Type

export const SparkNetwork = Schema.Literal("mainnet", "regtest")
export type SparkNetwork = typeof SparkNetwork.Type

export const WalletStatus = Schema.Struct({
  walletId: Schema.NonEmptyString,
  mode: ExecutorMode,
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
export const decodePayBolt11HttpRequest = Schema.decodeUnknown(PayBolt11HttpRequest)

export const PayBolt11Response = Schema.Struct({
  requestId: Schema.NonEmptyString,
  walletId: Schema.NonEmptyString,
  payment: InvoicePaymentResult,
  quotedAmountMsats: Msats,
  windowSpendMsatsAfterPayment: Msats,
})
export type PayBolt11Response = typeof PayBolt11Response.Type

export const ErrorResponse = Schema.Struct({
  requestId: Schema.NonEmptyString,
  code: Schema.NonEmptyString,
  message: Schema.NonEmptyString,
  details: Schema.optional(Schema.Record({ key: Schema.String, value: Schema.Unknown })),
})
export type ErrorResponse = typeof ErrorResponse.Type

