import { Context, Effect } from "effect"

import type { InvoicePaymentRequest, InvoicePaymentResult } from "../contracts/payment.js"
import type {
  PaymentFailedError,
  PaymentMissingPreimageError,
  PaymentTimeoutError,
} from "../errors/lightningErrors.js"

export type SparkPaymentError =
  | PaymentFailedError
  | PaymentTimeoutError
  | PaymentMissingPreimageError

export type SparkPaymentApi = Readonly<{
  readonly payBolt11: (
    request: InvoicePaymentRequest,
  ) => Effect.Effect<InvoicePaymentResult, SparkPaymentError>
}>

export class SparkPaymentService extends Context.Tag("@openagents/lightning-effect/SparkPaymentService")<
  SparkPaymentService,
  SparkPaymentApi
>() {}
