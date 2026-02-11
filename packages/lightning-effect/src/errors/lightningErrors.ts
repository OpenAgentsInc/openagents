import { Schema } from "effect"

import { Msats } from "../contracts/payment.js"

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
  },
) {}

export class DomainNotAllowedError extends Schema.TaggedError<DomainNotAllowedError>()(
  "DomainNotAllowedError",
  {
    host: Schema.NonEmptyString,
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

export type LightningEffectError =
  | ChallengeParseError
  | BudgetExceededError
  | DomainNotAllowedError
  | PaymentFailedError
  | CredentialMissingError
  | AuthorizationSerializeError
