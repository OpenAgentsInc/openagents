import { Schema } from "effect"

export class LndContractDecodeError extends Schema.TaggedError<LndContractDecodeError>()(
  "LndContractDecodeError",
  {
    contract: Schema.NonEmptyString,
    reason: Schema.String,
  },
) {}

export class LndServiceUnavailableError extends Schema.TaggedError<LndServiceUnavailableError>()(
  "LndServiceUnavailableError",
  {
    service: Schema.NonEmptyString,
    reason: Schema.String,
  },
) {}

export type LndEffectError = LndContractDecodeError | LndServiceUnavailableError
