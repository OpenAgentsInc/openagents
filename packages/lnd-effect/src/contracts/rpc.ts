import { Schema } from "effect"

export const LndHttpMethod = Schema.Literal("GET", "POST", "PUT", "PATCH", "DELETE")
export type LndHttpMethod = typeof LndHttpMethod.Type

export const LndRpcRequest = Schema.Struct({
  method: LndHttpMethod,
  path: Schema.NonEmptyString,
  body: Schema.optional(Schema.Unknown),
})
export type LndRpcRequest = typeof LndRpcRequest.Type

export const LndRpcResponse = Schema.Struct({
  status: Schema.Int,
  body: Schema.optional(Schema.Unknown),
})
export type LndRpcResponse = typeof LndRpcResponse.Type

export const LndInvoice = Schema.Struct({
  invoice: Schema.NonEmptyString,
  amountSat: Schema.Int.pipe(Schema.nonNegative()),
  createdAtMs: Schema.Int.pipe(Schema.nonNegative()),
})
export type LndInvoice = typeof LndInvoice.Type

export const LndPayment = Schema.Struct({
  paymentHash: Schema.NonEmptyString,
  amountSat: Schema.Int.pipe(Schema.nonNegative()),
  status: Schema.Literal("in_flight", "succeeded", "failed"),
  preimageHex: Schema.optional(Schema.NonEmptyString),
  updatedAtMs: Schema.Int.pipe(Schema.nonNegative()),
})
export type LndPayment = typeof LndPayment.Type

export const decodeLndRpcRequest = Schema.decodeUnknown(LndRpcRequest)
export const decodeLndRpcResponse = Schema.decodeUnknown(LndRpcResponse)
