import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { makeWalletExecutorHttpServer } from "../src/http/server.js"
import { makeWalletTestLayer, makeTestConfig } from "./fixtures.js"

describe("wallet executor http server", () => {
  it.scoped("serves status and pay endpoints", () =>
    Effect.gen(function* () {
      const config = makeTestConfig({
        port: 8801,
      })
      const layer = makeWalletTestLayer({ config })
      const server = yield* makeWalletExecutorHttpServer.pipe(Effect.provide(layer))
      yield* Effect.addFinalizer(() => server.close.pipe(Effect.orDie))

      const statusResponse = yield* Effect.tryPromise({
        try: async () => await fetch(`${server.address}/status`),
        catch: (error) => new Error(String(error)),
      })
      expect(statusResponse.status).toBe(200)
      const statusJson = (yield* Effect.tryPromise({
        try: async () => await statusResponse.json(),
        catch: (error) => new Error(String(error)),
      })) as { ok: boolean; status: { walletId: string } }
      expect(statusJson.ok).toBe(true)
      expect(statusJson.status.walletId).toBe("test-wallet")

      const payResponse = yield* Effect.tryPromise({
        try: async () =>
          await fetch(`${server.address}/pay-bolt11`, {
            method: "POST",
            headers: {
              "content-type": "application/json",
            },
            body: JSON.stringify({
              requestId: "http-integration-pay-1",
              payment: {
                invoice: "lnbc1httpsuccess",
                maxAmountMsats: 100_000,
                host: "sats4ai.com",
              },
            }),
          }),
        catch: (error) => new Error(String(error)),
      })

      expect(payResponse.status).toBe(200)
      const payJson = (yield* Effect.tryPromise({
        try: async () => await payResponse.json(),
        catch: (error) => new Error(String(error)),
      })) as {
        ok: boolean
        result: {
          payment: { paymentId: string; preimageHex: string }
        }
      }

      expect(payJson.ok).toBe(true)
      expect(payJson.result.payment.paymentId).toMatch(/^mock-pay-/)
      expect(payJson.result.payment.preimageHex).toHaveLength(64)
    }),
  )

  it.scoped("returns typed deny reason for disallowed host", () =>
    Effect.gen(function* () {
      const config = makeTestConfig({
        port: 8802,
      })
      const layer = makeWalletTestLayer({ config })
      const server = yield* makeWalletExecutorHttpServer.pipe(Effect.provide(layer))
      yield* Effect.addFinalizer(() => server.close.pipe(Effect.orDie))

      const response = yield* Effect.tryPromise({
        try: async () =>
          await fetch(`${server.address}/pay-bolt11`, {
            method: "POST",
            headers: {
              "content-type": "application/json",
            },
            body: JSON.stringify({
              requestId: "http-deny-1",
              payment: {
                invoice: "lnbc1denied",
                maxAmountMsats: 100_000,
                host: "example.com",
              },
            }),
          }),
        catch: (error) => new Error(String(error)),
      })

      expect(response.status).toBe(403)
      const payload = (yield* Effect.tryPromise({
        try: async () => await response.json(),
        catch: (error) => new Error(String(error)),
      })) as { ok: boolean; error: { code: string } }
      expect(payload.ok).toBe(false)
      expect(payload.error.code).toBe("host_not_allowed")
    }),
  )

  it.scoped("returns 400 for malformed request body", () =>
    Effect.gen(function* () {
      const config = makeTestConfig({
        port: 8803,
      })
      const layer = makeWalletTestLayer({ config })
      const server = yield* makeWalletExecutorHttpServer.pipe(Effect.provide(layer))
      yield* Effect.addFinalizer(() => server.close.pipe(Effect.orDie))

      const response = yield* Effect.tryPromise({
        try: async () =>
          await fetch(`${server.address}/pay-bolt11`, {
            method: "POST",
            headers: {
              "content-type": "application/json",
            },
            body: JSON.stringify({
              requestId: "http-bad-body",
              payment: {
                invoice: "",
              },
            }),
          }),
        catch: (error) => new Error(String(error)),
      })

      expect(response.status).toBe(400)
      const payload = (yield* Effect.tryPromise({
        try: async () => await response.json(),
        catch: (error) => new Error(String(error)),
      })) as { ok: boolean; error: { code: string } }

      expect(payload.ok).toBe(false)
      expect(payload.error.code).toBe("invalid_request")
    }),
  )

  it.scoped("enforces bearer auth when configured", () =>
    Effect.gen(function* () {
      const config = makeTestConfig({
        port: 8804,
        authToken: "test-token",
      })
      const layer = makeWalletTestLayer({ config })
      const server = yield* makeWalletExecutorHttpServer.pipe(Effect.provide(layer))
      yield* Effect.addFinalizer(() => server.close.pipe(Effect.orDie))

      const unauthorizedResponse = yield* Effect.tryPromise({
        try: async () => await fetch(`${server.address}/status`),
        catch: (error) => new Error(String(error)),
      })
      expect(unauthorizedResponse.status).toBe(401)

      const unauthorizedPayload = (yield* Effect.tryPromise({
        try: async () => await unauthorizedResponse.json(),
        catch: (error) => new Error(String(error)),
      })) as { ok: boolean; error: { code: string } }
      expect(unauthorizedPayload.ok).toBe(false)
      expect(unauthorizedPayload.error.code).toBe("unauthorized")

      const authorizedResponse = yield* Effect.tryPromise({
        try: async () =>
          await fetch(`${server.address}/status`, {
            headers: {
              authorization: "Bearer test-token",
            },
          }),
        catch: (error) => new Error(String(error)),
      })
      expect(authorizedResponse.status).toBe(200)
    }),
  )

  it.scoped("serves compatibility /wallets/* routes", () =>
    Effect.gen(function* () {
      const config = makeTestConfig({
        port: 8805,
      })
      const layer = makeWalletTestLayer({ config })
      const server = yield* makeWalletExecutorHttpServer.pipe(Effect.provide(layer))
      yield* Effect.addFinalizer(() => server.close.pipe(Effect.orDie))

      const createPayerResponse = yield* Effect.tryPromise({
        try: async () =>
          await fetch(`${server.address}/wallets/create`, {
            method: "POST",
            headers: {
              "content-type": "application/json",
            },
            body: JSON.stringify({
              walletId: "mock-payer",
            }),
          }),
        catch: (error) => new Error(String(error)),
      })
      expect(createPayerResponse.status).toBe(200)
      const createPayerJson = (yield* Effect.tryPromise({
        try: async () => await createPayerResponse.json(),
        catch: (error) => new Error(String(error)),
      })) as {
        ok: boolean
        result: { walletId: string; mnemonic: string; sparkAddress: string; balanceSats: number }
      }
      expect(createPayerJson.ok).toBe(true)
      expect(createPayerJson.result.walletId).toBe("mock-payer")
      expect(createPayerJson.result.mnemonic.split(" ").length).toBeGreaterThanOrEqual(12)

      const createReceiverResponse = yield* Effect.tryPromise({
        try: async () =>
          await fetch(`${server.address}/wallets/create`, {
            method: "POST",
            headers: {
              "content-type": "application/json",
            },
            body: JSON.stringify({
              walletId: "mock-receiver",
            }),
          }),
        catch: (error) => new Error(String(error)),
      })
      expect(createReceiverResponse.status).toBe(200)
      const createReceiverJson = (yield* Effect.tryPromise({
        try: async () => await createReceiverResponse.json(),
        catch: (error) => new Error(String(error)),
      })) as {
        ok: boolean
        result: { walletId: string; mnemonic: string; sparkAddress: string; balanceSats: number }
      }
      expect(createReceiverJson.ok).toBe(true)

      const createInvoiceResponse = yield* Effect.tryPromise({
        try: async () =>
          await fetch(`${server.address}/wallets/create-invoice`, {
            method: "POST",
            headers: {
              "content-type": "application/json",
            },
            body: JSON.stringify({
              walletId: "mock-receiver",
              mnemonic: createReceiverJson.result.mnemonic,
              amountSats: 5,
              description: "integration test",
            }),
          }),
        catch: (error) => new Error(String(error)),
      })
      expect(createInvoiceResponse.status).toBe(200)
      const createInvoiceJson = (yield* Effect.tryPromise({
        try: async () => await createInvoiceResponse.json(),
        catch: (error) => new Error(String(error)),
      })) as {
        ok: boolean
        result: { paymentRequest: string }
      }
      expect(createInvoiceJson.ok).toBe(true)
      expect(createInvoiceJson.result.paymentRequest.startsWith("lnmock")).toBe(true)

      const payInvoiceResponse = yield* Effect.tryPromise({
        try: async () =>
          await fetch(`${server.address}/wallets/pay-bolt11`, {
            method: "POST",
            headers: {
              "content-type": "application/json",
            },
            body: JSON.stringify({
              walletId: "mock-payer",
              mnemonic: createPayerJson.result.mnemonic,
              invoice: createInvoiceJson.result.paymentRequest,
              maxAmountMsats: 20_000,
              timeoutMs: 12_000,
              host: "sats4ai.com",
            }),
          }),
        catch: (error) => new Error(String(error)),
      })
      expect(payInvoiceResponse.status).toBe(200)
      const payInvoiceJson = (yield* Effect.tryPromise({
        try: async () => await payInvoiceResponse.json(),
        catch: (error) => new Error(String(error)),
      })) as {
        ok: boolean
        result: { preimage: string; status: string }
      }
      expect(payInvoiceJson.ok).toBe(true)
      expect(payInvoiceJson.result.preimage).toHaveLength(64)
      expect(payInvoiceJson.result.status).toBe("completed")

      const sendSparkResponse = yield* Effect.tryPromise({
        try: async () =>
          await fetch(`${server.address}/wallets/send-spark`, {
            method: "POST",
            headers: {
              "content-type": "application/json",
            },
            body: JSON.stringify({
              walletId: "mock-payer",
              mnemonic: createPayerJson.result.mnemonic,
              sparkAddress: createReceiverJson.result.sparkAddress,
              amountSats: 1,
              timeoutMs: 12_000,
            }),
          }),
        catch: (error) => new Error(String(error)),
      })
      expect(sendSparkResponse.status).toBe(200)

      const receiverStatusResponse = yield* Effect.tryPromise({
        try: async () =>
          await fetch(`${server.address}/wallets/status`, {
            method: "POST",
            headers: {
              "content-type": "application/json",
            },
            body: JSON.stringify({
              walletId: "mock-receiver",
              mnemonic: createReceiverJson.result.mnemonic,
            }),
          }),
        catch: (error) => new Error(String(error)),
      })
      expect(receiverStatusResponse.status).toBe(200)
      const receiverStatusJson = (yield* Effect.tryPromise({
        try: async () => await receiverStatusResponse.json(),
        catch: (error) => new Error(String(error)),
      })) as {
        ok: boolean
        result: { balanceSats: number }
      }
      expect(receiverStatusJson.ok).toBe(true)
      expect(receiverStatusJson.result.balanceSats).toBeGreaterThan(1000)
    }),
  )

})
