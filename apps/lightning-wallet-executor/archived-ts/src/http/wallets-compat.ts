import SparkSdk from "@breeztech/breez-sdk-spark/nodejs"
import { generateMnemonic } from "@scure/bip39"
import { wordlist as englishWordlist } from "@scure/bip39/wordlists/english"
import crypto from "node:crypto"

import type { WalletExecutorConfig } from "../runtime/config.js"

type JsonObject = Record<string, unknown>

type CompatWalletRecord = {
  walletId: string
  mnemonic: string
  sparkAddress: string
  lightningAddress: string
  identityPubkey: string
  balanceSats: number
}

type CompatInvoiceRecord = {
  walletId: string
  amountMsats: number
  createdAtMs: number
}

const mockWallets = new Map<string, CompatWalletRecord>()
const mockWalletBySparkAddress = new Map<string, string>()
const mockInvoices = new Map<string, CompatInvoiceRecord>()

export class WalletCompatHttpError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
    readonly details?: JsonObject,
  ) {
    super(message)
    this.name = "WalletCompatHttpError"
  }
}

export const isWalletCompatHttpError = (value: unknown): value is WalletCompatHttpError =>
  value instanceof WalletCompatHttpError

const asObject = (value: unknown): JsonObject => {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new WalletCompatHttpError(400, "invalid_request", "request body must be a JSON object")
  }
  return value as JsonObject
}

const requireString = (payload: JsonObject, key: string): string => {
  const value = payload[key]
  if (typeof value !== "string" || value.trim() === "") {
    throw new WalletCompatHttpError(400, "invalid_request", `${key} is required`)
  }
  return value.trim()
}

const optionalString = (payload: JsonObject, key: string): string | null => {
  const value = payload[key]
  if (value == null) return null
  if (typeof value !== "string") {
    throw new WalletCompatHttpError(400, "invalid_request", `${key} must be a string`)
  }
  const normalized = value.trim()
  return normalized === "" ? null : normalized
}

const requireInt = (payload: JsonObject, key: string, min = 1): number => {
  const value = payload[key]
  if (typeof value !== "number" || !Number.isFinite(value) || !Number.isInteger(value) || value < min) {
    throw new WalletCompatHttpError(400, "invalid_request", `${key} must be an integer >= ${min}`)
  }
  return value
}

const optionalInt = (payload: JsonObject, key: string, min = 1): number | null => {
  const value = payload[key]
  if (value == null) return null
  if (typeof value !== "number" || !Number.isFinite(value) || !Number.isInteger(value) || value < min) {
    throw new WalletCompatHttpError(400, "invalid_request", `${key} must be an integer >= ${min}`)
  }
  return value
}

const normalizeHost = (host: string | null): string | null => {
  if (!host) return null
  const normalized = host.trim().toLowerCase()
  return normalized === "" ? null : normalized
}

const amountMsatsFromBigint = (value: bigint): number => {
  const sats = Number(value > BigInt(Number.MAX_SAFE_INTEGER) ? BigInt(Number.MAX_SAFE_INTEGER) : value)
  return Math.max(0, sats * 1000)
}

const paidAtMsFromTimestamp = (value: number): number =>
  value > 1_000_000_000_000 ? Math.floor(value) : Math.floor(value * 1000)

const normalizePreimage = (value: unknown): string | null => {
  if (typeof value !== "string") return null
  const normalized = value.trim().toLowerCase()
  if (!/^[0-9a-f]{64}$/i.test(normalized)) return null
  return normalized
}

const allowHostOrThrow = (config: WalletExecutorConfig, host: string | null): void => {
  const normalized = normalizeHost(host)
  if (!normalized) return
  if (config.allowedHosts.size === 0 || config.allowedHosts.has(normalized)) return

  throw new WalletCompatHttpError(403, "host_not_allowed", `host ${normalized} is not in the wallet allowlist`, {
    host: normalized,
    allowlistHosts: [...config.allowedHosts],
  })
}

const defaultMnemonic = (): string => generateMnemonic(englishWordlist, 128)

const mockIdentity = (walletId: string): string =>
  `mock-${crypto.createHash("sha256").update(walletId).digest("hex").slice(0, 16)}`

const mockLightningAddress = (walletId: string): string => `${walletId.replace(/[^a-z0-9-]/gi, "-")}@spark.mock`

const ensureMockWallet = (walletId: string, mnemonic?: string | null): CompatWalletRecord => {
  const existing = mockWallets.get(walletId)
  if (existing) {
    if (mnemonic && mnemonic.trim() !== "") {
      existing.mnemonic = mnemonic.trim().replace(/\s+/g, " ")
    }
    return existing
  }

  const created: CompatWalletRecord = {
    walletId,
    mnemonic: (mnemonic && mnemonic.trim() !== "" ? mnemonic : defaultMnemonic()).trim().replace(/\s+/g, " "),
    sparkAddress: mockLightningAddress(walletId),
    lightningAddress: mockLightningAddress(walletId),
    identityPubkey: mockIdentity(walletId),
    balanceSats: 1000,
  }

  mockWallets.set(walletId, created)
  mockWalletBySparkAddress.set(created.sparkAddress.toLowerCase(), walletId)
  return created
}

const mockCreateInvoice = (walletId: string, amountSats: number): string => {
  const invoice = `lnmock${Date.now().toString(36)}${crypto.randomBytes(8).toString("hex")}`
  mockInvoices.set(invoice, {
    walletId,
    amountMsats: amountSats * 1000,
    createdAtMs: Date.now(),
  })
  return invoice
}

const mockPayInvoice = (payerWalletId: string, invoice: string, maxAmountMsats: number): {
  paymentId: string
  preimage: string
  status: "completed"
  amountMsats: number
  paidAtMs: number
} => {
  const quoted = mockInvoices.get(invoice)
  if (!quoted) {
    throw new WalletCompatHttpError(404, "invoice_not_found", "invoice not found in mock ledger")
  }

  if (quoted.amountMsats > maxAmountMsats) {
    throw new WalletCompatHttpError(422, "quoted_amount_exceeds_cap", "quoted amount exceeds maxAmountMsats", {
      quotedAmountMsats: quoted.amountMsats,
      maxAmountMsats,
    })
  }

  const payer = ensureMockWallet(payerWalletId)
  const recipient = ensureMockWallet(quoted.walletId)

  if (payer.balanceSats * 1000 < quoted.amountMsats) {
    throw new WalletCompatHttpError(402, "insufficient_balance", "mock wallet has insufficient balance", {
      walletId: payerWalletId,
      requiredMsats: quoted.amountMsats,
      availableMsats: payer.balanceSats * 1000,
    })
  }

  payer.balanceSats = Math.max(0, payer.balanceSats - Math.ceil(quoted.amountMsats / 1000))
  recipient.balanceSats += Math.ceil(quoted.amountMsats / 1000)

  mockInvoices.delete(invoice)

  return {
    paymentId: `mock-pay-${crypto.randomUUID()}`,
    preimage: crypto.randomBytes(32).toString("hex"),
    status: "completed",
    amountMsats: quoted.amountMsats,
    paidAtMs: Date.now(),
  }
}

type DynamicSparkSdk = {
  disconnect: () => Promise<void>
  getInfo: (input: { ensureSynced: boolean }) => Promise<{ identityPubkey: string; balanceSats: number }>
  getLightningAddress: () => Promise<{ lightningAddress: string } | undefined>
  receivePayment: (input: { paymentMethod: unknown }) => Promise<{ paymentRequest: string }>
  prepareSendPayment: (input: { paymentRequest: string; amount?: bigint }) => Promise<{ amount: bigint; paymentMethod: { type: string } }>
  sendPayment: (input: { prepareResponse: unknown; options?: unknown }) => Promise<{
    payment: {
      id: string
      status: string
      amount: bigint
      timestamp: number
      details?: {
        type?: string
        preimage?: string
      }
    }
  }>
}

const withSparkWallet = async <T>(
  config: WalletExecutorConfig,
  walletId: string,
  mnemonic: string,
  run: (sdk: DynamicSparkSdk) => Promise<T>,
): Promise<T> => {
  if (!config.sparkApiKey || config.sparkApiKey.trim() === "") {
    throw new WalletCompatHttpError(500, "config_error", "spark api key is missing")
  }

  const sparkConfig = SparkSdk.defaultConfig(config.network)
  sparkConfig.apiKey = config.sparkApiKey

  const seed = {
    type: "mnemonic" as const,
    mnemonic,
  }

  let sdk: DynamicSparkSdk | null = null

  try {
    let builder = SparkSdk.SdkBuilder.new(sparkConfig, seed)
    builder = await builder.withDefaultStorage(`./output/spark-wallet-executor/${walletId}`)
    sdk = (await builder.build()) as DynamicSparkSdk

    return await run(sdk)
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    throw new WalletCompatHttpError(502, "spark_executor_error", message)
  } finally {
    if (sdk) {
      try {
        await sdk.disconnect()
      } catch {
        // ignore disconnect errors
      }
    }
  }
}

const sparkWalletSnapshot = async (
  config: WalletExecutorConfig,
  walletId: string,
  mnemonic: string,
): Promise<CompatWalletRecord> =>
  withSparkWallet(config, walletId, mnemonic, async (sdk) => {
    const info = await sdk.getInfo({ ensureSynced: true })
    let lightningAddress: string | null = null

    try {
      const addressInfo = await sdk.getLightningAddress()
      lightningAddress = addressInfo?.lightningAddress ?? null
    } catch {
      lightningAddress = null
    }

    return {
      walletId,
      mnemonic,
      sparkAddress: lightningAddress ?? walletId,
      lightningAddress: lightningAddress ?? "",
      identityPubkey: info.identityPubkey,
      balanceSats: Math.max(0, Math.floor(info.balanceSats)),
    }
  })

const makeWalletPayload = (record: CompatWalletRecord): JsonObject => ({
  walletId: record.walletId,
  mnemonic: record.mnemonic,
  sparkAddress: record.sparkAddress,
  lightningAddress: record.lightningAddress,
  identityPubkey: record.identityPubkey,
  balanceSats: record.balanceSats,
  status: "active",
})

export const handleCompatWalletRoute = async (
  method: string,
  path: string,
  rawBody: unknown,
  config: WalletExecutorConfig,
): Promise<{ status: number; body: JsonObject }> => {
  if (method !== "POST") {
    throw new WalletCompatHttpError(404, "not_found", "route not found")
  }

  const body = asObject(rawBody)

  if (path === "/wallets/create") {
    const walletId = requireString(body, "walletId")
    const requestedMnemonic = optionalString(body, "mnemonic")
    const mnemonic = (requestedMnemonic ?? defaultMnemonic()).replace(/\s+/g, " ").trim()

    const wallet =
      config.mode === "spark"
        ? await sparkWalletSnapshot(config, walletId, mnemonic)
        : ensureMockWallet(walletId, mnemonic)

    return {
      status: 200,
      body: { ok: true, result: makeWalletPayload(wallet) },
    }
  }

  if (path === "/wallets/status") {
    const walletId = requireString(body, "walletId")
    const mnemonic = requireString(body, "mnemonic").replace(/\s+/g, " ").trim()

    const wallet =
      config.mode === "spark"
        ? await sparkWalletSnapshot(config, walletId, mnemonic)
        : ensureMockWallet(walletId, mnemonic)

    return {
      status: 200,
      body: { ok: true, result: makeWalletPayload(wallet) },
    }
  }

  if (path === "/wallets/create-invoice") {
    const walletId = requireString(body, "walletId")
    const mnemonic = requireString(body, "mnemonic").replace(/\s+/g, " ").trim()
    const amountSats = requireInt(body, "amountSats", 1)
    const description = optionalString(body, "description") ?? `OpenAgents wallet invoice (${walletId})`

    if (config.mode !== "spark") {
      const wallet = ensureMockWallet(walletId, mnemonic)
      const invoice = mockCreateInvoice(wallet.walletId, amountSats)
      return {
        status: 200,
        body: {
          ok: true,
          result: {
            walletId,
            paymentRequest: invoice,
            invoice,
            amountSats,
            description,
            expiresAt: null,
          },
        },
      }
    }

    const invoice = await withSparkWallet(config, walletId, mnemonic, async (sdk) => {
      const receive = await sdk.receivePayment({
        paymentMethod: {
          type: "bolt11Invoice",
          description,
          amountSats,
        },
      })
      return receive.paymentRequest
    })

    return {
      status: 200,
      body: {
        ok: true,
        result: {
          walletId,
          paymentRequest: invoice,
          invoice,
          amountSats,
          description,
          expiresAt: null,
        },
      },
    }
  }

  if (path === "/wallets/pay-bolt11") {
    const walletId = requireString(body, "walletId")
    const mnemonic = requireString(body, "mnemonic").replace(/\s+/g, " ").trim()
    const invoice = requireString(body, "invoice")
    const maxAmountMsats = requireInt(body, "maxAmountMsats", 1_000)
    const timeoutMs = optionalInt(body, "timeoutMs", 1_000) ?? 12_000
    const host = normalizeHost(optionalString(body, "host"))

    allowHostOrThrow(config, host)

    if (config.mode !== "spark") {
      const outcome = mockPayInvoice(walletId, invoice, maxAmountMsats)
      return {
        status: 200,
        body: {
          ok: true,
          result: {
            walletId,
            paymentId: outcome.paymentId,
            preimage: outcome.preimage,
            status: outcome.status,
            amountMsats: outcome.amountMsats,
            paidAtMs: outcome.paidAtMs,
          },
        },
      }
    }

    const result = await withSparkWallet(config, walletId, mnemonic, async (sdk) => {
      const prepared = await sdk.prepareSendPayment({ paymentRequest: invoice })
      const quotedAmountMsats = amountMsatsFromBigint(prepared.amount)

      if (quotedAmountMsats > maxAmountMsats) {
        throw new WalletCompatHttpError(422, "quoted_amount_exceeds_cap", "quoted amount exceeds maxAmountMsats", {
          quotedAmountMsats,
          maxAmountMsats,
        })
      }

      const sent = await sdk.sendPayment({
        prepareResponse: prepared,
        options: {
          type: "bolt11Invoice",
          preferSpark: true,
          completionTimeoutSecs: Math.max(1, Math.ceil(timeoutMs / 1000)),
        },
      })

      const preimage = normalizePreimage(sent.payment.details?.preimage)
      const status = sent.payment.status === "failed" ? "failed" : sent.payment.status === "pending" ? "pending" : "completed"

      return {
        paymentId: sent.payment.id,
        preimage,
        status,
        amountMsats: amountMsatsFromBigint(sent.payment.amount),
        paidAtMs: paidAtMsFromTimestamp(sent.payment.timestamp),
      }
    })

    return {
      status: 200,
      body: {
        ok: true,
        result: {
          walletId,
          paymentId: result.paymentId,
          preimage: result.preimage,
          status: result.status,
          amountMsats: result.amountMsats,
          paidAtMs: result.paidAtMs,
        },
      },
    }
  }

  if (path === "/wallets/send-spark") {
    const walletId = requireString(body, "walletId")
    const mnemonic = requireString(body, "mnemonic").replace(/\s+/g, " ").trim()
    const sparkAddress = requireString(body, "sparkAddress")
    const amountSats = requireInt(body, "amountSats", 1)
    const timeoutMs = optionalInt(body, "timeoutMs", 1_000) ?? 12_000

    if (config.mode !== "spark") {
      const sender = ensureMockWallet(walletId, mnemonic)
      const recipientWalletId = mockWalletBySparkAddress.get(sparkAddress.toLowerCase())
      if (!recipientWalletId) {
        throw new WalletCompatHttpError(404, "recipient_not_found", "mock recipient spark address not found")
      }
      const recipient = ensureMockWallet(recipientWalletId)

      if (sender.balanceSats < amountSats) {
        throw new WalletCompatHttpError(402, "insufficient_balance", "mock wallet has insufficient balance")
      }

      sender.balanceSats -= amountSats
      recipient.balanceSats += amountSats

      return {
        status: 200,
        body: {
          ok: true,
          result: {
            walletId,
            paymentId: `mock-spark-${crypto.randomUUID()}`,
            status: "completed",
            amountSats,
            amountMsats: amountSats * 1000,
            paidAtMs: Date.now(),
          },
        },
      }
    }

    const result = await withSparkWallet(config, walletId, mnemonic, async (sdk) => {
      const prepared = await sdk.prepareSendPayment({
        paymentRequest: sparkAddress,
        amount: BigInt(amountSats),
      })

      const sent = await sdk.sendPayment({
        prepareResponse: prepared,
        options: {
          type: "sparkAddress",
          htlcOptions: {
            timeoutSecs: Math.max(1, Math.ceil(timeoutMs / 1000)),
          },
        },
      })

      const status = sent.payment.status === "failed" ? "failed" : sent.payment.status === "pending" ? "pending" : "completed"

      return {
        paymentId: sent.payment.id,
        status,
        amountMsats: amountMsatsFromBigint(sent.payment.amount),
        paidAtMs: paidAtMsFromTimestamp(sent.payment.timestamp),
      }
    })

    return {
      status: 200,
      body: {
        ok: true,
        result: {
          walletId,
          paymentId: result.paymentId,
          status: result.status,
          amountSats,
          amountMsats: result.amountMsats,
          paidAtMs: result.paidAtMs,
        },
      },
    }
  }

  throw new WalletCompatHttpError(404, "not_found", "route not found")
}
