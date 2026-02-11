import { Schema } from "effect"

import { Msats } from "./payment.js"

export const SpendPolicy = Schema.Struct({
  defaultMaxSpendMsats: Msats,
  allowedHosts: Schema.Array(Schema.NonEmptyString),
  blockedHosts: Schema.Array(Schema.NonEmptyString),
})
export type SpendPolicy = typeof SpendPolicy.Type

export const PolicyCheckInput = Schema.Struct({
  host: Schema.NonEmptyString,
  quotedAmountMsats: Msats,
  maxSpendMsats: Msats,
})
export type PolicyCheckInput = typeof PolicyCheckInput.Type
