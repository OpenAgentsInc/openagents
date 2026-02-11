import { Schema } from "effect"

const TimestampMs = Schema.Int.pipe(Schema.nonNegative())

export const LndNetwork = Schema.Literal("mainnet", "testnet", "signet", "regtest", "simnet")
export type LndNetwork = typeof LndNetwork.Type

export const LndWalletState = Schema.Literal("uninitialized", "locked", "unlocked")
export type LndWalletState = typeof LndWalletState.Type

export const LndSyncState = Schema.Struct({
  syncedToChain: Schema.Boolean,
  blockHeight: Schema.Int.pipe(Schema.nonNegative()),
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

export const decodeLndNodeInfo = Schema.decodeUnknown(LndNodeInfo)
export const decodeLndNodeInfoSync = Schema.decodeUnknownSync(LndNodeInfo)
export const encodeLndNodeInfo = Schema.encode(LndNodeInfo)
export const encodeLndNodeInfoSync = Schema.encodeSync(LndNodeInfo)
