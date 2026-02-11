import { Schema } from "effect"

export const Msats = Schema.Int.pipe(Schema.nonNegative())
export type Msats = typeof Msats.Type

export const InvoicePaymentRequest = Schema.Struct({
  invoice: Schema.NonEmptyString,
  maxAmountMsats: Msats,
  host: Schema.NonEmptyString,
})
export type InvoicePaymentRequest = typeof InvoicePaymentRequest.Type

export const InvoicePaymentResult = Schema.Struct({
  paymentId: Schema.NonEmptyString,
  amountMsats: Msats,
  preimageHex: Schema.NonEmptyString,
  paidAtMs: Schema.Int.pipe(Schema.nonNegative()),
})
export type InvoicePaymentResult = typeof InvoicePaymentResult.Type
