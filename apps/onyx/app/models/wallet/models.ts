import { types } from "mobx-state-tree"

export const NostrKeysModel = types.model("NostrKeys", {
  privateKey: types.string,
  publicKey: types.string,
  npub: types.string,
  nsec: types.string,
})

export const TransactionModel = types.model("Transaction", {
  id: types.string,
  amount: types.number,
  timestamp: types.number,
  type: types.enumeration(["send", "receive"]),
  status: types.enumeration(["pending", "complete", "failed"]),
  description: types.maybe(types.string),
  paymentHash: types.maybe(types.string),
  fee: types.maybe(types.number),
})
