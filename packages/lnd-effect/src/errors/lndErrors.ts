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

export class LndTransportError extends Schema.TaggedError<LndTransportError>()(
  "LndTransportError",
  {
    operation: Schema.NonEmptyString,
    reason: Schema.String,
    status: Schema.optional(Schema.Int),
  },
) {}

export class LndAuthenticationError extends Schema.TaggedError<LndAuthenticationError>()(
  "LndAuthenticationError",
  {
    operation: Schema.NonEmptyString,
    reason: Schema.String,
    status: Schema.Int,
  },
) {}

export class LndResponseDecodeError extends Schema.TaggedError<LndResponseDecodeError>()(
  "LndResponseDecodeError",
  {
    operation: Schema.NonEmptyString,
    reason: Schema.String,
    status: Schema.optional(Schema.Int),
    body: Schema.optional(Schema.String),
  },
) {}

export class LndWalletOperationError extends Schema.TaggedError<LndWalletOperationError>()(
  "LndWalletOperationError",
  {
    operation: Schema.NonEmptyString,
    reason: Schema.String,
  },
) {}

export type LndEffectError =
  | LndContractDecodeError
  | LndServiceUnavailableError
  | LndTransportError
  | LndAuthenticationError
  | LndResponseDecodeError
  | LndWalletOperationError
