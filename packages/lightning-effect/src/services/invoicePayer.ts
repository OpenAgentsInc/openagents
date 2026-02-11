import { Context, Effect } from "effect"

import { InvoicePaymentRequest, InvoicePaymentResult } from "../contracts/payment.js"
import { PaymentFailedError } from "../errors/lightningErrors.js"

export type InvoicePayerApi = Readonly<{
  readonly payInvoice: (
    request: InvoicePaymentRequest,
  ) => Effect.Effect<InvoicePaymentResult, PaymentFailedError>
}>

export class InvoicePayerService extends Context.Tag("@openagents/lightning-effect/InvoicePayerService")<
  InvoicePayerService,
  InvoicePayerApi
>() {}
