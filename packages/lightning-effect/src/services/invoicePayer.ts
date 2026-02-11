import { Context, Effect } from "effect"

import { InvoicePaymentRequest, InvoicePaymentResult } from "../contracts/payment.js"
import {
  PaymentFailedError,
  PaymentMissingPreimageError,
  PaymentTimeoutError,
} from "../errors/lightningErrors.js"

export type InvoicePayerError =
  | PaymentFailedError
  | PaymentTimeoutError
  | PaymentMissingPreimageError

export type InvoicePayerApi = Readonly<{
  readonly payInvoice: (
    request: InvoicePaymentRequest,
  ) => Effect.Effect<InvoicePaymentResult, InvoicePayerError>
}>

export class InvoicePayerService extends Context.Tag("@openagents/lightning-effect/InvoicePayerService")<
  InvoicePayerService,
  InvoicePayerApi
>() {}
