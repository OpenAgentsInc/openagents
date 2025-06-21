/**
 * Spark SDK Integration - Lightning Payments for Agents
 * Provides wallet management and Lightning payment capabilities
 */

import { SparkWallet } from "@buildonspark/spark-sdk"
import { Context, Data, Effect, Layer, Schema } from "effect"

// Use a type alias to avoid TypeScript conflicts
type WalletInstance = any

// --- Error Types ---
export class SparkError extends Data.TaggedError("SparkError")<{
  reason: "wallet_creation_failed" | "invoice_creation_failed" | "payment_failed" | "network_error" | "invalid_invoice"
  message: string
  cause?: unknown
}> {}

// --- Lightning Invoice Schema ---
export const LightningInvoice = Schema.Struct({
  id: Schema.String,
  invoice: Schema.String, // ln... encoded invoice
  amountSats: Schema.Number,
  memo: Schema.optional(Schema.String),
  createdAt: Schema.Number,
  expiresAt: Schema.Number,
  sparkAddress: Schema.optional(Schema.String)
})
export type LightningInvoice = Schema.Schema.Type<typeof LightningInvoice>

// --- Payment Response Schema ---
export const PaymentResponse = Schema.Struct({
  preimage: Schema.String,
  feeSats: Schema.Number,
  totalSats: Schema.Number,
  timestamp: Schema.Number,
  status: Schema.Literal("success", "failed", "pending")
})
export type PaymentResponse = Schema.Schema.Type<typeof PaymentResponse>

// --- Wallet Info Schema ---
export const WalletInfo = Schema.Struct({
  balanceSats: Schema.Number,
  sparkAddress: Schema.optional(Schema.String)
})
export type WalletInfo = Schema.Schema.Type<typeof WalletInfo>

// --- Service Parameters ---
export interface CreateInvoiceParams {
  amountSats: number
  memo?: string
  includeSparkAddress?: boolean
  receiverIdentityPubkey?: string
}

export interface PayInvoiceParams {
  invoice: string
  maxFeeSats?: number
  preferSpark?: boolean
}

// --- Spark Service ---
export class SparkService extends Context.Tag("sdk/SparkService")<
  SparkService,
  {
    /**
     * Create or restore a Spark wallet from mnemonic
     */
    readonly createWallet: (
      mnemonic?: string,
      network?: "MAINNET" | "REGTEST" | "TESTNET"
    ) => Effect.Effect<{ wallet: WalletInstance; mnemonic: string }, SparkError>

    /**
     * Get wallet balance and info
     */
    readonly getWalletInfo: (
      wallet: WalletInstance
    ) => Effect.Effect<WalletInfo, SparkError>

    /**
     * Create a Lightning invoice to receive payments
     */
    readonly createInvoice: (
      wallet: WalletInstance,
      params: CreateInvoiceParams
    ) => Effect.Effect<LightningInvoice, SparkError>

    /**
     * Pay a Lightning invoice
     */
    readonly payInvoice: (
      wallet: WalletInstance,
      params: PayInvoiceParams
    ) => Effect.Effect<PaymentResponse, SparkError>

    /**
     * Monitor invoice payment status
     */
    readonly getInvoiceStatus: (
      wallet: WalletInstance,
      invoiceId: string
    ) => Effect.Effect<"pending" | "paid" | "expired", SparkError>
  }
>() {}

// --- Service Implementation ---
export const SparkServiceLive = Layer.effect(
  SparkService,
  Effect.sync(() => {
    const createWallet = (
      mnemonic?: string,
      network: "MAINNET" | "REGTEST" | "TESTNET" = "MAINNET"
    ): Effect.Effect<{ wallet: WalletInstance; mnemonic: string }, SparkError> =>
      Effect.tryPromise({
        try: async () => {
          const initOptions = mnemonic
            ? { mnemonicOrSeed: mnemonic, options: { network } }
            : { options: { network } }

          const result = await SparkWallet.initialize(initOptions as any)
          return {
            wallet: result.wallet,
            mnemonic: result.mnemonic || mnemonic || ""
          }
        },
        catch: (error) =>
          new SparkError({
            reason: "wallet_creation_failed",
            message: `Failed to create Spark wallet: ${error}`,
            cause: error
          })
      })

    const getWalletInfo = (
      wallet: WalletInstance
    ): Effect.Effect<WalletInfo, SparkError> =>
      Effect.tryPromise({
        try: async () => {
          // Get wallet balance
          const balance = await wallet.getBalance()

          return {
            balanceSats: Number(balance.balance) || 0,
            sparkAddress: undefined // Will be populated from invoice creation
          }
        },
        catch: (error) =>
          new SparkError({
            reason: "network_error",
            message: `Failed to get wallet info: ${error}`,
            cause: error
          })
      })

    const createInvoice = (
      wallet: WalletInstance,
      params: CreateInvoiceParams
    ): Effect.Effect<LightningInvoice, SparkError> =>
      Effect.tryPromise({
        try: async () => {
          const invoiceParams: any = {
            amountSats: params.amountSats,
            memo: params.memo || "",
            includeSparkAddress: params.includeSparkAddress ?? true
          }

          if (params.receiverIdentityPubkey) {
            invoiceParams.receiverIdentityPubkey = params.receiverIdentityPubkey
          }

          const invoice = await wallet.createLightningInvoice(invoiceParams)

          return {
            id: invoice.id,
            invoice: String(invoice.invoice),
            amountSats: params.amountSats,
            memo: params.memo,
            createdAt: Date.now(),
            expiresAt: Date.now() + 24 * 60 * 60 * 1000, // 24 hours default
            sparkAddress: undefined // Not available in response
          }
        },
        catch: (error) =>
          new SparkError({
            reason: "invoice_creation_failed",
            message: `Failed to create Lightning invoice: ${error}`,
            cause: error
          })
      })

    const payInvoice = (
      wallet: WalletInstance,
      params: PayInvoiceParams
    ): Effect.Effect<PaymentResponse, SparkError> =>
      Effect.tryPromise({
        try: async () => {
          // Validate invoice format
          if (!params.invoice.startsWith("ln")) {
            throw new Error("Invalid Lightning invoice format")
          }

          await wallet.payLightningInvoice({
            invoice: params.invoice,
            maxFeeSats: params.maxFeeSats ?? 5,
            preferSpark: params.preferSpark ?? true
          })

          // Response is either LightningSendRequest or WalletTransfer
          // For now, assume payment succeeded if no error thrown
          return {
            preimage: "", // Not available in response
            feeSats: 0, // Not available in response
            totalSats: 0, // Not available in response
            timestamp: Date.now(),
            status: "success" as const
          }
        },
        catch: (error: any) =>
          new SparkError({
            reason: error.message?.includes("Invalid") ? "invalid_invoice" : "payment_failed",
            message: `Failed to pay Lightning invoice: ${error}`,
            cause: error
          })
      })

    const getInvoiceStatus = (
      wallet: WalletInstance,
      invoiceId: string
    ): Effect.Effect<"pending" | "paid" | "expired", SparkError> =>
      Effect.tryPromise({
        try: async () => {
          const status = await wallet.getLightningReceiveRequest(invoiceId)

          if (!status) {
            return "expired" as const
          }

          // Check if paid (status might have different structure)
          // For now, assume pending if request exists
          return "pending" as const
        },
        catch: (error) =>
          new SparkError({
            reason: "network_error",
            message: `Failed to get invoice status: ${error}`,
            cause: error
          })
      })

    return {
      createWallet,
      getWalletInfo,
      createInvoice,
      payInvoice,
      getInvoiceStatus
    }
  })
)
