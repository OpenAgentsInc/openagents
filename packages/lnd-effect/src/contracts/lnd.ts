import { Effect, Schema } from "effect"

import { LndContractDecodeError } from "../errors/lndErrors.js"

const TimestampMs = Schema.Int.pipe(Schema.nonNegative())
const SatAmount = Schema.Int.pipe(Schema.nonNegative())

const decodeWithTypedError = <A>(
  contract: string,
  schema: Schema.Schema<A>,
  input: unknown,
): Effect.Effect<A, LndContractDecodeError> =>
  Schema.decodeUnknown(schema)(input).pipe(
    Effect.mapError((error) =>
      LndContractDecodeError.make({
        contract,
        reason: String(error),
      }),
    ),
  )

const decodeWithTypedErrorSync = <A>(contract: string, schema: Schema.Schema<A>, input: unknown): A => {
  try {
    return Schema.decodeUnknownSync(schema)(input)
  } catch (error) {
    throw LndContractDecodeError.make({
      contract,
      reason: String(error),
    })
  }
}

export const LndNetwork = Schema.Literal("mainnet", "testnet", "signet", "regtest", "simnet")
export type LndNetwork = typeof LndNetwork.Type

export const LndWalletState = Schema.Literal(
  "uninitialized",
  "initialized",
  "locked",
  "unlocked",
)
export type LndWalletState = typeof LndWalletState.Type

export const LndSyncState = Schema.Struct({
  syncedToChain: Schema.Boolean,
  blockHeight: SatAmount,
  blockHash: Schema.NonEmptyString,
})
export type LndSyncState = typeof LndSyncState.Type

export const LndNodeInfo = Schema.Struct({
  nodePubkey: Schema.NonEmptyString,
  alias: Schema.NonEmptyString,
  network: LndNetwork,
  walletState: LndWalletState,
  sync: LndSyncState,
  updatedAtMs: TimestampMs,
})
export type LndNodeInfo = typeof LndNodeInfo.Type

export const LndBalanceSummary = Schema.Struct({
  confirmedSat: SatAmount,
  unconfirmedSat: SatAmount,
  channelLocalSat: SatAmount,
  channelRemoteSat: SatAmount,
  pendingOpenSat: SatAmount,
  updatedAtMs: TimestampMs,
})
export type LndBalanceSummary = typeof LndBalanceSummary.Type

export const LndChannelSummary = Schema.Struct({
  openChannels: SatAmount,
  activeChannels: SatAmount,
  inactiveChannels: SatAmount,
  pendingChannels: SatAmount,
  updatedAtMs: TimestampMs,
})
export type LndChannelSummary = typeof LndChannelSummary.Type

export const LndNodeSnapshot = Schema.Struct({
  info: LndNodeInfo,
  balances: LndBalanceSummary,
  channels: LndChannelSummary,
})
export type LndNodeSnapshot = typeof LndNodeSnapshot.Type

export const decodeLndSyncState = (input: unknown) => decodeWithTypedError("LndSyncState", LndSyncState, input)
export const decodeLndSyncStateSync = (input: unknown) =>
  decodeWithTypedErrorSync("LndSyncState", LndSyncState, input)
export const encodeLndSyncState = Schema.encode(LndSyncState)
export const encodeLndSyncStateSync = Schema.encodeSync(LndSyncState)

export const decodeLndNodeInfo = (input: unknown) => decodeWithTypedError("LndNodeInfo", LndNodeInfo, input)
export const decodeLndNodeInfoSync = (input: unknown) =>
  decodeWithTypedErrorSync("LndNodeInfo", LndNodeInfo, input)
export const encodeLndNodeInfo = Schema.encode(LndNodeInfo)
export const encodeLndNodeInfoSync = Schema.encodeSync(LndNodeInfo)

export const decodeLndBalanceSummary = (input: unknown) =>
  decodeWithTypedError("LndBalanceSummary", LndBalanceSummary, input)
export const decodeLndBalanceSummarySync = (input: unknown) =>
  decodeWithTypedErrorSync("LndBalanceSummary", LndBalanceSummary, input)
export const encodeLndBalanceSummary = Schema.encode(LndBalanceSummary)
export const encodeLndBalanceSummarySync = Schema.encodeSync(LndBalanceSummary)

export const decodeLndChannelSummary = (input: unknown) =>
  decodeWithTypedError("LndChannelSummary", LndChannelSummary, input)
export const decodeLndChannelSummarySync = (input: unknown) =>
  decodeWithTypedErrorSync("LndChannelSummary", LndChannelSummary, input)
export const encodeLndChannelSummary = Schema.encode(LndChannelSummary)
export const encodeLndChannelSummarySync = Schema.encodeSync(LndChannelSummary)

export const decodeLndNodeSnapshot = (input: unknown) =>
  decodeWithTypedError("LndNodeSnapshot", LndNodeSnapshot, input)
export const decodeLndNodeSnapshotSync = (input: unknown) =>
  decodeWithTypedErrorSync("LndNodeSnapshot", LndNodeSnapshot, input)
export const encodeLndNodeSnapshot = Schema.encode(LndNodeSnapshot)
export const encodeLndNodeSnapshotSync = Schema.encodeSync(LndNodeSnapshot)
