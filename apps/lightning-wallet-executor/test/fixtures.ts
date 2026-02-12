import { Layer } from "effect"

import {
  defaultWalletExecutorConfig,
  type WalletExecutorConfig,
  makeWalletExecutorConfigTestLayer,
} from "../src/runtime/config.js"
import { makeMnemonicSecretProviderTestLayer } from "../src/runtime/secrets.js"
import { makeSparkGatewayMockLayer, type MockSparkGatewayConfig } from "../src/spark/gateway.js"
import { WalletExecutorLive } from "../src/wallet/executor.js"

export const makeTestConfig = (overrides?: Partial<WalletExecutorConfig>): WalletExecutorConfig => ({
  ...defaultWalletExecutorConfig(),
  mode: "mock",
  walletId: "test-wallet",
  host: "127.0.0.1",
  port: 8799,
  network: "regtest",
  allowedHosts: new Set(["sats4ai.com", "l402.openagents.com"]),
  ...overrides,
})

export const makeWalletTestLayer = (input?: {
  readonly config?: WalletExecutorConfig
  readonly mock?: MockSparkGatewayConfig
}) => {
  const configLayer = makeWalletExecutorConfigTestLayer(input?.config ?? makeTestConfig())
  const secretLayer = makeMnemonicSecretProviderTestLayer(
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
  )
  const sparkLayer = makeSparkGatewayMockLayer(input?.mock)
  const walletLayer = WalletExecutorLive.pipe(
    Layer.provideMerge(configLayer),
    Layer.provideMerge(sparkLayer),
  )

  return Layer.mergeAll(configLayer, secretLayer, sparkLayer, walletLayer)
}

