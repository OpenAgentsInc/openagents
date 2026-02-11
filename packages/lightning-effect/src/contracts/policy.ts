import { Schema } from "effect"

import { Msats } from "./payment.js"

export const SpendPolicyDenialCode = Schema.Literal(
  "host_blocked",
  "host_not_allowlisted",
  "amount_over_cap",
)
export type SpendPolicyDenialCode = typeof SpendPolicyDenialCode.Type

export const SpendPolicyDenialReason = Schema.Struct({
  code: SpendPolicyDenialCode,
  message: Schema.NonEmptyString,
})
export type SpendPolicyDenialReason = typeof SpendPolicyDenialReason.Type

export const SpendPolicy = Schema.Struct({
  defaultMaxSpendMsats: Msats,
  allowedHosts: Schema.Array(Schema.NonEmptyString),
  blockedHosts: Schema.Array(Schema.NonEmptyString),
})
export type SpendPolicy = typeof SpendPolicy.Type
export const decodeSpendPolicy = Schema.decodeUnknown(SpendPolicy)
export const decodeSpendPolicySync = Schema.decodeUnknownSync(SpendPolicy)
export const encodeSpendPolicy = Schema.encode(SpendPolicy)
export const encodeSpendPolicySync = Schema.encodeSync(SpendPolicy)

export const PolicyCheckInput = Schema.Struct({
  host: Schema.NonEmptyString,
  quotedAmountMsats: Msats,
  maxSpendMsats: Msats,
})
export type PolicyCheckInput = typeof PolicyCheckInput.Type
export const decodePolicyCheckInput = Schema.decodeUnknown(PolicyCheckInput)
export const decodePolicyCheckInputSync = Schema.decodeUnknownSync(PolicyCheckInput)
export const encodePolicyCheckInput = Schema.encode(PolicyCheckInput)
export const encodePolicyCheckInputSync = Schema.encodeSync(PolicyCheckInput)
