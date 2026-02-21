import { SecretManagerServiceClient } from "@google-cloud/secret-manager"
import { Context, Effect, Layer } from "effect"
import { validateWords } from "nostr-effect"

import { SecretLoadError } from "../errors.js"
import { WalletExecutorConfigService } from "./config.js"

export type MnemonicSecretProviderApi = Readonly<{
  loadMnemonic: () => Effect.Effect<string, SecretLoadError>
}>

export class MnemonicSecretProviderService extends Context.Tag(
  "@openagents/lightning-wallet-executor/MnemonicSecretProviderService",
)<MnemonicSecretProviderService, MnemonicSecretProviderApi>() {}

const validateMnemonicOrFail = (
  provider: "env" | "gcp",
  secretRef: string,
  value: string,
): Effect.Effect<string, SecretLoadError> =>
  Effect.gen(function* () {
    const normalized = value.trim().replace(/\s+/g, " ")
    if (!normalized) {
      return yield* SecretLoadError.make({
        provider,
        secretRef,
        message: "mnemonic secret resolved to empty value",
      })
    }
    if (!validateWords(normalized)) {
      return yield* SecretLoadError.make({
        provider,
        secretRef,
        message: "mnemonic failed validation",
      })
    }
    return normalized
  })

export const MnemonicSecretProviderLive = Layer.effect(
  MnemonicSecretProviderService,
  Effect.gen(function* () {
    const config = yield* WalletExecutorConfigService

    if (config.mnemonicProvider === "gcp") {
      const secretVersion = config.mnemonicSecretVersion
      if (!secretVersion) {
        return yield* SecretLoadError.make({
          provider: "gcp",
          secretRef: "OA_LIGHTNING_WALLET_MNEMONIC_SECRET_VERSION",
          message: "secret version is not configured",
        })
      }

      const client = new SecretManagerServiceClient()
      return MnemonicSecretProviderService.of({
        loadMnemonic: () =>
          Effect.tryPromise({
            try: async () => {
              const [response] = await client.accessSecretVersion({ name: secretVersion })
              return response.payload?.data?.toString("utf8") ?? ""
            },
            catch: (error) =>
              SecretLoadError.make({
                provider: "gcp",
                secretRef: secretVersion,
                message: `failed to access secret version: ${String(error)}`,
              }),
          }).pipe(Effect.flatMap((value) => validateMnemonicOrFail("gcp", secretVersion, value))),
      })
    }

    return MnemonicSecretProviderService.of({
      loadMnemonic: () =>
        Effect.gen(function* () {
          const value = process.env[config.mnemonicEnvVar] ?? ""
          if (!value.trim()) {
            return yield* SecretLoadError.make({
              provider: "env",
              secretRef: config.mnemonicEnvVar,
              message: "environment variable is missing",
            })
          }
          return yield* validateMnemonicOrFail("env", config.mnemonicEnvVar, value)
        }),
    })
  }),
)

export const makeMnemonicSecretProviderTestLayer = (mnemonic: string) =>
  Layer.succeed(
    MnemonicSecretProviderService,
    MnemonicSecretProviderService.of({
      loadMnemonic: () => validateMnemonicOrFail("env", "test", mnemonic),
    }),
  )

