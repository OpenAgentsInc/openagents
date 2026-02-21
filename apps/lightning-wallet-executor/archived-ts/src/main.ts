import { Console, Effect, Layer } from "effect"

import { makeWalletExecutorRuntimeLayer } from "./app.js"
import { makeWalletExecutorHttpServer } from "./http/server.js"
import {
  WalletExecutorConfigLive,
  defaultWalletExecutorConfig,
  type WalletExecutorConfig,
} from "./runtime/config.js"
import { MnemonicSecretProviderLive } from "./runtime/secrets.js"
import { SparkGatewayLive } from "./spark/gateway.js"
import { WalletExecutorLive } from "./wallet/executor.js"

const modeFromArgv = (argv: ReadonlyArray<string>): "serve" | "smoke" =>
  argv.includes("smoke") ? "smoke" : "serve"

const makeLiveRuntimeLayer = () => {
  const configLayer = WalletExecutorConfigLive
  const secretLayer = MnemonicSecretProviderLive.pipe(Layer.provide(configLayer))
  const sparkLayer = SparkGatewayLive.pipe(
    Layer.provideMerge(configLayer),
    Layer.provideMerge(secretLayer),
  )
  const walletLayer = WalletExecutorLive.pipe(
    Layer.provideMerge(configLayer),
    Layer.provideMerge(sparkLayer),
  )

  return Layer.mergeAll(configLayer, secretLayer, sparkLayer, walletLayer)
}

const makeSmokeConfig = (): WalletExecutorConfig => ({
  ...defaultWalletExecutorConfig(),
  mode: "mock",
  host: "127.0.0.1",
  port: 8798,
  walletId: "smoke-wallet",
  allowedHosts: new Set(["sats4ai.com", "l402.openagents.com"]),
})

const runServe = Effect.gen(function* () {
  const server = yield* makeWalletExecutorHttpServer.pipe(Effect.provide(makeLiveRuntimeLayer()))
  yield* Console.log(`[lightning-wallet-executor] listening ${server.address}`)
  yield* Effect.never
})

const runSmoke = Effect.gen(function* () {
  const server = yield* makeWalletExecutorHttpServer.pipe(
    Effect.provide(makeWalletExecutorRuntimeLayer(makeSmokeConfig())),
  )
  yield* Console.log(`[lightning-wallet-executor:smoke] listening ${server.address}`)

  const statusResponse = yield* Effect.tryPromise({
    try: async () => await fetch(`${server.address}/status`),
    catch: (error) => new Error(String(error)),
  })

  if (!statusResponse.ok) {
    const body = yield* Effect.tryPromise({
      try: async () => await statusResponse.text(),
      catch: () => "",
    })
    yield* server.close
    return yield* Effect.fail(new Error(`status endpoint returned ${statusResponse.status}: ${body}`))
  }

  const payResponse = yield* Effect.tryPromise({
    try: async () =>
      await fetch(`${server.address}/pay-bolt11`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
        },
        body: JSON.stringify({
          requestId: "smoke-pay-1",
          payment: {
            invoice: "lnbc1smoketestinvoice",
            maxAmountMsats: 100_000,
            host: "sats4ai.com",
          },
        }),
      }),
    catch: (error) => new Error(String(error)),
  })

  if (!payResponse.ok) {
    const body = yield* Effect.tryPromise({
      try: async () => await payResponse.text(),
      catch: () => "",
    })
    yield* server.close
    return yield* Effect.fail(new Error(`pay endpoint returned ${payResponse.status}: ${body}`))
  }

  yield* Console.log("[lightning-wallet-executor:smoke] status->pay succeeded")
  yield* server.close
})

const command = modeFromArgv(process.argv.slice(2))
const main: Effect.Effect<void, unknown, never> = command === "smoke" ? runSmoke : runServe

void Effect.runPromise(main).catch((error) => {
  console.error(error)
  process.exitCode = 1
})

