import { Instance, types } from "mobx-state-tree"
import { NostrKeys } from "@/services/nostr/nostr.types"
import { withSetPropAction } from "../_helpers/withSetPropAction"
import * as actions from "./actions"
import { NostrKeysModel, TransactionModel } from "./models"
import { IWalletStore } from "./types"
import { createViews } from "./views"

const WalletStoreModel = types
  .model("WalletStore")
  .props({
    isInitialized: types.optional(types.boolean, false),
    setupComplete: types.optional(types.boolean, false),
    error: types.maybeNull(types.string),
    balanceSat: types.optional(types.number, 0),
    pendingSendSat: types.optional(types.number, 0),
    pendingReceiveSat: types.optional(types.number, 0),
    transactions: types.array(TransactionModel),
    mnemonic: types.maybe(types.string),
    nostrKeys: types.maybe(NostrKeysModel),
  })
  .views(createViews)
  .actions(withSetPropAction)
  .actions(self => ({
    setNostrKeys(nostrKeys: NostrKeys) {
      self.nostrKeys = nostrKeys
    },
    setBalanceSat(balanceSat: number) {
      self.balanceSat = balanceSat
    },
    setPendingSendSat(pendingSendSat: number) {
      self.pendingSendSat = pendingSendSat
    },
    setPendingReceiveSat(pendingReceiveSat: number) {
      self.pendingReceiveSat = pendingReceiveSat
    },
    setInitialized(isInitialized: boolean) {
      self.isInitialized = isInitialized
    },
    setSetupComplete(complete: boolean) {
      self.setupComplete = complete
    },
    setMnemonic(mnemonic: string | undefined) {  // Updated to match types.maybe(types.string)
      self.mnemonic = mnemonic
    },
    setError(message: string | null) {
      self.error = message
    },
    setTransactions(transactions: any[]) {
      self.transactions.replace(transactions)
    },
  }))
  .actions(self => ({
    async setup() {
      return await actions.setup(self as unknown as IWalletStore)
    },
    async fetchBalanceInfo() {
      await actions.fetchBalanceInfo(self as unknown as IWalletStore)
    },
    async disconnect() {
      return await actions.disconnect(self as unknown as IWalletStore)
    },
    async fetchTransactions() {
      return await actions.fetchTransactions(self as unknown as IWalletStore)
    },
    async sendPayment(bolt11: string, amount: number) {
      return await actions.sendPayment(self as unknown as IWalletStore, bolt11, amount)
    },
    async receivePayment(amount: number, description?: string) {
      return await actions.receivePayment(self as unknown as IWalletStore, amount, description)
    },
    async restoreWallet(mnemonic: string) {
      return await actions.restoreWallet(self as unknown as IWalletStore, mnemonic)
    },
  }))

export { WalletStoreModel }
export type WalletStoreType = Instance<typeof WalletStoreModel>
