import { Effect } from "effect"
import { describe, expect, it } from "@effect/vitest"

import { loadWalletExecutorConfig } from "../src/runtime/config.js"

describe("wallet-executor config", () => {
  it.effect("fails closed when spark mode is missing API key", () =>
    Effect.gen(function* () {
      const attempted = yield* Effect.either(
        loadWalletExecutorConfig({
          OA_LIGHTNING_WALLET_EXECUTOR_MODE: "spark",
          OA_LIGHTNING_WALLET_ALLOWED_HOSTS: "sats4ai.com",
        }),
      )

      expect(attempted._tag).toBe("Left")
      if (attempted._tag === "Left") {
        expect(attempted.left._tag).toBe("WalletExecutorConfigError")
        if (attempted.left._tag === "WalletExecutorConfigError") {
          expect(attempted.left.field).toBe("OA_LIGHTNING_SPARK_API_KEY")
        }
      }
    }),
  )

  it.effect("fails closed when spark mode has empty allowlist", () =>
    Effect.gen(function* () {
      const attempted = yield* Effect.either(
        loadWalletExecutorConfig({
          OA_LIGHTNING_WALLET_EXECUTOR_MODE: "spark",
          OA_LIGHTNING_SPARK_API_KEY: "spark_api_key_test",
        }),
      )

      expect(attempted._tag).toBe("Left")
      if (attempted._tag === "Left") {
        expect(attempted.left._tag).toBe("WalletExecutorConfigError")
        if (attempted.left._tag === "WalletExecutorConfigError") {
          expect(attempted.left.field).toBe("OA_LIGHTNING_WALLET_ALLOWED_HOSTS")
        }
      }
    }),
  )

  it.effect("accepts valid spark mode config", () =>
    Effect.gen(function* () {
      const config = yield* loadWalletExecutorConfig({
        OA_LIGHTNING_WALLET_EXECUTOR_MODE: "spark",
        OA_LIGHTNING_SPARK_API_KEY: "spark_api_key_test",
        OA_LIGHTNING_WALLET_ALLOWED_HOSTS: "sats4ai.com,l402.openagents.com",
        OA_LIGHTNING_WALLET_MNEMONIC_PROVIDER: "gcp",
        OA_LIGHTNING_WALLET_MNEMONIC_SECRET_VERSION: "projects/p/secrets/s/versions/latest",
        OA_LIGHTNING_SPARK_NETWORK: "mainnet",
      })

      expect(config.mode).toBe("spark")
      expect(config.network).toBe("mainnet")
      expect(config.allowedHosts.has("sats4ai.com")).toBe(true)
      expect(config.allowedHosts.has("l402.openagents.com")).toBe(true)
      expect(config.sparkApiKey).toBe("spark_api_key_test")
      expect(config.mnemonicProvider).toBe("gcp")
    }),
  )
})

