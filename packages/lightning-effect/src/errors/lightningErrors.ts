import { Schema } from "effect"

import { Msats } from "../contracts/payment.js"
import { SpendPolicyDenialCode } from "../contracts/policy.js"

export class ChallengeParseError extends Schema.TaggedError<ChallengeParseError>()(
  "ChallengeParseError",
  {
    header: Schema.String,
    reason: Schema.String,
  },
) {}

export class BudgetExceededError extends Schema.TaggedError<BudgetExceededError>()(
  "BudgetExceededError",
  {
    maxSpendMsats: Msats,
    quotedAmountMsats: Msats,
    reasonCode: SpendPolicyDenialCode,
    reason: Schema.NonEmptyString,
  },
) {}

export class DomainNotAllowedError extends Schema.TaggedError<DomainNotAllowedError>()(
  "DomainNotAllowedError",
  {
    host: Schema.NonEmptyString,
    reasonCode: SpendPolicyDenialCode,
    reason: Schema.String,
  },
) {}

export class PaymentFailedError extends Schema.TaggedError<PaymentFailedError>()(
  "PaymentFailedError",
  {
    invoice: Schema.String,
    reason: Schema.String,
  },
) {}

export class PaymentTimeoutError extends Schema.TaggedError<PaymentTimeoutError>()(
  "PaymentTimeoutError",
  {
    invoice: Schema.String,
    timeoutMs: Schema.Int.pipe(Schema.positive()),
  },
) {}

export class PaymentMissingPreimageError extends Schema.TaggedError<PaymentMissingPreimageError>()(
  "PaymentMissingPreimageError",
  {
    invoice: Schema.String,
    paymentId: Schema.optional(Schema.String),
  },
) {}

export class CredentialMissingError extends Schema.TaggedError<CredentialMissingError>()(
  "CredentialMissingError",
  {
    host: Schema.String,
    reason: Schema.String,
  },
) {}

export class AuthorizationSerializeError extends Schema.TaggedError<AuthorizationSerializeError>()(
  "AuthorizationSerializeError",
  {
    reason: Schema.String,
  },
) {}

export class L402TransportError extends Schema.TaggedError<L402TransportError>()(
  "L402TransportError",
  {
    reason: Schema.String,
    status: Schema.optional(Schema.Int),
  },
) {}

export type LightningEffectError =
  | ChallengeParseError
  | BudgetExceededError
  | DomainNotAllowedError
  | PaymentFailedError
  | PaymentTimeoutError
  | PaymentMissingPreimageError
  | CredentialMissingError
  | AuthorizationSerializeError
  | L402TransportError
