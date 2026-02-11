import { Clock, Effect, Layer } from "effect"

import type { LndNodeInfo, LndWalletState } from "../contracts/lnd.js"
import type { LndInvoice, LndPayment, LndRpcRequest } from "../contracts/rpc.js"
import { LndInvoiceService } from "../services/lndInvoiceService.js"
import { LndNodeService } from "../services/lndNodeService.js"
import { LndPaymentService } from "../services/lndPaymentService.js"
import { LndTransportService } from "../services/lndTransportService.js"
import { LndWalletService } from "../services/lndWalletService.js"

const defaultNodeInfo: LndNodeInfo = {
  nodePubkey: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
  alias: "openagents-local",
  network: "regtest",
  walletState: "locked",
  sync: {
    syncedToChain: false,
    blockHeight: 0,
    blockHash: "0f9188f13cb7b2c9e5e30a77f7f8b6f1d5f1a6d8b9e4d4e3f7a2f3c4b5d6e7f8",
  },
  updatedAtMs: 1_700_000_000_000,
}

const deterministicHex = (input: string): string => {
  let acc = 0
  for (let index = 0; index < input.length; index += 1) {
    acc = (acc + input.charCodeAt(index) * (index + 97)) % 0xffffffff
  }

  const chunks: Array<string> = []
  for (let index = 0; index < 8; index += 1) {
    chunks.push(((acc + index * 2654435761) >>> 0).toString(16).padStart(8, "0"))
  }

  return chunks.join("").slice(0, 64)
}

const deterministicInvoice = (amountSat: number): string =>
  `lnbcrt${Math.max(0, Math.floor(amountSat))}${deterministicHex(String(amountSat)).slice(0, 32)}`

const deterministicPaymentHash = (seed: string): string => deterministicHex(seed)

export const makeLndNodeDeterministicLayer = (input?: { readonly nodeInfo?: LndNodeInfo }) => {
  const nodeInfo = input?.nodeInfo ?? defaultNodeInfo

  return Layer.succeed(
    LndNodeService,
    LndNodeService.of({
      getNodeInfo: () => Effect.succeed(nodeInfo),
    }),
  )
}

export const makeLndDeterministicLayer = (input?: {
  readonly nodeInfo?: LndNodeInfo
  readonly walletState?: LndWalletState
}) => {
  const nodeInfo = input?.nodeInfo ?? defaultNodeInfo
  const walletState = input?.walletState ?? nodeInfo.walletState

  return Layer.mergeAll(
    makeLndNodeDeterministicLayer({ nodeInfo }),
    Layer.succeed(
      LndWalletService,
      LndWalletService.of({
        getWalletState: () => Effect.succeed(walletState),
      }),
    ),
    Layer.succeed(
      LndTransportService,
      LndTransportService.of({
        send: (request: LndRpcRequest) =>
          Effect.succeed({
            status: 200,
            body: {
              ok: true,
              path: request.path,
              method: request.method,
            },
          }),
      }),
    ),
    Layer.succeed(
      LndInvoiceService,
      LndInvoiceService.of({
        createInvoice: (params: { readonly amountSat: number }) =>
          Effect.gen(function* () {
            const createdAtMs = yield* Clock.currentTimeMillis
            const invoice: LndInvoice = {
              invoice: deterministicInvoice(params.amountSat),
              amountSat: Math.max(0, Math.floor(params.amountSat)),
              createdAtMs,
            }
            return invoice
          }),
      }),
    ),
    Layer.succeed(
      LndPaymentService,
      LndPaymentService.of({
        trackPayment: (paymentHash: string) =>
          Effect.gen(function* () {
            const updatedAtMs = yield* Clock.currentTimeMillis
            const payment: LndPayment = {
              paymentHash: deterministicPaymentHash(paymentHash),
              amountSat: 0,
              status: "succeeded",
              preimageHex: deterministicHex(`${paymentHash}:preimage`),
              updatedAtMs,
            }
            return payment
          }),
      }),
    ),
  )
}
