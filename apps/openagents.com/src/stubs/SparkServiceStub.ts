/**
 * Browser-compatible stub for SparkService
 * This avoids importing the Node.js-only @buildonspark/spark-sdk
 */

import { Context, Data, Effect, Layer, Schema } from "effect"

// --- Error Types ---
export class SparkError extends Data.TaggedError("SparkError")<{
  reason: "wallet_creation_failed" | "invoice_creation_failed" | "payment_failed" | "network_error" | "invalid_invoice"
  message: string
  cause?: unknown
}> {}

// --- Lightning Invoice Schema ---
export const LightningInvoice = Schema.Struct({
  id: Schema.String,
  invoice: Schema.String,
  amountSats: Schema.Number,
  memo: Schema.optional(Schema.String),
  createdAt: Schema.Number,
  expiresAt: Schema.Number,
  sparkAddress: Schema.optional(Schema.String)
})
export type LightningInvoice = Schema.Schema.Type<typeof LightningInvoice>

// --- Payment Schema ---
export const PaymentResult = Schema.Struct({
  success: Schema.Boolean,
  transactionId: Schema.optional(Schema.String),
  amountSats: Schema.Number,
  fee: Schema.optional(Schema.Number),
  timestamp: Schema.Number
})
export type PaymentResult = Schema.Schema.Type<typeof PaymentResult>

// --- Wallet Info Schema ---
export const WalletInfo = Schema.Struct({
  id: Schema.String,
  sparkAddress: Schema.String,
  balanceSats: Schema.Number,
  pendingSats: Schema.Number
})
export type WalletInfo = Schema.Schema.Type<typeof WalletInfo>

// --- Service Interface ---
export class SparkService extends Context.Tag("SparkService")<
  SparkService,
  {
    readonly createWallet: (agentId: string) => Effect.Effect<WalletInfo, SparkError>
    readonly getWalletInfo: (walletId: string) => Effect.Effect<WalletInfo, SparkError>
    readonly createInvoice: (params: {
      amountSats: number
      memo?: string
      walletId: string
    }) => Effect.Effect<LightningInvoice, SparkError>
    readonly payInvoice: (params: {
      invoice: string
      walletId: string
    }) => Effect.Effect<PaymentResult, SparkError>
  }
>() {}

// --- Browser Stub Implementation ---
export const SparkServiceLive = Layer.succeed(
  SparkService,
  SparkService.of({
    createWallet: () =>
      Effect.fail(
        new SparkError({
          reason: "wallet_creation_failed",
          message: "Lightning payments not available in browser"
        })
      ),

    getWalletInfo: () =>
      Effect.fail(
        new SparkError({
          reason: "network_error",
          message: "Lightning payments not available in browser"
        })
      ),

    createInvoice: () =>
      Effect.fail(
        new SparkError({
          reason: "invoice_creation_failed",
          message: "Lightning payments not available in browser"
        })
      ),

    payInvoice: () =>
      Effect.fail(
        new SparkError({
          reason: "payment_failed",
          message: "Lightning payments not available in browser"
        })
      )
  })
)
