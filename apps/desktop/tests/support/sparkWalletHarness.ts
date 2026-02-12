import fs from "node:fs"
import os from "node:os"
import path from "node:path"

import { Layer } from "effect"
import type {
  PrepareSendPaymentRequest,
  PrepareSendPaymentResponse,
  SendPaymentRequest,
  SendPaymentResponse,
} from "@breeztech/breez-sdk-spark/nodejs"

import { DesktopSecureStorageInMemoryTestLayer } from "../../src/main/desktopSecureStorage"
import {
  type SparkWalletManagerConfig,
  SparkWalletManagerConfigLive,
  SparkWalletManagerLive,
  defaultSparkWalletManagerConfig,
} from "../../src/main/sparkWalletManager"

const makePrepareResponse = (invoice: string): PrepareSendPaymentResponse => ({
  paymentMethod: {
    type: "bolt11Invoice",
    invoiceDetails: {
      expiry: 3600,
      invoice: {
        bolt11: invoice,
        source: {},
      },
      minFinalCltvExpiryDelta: 18,
      network: "regtest",
      payeePubkey: "02".repeat(33),
      paymentHash: "ab".repeat(32),
      paymentSecret: "cd".repeat(32),
      routingHints: [],
      timestamp: 1_700_000_000,
    },
    lightningFeeSats: 1,
  },
  amount: BigInt(2),
  feePolicy: "feesExcluded",
})

const makeSendResponse = (invoice: string): SendPaymentResponse => ({
  payment: {
    id: "spark-payment-1",
    paymentType: "send",
    status: "completed",
    amount: BigInt(2),
    fees: BigInt(0),
    timestamp: 1_700_000_100,
    method: "lightning",
    details: {
      type: "lightning",
      invoice,
      paymentHash: "ab".repeat(32),
      destinationPubkey: "02".repeat(33),
      preimage: "ef".repeat(32),
    },
  },
})

export type SparkWalletHarness = Readonly<{
  readonly rootDir: string
  readonly connectMnemonics: Array<string>
  readonly prepareRequests: Array<PrepareSendPaymentRequest>
  readonly sendRequests: Array<SendPaymentRequest>
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  readonly layer: Layer.Layer<any, never, never>
  readonly cleanup: () => void
}>

export const makeSparkWalletHarness = (): SparkWalletHarness => {
  const rootDir = fs.mkdtempSync(path.join(os.tmpdir(), "oa-spark-wallet-"))
  const connectMnemonics: Array<string> = []
  const prepareRequests: Array<PrepareSendPaymentRequest> = []
  const sendRequests: Array<SendPaymentRequest> = []

  const config = {
    ...defaultSparkWalletManagerConfig({
      userDataPath: rootDir,
      env: {},
    }),
    network: "regtest" as const,
    apiKey: "test-api-key",
    sdkConnect: async (input) => {
      connectMnemonics.push(input.mnemonic)
      return {
        disconnect: async () => undefined,
        getInfo: async () => ({
          identityPubkey: "spark-pub-test",
          balanceSats: 1234,
          tokenBalances: new Map<string, never>(),
        }),
        prepareSendPayment: async (request) => {
          prepareRequests.push(request)
          return makePrepareResponse(request.paymentRequest)
        },
        sendPayment: async (request) => {
          sendRequests.push(request)
          const invoiceDetails = request.prepareResponse.paymentMethod
          const invoice = invoiceDetails.type === "bolt11Invoice"
            ? invoiceDetails.invoiceDetails.invoice.bolt11
            : "lnbcrt1fallback"
          return makeSendResponse(invoice)
        },
      }
    },
  } satisfies SparkWalletManagerConfig

  const managerLayer = Layer.provideMerge(
    SparkWalletManagerLive,
    Layer.mergeAll(
      DesktopSecureStorageInMemoryTestLayer,
      SparkWalletManagerConfigLive(config),
    ),
  )

  return {
    rootDir,
    connectMnemonics,
    prepareRequests,
    sendRequests,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    layer: managerLayer as Layer.Layer<any, never, never>,
    cleanup: () => {
      fs.rmSync(rootDir, { recursive: true, force: true })
    },
  }
}
