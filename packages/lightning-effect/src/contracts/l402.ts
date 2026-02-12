import { Schema } from "effect"

import { Msats } from "./payment.js"

export const HttpMethod = Schema.Literal("GET", "POST", "PUT", "PATCH", "DELETE")
export type HttpMethod = typeof HttpMethod.Type

export const HeaderRecord = Schema.Record({
  key: Schema.String,
  value: Schema.String,
})
export type HeaderRecord = typeof HeaderRecord.Type

export const L402Challenge = Schema.Struct({
  invoice: Schema.NonEmptyString,
  macaroon: Schema.NonEmptyString,
  amountMsats: Schema.optional(Msats),
  issuer: Schema.optional(Schema.NonEmptyString),
})
export type L402Challenge = typeof L402Challenge.Type
export const decodeL402Challenge = Schema.decodeUnknown(L402Challenge)
export const decodeL402ChallengeSync = Schema.decodeUnknownSync(L402Challenge)
export const encodeL402Challenge = Schema.encode(L402Challenge)
export const encodeL402ChallengeSync = Schema.encodeSync(L402Challenge)

export const L402Credential = Schema.Struct({
  host: Schema.NonEmptyString,
  scope: Schema.optional(Schema.NonEmptyString),
  macaroon: Schema.NonEmptyString,
  preimageHex: Schema.NonEmptyString,
  amountMsats: Msats,
  issuedAtMs: Schema.Int.pipe(Schema.nonNegative()),
})
export type L402Credential = typeof L402Credential.Type
export const decodeL402Credential = Schema.decodeUnknown(L402Credential)
export const decodeL402CredentialSync = Schema.decodeUnknownSync(L402Credential)
export const encodeL402Credential = Schema.encode(L402Credential)
export const encodeL402CredentialSync = Schema.encodeSync(L402Credential)

export const L402FetchRequest = Schema.Struct({
  url: Schema.NonEmptyString,
  method: Schema.optional(HttpMethod),
  headers: Schema.optional(HeaderRecord),
  body: Schema.optional(Schema.String),
  maxSpendMsats: Msats,
  challengeHeader: Schema.optional(Schema.NonEmptyString),
  forceRefresh: Schema.optional(Schema.Boolean),
  scope: Schema.optional(Schema.NonEmptyString),
  cacheTtlMs: Schema.optional(Schema.Int.pipe(Schema.nonNegative())),
})
export type L402FetchRequest = typeof L402FetchRequest.Type
export const decodeL402FetchRequest = Schema.decodeUnknown(L402FetchRequest)
export const decodeL402FetchRequestSync = Schema.decodeUnknownSync(L402FetchRequest)
export const encodeL402FetchRequest = Schema.encode(L402FetchRequest)
export const encodeL402FetchRequestSync = Schema.encodeSync(L402FetchRequest)

export const L402CacheStatus = Schema.Literal("miss", "hit", "stale", "invalid")
export type L402CacheStatus = typeof L402CacheStatus.Type

export const L402FetchResult = Schema.Struct({
  url: Schema.NonEmptyString,
  host: Schema.NonEmptyString,
  scope: Schema.NonEmptyString,
  statusCode: Schema.Int,
  authorizationHeader: Schema.NullOr(Schema.NonEmptyString),
  cacheStatus: L402CacheStatus,
  fromCache: Schema.Boolean,
  paid: Schema.Boolean,
  amountMsats: Msats,
  paymentId: Schema.NullOr(Schema.NonEmptyString),
  proofReference: Schema.NonEmptyString,
  responseBody: Schema.optional(Schema.String),
  responseContentType: Schema.optional(Schema.String),
})
export type L402FetchResult = typeof L402FetchResult.Type
export const decodeL402FetchResult = Schema.decodeUnknown(L402FetchResult)
export const decodeL402FetchResultSync = Schema.decodeUnknownSync(L402FetchResult)
export const encodeL402FetchResult = Schema.encode(L402FetchResult)
export const encodeL402FetchResultSync = Schema.encodeSync(L402FetchResult)

export const L402TransportRequest = Schema.Struct({
  url: Schema.NonEmptyString,
  method: Schema.optional(HttpMethod),
  headers: Schema.optional(HeaderRecord),
  body: Schema.optional(Schema.String),
})
export type L402TransportRequest = typeof L402TransportRequest.Type
export const decodeL402TransportRequest = Schema.decodeUnknown(L402TransportRequest)
export const decodeL402TransportRequestSync = Schema.decodeUnknownSync(L402TransportRequest)
export const encodeL402TransportRequest = Schema.encode(L402TransportRequest)
export const encodeL402TransportRequestSync = Schema.encodeSync(L402TransportRequest)

export const L402TransportResponse = Schema.Struct({
  status: Schema.Int,
  headers: HeaderRecord,
  body: Schema.optional(Schema.String),
})
export type L402TransportResponse = typeof L402TransportResponse.Type
export const decodeL402TransportResponse = Schema.decodeUnknown(L402TransportResponse)
export const decodeL402TransportResponseSync = Schema.decodeUnknownSync(L402TransportResponse)
export const encodeL402TransportResponse = Schema.encode(L402TransportResponse)
export const encodeL402TransportResponseSync = Schema.encodeSync(L402TransportResponse)
