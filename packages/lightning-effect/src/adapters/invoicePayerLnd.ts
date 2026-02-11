import { Effect, Layer } from "effect"

import type { InvoicePaymentRequest, InvoicePaymentResult } from "../contracts/payment.js"
import {
  PaymentFailedError,
  PaymentMissingPreimageError,
  PaymentTimeoutError,
} from "../errors/lightningErrors.js"
import { InvoicePayerService } from "../services/invoicePayer.js"

type FetchLike = (input: string, init?: RequestInit) => Promise<Response>

export type InvoicePayerLndLayerOptions = Readonly<{
  readonly endpoint: string
  readonly macaroonHex?: string
  readonly timeoutMs?: number
  readonly fetchImplementation?: FetchLike
  readonly additionalHeaders?: Record<string, string>
}>

const defaultTimeoutMs = 10_000

const asRecord = (value: unknown): Record<string, unknown> | null =>
  value && typeof value === "object" ? (value as Record<string, unknown>) : null

const findFirstString = (record: Record<string, unknown>, keys: ReadonlyArray<string>): string | undefined => {
  for (const key of keys) {
    const value = record[key]
    if (typeof value === "string" && value.trim().length > 0) return value.trim()
  }
  return undefined
}

const parseMsats = (record: Record<string, unknown>, request: InvoicePaymentRequest): number => {
  const paymentRoute = asRecord(record.payment_route) ?? asRecord(record.paymentRoute)

  const msatsCandidates: Array<unknown> = [
    record.value_msat,
    record.valueMsat,
    paymentRoute?.total_amt_msat,
    paymentRoute?.totalAmtMsat,
  ]

  for (const candidate of msatsCandidates) {
    if (typeof candidate === "number" && Number.isFinite(candidate)) {
      return Math.max(0, Math.floor(candidate))
    }
    if (typeof candidate === "string" && candidate.trim().length > 0 && Number.isFinite(Number(candidate))) {
      return Math.max(0, Math.floor(Number(candidate)))
    }
  }

  const satsCandidates: Array<unknown> = [
    record.value_sat,
    record.valueSat,
    paymentRoute?.total_amt,
    paymentRoute?.totalAmt,
  ]

  for (const candidate of satsCandidates) {
    if (typeof candidate === "number" && Number.isFinite(candidate)) {
      return Math.max(0, Math.floor(candidate * 1000))
    }
    if (typeof candidate === "string" && candidate.trim().length > 0 && Number.isFinite(Number(candidate))) {
      return Math.max(0, Math.floor(Number(candidate) * 1000))
    }
  }

  return request.maxAmountMsats
}

const normalizePreimageHex = (value: string | undefined): string | undefined => {
  if (!value) return undefined
  const normalized = value.trim().toLowerCase()
  if (!/^[0-9a-f]+$/.test(normalized)) return undefined
  if (normalized.length % 2 !== 0) return undefined
  return normalized
}

const derivePaymentId = (seed: string): string => {
  let acc = 0
  for (let i = 0; i < seed.length; i += 1) {
    acc = (acc + seed.charCodeAt(i) * (i + 31)) % 0xffffffff
  }
  const suffix = (acc >>> 0).toString(16).padStart(8, "0")
  return `pay_${suffix}`
}

const isAbortError = (cause: unknown): boolean => {
  if (cause instanceof Error && cause.name === "AbortError") return true
  if (typeof DOMException !== "undefined" && cause instanceof DOMException && cause.name === "AbortError") {
    return true
  }
  const record = asRecord(cause)
  return Boolean(record && record.name === "AbortError")
}

const parseResponseBody = Effect.fn("InvoicePayerLnd.parseResponseBody")(function* (
  request: InvoicePaymentRequest,
  response: Response,
  body: unknown,
) {
  const payload = asRecord(body)
  if (!payload) {
    return yield* PaymentFailedError.make({
      invoice: request.invoice,
      reason: "LND payment response is not a JSON object",
    })
  }

  const paymentError = findFirstString(payload, ["payment_error", "paymentError", "error", "message"])
  if (paymentError && paymentError.length > 0) {
    return yield* PaymentFailedError.make({
      invoice: request.invoice,
      reason: paymentError,
    })
  }

  if (!response.ok) {
    return yield* PaymentFailedError.make({
      invoice: request.invoice,
      reason: `LND payment request failed with HTTP ${response.status}`,
    })
  }

  const preimageHex = normalizePreimageHex(
    findFirstString(payload, [
      "payment_preimage",
      "paymentPreimage",
      "payment_preimage_hex",
      "paymentPreimageHex",
      "preimage",
      "preimage_hex",
      "preimageHex",
    ]),
  )

  const paymentId =
    findFirstString(payload, ["payment_hash", "paymentHash", "payment_id", "paymentId"]) ??
    (preimageHex ? derivePaymentId(preimageHex) : undefined)

  if (!preimageHex) {
    return yield* PaymentMissingPreimageError.make({
      invoice: request.invoice,
      ...(paymentId ? { paymentId } : {}),
    })
  }

  const result: InvoicePaymentResult = {
    paymentId: paymentId ?? derivePaymentId(`${request.invoice}:${request.host}`),
    amountMsats: parseMsats(payload, request),
    preimageHex,
    paidAtMs: Date.now(),
  }
  return result
})

export const makeInvoicePayerLndLayer = (options: InvoicePayerLndLayerOptions) => {
  const fetchFn = options.fetchImplementation ?? fetch
  const timeoutMs = Math.max(1, Math.floor(options.timeoutMs ?? defaultTimeoutMs))

  return Layer.succeed(
    InvoicePayerService,
    InvoicePayerService.of({
      payInvoice: (request: InvoicePaymentRequest) =>
        Effect.gen(function* () {
          const responseAndBody = yield* Effect.tryPromise({
            try: async () => {
              const controller = new AbortController()
              const timeout = setTimeout(() => controller.abort(), timeoutMs)

              try {
                const headers: Record<string, string> = {
                  "content-type": "application/json",
                  ...(options.additionalHeaders ?? {}),
                }
                if (options.macaroonHex && options.macaroonHex.trim().length > 0) {
                  headers["grpc-metadata-macaroon"] = options.macaroonHex.trim()
                }

                const response = await fetchFn(options.endpoint, {
                  method: "POST",
                  headers,
                  body: JSON.stringify({
                    payment_request: request.invoice,
                    fee_limit_msat: String(request.maxAmountMsats),
                  }),
                  signal: controller.signal,
                })

                let body: unknown = null
                try {
                  body = await response.json()
                } catch {
                  body = null
                }

                return { response, body }
              } finally {
                clearTimeout(timeout)
              }
            },
            catch: (cause) => {
              if (isAbortError(cause)) {
                return PaymentTimeoutError.make({
                  invoice: request.invoice,
                  timeoutMs,
                })
              }
              return PaymentFailedError.make({
                invoice: request.invoice,
                reason: `LND payment transport failed: ${String(cause)}`,
              })
            },
          })

          return yield* parseResponseBody(
            request,
            responseAndBody.response,
            responseAndBody.body,
          )
        }),
    }),
  )
}
