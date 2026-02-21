import { Layer } from "effect"

import { type WalletExecutorConfig, makeWalletExecutorConfigTestLayer } from "./runtime/config.js"
import { MnemonicSecretProviderLive, makeMnemonicSecretProviderTestLayer } from "./runtime/secrets.js"
import { SparkGatewayLive, makeSparkGatewayMockLayer, type MockSparkGatewayConfig } from "./spark/gateway.js"
import { WalletExecutorLive } from "./wallet/executor.js"

const testMnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

export const makeWalletExecutorRuntimeLayer = (config: WalletExecutorConfig) => {
  const configLayer = makeWalletExecutorConfigTestLayer(config)
  const secretLayer =
    config.mode === "mock"
      ? makeMnemonicSecretProviderTestLayer(testMnemonic)
      : MnemonicSecretProviderLive.pipe(Layer.provide(configLayer))

  const sparkLayer =
    config.mode === "mock"
      ? makeSparkGatewayMockLayer()
      : SparkGatewayLive.pipe(
          Layer.provideMerge(configLayer),
          Layer.provideMerge(secretLayer),
        )

  const walletLayer = WalletExecutorLive.pipe(
    Layer.provideMerge(configLayer),
    Layer.provideMerge(sparkLayer),
  )

  return Layer.mergeAll(configLayer, secretLayer, sparkLayer, walletLayer)
}

export const makeWalletExecutorTestRuntimeLayer = (input: {
  config: WalletExecutorConfig
  mock?: MockSparkGatewayConfig
  mnemonic?: string
}) => {
  const configLayer = makeWalletExecutorConfigTestLayer(input.config)
  const secretLayer = makeMnemonicSecretProviderTestLayer(input.mnemonic ?? testMnemonic)
  const sparkLayer = makeSparkGatewayMockLayer(input.mock)
  const walletLayer = WalletExecutorLive.pipe(
    Layer.provideMerge(configLayer),
    Layer.provideMerge(sparkLayer),
  )
  return Layer.mergeAll(configLayer, secretLayer, sparkLayer, walletLayer)
}

