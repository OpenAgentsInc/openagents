import { Schema } from "effect"

export const Msats = Schema.Int.pipe(Schema.nonNegative())
export type Msats = typeof Msats.Type

export const InvoicePaymentRequest = Schema.Struct({
  invoice: Schema.NonEmptyString,
  maxAmountMsats: Msats,
  host: Schema.NonEmptyString,
})
export type InvoicePaymentRequest = typeof InvoicePaymentRequest.Type
export const decodeInvoicePaymentRequest = Schema.decodeUnknown(InvoicePaymentRequest)
export const decodeInvoicePaymentRequestSync = Schema.decodeUnknownSync(InvoicePaymentRequest)
export const encodeInvoicePaymentRequest = Schema.encode(InvoicePaymentRequest)
export const encodeInvoicePaymentRequestSync = Schema.encodeSync(InvoicePaymentRequest)

export const InvoicePaymentResult = Schema.Struct({
  paymentId: Schema.NonEmptyString,
  amountMsats: Msats,
  preimageHex: Schema.NonEmptyString,
  paidAtMs: Schema.Int.pipe(Schema.nonNegative()),
})
export type InvoicePaymentResult = typeof InvoicePaymentResult.Type
export const decodeInvoicePaymentResult = Schema.decodeUnknown(InvoicePaymentResult)
export const decodeInvoicePaymentResultSync = Schema.decodeUnknownSync(InvoicePaymentResult)
export const encodeInvoicePaymentResult = Schema.encode(InvoicePaymentResult)
export const encodeInvoicePaymentResultSync = Schema.encodeSync(InvoicePaymentResult)
