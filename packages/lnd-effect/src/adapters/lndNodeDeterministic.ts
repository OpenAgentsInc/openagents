import { Clock, Effect, Layer, Ref } from "effect"

import type {
  LndBalanceSummary,
  LndChannelSummary,
  LndNodeInfo,
  LndNodeSnapshot,
  LndWalletState,
} from "../contracts/lnd.js"
import type {
  LndInvoiceCreateRequest,
  LndInvoiceListResult,
  LndInvoiceLookupRequest,
  LndInvoiceRecord,
  LndPaymentListResult,
  LndPaymentRecord,
  LndPaymentSendRequest,
  LndPaymentTrackRequest,
  LndRpcRequest,
  LndWalletInitializeRequest,
  LndWalletRestoreRequest,
  LndWalletUnlockRequest,
} from "../contracts/rpc.js"
import { LndWalletOperationError } from "../errors/lndErrors.js"
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

const defaultBalances: LndBalanceSummary = {
  confirmedSat: 0,
  unconfirmedSat: 0,
  channelLocalSat: 0,
  channelRemoteSat: 0,
  pendingOpenSat: 0,
  updatedAtMs: 1_700_000_000_000,
}

const defaultChannels: LndChannelSummary = {
  openChannels: 0,
  activeChannels: 0,
  inactiveChannels: 0,
  pendingChannels: 0,
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

const paginate = <A>(items: ReadonlyArray<A>, input?: { readonly limit?: number; readonly offset?: number }) => {
  const offset = Math.max(0, Math.floor(input?.offset ?? 0))
  const limit = Math.max(1, Math.min(250, Math.floor(input?.limit ?? 100)))
  const sliced = items.slice(offset, offset + limit)
  const nextOffset = offset + sliced.length < items.length ? offset + sliced.length : undefined
  return { sliced, nextOffset }
}

export const makeLndNodeDeterministicLayer = (input?: {
  readonly nodeInfo?: LndNodeInfo
  readonly balances?: LndBalanceSummary
  readonly channels?: LndChannelSummary
}) => {
  const nodeInfo = input?.nodeInfo ?? defaultNodeInfo
  const balances = input?.balances ?? defaultBalances
  const channels = input?.channels ?? defaultChannels

  return Layer.succeed(
    LndNodeService,
    LndNodeService.of({
      getNodeInfo: () => Effect.succeed(nodeInfo),
      getBalanceSummary: () => Effect.succeed(balances),
      getChannelSummary: () => Effect.succeed(channels),
      getNodeSnapshot: () =>
        Effect.succeed<LndNodeSnapshot>({
          info: nodeInfo,
          balances,
          channels,
        }),
    }),
  )
}

export const makeLndDeterministicLayer = (input?: {
  readonly nodeInfo?: LndNodeInfo
  readonly walletState?: LndWalletState
  readonly walletPassphrase?: string
  readonly balances?: LndBalanceSummary
  readonly channels?: LndChannelSummary
  readonly seedInvoices?: ReadonlyArray<LndInvoiceRecord>
  readonly seedPayments?: ReadonlyArray<LndPaymentRecord>
}) =>
  Layer.mergeAll(
    makeLndNodeDeterministicLayer({
      ...(input?.nodeInfo !== undefined ? { nodeInfo: input.nodeInfo } : {}),
      ...(input?.balances !== undefined ? { balances: input.balances } : {}),
      ...(input?.channels !== undefined ? { channels: input.channels } : {}),
    }),
    Layer.effect(
      LndWalletService,
      Effect.gen(function* () {
        const walletStateRef = yield* Ref.make<LndWalletState>(
          input?.walletState ?? input?.nodeInfo?.walletState ?? "locked",
        )
        const passphraseHashRef = yield* Ref.make<string | null>(
          input?.walletPassphrase ? deterministicHex(input.walletPassphrase) : null,
        )
        const hashPassphrase = (value: string): string => deterministicHex(value)

        const initializeWallet = (request: LndWalletInitializeRequest) =>
          Effect.gen(function* () {
            const current = yield* Ref.get(walletStateRef)
            if (current !== "uninitialized") {
              return yield* LndWalletOperationError.make({
                operation: "initializeWallet",
                reason: `wallet_state_${current}`,
              })
            }
            yield* Ref.set(passphraseHashRef, hashPassphrase(request.passphrase))
            yield* Ref.set(walletStateRef, "locked")
            return "locked" as const
          })

        const unlockWallet = (request: LndWalletUnlockRequest) =>
          Effect.gen(function* () {
            const current = yield* Ref.get(walletStateRef)
            if (current === "uninitialized") {
              return yield* LndWalletOperationError.make({
                operation: "unlockWallet",
                reason: "wallet_uninitialized",
              })
            }

            const expectedHash = yield* Ref.get(passphraseHashRef)
            if (!expectedHash || expectedHash !== hashPassphrase(request.passphrase)) {
              return yield* LndWalletOperationError.make({
                operation: "unlockWallet",
                reason: "invalid_passphrase",
              })
            }

            yield* Ref.set(walletStateRef, "unlocked")
            return "unlocked" as const
          })

        const restoreWallet = (request: LndWalletRestoreRequest) =>
          Effect.gen(function* () {
            if (request.seedMnemonic.length < 12 || request.seedMnemonic.length > 24) {
              return yield* LndWalletOperationError.make({
                operation: "restoreWallet",
                reason: "invalid_seed_length",
              })
            }
            yield* Ref.set(passphraseHashRef, hashPassphrase(request.passphrase))
            yield* Ref.set(walletStateRef, "locked")
            return "locked" as const
          })

        const lockWallet = () =>
          Effect.gen(function* () {
            const current = yield* Ref.get(walletStateRef)
            if (current === "uninitialized") {
              return "uninitialized" as const
            }
            yield* Ref.set(walletStateRef, "locked")
            return "locked" as const
          })

        return LndWalletService.of({
          getWalletState: () => Ref.get(walletStateRef),
          initializeWallet,
          unlockWallet,
          restoreWallet,
          lockWallet,
        })
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
    Layer.effect(
      LndInvoiceService,
      Effect.gen(function* () {
        const invoiceRef = yield* Ref.make<ReadonlyArray<LndInvoiceRecord>>(input?.seedInvoices ?? [])

        const createInvoice = (params: LndInvoiceCreateRequest) =>
          Effect.gen(function* () {
            const now = yield* Clock.currentTimeMillis
            const invoice: LndInvoiceRecord = {
              paymentRequest: deterministicInvoice(params.amountSat),
              rHash: deterministicHex(`invoice:${params.amountSat}:${params.memo ?? ""}`),
              amountSat: Math.max(0, Math.floor(params.amountSat)),
              settled: false,
              createdAtMs: now,
            }
            yield* Ref.update(invoiceRef, (rows) => [...rows, invoice])
            return invoice
          })

        const getInvoice = (lookup: LndInvoiceLookupRequest) =>
          Ref.get(invoiceRef).pipe(
            Effect.map((rows) =>
              rows.find((row) => row.paymentRequest === lookup.paymentRequest) ?? null,
            ),
          )

        const listInvoices = (opts?: { readonly limit?: number; readonly offset?: number }) =>
          Ref.get(invoiceRef).pipe(
            Effect.map((rows) => {
              const { sliced, nextOffset } = paginate(rows, opts)
              const result: LndInvoiceListResult = {
                invoices: sliced,
                ...(nextOffset !== undefined ? { nextOffset } : {}),
              }
              return result
            }),
          )

        return LndInvoiceService.of({
          createInvoice,
          getInvoice,
          listInvoices,
        })
      }),
    ),
    Layer.effect(
      LndPaymentService,
      Effect.gen(function* () {
        const paymentRef = yield* Ref.make<ReadonlyArray<LndPaymentRecord>>(input?.seedPayments ?? [])

        const sendPayment = (request: LndPaymentSendRequest) =>
          Effect.gen(function* () {
            const now = yield* Clock.currentTimeMillis
            const paymentHash = deterministicPaymentHash(request.paymentRequest)
            const payment: LndPaymentRecord = {
              paymentHash,
              paymentPreimageHex: deterministicHex(`${request.paymentRequest}:preimage`),
              amountSat: 0,
              feeSat: Math.max(0, Math.floor(request.feeLimitSat ?? 0)),
              status: "succeeded",
              createdAtMs: now,
              updatedAtMs: now,
            }
            yield* Ref.update(paymentRef, (rows) => [...rows, payment])
            return payment
          })

        const trackPayment = (request: LndPaymentTrackRequest) =>
          Ref.get(paymentRef).pipe(
            Effect.map((rows) => {
              const existing = rows.find((row) => row.paymentHash === request.paymentHash)
              if (existing) return existing
              const now = Date.now()
              return {
                paymentHash: request.paymentHash,
                amountSat: 0,
                feeSat: 0,
                status: "in_flight",
                createdAtMs: now,
                updatedAtMs: now,
              } satisfies LndPaymentRecord
            }),
          )

        const listPayments = (opts?: { readonly limit?: number; readonly offset?: number }) =>
          Ref.get(paymentRef).pipe(
            Effect.map((rows) => {
              const { sliced, nextOffset } = paginate(rows, opts)
              const result: LndPaymentListResult = {
                payments: sliced,
                ...(nextOffset !== undefined ? { nextOffset } : {}),
              }
              return result
            }),
          )

        return LndPaymentService.of({
          sendPayment,
          trackPayment,
          listPayments,
        })
      }),
    ),
  )
