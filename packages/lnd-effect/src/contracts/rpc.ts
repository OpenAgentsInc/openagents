import { Effect, Schema } from "effect"

import { LndContractDecodeError } from "../errors/lndErrors.js"

const TimestampMs = Schema.Int.pipe(Schema.nonNegative())
const SatAmount = Schema.Int.pipe(Schema.nonNegative())

const decodeWithTypedError = <A>(
  contract: string,
  schema: Schema.Schema<A>,
  input: unknown,
): Effect.Effect<A, LndContractDecodeError> =>
  Schema.decodeUnknown(schema)(input).pipe(
    Effect.mapError((error) =>
      LndContractDecodeError.make({
        contract,
        reason: String(error),
      }),
    ),
  )

const decodeWithTypedErrorSync = <A>(contract: string, schema: Schema.Schema<A>, input: unknown): A => {
  try {
    return Schema.decodeUnknownSync(schema)(input)
  } catch (error) {
    throw LndContractDecodeError.make({
      contract,
      reason: String(error),
    })
  }
}

export const LndHttpMethod = Schema.Literal("GET", "POST", "PUT", "PATCH", "DELETE")
export type LndHttpMethod = typeof LndHttpMethod.Type

export const LndRpcRequest = Schema.Struct({
  method: LndHttpMethod,
  path: Schema.NonEmptyString,
  query: Schema.optional(Schema.Record({ key: Schema.String, value: Schema.String })),
  body: Schema.optional(Schema.Unknown),
})
export type LndRpcRequest = typeof LndRpcRequest.Type

export const LndRpcResponse = Schema.Struct({
  status: Schema.Int,
  headers: Schema.optional(Schema.Record({ key: Schema.String, value: Schema.String })),
  body: Schema.optional(Schema.Unknown),
})
export type LndRpcResponse = typeof LndRpcResponse.Type

export const LndInvoiceCreateRequest = Schema.Struct({
  amountSat: SatAmount,
  memo: Schema.optional(Schema.String),
  expirySeconds: Schema.optional(Schema.Int.pipe(Schema.positive())),
})
export type LndInvoiceCreateRequest = typeof LndInvoiceCreateRequest.Type

export const LndInvoiceLookupRequest = Schema.Struct({
  paymentRequest: Schema.NonEmptyString,
})
export type LndInvoiceLookupRequest = typeof LndInvoiceLookupRequest.Type

export const LndInvoiceRecord = Schema.Struct({
  paymentRequest: Schema.NonEmptyString,
  rHash: Schema.NonEmptyString,
  amountSat: SatAmount,
  settled: Schema.Boolean,
  createdAtMs: TimestampMs,
  settledAtMs: Schema.optional(TimestampMs),
})
export type LndInvoiceRecord = typeof LndInvoiceRecord.Type

export const LndInvoiceListResult = Schema.Struct({
  invoices: Schema.Array(LndInvoiceRecord),
  nextOffset: Schema.optional(Schema.Int.pipe(Schema.nonNegative())),
})
export type LndInvoiceListResult = typeof LndInvoiceListResult.Type

export const LndPaymentSendRequest = Schema.Struct({
  paymentRequest: Schema.NonEmptyString,
  feeLimitSat: Schema.optional(SatAmount),
  timeoutSeconds: Schema.optional(Schema.Int.pipe(Schema.positive())),
})
export type LndPaymentSendRequest = typeof LndPaymentSendRequest.Type

export const LndPaymentTrackRequest = Schema.Struct({
  paymentHash: Schema.NonEmptyString,
})
export type LndPaymentTrackRequest = typeof LndPaymentTrackRequest.Type

export const LndPaymentRecord = Schema.Struct({
  paymentHash: Schema.NonEmptyString,
  paymentPreimageHex: Schema.optional(Schema.NonEmptyString),
  amountSat: SatAmount,
  feeSat: SatAmount,
  status: Schema.Literal("in_flight", "succeeded", "failed"),
  failureReason: Schema.optional(Schema.String),
  createdAtMs: TimestampMs,
  updatedAtMs: TimestampMs,
})
export type LndPaymentRecord = typeof LndPaymentRecord.Type

export const LndPaymentListResult = Schema.Struct({
  payments: Schema.Array(LndPaymentRecord),
  nextOffset: Schema.optional(Schema.Int.pipe(Schema.nonNegative())),
})
export type LndPaymentListResult = typeof LndPaymentListResult.Type

export const decodeLndRpcRequest = (input: unknown) => decodeWithTypedError("LndRpcRequest", LndRpcRequest, input)
export const decodeLndRpcRequestSync = (input: unknown) =>
  decodeWithTypedErrorSync("LndRpcRequest", LndRpcRequest, input)
export const encodeLndRpcRequest = Schema.encode(LndRpcRequest)
export const encodeLndRpcRequestSync = Schema.encodeSync(LndRpcRequest)

export const decodeLndRpcResponse = (input: unknown) =>
  decodeWithTypedError("LndRpcResponse", LndRpcResponse, input)
export const decodeLndRpcResponseSync = (input: unknown) =>
  decodeWithTypedErrorSync("LndRpcResponse", LndRpcResponse, input)
export const encodeLndRpcResponse = Schema.encode(LndRpcResponse)
export const encodeLndRpcResponseSync = Schema.encodeSync(LndRpcResponse)

export const decodeLndInvoiceCreateRequest = (input: unknown) =>
  decodeWithTypedError("LndInvoiceCreateRequest", LndInvoiceCreateRequest, input)
export const decodeLndInvoiceCreateRequestSync = (input: unknown) =>
  decodeWithTypedErrorSync("LndInvoiceCreateRequest", LndInvoiceCreateRequest, input)
export const encodeLndInvoiceCreateRequest = Schema.encode(LndInvoiceCreateRequest)
export const encodeLndInvoiceCreateRequestSync = Schema.encodeSync(LndInvoiceCreateRequest)

export const decodeLndInvoiceLookupRequest = (input: unknown) =>
  decodeWithTypedError("LndInvoiceLookupRequest", LndInvoiceLookupRequest, input)
export const decodeLndInvoiceLookupRequestSync = (input: unknown) =>
  decodeWithTypedErrorSync("LndInvoiceLookupRequest", LndInvoiceLookupRequest, input)
export const encodeLndInvoiceLookupRequest = Schema.encode(LndInvoiceLookupRequest)
export const encodeLndInvoiceLookupRequestSync = Schema.encodeSync(LndInvoiceLookupRequest)

export const decodeLndInvoiceRecord = (input: unknown) =>
  decodeWithTypedError("LndInvoiceRecord", LndInvoiceRecord, input)
export const decodeLndInvoiceRecordSync = (input: unknown) =>
  decodeWithTypedErrorSync("LndInvoiceRecord", LndInvoiceRecord, input)
export const encodeLndInvoiceRecord = Schema.encode(LndInvoiceRecord)
export const encodeLndInvoiceRecordSync = Schema.encodeSync(LndInvoiceRecord)

export const decodeLndInvoiceListResult = (input: unknown) =>
  decodeWithTypedError("LndInvoiceListResult", LndInvoiceListResult, input)
export const decodeLndInvoiceListResultSync = (input: unknown) =>
  decodeWithTypedErrorSync("LndInvoiceListResult", LndInvoiceListResult, input)
export const encodeLndInvoiceListResult = Schema.encode(LndInvoiceListResult)
export const encodeLndInvoiceListResultSync = Schema.encodeSync(LndInvoiceListResult)

export const decodeLndPaymentSendRequest = (input: unknown) =>
  decodeWithTypedError("LndPaymentSendRequest", LndPaymentSendRequest, input)
export const decodeLndPaymentSendRequestSync = (input: unknown) =>
  decodeWithTypedErrorSync("LndPaymentSendRequest", LndPaymentSendRequest, input)
export const encodeLndPaymentSendRequest = Schema.encode(LndPaymentSendRequest)
export const encodeLndPaymentSendRequestSync = Schema.encodeSync(LndPaymentSendRequest)

export const decodeLndPaymentTrackRequest = (input: unknown) =>
  decodeWithTypedError("LndPaymentTrackRequest", LndPaymentTrackRequest, input)
export const decodeLndPaymentTrackRequestSync = (input: unknown) =>
  decodeWithTypedErrorSync("LndPaymentTrackRequest", LndPaymentTrackRequest, input)
export const encodeLndPaymentTrackRequest = Schema.encode(LndPaymentTrackRequest)
export const encodeLndPaymentTrackRequestSync = Schema.encodeSync(LndPaymentTrackRequest)

export const decodeLndPaymentRecord = (input: unknown) =>
  decodeWithTypedError("LndPaymentRecord", LndPaymentRecord, input)
export const decodeLndPaymentRecordSync = (input: unknown) =>
  decodeWithTypedErrorSync("LndPaymentRecord", LndPaymentRecord, input)
export const encodeLndPaymentRecord = Schema.encode(LndPaymentRecord)
export const encodeLndPaymentRecordSync = Schema.encodeSync(LndPaymentRecord)

export const decodeLndPaymentListResult = (input: unknown) =>
  decodeWithTypedError("LndPaymentListResult", LndPaymentListResult, input)
export const decodeLndPaymentListResultSync = (input: unknown) =>
  decodeWithTypedErrorSync("LndPaymentListResult", LndPaymentListResult, input)
export const encodeLndPaymentListResult = Schema.encode(LndPaymentListResult)
export const encodeLndPaymentListResultSync = Schema.encodeSync(LndPaymentListResult)
