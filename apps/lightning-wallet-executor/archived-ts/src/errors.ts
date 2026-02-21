import { Schema } from "effect"

import { Msats } from "./contracts.js"

export class WalletExecutorConfigError extends Schema.TaggedError<WalletExecutorConfigError>()(
  "WalletExecutorConfigError",
  {
    field: Schema.NonEmptyString,
    message: Schema.NonEmptyString,
  },
) {}

export class SecretLoadError extends Schema.TaggedError<SecretLoadError>()("SecretLoadError", {
  provider: Schema.NonEmptyString,
  secretRef: Schema.NonEmptyString,
  message: Schema.NonEmptyString,
}) {}

export const PolicyDenialCode = Schema.Literal(
  "host_not_allowed",
  "request_cap_exceeded",
  "quoted_amount_exceeds_cap",
  "window_cap_exceeded",
)
export type PolicyDenialCode = typeof PolicyDenialCode.Type

export class PolicyDeniedError extends Schema.TaggedError<PolicyDeniedError>()("PolicyDeniedError", {
  code: PolicyDenialCode,
  message: Schema.NonEmptyString,
  host: Schema.optional(Schema.NonEmptyString),
  maxAllowedMsats: Schema.optional(Msats),
  quotedAmountMsats: Schema.optional(Msats),
  windowSpendMsats: Schema.optional(Msats),
  windowCapMsats: Schema.optional(Msats),
}) {}

export class SparkGatewayError extends Schema.TaggedError<SparkGatewayError>()("SparkGatewayError", {
  code: Schema.Literal(
    "api_key_missing",
    "mnemonic_missing",
    "mnemonic_invalid",
    "connect_failed",
    "prepare_failed",
    "send_failed",
    "payment_pending",
    "payment_failed",
    "payment_missing_preimage",
    "unsupported_payment_method",
  ),
  message: Schema.NonEmptyString,
}) {}

export class HttpRequestDecodeError extends Schema.TaggedError<HttpRequestDecodeError>()("HttpRequestDecodeError", {
  message: Schema.NonEmptyString,
}) {}

export type WalletExecutorError =
  | WalletExecutorConfigError
  | SecretLoadError
  | PolicyDeniedError
  | SparkGatewayError
  | HttpRequestDecodeError

